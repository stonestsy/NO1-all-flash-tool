use super::util::*;
use serde_json::{json, Value};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use tauri::Emitter;

// 按 123pan Web 公开契约及原程序已恢复常量实现；非 0/200 业务码均显式报错。
const LOGIN_BASE: &str = "https://login.123pan.com/api/user";
const API_BASE: &str = "https://yun.123pan.com/b/api";

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PanSession {
    token: String,
    #[serde(default)]
    passport: Option<String>,
    #[serde(default)]
    user: Value,
}

static SESSION: LazyLock<Mutex<Option<PanSession>>> = LazyLock::new(|| Mutex::new(None));
static DOWNLOAD_ACTIVE: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_PAUSED: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_CANCEL: AtomicBool = AtomicBool::new(false);

struct PanDownloadStopped;

struct DownloadGuard;
impl Drop for DownloadGuard {
    fn drop(&mut self) {
        DOWNLOAD_ACTIVE.store(false, Ordering::SeqCst);
        DOWNLOAD_PAUSED.store(false, Ordering::SeqCst);
        DOWNLOAD_CANCEL.store(false, Ordering::SeqCst);
    }
}

fn session() -> MutexGuard<'static, Option<PanSession>> {
    SESSION.lock().unwrap_or_else(|e| e.into_inner())
}

fn session_path() -> PathBuf {
    data_dir("pan123").join("pan123_session.json")
}

fn public_session(value: &PanSession) -> Value {
    json!({
        "logged_in": true,
        "passport": value.passport,
        "user": value.user,
    })
}

fn logged_out() -> Value {
    json!({"logged_in": false, "passport": Value::Null, "user": Value::Null})
}

fn save_session(value: &PanSession) -> Result<(), String> {
    let path = session_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建 123 云盘会话目录失败: {}", e))?;
    }
    let data = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| format!("保存 123 云盘会话失败: {}", e))
}

fn clear_session() {
    *session() = None;
    let _ = std::fs::remove_file(session_path());
}

fn clear_session_if(token: &str) {
    let matches = session()
        .as_ref()
        .map(|s| s.token == token)
        .unwrap_or(false);
    if matches {
        clear_session();
    }
}

fn opt_string(value: &Option<Value>) -> Option<String> {
    value
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn as_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn as_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(v) => Some(*v),
        Value::Number(n) => n.as_i64().map(|v| v != 0),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => Some(true),
            "false" | "0" | "no" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn pick<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| value.get(*name))
}

fn api_message(value: &Value) -> String {
    pick(value, &["message", "Message", "msg", "tips", "Tips"])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("未知错误")
        .to_string()
}

// 共享 HTTP 工具不支持 Authorization 请求头，这里仅补充 123 云盘所需的 WinHTTP 封装。
#[link(name = "winhttp")]
extern "system" {
    fn WinHttpOpen(
        agent: *const u16,
        access_type: u32,
        proxy: *const u16,
        bypass: *const u16,
        flags: u32,
    ) -> isize;
    fn WinHttpSetTimeouts(
        internet: isize,
        resolve: i32,
        connect: i32,
        send: i32,
        receive: i32,
    ) -> i32;
    fn WinHttpConnect(session: isize, server: *const u16, port: u16, reserved: usize) -> isize;
    fn WinHttpOpenRequest(
        connect: isize,
        verb: *const u16,
        object: *const u16,
        version: *const u16,
        referrer: *const u16,
        accept_types: *const u16,
        flags: u32,
    ) -> isize;
    fn WinHttpSendRequest(
        request: isize,
        headers: *const u16,
        headers_len: u32,
        optional: *const c_void,
        optional_len: u32,
        total_len: u32,
        context: usize,
    ) -> i32;
    fn WinHttpReceiveResponse(request: isize, reserved: *mut c_void) -> i32;
    fn WinHttpReadData(request: isize, buffer: *mut c_void, to_read: u32, read: *mut u32) -> i32;
    fn WinHttpQueryHeaders(
        request: isize,
        info_level: u32,
        name: *const u16,
        buffer: *mut c_void,
        buffer_len: *mut u32,
        index: *mut u32,
    ) -> i32;
    fn WinHttpCloseHandle(handle: isize) -> i32;
}

struct PanHttpHandle(isize);
impl Drop for PanHttpHandle {
    fn drop(&mut self) {
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

fn pan_http(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<&[u8]>,
) -> Result<(u32, Vec<u8>), String> {
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "123 云盘仅支持 https URL".to_string())?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if token
        .map(|t| t.contains('\r') || t.contains('\n'))
        .unwrap_or(false)
    {
        return Err("123 云盘 token 含非法字符".to_string());
    }
    let mut headers = String::from(
        "Accept: application/json, text/plain, */*\r\n\
         Accept-Language: zh-CN,zh;q=0.9\r\n\
         platform: web\r\n\
         app-version: 3.0.0\r\n\
         webapp-version: 3.0.0\r\n\
         Origin: https://yun.123pan.com\r\n\
         Referer: https://yun.123pan.com/\r\n",
    );
    if body.is_some() {
        headers.push_str("Content-Type: application/json\r\n");
    }
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        headers.push_str(&format!("Authorization: Bearer {}\r\n", token));
    }
    let agent = wide_null("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36");
    let host_w = wide_null(host);
    let path_w = wide_null(path);
    let verb_w = wide_null(method);
    let headers_w = wide_null(&headers);
    unsafe {
        let http = PanHttpHandle(WinHttpOpen(
            agent.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ));
        if http.0 == 0 {
            return Err(format!("WinHttpOpen failed: {}", last_error()));
        }
        WinHttpSetTimeouts(http.0, 5000, 8000, 10000, 20000);
        let connect = PanHttpHandle(WinHttpConnect(http.0, host_w.as_ptr(), 443, 0));
        if connect.0 == 0 {
            return Err(format!("WinHttpConnect failed: {}", last_error()));
        }
        let request = PanHttpHandle(WinHttpOpenRequest(
            connect.0,
            verb_w.as_ptr(),
            path_w.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        ));
        if request.0 == 0 {
            return Err(format!("WinHttpOpenRequest failed: {}", last_error()));
        }
        let body_len = body.map(|b| b.len()).unwrap_or(0);
        if WinHttpSendRequest(
            request.0,
            headers_w.as_ptr(),
            (headers_w.len() - 1) as u32,
            body.map(|b| b.as_ptr() as *const c_void)
                .unwrap_or(std::ptr::null()),
            body_len as u32,
            body_len as u32,
            0,
        ) == 0
        {
            return Err(format!("WinHttpSendRequest failed: {}", last_error()));
        }
        if WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0 {
            return Err(format!("WinHttpReceiveResponse failed: {}", last_error()));
        }
        let mut status = 0u32;
        let mut status_size = 4u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut c_void,
            &mut status_size,
            std::ptr::null_mut(),
        );
        let mut bytes = Vec::new();
        loop {
            let mut chunk = [0u8; 16384];
            let mut read = 0u32;
            if WinHttpReadData(
                request.0,
                chunk.as_mut_ptr() as *mut c_void,
                chunk.len() as u32,
                &mut read,
            ) == 0
            {
                return Err(format!("WinHttpReadData failed: {}", last_error()));
            }
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read as usize]);
            if bytes.len() > 16 * 1024 * 1024 {
                return Err("123 云盘响应体过大".to_string());
            }
        }
        Ok((status, bytes))
    }
}

fn pan_api(
    method: &str,
    url: &str,
    token: Option<&str>,
    payload: Option<&Value>,
    action: &str,
) -> Result<Value, String> {
    let body = match payload {
        Some(value) => Some(serde_json::to_vec(value).map_err(|e| e.to_string())?),
        None => None,
    };
    let (status, bytes) = pan_http(method, url, token, body.as_deref())
        .map_err(|e| format!("{}请求失败: {}", action, e))?;
    if bytes.is_empty() {
        return Err(format!("{}：空响应（HTTP {}）", action, status));
    }
    let value =
        parse_json(&bytes).ok_or_else(|| format!("{}：非 JSON 响应（HTTP {}）", action, status))?;
    let code = value.get("code").and_then(as_i64);
    if status == 401 || code == Some(401) {
        if let Some(token) = token {
            clear_session_if(token);
        }
        return Err("接口登录已失效，请重新登录".to_string());
    }
    if !(200..300).contains(&status) {
        return Err(format!(
            "{}失败（HTTP {}）: {}",
            action,
            status,
            api_message(&value)
        ));
    }
    if let Some(code) = code {
        if code != 0 && code != 200 {
            return Err(format!("{}失败: {}", action, api_message(&value)));
        }
    }
    Ok(value)
}

fn response_data(value: &Value) -> &Value {
    value.get("data").unwrap_or(&Value::Null)
}

fn current_token() -> Result<String, String> {
    session()
        .as_ref()
        .map(|s| s.token.clone())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "尚未登录 123 云盘".to_string())
}

fn make_user(data: &Value, passport_hint: Option<&str>) -> (Option<String>, Value) {
    let raw = data.get("user").filter(|v| v.is_object()).unwrap_or(data);
    let passport = pick(raw, &["Passport", "passport", "account", "Account"])
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| passport_hint.map(str::to_string));
    let nickname = pick(raw, &["Nickname", "nickname", "NickName"])
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let vip_level = pick(raw, &["VipLevel", "vipLevel", "vip_level"])
        .and_then(as_u64)
        .unwrap_or(0);
    let vip = pick(raw, &["Vip", "vip"])
        .and_then(as_bool)
        .unwrap_or(vip_level > 0);
    let space_used = pick(raw, &["SpaceUsed", "spaceUsed", "space_used"])
        .and_then(as_u64)
        .unwrap_or(0);
    let space_permanent = pick(
        raw,
        &["SpacePermanent", "spacePermanent", "space_permanent"],
    )
    .and_then(as_u64)
    .unwrap_or(0);
    let direct_traffic =
        pick(raw, &["DirectTraffic", "directTraffic", "direct_traffic"]).and_then(as_u64);
    let vip_expire = pick(raw, &["VipExpire", "vipExpire", "vip_expire"])
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let user = json!({
        "passport": passport,
        "nickname": nickname,
        "vip": vip,
        "vip_level": vip_level,
        "vip_expire": vip_expire,
        "space_used": space_used,
        "space_permanent": space_permanent,
        "direct_traffic": direct_traffic,
    });
    (passport, user)
}

fn login_with_token(token: String, passport_hint: Option<String>) -> Result<Value, String> {
    if token.trim().is_empty() {
        return Err("token 为空".to_string());
    }
    let url = format!("{}/user/info", API_BASE);
    let response = pan_api("GET", &url, Some(&token), None, "获取用户信息")?;
    let (passport, user) = make_user(response_data(&response), passport_hint.as_deref());
    let value = PanSession {
        token,
        passport,
        user,
    };
    save_session(&value)?;
    let result = public_session(&value);
    *session() = Some(value);
    Ok(result)
}

fn token_from_login(response: &Value) -> Result<String, String> {
    pick(
        response_data(response),
        &["token", "Token", "accessToken", "access_token"],
    )
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|s| !s.is_empty())
    .map(str::to_string)
    .ok_or_else(|| "登录成功但未返回 token".to_string())
}

fn password_login_blocking(passport: String, password: String) -> Result<Value, String> {
    let url = format!("{}/sign_in", LOGIN_BASE);
    let response = pan_api(
        "POST",
        &url,
        None,
        Some(&json!({"passport": passport, "password": password, "remember": true})),
        "登录",
    )?;
    let token = token_from_login(&response)?;
    login_with_token(token, Some(passport))
}

fn cloud_credentials() -> Option<(String, String)> {
    let value = super::creds::load_login_credentials();
    if !value.get("remember").and_then(as_bool).unwrap_or(false) {
        return None;
    }
    let username = value
        .get("username")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let password = value
        .get("password")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    Some((username, password))
}

#[tauri::command]
pub fn pan123_restore() -> Value {
    let Ok(bytes) = std::fs::read(session_path()) else {
        *session() = None;
        return logged_out();
    };
    let Ok(value) = serde_json::from_slice::<PanSession>(&bytes) else {
        *session() = None;
        return logged_out();
    };
    if value.token.trim().is_empty() {
        *session() = None;
        return logged_out();
    }
    let result = public_session(&value);
    *session() = Some(value);
    result
}

#[tauri::command]
pub fn pan123_status() -> Value {
    session()
        .as_ref()
        .map(public_session)
        .unwrap_or_else(logged_out)
}

#[tauri::command]
pub async fn pan123_login(
    passport: Option<Value>,
    password: Option<Value>,
) -> Result<Value, String> {
    let passport = opt_string(&passport).ok_or_else(|| "请输入账号".to_string())?;
    let password = password
        .as_ref()
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| "请输入密码".to_string())?;
    let passport_save = passport.clone();
    let password_save = password.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || password_login_blocking(passport, password))
            .await
            .map_err(|e| e.to_string())??;
    // 只有账号密码登录写入一键云登凭据。
    let _ = super::creds::save_login_credentials(
        Some(Value::String(password_save)),
        Some(Value::String(passport_save)),
    );
    Ok(result)
}

#[tauri::command]
pub async fn pan123_send_vcode(
    passport: Option<Value>,
    traceless: Option<Value>,
) -> Result<Value, String> {
    let passport = opt_string(&passport).ok_or_else(|| "请输入手机号".to_string())?;
    let traceless = opt_string(&traceless).ok_or_else(|| "人机验证未完成，请重试".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("{}/get_vcode", LOGIN_BASE);
        let response = pan_api(
            "POST",
            &url,
            None,
            Some(&json!({
                "passport": passport,
                "operation": 1,
                "step": 1,
                "aliVersion": "1.0",
                "traceless": traceless,
            })),
            "发送验证码",
        )?;
        let data = response_data(&response);
        Ok(pick(data, &["Timestamp", "timestamp", "timeStamp"])
            .and_then(as_u64)
            .map(|v| json!(v))
            .unwrap_or(Value::Null))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn pan123_login_sms(
    passport: Option<Value>,
    time_stamp: Option<Value>,
    vcode: Option<Value>,
) -> Result<Value, String> {
    let passport = opt_string(&passport).ok_or_else(|| "请输入手机号".to_string())?;
    let vcode = opt_string(&vcode).ok_or_else(|| "请输入验证码".to_string())?;
    if passport.len() != 11 || !passport.chars().all(|c| c.is_ascii_digit()) {
        return Err("验证码登录仅支持手机号".to_string());
    }
    let timestamp = time_stamp.as_ref().and_then(as_u64);
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("{}/sign_in", LOGIN_BASE);
        let response = pan_api(
            "POST",
            &url,
            None,
            Some(&json!({
                "passport": passport,
                "v_code": vcode,
                "remember": true,
                "gray": true,
                "time_stamp": timestamp,
            })),
            "验证码登录",
        )?;
        let token = token_from_login(&response)?;
        login_with_token(token, Some(passport))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn query_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{:02X}", byte));
        }
    }
    out
}

#[tauri::command]
pub async fn pan123_qr_start() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("{}/qr-code/generate", LOGIN_BASE);
        let response = pan_api("GET", &url, None, None, "获取扫码二维码")?;
        let data = response_data(&response);
        let uni_id = pick(data, &["uniID", "uniId", "uni_id"])
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "未返回 uniID".to_string())?
            .to_string();
        let qr_content = data
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "https://yun.123pan.cn/wx-app-login.html?env=prod&uniID={}&source=123pan&type=login",
                    query_escape(&uni_id)
                )
            });
        let qr_svg = make_qr_svg(&qr_content)?;
        Ok(json!({"uni_id": uni_id, "qr_content": qr_content, "qr_svg": qr_svg}))
    })
    .await
    .map_err(|e| e.to_string())?
}

fn qr_status_text(data: &Value) -> String {
    pick(data, &["loginStatus", "login_status", "status"])
        .map(|v| match v {
            Value::String(s) => s.trim().to_ascii_lowercase(),
            _ => as_i64(v).map(|n| n.to_string()).unwrap_or_default(),
        })
        .unwrap_or_default()
}

fn qr_string(data: &Value, names: &[&str]) -> String {
    pick(data, names)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn qr_login_blocking(uni_id: &str, data: &Value) -> Result<Value, String> {
    if let Ok(token) = token_from_login(&json!({"data": data})) {
        return login_with_token(token, None);
    }
    let scan_platform = qr_string(data, &["scanPlatform", "scan_platform"]).to_ascii_lowercase();
    let login_type = qr_string(data, &["loginType", "login_type"]).to_ascii_lowercase();
    let wechat = scan_platform.contains("wechat")
        || scan_platform.contains("wx")
        || login_type.contains("wechat")
        || login_type.contains("wx")
        || pick(data, &["wxCode", "wechat_code", "redirectUrl"]).is_some();
    if !wechat {
        return Err("扫码成功但未返回登录凭证".to_string());
    }
    let url = format!(
        "{}/qr-code/wx_code?uniID={}",
        LOGIN_BASE,
        query_escape(uni_id)
    );
    let response = pan_api("GET", &url, None, None, "获取微信登录码")?;
    let wx_data = response_data(&response);
    let code = pick(wx_data, &["wxCode", "wx_code", "wechat_code"])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "微信授权码为空，请使用 123 云盘 App 扫码".to_string())?
        .to_string();
    let sign_url = format!("{}/sign_in", LOGIN_BASE);
    let login = pan_api(
        "POST",
        &sign_url,
        None,
        Some(&json!({"wechat_code": code, "remember": true})),
        "微信登录",
    )?;
    let token = token_from_login(&login)?;
    login_with_token(token, None)
}

#[tauri::command]
pub async fn pan123_qr_poll(uni_id: Option<Value>) -> Result<Value, String> {
    let uni_id = opt_string(&uni_id).ok_or_else(|| "缺少扫码会话".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!(
            "{}/qr-code/result?uniID={}",
            LOGIN_BASE,
            query_escape(&uni_id)
        );
        let response = pan_api("GET", &url, None, None, "查询扫码状态")?;
        let data = response_data(&response);
        let status = qr_status_text(data);
        let login_status = pick(data, &["loginStatus", "login_status"])
            .cloned()
            .unwrap_or(Value::Null);
        let scan_platform = qr_string(data, &["scanPlatform", "scan_platform"]);
        let base = |state: &str, message: &str, session: Value| {
            json!({
                "state": state,
                "message": message,
                "login_status": login_status,
                "scan_platform": scan_platform,
                "session": session,
            })
        };
        match status.as_str() {
            "2" | "done" | "success" | "succeeded" | "confirmed" | "authorized" | "finish"
            | "finished" => {
                let session = qr_login_blocking(&uni_id, data)?;
                Ok(base("done", "登录成功", session))
            }
            "1" | "scan" | "scanned" => Ok(base("scanned", "已扫码，请在手机上确认", Value::Null)),
            "3" | "cancel" | "cancelled" | "canceled" | "rejected" => {
                Ok(base("rejected", "已取消授权，请刷新二维码", Value::Null))
            }
            "4" | "expire" | "expired" => Ok(base("expired", "二维码已过期，请刷新", Value::Null)),
            _ => Ok(base("waiting", "等待扫码…", Value::Null)),
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn pan123_cloud_available() -> Value {
    Value::Bool(cloud_credentials().is_some())
}

#[tauri::command]
pub async fn pan123_cloud_login() -> Result<Value, String> {
    let (passport, password) = cloud_credentials().ok_or_else(|| "云登配置不完整".to_string())?;
    tauri::async_runtime::spawn_blocking(move || password_login_blocking(passport, password))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn pan123_logout() -> Value {
    clear_session();
    json!({"ok": true})
}

fn normalize_file(raw: &Value) -> Option<Value> {
    let file_id = pick(raw, &["FileId", "fileId", "file_id"]).and_then(as_u64)?;
    let filename = pick(raw, &["FileName", "fileName", "filename"])
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let is_dir = pick(raw, &["Type", "type"])
        .and_then(as_u64)
        .map(|v| v == 1)
        .or_else(|| pick(raw, &["is_dir", "isDir"]).and_then(as_bool))
        .unwrap_or(false);
    let size = pick(raw, &["Size", "size"]).and_then(as_u64).unwrap_or(0);
    let parent_id = pick(raw, &["ParentFileId", "parentFileId", "parent_id"])
        .and_then(as_u64)
        .unwrap_or(0);
    let s3_key_flag = pick(raw, &["S3KeyFlag", "s3KeyFlag", "s3keyFlag", "s3_key_flag"])
        .cloned()
        .unwrap_or(json!(0));
    Some(json!({
        "file_id": file_id,
        "filename": filename,
        "size": size,
        "is_dir": is_dir,
        "parent_id": parent_id,
        "s3_key_flag": s3_key_flag,
    }))
}

fn list_files_blocking(parent_file_id: u64) -> Result<Value, String> {
    let token = current_token()?;
    let url = format!("{}/file/list/new", API_BASE);
    let mut next = 0i64;
    let mut files = Vec::new();
    loop {
        let response = pan_api(
            "POST",
            &url,
            Some(&token),
            Some(&json!({
                "driveId": 0,
                "limit": 100,
                "next": next,
                "orderBy": "file_id",
                "orderDirection": "desc",
                "parentFileId": parent_file_id,
                "trashed": false,
                "PageOnlyLookAbnormalFile": false,
                "homeListFile": false,
                "event": "homeListFile",
                "operateType": 1,
                "inDirectSpace": false,
            })),
            "列出目录",
        )?;
        let data = response_data(&response);
        let list = pick(data, &["InfoList", "infoList", "fileList", "file_list"])
            .and_then(Value::as_array)
            .ok_or_else(|| "目录列表格式错误".to_string())?;
        files.extend(list.iter().filter_map(normalize_file));
        let new_next = pick(data, &["Next", "next"]).and_then(as_i64).unwrap_or(-1);
        if new_next == -1 || new_next == next {
            break;
        }
        next = new_next;
    }
    Ok(Value::Array(files))
}

#[tauri::command]
pub async fn pan123_list_files(parent_file_id: Option<Value>) -> Result<Value, String> {
    let parent_file_id = parent_file_id.as_ref().and_then(as_u64).unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || list_files_blocking(parent_file_id))
        .await
        .map_err(|e| e.to_string())?
}

fn download_url_blocking(item: &Value) -> Result<String, String> {
    let token = current_token()?;
    let file_id = pick(item, &["file_id", "fileId", "FileId"])
        .and_then(as_u64)
        .ok_or_else(|| "文件缺少 file_id".to_string())?;
    let is_dir = pick(item, &["is_dir", "isDir"])
        .and_then(as_bool)
        .unwrap_or(false);
    let (endpoint, payload) = if is_dir {
        (
            "file/batch_download_info",
            json!({"fileIds": [file_id], "operation": 1}),
        )
    } else {
        let filename = pick(item, &["filename", "fileName", "FileName"])
            .and_then(Value::as_str)
            .unwrap_or("");
        let s3_key_flag = pick(
            item,
            &["s3_key_flag", "s3keyFlag", "s3KeyFlag", "S3KeyFlag"],
        )
        .cloned()
        .unwrap_or(json!(0));
        (
            "file/download_info",
            json!({"fileId": file_id, "s3keyFlag": s3_key_flag, "fileName": filename}),
        )
    };
    let url = format!("{}/{}", API_BASE, endpoint);
    let response = pan_api("POST", &url, Some(&token), Some(&payload), "获取下载地址")?;
    let data = response_data(&response);
    if let Some(url) = data.as_str() {
        if url.starts_with("https://") {
            return Ok(url.to_string());
        }
    }
    pick(
        data,
        &[
            "DownloadUrl",
            "DownloadURL",
            "downloadUrl",
            "download_url",
            "redirectUrl",
            "redirect_url",
        ],
    )
    .and_then(Value::as_str)
    .map(str::trim)
    .filter(|s| s.starts_with("https://"))
    .map(str::to_string)
    .ok_or_else(|| "未获取到下载地址".to_string())
}

#[tauri::command]
pub async fn pan123_download_url(item: Option<Value>) -> Result<Value, String> {
    let item = item.ok_or_else(|| "缺少文件信息".to_string())?;
    tauri::async_runtime::spawn_blocking(move || download_url_blocking(&item).map(Value::String))
        .await
        .map_err(|e| e.to_string())?
}

fn create_share_blocking(
    file_ids: Vec<u64>,
    share_name: String,
    share_pwd: String,
) -> Result<Value, String> {
    let token = current_token()?;
    let url = format!("{}/share/create", API_BASE);
    let response = pan_api(
        "POST",
        &url,
        Some(&token),
        Some(&json!({
            "driveId": 0,
            "expiration": "2099-12-12T08:00:00+08:00",
            "fileIdList": file_ids,
            "shareName": share_name,
            "sharePwd": share_pwd,
            "event": "shareCreate",
        })),
        "创建分享",
    )?;
    let data = response_data(&response);
    let key = pick(data, &["ShareKey", "shareKey", "share_key"])
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "分享成功但未返回 ShareKey".to_string())?;
    Ok(json!({"share_url": format!("https://www.123pan.com/s/{}", key)}))
}

#[tauri::command]
pub async fn pan123_create_share(
    file_ids: Option<Value>,
    share_name: Option<Value>,
    share_pwd: Option<Value>,
) -> Result<Value, String> {
    let ids = file_ids
        .as_ref()
        .and_then(Value::as_array)
        .ok_or_else(|| "fileIds 必须是数组".to_string())?
        .iter()
        .map(|v| as_u64(v).ok_or_else(|| "fileIds 含无效文件 id".to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    if ids.is_empty() {
        return Err("请选择要分享的文件".to_string());
    }
    let share_name = opt_string(&share_name).unwrap_or_else(|| "分享文件".to_string());
    let share_pwd = share_pwd
        .as_ref()
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    tauri::async_runtime::spawn_blocking(move || create_share_blocking(ids, share_name, share_pwd))
        .await
        .map_err(|e| e.to_string())?
}

fn share_key(input: &str) -> Result<String, String> {
    let input = input.trim();
    let raw = if let Some(pos) = input.find("/s/") {
        &input[pos + 3..]
    } else {
        input.strip_prefix("s/").unwrap_or(input)
    };
    let end = raw
        .find(|c: char| c.is_whitespace() || matches!(c, '/' | '?' | '#' | '&'))
        .unwrap_or(raw.len());
    let key = raw[..end].trim_matches(|c: char| matches!(c, '.' | ',' | ';' | '，' | '。' | '；'));
    if key.is_empty()
        || key.len() > 256
        || !key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        return Err("无法从链接中解析分享码或分享码格式不正确".to_string());
    }
    Ok(key.to_string())
}

fn share_password(input: &str) -> Option<String> {
    for marker in [
        "提取码:",
        "提取码：",
        "密码:",
        "密码：",
        "pwd:",
        "password:",
        "pwd=",
    ] {
        if let Some(pos) = input.find(marker) {
            let rest = &input[pos + marker.len()..];
            let end = rest
                .find(|c: char| c.is_whitespace() || c == '&')
                .unwrap_or(rest.len());
            let value = rest[..end]
                .trim_matches(|c: char| matches!(c, '.' | ',' | ';' | '，' | '。' | '；'));
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn save_share_blocking(
    input: String,
    password: Option<String>,
    target_parent_id: u64,
) -> Result<Value, String> {
    let token = current_token()?;
    let key = share_key(&input)?;
    let password = password
        .filter(|s| !s.is_empty())
        .or_else(|| share_password(&input))
        .unwrap_or_default();
    let get_url = format!("{}/share/get", API_BASE);
    let response = pan_api(
        "POST",
        &get_url,
        Some(&token),
        Some(&json!({"ShareKey": key, "SharePwd": password})),
        "读取分享",
    )?;
    let data = response_data(&response);
    let files = pick(
        data,
        &[
            "InfoList",
            "infoList",
            "ShareFileList",
            "shareFileList",
            "fileList",
            "file_list",
        ],
    )
    .and_then(Value::as_array)
    .filter(|files| !files.is_empty())
    .cloned()
    .ok_or_else(|| "分享为空或已失效（检查链接与提取码）".to_string())?;
    let copy_url = format!("{}/file/copy/async", API_BASE);
    pan_api(
        "POST",
        &copy_url,
        Some(&token),
        Some(&json!({
            "share_key": key,
            "share_pwd": password,
            "current_level": 1,
            "event": "transfer",
            "file_list": files,
            "parent_file_id": target_parent_id,
            "drive_id": 0,
        })),
        "转存分享",
    )?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub async fn pan123_save_share(
    share_input: Option<Value>,
    share_pwd: Option<Value>,
    target_parent_id: Option<Value>,
) -> Result<Value, String> {
    let input = opt_string(&share_input).ok_or_else(|| "请输入分享链接或分享码".to_string())?;
    let password = share_pwd
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_string);
    let target_parent_id = target_parent_id.as_ref().and_then(as_u64).unwrap_or(0);
    tauri::async_runtime::spawn_blocking(move || {
        save_share_blocking(input, password, target_parent_id)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[link(name = "kernel32")]
extern "system" {
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        free_available: *mut u64,
        total_bytes: *mut u64,
        total_free: *mut u64,
    ) -> i32;
}

fn existing_directory(path: &Path) -> Result<PathBuf, String> {
    let mut dir = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    };
    if dir.as_os_str().is_empty() {
        dir = std::env::current_dir().map_err(|e| e.to_string())?;
    }
    while !dir.exists() {
        if !dir.pop() {
            return Err(format!("目录不存在: {}", path.display()));
        }
    }
    if dir.is_file() {
        dir = dir
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "无法确定磁盘目录".to_string())?;
    }
    Ok(dir)
}

fn disk_space(path: &Path) -> Result<u64, String> {
    let dir = existing_directory(path)?;
    let wide = wide_null(&dir.to_string_lossy());
    let mut free = 0u64;
    let mut total = 0u64;
    let mut total_free = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free, &mut total, &mut total_free) };
    if ok == 0 {
        Err(format!("无法读取磁盘可用空间: {}", last_error()))
    } else {
        Ok(free)
    }
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[unit])
    } else {
        format!("{:.2} {}", value, UNITS[unit])
    }
}

fn disk_space_info(need_bytes: u64, path: &Path) -> Result<Value, String> {
    let free_bytes = disk_space(path)?;
    let enough = free_bytes >= need_bytes;
    let message = if enough {
        format!("磁盘可用空间 {}", human_bytes(free_bytes))
    } else {
        format!(
            "磁盘空间不足：文件约需 {}，当前盘可用 {}，请换盘或清理空间后再下载",
            human_bytes(need_bytes),
            human_bytes(free_bytes)
        )
    };
    Ok(json!({
        "free_bytes": free_bytes,
        "need_bytes": need_bytes,
        "enough": enough,
        "message": message,
    }))
}

#[tauri::command]
pub fn pan123_check_disk_space(
    need_bytes: Option<Value>,
    path: Option<Value>,
) -> Result<Value, String> {
    let need_bytes = need_bytes
        .as_ref()
        .and_then(as_u64)
        .ok_or_else(|| "缺少 needBytes".to_string())?;
    let path = opt_string(&path).ok_or_else(|| "缺少 path".to_string())?;
    disk_space_info(need_bytes, Path::new(&path))
}

fn emit_download(
    app: &tauri::AppHandle,
    status: &str,
    filename: &str,
    downloaded: u64,
    total: u64,
    speed_bps: f64,
) {
    let percent = if status == "done" {
        100.0
    } else if total > 0 {
        (downloaded as f64 * 100.0 / total as f64).min(100.0)
    } else {
        0.0
    };
    let _ = app.emit(
        "pan123:download-progress",
        json!({
            "status": status,
            "filename": filename,
            "downloaded": downloaded,
            "total": total,
            "percent": percent,
            "speed_bps": speed_bps,
        }),
    );
}

#[tauri::command]
pub async fn pan123_download_file(
    app: tauri::AppHandle,
    item: Option<Value>,
    save_path: Option<Value>,
    skip_space_check: Option<Value>,
) -> Result<Value, String> {
    let item = item.ok_or_else(|| "缺少文件信息".to_string())?;
    let save_path = opt_string(&save_path).ok_or_else(|| "缺少 savePath".to_string())?;
    let skip_space_check = skip_space_check.as_ref().and_then(as_bool).unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        if DOWNLOAD_ACTIVE
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err("已有 123 云盘下载任务正在进行".to_string());
        }
        DOWNLOAD_PAUSED.store(false, Ordering::SeqCst);
        DOWNLOAD_CANCEL.store(false, Ordering::SeqCst);
        let _guard = DownloadGuard;
        let is_dir = item.get("is_dir").and_then(as_bool).unwrap_or(false);
        let base_name = item
            .get("filename")
            .and_then(Value::as_str)
            .unwrap_or("download");
        let filename = if is_dir {
            format!("{}.zip", base_name)
        } else {
            base_name.to_string()
        };
        let item_size = if is_dir {
            0
        } else {
            item.get("size").and_then(as_u64).unwrap_or(0)
        };
        let final_path = PathBuf::from(&save_path);
        let temp_path = PathBuf::from(format!("{}.123pan.tmp", save_path));
        emit_download(&app, "preparing", &filename, 0, item_size, 0.0);
        if !skip_space_check && item_size > 0 {
            let info = disk_space_info(item_size, &final_path)?;
            if !info.get("enough").and_then(as_bool).unwrap_or(false) {
                return Err(info
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("磁盘空间不足")
                    .to_string());
            }
        }
        let url = download_url_blocking(&item)?;
        if DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
            return Err("已取消".to_string());
        }
        let started = Instant::now();
        let mut last_emit = Instant::now() - Duration::from_millis(200);
        let app_progress = app.clone();
        let progress_name = filename.clone();
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            download_with_progress(&url, &temp_path, move |downloaded, total| {
                if DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
                    std::panic::resume_unwind(Box::new(PanDownloadStopped));
                }
                let shown_total = if total > 0 { total } else { item_size };
                if DOWNLOAD_PAUSED.load(Ordering::SeqCst) {
                    emit_download(
                        &app_progress,
                        "paused",
                        &progress_name,
                        downloaded,
                        shown_total,
                        0.0,
                    );
                    while DOWNLOAD_PAUSED.load(Ordering::SeqCst) {
                        if DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
                            std::panic::resume_unwind(Box::new(PanDownloadStopped));
                        }
                        std::thread::sleep(Duration::from_millis(50));
                    }
                    last_emit = Instant::now();
                }
                let speed = downloaded as f64 / started.elapsed().as_secs_f64().max(0.001);
                if last_emit.elapsed() >= Duration::from_millis(100) {
                    last_emit = Instant::now();
                    emit_download(
                        &app_progress,
                        "downloading",
                        &progress_name,
                        downloaded,
                        shown_total,
                        speed,
                    );
                }
            })
        }));
        if DOWNLOAD_CANCEL.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&temp_path);
            emit_download(&app, "cancelled", &filename, 0, item_size, 0.0);
            return Err("已取消".to_string());
        }
        match outcome {
            Ok(Ok(bytes)) => {
                if final_path.exists() {
                    std::fs::remove_file(&final_path)
                        .map_err(|e| format!("替换旧文件失败: {}", e))?;
                }
                std::fs::rename(&temp_path, &final_path)
                    .map_err(|e| format!("保存文件失败: {}", e))?;
                let total = if item_size > 0 { item_size } else { bytes };
                emit_download(&app, "done", &filename, bytes, total, 0.0);
                Ok(Value::String(final_path.to_string_lossy().to_string()))
            }
            Ok(Err(e)) => {
                let _ = std::fs::remove_file(&temp_path);
                Err(format!("下载失败: {}", e))
            }
            Err(_) => {
                let _ = std::fs::remove_file(&temp_path);
                Err("下载已中断".to_string())
            }
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn pan123_pause_download() -> Result<Value, String> {
    if !DOWNLOAD_ACTIVE.load(Ordering::SeqCst) {
        return Err("当前没有 123 云盘下载任务".to_string());
    }
    DOWNLOAD_PAUSED.store(true, Ordering::SeqCst);
    Ok(json!({"ok": true, "paused": true}))
}

#[tauri::command]
pub fn pan123_resume_download() -> Result<Value, String> {
    if !DOWNLOAD_ACTIVE.load(Ordering::SeqCst) {
        return Err("当前没有 123 云盘下载任务".to_string());
    }
    if !DOWNLOAD_PAUSED.swap(false, Ordering::SeqCst) {
        return Err("当前下载未暂停".to_string());
    }
    Ok(json!({"ok": true, "paused": false}))
}

#[tauri::command]
pub fn pan123_cancel_download() -> Result<Value, String> {
    if !DOWNLOAD_ACTIVE.load(Ordering::SeqCst) {
        return Err("当前没有 123 云盘下载任务".to_string());
    }
    DOWNLOAD_CANCEL.store(true, Ordering::SeqCst);
    DOWNLOAD_PAUSED.store(false, Ordering::SeqCst);
    Ok(json!({"ok": true, "cancelled": true}))
}

// 固定使用 QR Version 8-L（最多 192 字节），避免为单个二维码引入额外 crate。
fn append_bits(bits: &mut Vec<bool>, value: u32, count: usize) {
    for i in (0..count).rev() {
        bits.push(((value >> i) & 1) != 0);
    }
}

fn rs_multiply(x: u8, y: u8) -> u8 {
    let mut z = 0u8;
    let mut x = x;
    let mut y = y;
    for _ in 0..8 {
        if y & 1 != 0 {
            z ^= x;
        }
        x = if x & 0x80 != 0 {
            (x << 1) ^ 0x1D
        } else {
            x << 1
        };
        y >>= 1;
    }
    z
}

fn rs_divisor(degree: usize) -> Vec<u8> {
    let mut result = vec![0u8; degree - 1];
    result.push(1);
    let mut root = 1u8;
    for _ in 0..degree {
        for j in 0..result.len() {
            result[j] = rs_multiply(result[j], root);
            if j + 1 < result.len() {
                result[j] ^= result[j + 1];
            }
        }
        root = rs_multiply(root, 2);
    }
    result
}

fn rs_remainder(data: &[u8], divisor: &[u8]) -> Vec<u8> {
    let mut result = vec![0u8; divisor.len()];
    for byte in data {
        let factor = *byte ^ result.remove(0);
        result.push(0);
        for (out, coefficient) in result.iter_mut().zip(divisor.iter()) {
            *out ^= rs_multiply(*coefficient, factor);
        }
    }
    result
}

fn set_qr_module(
    modules: &mut [Vec<bool>],
    function: &mut [Vec<bool>],
    x: isize,
    y: isize,
    dark: bool,
) {
    let size = modules.len() as isize;
    if x >= 0 && y >= 0 && x < size && y < size {
        modules[y as usize][x as usize] = dark;
        function[y as usize][x as usize] = true;
    }
}

fn draw_qr_format(modules: &mut [Vec<bool>], function: &mut [Vec<bool>], mask: u32) {
    let size = modules.len() as isize;
    let data = (1u32 << 3) | mask; // Error correction L
    let mut rem = data;
    for _ in 0..10 {
        rem = (rem >> 1) ^ if rem & 1 != 0 { 0x537 } else { 0 };
    }
    let bits = ((data << 10) | rem) ^ 0x5412;
    let bit = |i: usize| ((bits >> i) & 1) != 0;
    for i in 0..=5 {
        set_qr_module(modules, function, 8, i, bit(i as usize));
    }
    set_qr_module(modules, function, 8, 7, bit(6));
    set_qr_module(modules, function, 8, 8, bit(7));
    set_qr_module(modules, function, 7, 8, bit(8));
    for i in 9..15 {
        set_qr_module(modules, function, 14 - i, 8, bit(i as usize));
    }
    for i in 0..8 {
        set_qr_module(modules, function, size - 1 - i, 8, bit(i as usize));
    }
    for i in 8..15 {
        set_qr_module(modules, function, 8, size - 15 + i, bit(i as usize));
    }
    set_qr_module(modules, function, 8, size - 8, true);
}

fn make_qr_svg(text: &str) -> Result<String, String> {
    const VERSION: usize = 8;
    const SIZE: usize = VERSION * 4 + 17;
    const DATA_CODEWORDS: usize = 194;
    const BLOCKS: usize = 2;
    const ECC_CODEWORDS: usize = 24;
    let bytes = text.as_bytes();
    if bytes.len() > 192 {
        return Err("二维码内容过长".to_string());
    }
    let capacity = DATA_CODEWORDS * 8;
    let mut bits = Vec::with_capacity(capacity);
    append_bits(&mut bits, 4, 4); // Byte mode
    append_bits(&mut bits, bytes.len() as u32, 8);
    for byte in bytes {
        append_bits(&mut bits, *byte as u32, 8);
    }
    for _ in 0..4.min(capacity - bits.len()) {
        bits.push(false);
    }
    while bits.len() % 8 != 0 {
        bits.push(false);
    }
    let mut data = Vec::with_capacity(DATA_CODEWORDS);
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for bit in chunk {
            byte = (byte << 1) | u8::from(*bit);
        }
        data.push(byte);
    }
    let mut pad = 0xECu8;
    while data.len() < DATA_CODEWORDS {
        data.push(pad);
        pad ^= 0xEC ^ 0x11;
    }
    let divisor = rs_divisor(ECC_CODEWORDS);
    let block_len = DATA_CODEWORDS / BLOCKS;
    let ecc: Vec<Vec<u8>> = data
        .chunks(block_len)
        .map(|block| rs_remainder(block, &divisor))
        .collect();
    let mut codewords = Vec::with_capacity(DATA_CODEWORDS + BLOCKS * ECC_CODEWORDS);
    for i in 0..block_len {
        for block in 0..BLOCKS {
            codewords.push(data[block * block_len + i]);
        }
    }
    for i in 0..ECC_CODEWORDS {
        for block in ecc.iter() {
            codewords.push(block[i]);
        }
    }

    let mut modules = vec![vec![false; SIZE]; SIZE];
    let mut function = vec![vec![false; SIZE]; SIZE];
    for i in 0..SIZE as isize {
        set_qr_module(&mut modules, &mut function, 6, i, i % 2 == 0);
        set_qr_module(&mut modules, &mut function, i, 6, i % 2 == 0);
    }
    for (cx, cy) in [
        (3isize, 3isize),
        (SIZE as isize - 4, 3),
        (3, SIZE as isize - 4),
    ] {
        for dy in -4isize..=4 {
            for dx in -4isize..=4 {
                let distance = dx.abs().max(dy.abs());
                set_qr_module(
                    &mut modules,
                    &mut function,
                    cx + dx,
                    cy + dy,
                    distance != 2 && distance != 4,
                );
            }
        }
    }
    let positions = [6isize, 24, 42];
    for (i, x) in positions.iter().enumerate() {
        for (j, y) in positions.iter().enumerate() {
            if (i == 0 && j == 0) || (i == 0 && j == 2) || (i == 2 && j == 0) {
                continue;
            }
            for dy in -2isize..=2 {
                for dx in -2isize..=2 {
                    set_qr_module(
                        &mut modules,
                        &mut function,
                        x + dx,
                        y + dy,
                        dx.abs().max(dy.abs()) != 1,
                    );
                }
            }
        }
    }
    draw_qr_format(&mut modules, &mut function, 0);
    let mut rem = VERSION as u32;
    for _ in 0..12 {
        rem = (rem >> 1) ^ if rem & 1 != 0 { 0x1F25 } else { 0 };
    }
    let version_bits = ((VERSION as u32) << 12) | rem;
    for i in 0..18usize {
        let bit = ((version_bits >> i) & 1) != 0;
        let a = SIZE as isize - 11 + (i % 3) as isize;
        let b = (i / 3) as isize;
        set_qr_module(&mut modules, &mut function, a, b, bit);
        set_qr_module(&mut modules, &mut function, b, a, bit);
    }

    let mut bit_index = 0usize;
    let mut right = SIZE as isize - 1;
    let mut upward = true;
    while right >= 1 {
        if right == 6 {
            right = 5;
        }
        for vertical in 0..SIZE {
            let y = if upward {
                SIZE - 1 - vertical
            } else {
                vertical
            };
            for j in 0..2 {
                let x = (right - j) as usize;
                if !function[y][x] && bit_index < codewords.len() * 8 {
                    let dark = ((codewords[bit_index >> 3] >> (7 - (bit_index & 7))) & 1) != 0;
                    modules[y][x] = dark;
                    bit_index += 1;
                }
            }
        }
        upward = !upward;
        right -= 2;
    }
    for y in 0..SIZE {
        for x in 0..SIZE {
            if !function[y][x] && (x + y) % 2 == 0 {
                modules[y][x] = !modules[y][x];
            }
        }
    }
    draw_qr_format(&mut modules, &mut function, 0);

    let mut path = String::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            if modules[y][x] {
                path.push_str(&format!("M{},{}h1v1h-1z", x + 4, y + 4));
            }
        }
    }
    Ok(format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {0} {0}\" shape-rendering=\"crispEdges\"><rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/><path d=\"{1}\" fill=\"#0f172a\"/></svg>",
        SIZE + 8,
        path
    ))
}
