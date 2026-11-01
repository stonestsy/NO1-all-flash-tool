//! 命令实现共享基础设施：WinHTTP、子进程、工具路径、系统信息。
//! 仅依赖 Windows 系统 DLL（winhttp/kernel32/ntdll/shell32），无新增 crate。

use serde_json::{json, Value};
use std::ffi::c_void;
use std::path::{Path, PathBuf};

// ---------- WinHTTP ----------

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
    fn WinHttpWriteData(
        request: isize,
        buffer: *const c_void,
        to_write: u32,
        written: *mut u32,
    ) -> i32;
    fn WinHttpQueryHeaders(
        request: isize,
        info_level: u32,
        name: *const u16,
        buffer: *mut c_void,
        buffer_len: *mut u32,
        index: *mut u32,
    ) -> i32;
    fn WinHttpSetOption(handle: isize, option: u32, buffer: *const c_void, buffer_len: u32) -> i32;
    fn WinHttpCloseHandle(handle: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GetLastError() -> u32;
}

pub fn last_error() -> u32 {
    unsafe { GetLastError() }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn wide_null(s: &str) -> Vec<u16> {
    wide(s)
}

struct HttpHandle(isize);
impl Drop for HttpHandle {
    fn drop(&mut self) {
        unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// 执行一次 HTTP 请求并收集响应体。method 为 GET/POST 等。
pub fn http_request(
    method: &str,
    url: &str,
    body: Option<&[u8]>,
    content_type: Option<&str>,
) -> Result<Vec<u8>, String> {
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "仅支持 https URL".to_string())?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let agent = wide("aft/5.1.0");
    let host_w = wide(host);
    let path_w = wide(path);
    let verb_w = wide(method);
    unsafe {
        let session = HttpHandle(WinHttpOpen(
            agent.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ));
        if session.0 == 0 {
            return Err(format!("WinHttpOpen failed: {}", last_error()));
        }
        WinHttpSetTimeouts(session.0, 5000, 5000, 8000, 15000);
        let connect = HttpHandle(WinHttpConnect(session.0, host_w.as_ptr(), 443, 0));
        if connect.0 == 0 {
            return Err(format!("WinHttpConnect failed: {}", last_error()));
        }
        let request = HttpHandle(WinHttpOpenRequest(
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
        let header = content_type.map(|ct| wide(&format!("Content-Type: {}\r\n", ct)));
        let (header_ptr, header_len) = match &header {
            Some(h) => (h.as_ptr(), (h.len() - 1) as u32),
            None => (std::ptr::null(), 0),
        };
        let sent = WinHttpSendRequest(
            request.0,
            header_ptr,
            header_len,
            body.map(|b| b.as_ptr() as *const c_void)
                .unwrap_or(std::ptr::null()),
            body_len as u32,
            body_len as u32,
            0,
        );
        if sent == 0 {
            return Err(format!("WinHttpSendRequest failed: {}", last_error()));
        }
        if WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0 {
            return Err(format!("WinHttpReceiveResponse failed: {}", last_error()));
        }
        let mut buf: Vec<u8> = Vec::new();
        loop {
            let mut chunk = [0u8; 16384];
            let mut read: u32 = 0;
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
            buf.extend_from_slice(&chunk[..read as usize]);
            if buf.len() > 256 * 1024 * 1024 {
                return Err("响应体过大".to_string());
            }
        }
        Ok(buf)
    }
}

pub fn http_get(url: &str) -> Option<Vec<u8>> {
    http_request("GET", url, None, None).ok()
}

pub fn http_get_string(url: &str) -> Option<String> {
    http_get(url).map(|b| String::from_utf8_lossy(&b).to_string())
}

pub fn http_post_json(url: &str, payload: &Value) -> Result<Value, String> {
    let body = serde_json::to_vec(payload).map_err(|e| e.to_string())?;
    let resp = http_request("POST", url, Some(&body), Some("application/json"))?;
    parse_json(&resp).ok_or_else(|| "响应不是合法 JSON".to_string())
}

pub fn parse_json(body: &[u8]) -> Option<Value> {
    let start = body.iter().position(|b| *b == b'{' || *b == b'[')?;
    serde_json::from_slice(&body[start..]).ok()
}

/// 下载 URL 到本地文件（流式写盘）。
pub fn download_file(url: &str, dest: &Path) -> Result<u64, String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = http_request("GET", url, None, None)?;
    std::fs::write(dest, &data).map_err(|e| e.to_string())?;
    Ok(data.len() as u64)
}

/// 流式下载并回调进度（transferred, total；total 未知时为 0）。
pub fn download_with_progress<F: FnMut(u64, u64)>(
    url: &str,
    dest: &Path,
    mut on_progress: F,
) -> Result<u64, String> {
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_QUERY_CONTENT_LENGTH: u32 = 5;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "仅支持 https URL".to_string())?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let agent = wide("aft/5.1.0");
    let host_w = wide(host);
    let path_w = wide(path);
    let verb_w = wide("GET");
    unsafe {
        let session = HttpHandle(WinHttpOpen(
            agent.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ));
        if session.0 == 0 {
            return Err(format!("WinHttpOpen failed: {}", last_error()));
        }
        WinHttpSetTimeouts(session.0, 5000, 5000, 8000, 30000);
        let connect = HttpHandle(WinHttpConnect(session.0, host_w.as_ptr(), 443, 0));
        if connect.0 == 0 {
            return Err(format!("WinHttpConnect failed: {}", last_error()));
        }
        let request = HttpHandle(WinHttpOpenRequest(
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
        if WinHttpSendRequest(request.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(format!("WinHttpSendRequest failed: {}", last_error()));
        }
        if WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0 {
            return Err(format!("WinHttpReceiveResponse failed: {}", last_error()));
        }
        let mut total: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_CONTENT_LENGTH | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut total as *mut u32 as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
        );
        let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
        let mut transferred: u64 = 0;
        loop {
            let mut chunk = [0u8; 65536];
            let mut read: u32 = 0;
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
            std::io::Write::write_all(&mut file, &chunk[..read as usize])
                .map_err(|e| e.to_string())?;
            transferred += read as u64;
            on_progress(transferred, total as u64);
        }
        Ok(transferred)
    }
}

/// 禁用自动重定向发起 GET，返回 (状态码, 原始响应头文本)。
fn http_status_headers(url: &str) -> Result<(u32, String), String> {
    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_OPTION_DISABLE_FEATURE: u32 = 63;
    const WINHTTP_DISABLE_REDIRECTS: u32 = 0x0000_0040;
    const WINHTTP_QUERY_STATUS_CODE: u32 = 19;
    const WINHTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;
    const WINHTTP_QUERY_RAW_HEADERS_CRLF: u32 = 22;
    let rest = url
        .strip_prefix("https://")
        .ok_or_else(|| "仅支持 https URL".to_string())?;
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let agent = wide("aft/5.1.0");
    let host_w = wide(host);
    let path_w = wide(path);
    let verb_w = wide("GET");
    unsafe {
        let session = HttpHandle(WinHttpOpen(
            agent.as_ptr(),
            0,
            std::ptr::null(),
            std::ptr::null(),
            0,
        ));
        if session.0 == 0 {
            return Err(format!("WinHttpOpen failed: {}", last_error()));
        }
        WinHttpSetTimeouts(session.0, 5000, 5000, 8000, 15000);
        let connect = HttpHandle(WinHttpConnect(session.0, host_w.as_ptr(), 443, 0));
        if connect.0 == 0 {
            return Err(format!("WinHttpConnect failed: {}", last_error()));
        }
        let request = HttpHandle(WinHttpOpenRequest(
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
        let disable: u32 = WINHTTP_DISABLE_REDIRECTS;
        WinHttpSetOption(
            request.0,
            WINHTTP_OPTION_DISABLE_FEATURE,
            &disable as *const u32 as *const c_void,
            4,
        );
        if WinHttpSendRequest(request.0, std::ptr::null(), 0, std::ptr::null(), 0, 0, 0) == 0 {
            return Err(format!("WinHttpSendRequest failed: {}", last_error()));
        }
        if WinHttpReceiveResponse(request.0, std::ptr::null_mut()) == 0 {
            return Err(format!("WinHttpReceiveResponse failed: {}", last_error()));
        }
        let mut status: u32 = 0;
        let mut size = 4u32;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status as *mut u32 as *mut c_void,
            &mut size,
            std::ptr::null_mut(),
        );
        let mut buf_len: u32 = 0;
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_RAW_HEADERS_CRLF,
            std::ptr::null(),
            std::ptr::null_mut(),
            &mut buf_len,
            std::ptr::null_mut(),
        );
        let mut headers = String::new();
        if buf_len > 0 && buf_len < 1024 * 1024 {
            let mut buf = vec![0u16; (buf_len / 2 + 1) as usize];
            if WinHttpQueryHeaders(
                request.0,
                WINHTTP_QUERY_RAW_HEADERS_CRLF,
                std::ptr::null(),
                buf.as_mut_ptr() as *mut c_void,
                &mut buf_len,
                std::ptr::null_mut(),
            ) != 0
            {
                let len = (buf_len / 2) as usize;
                headers = String::from_utf16_lossy(&buf[..len.min(buf.len())]);
            }
        }
        Ok((status, headers))
    }
}

fn header_value<'a>(raw: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{}:", name.to_lowercase());
    for line in raw.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with(&prefix) {
            return Some(line[prefix.len()..].trim());
        }
    }
    None
}

fn join_url(base: &str, location: &str) -> String {
    if location.starts_with("http://") || location.starts_with("https://") {
        return location.to_string();
    }
    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let host_end = base[scheme_end..]
        .find('/')
        .map(|i| scheme_end + i)
        .unwrap_or(base.len());
    let origin = &base[..host_end];
    if location.starts_with('/') {
        format!("{}{}", origin, location)
    } else {
        format!("{}/{}", origin, location)
    }
}

/// 跟随重定向链并返回最终 URL（不下载响应体内容）。
pub fn resolve_redirect(url: &str, max_hops: usize) -> Result<String, String> {
    let mut current = url.to_string();
    for _ in 0..max_hops.max(1) {
        let (status, headers) = http_status_headers(&current)?;
        if (300..400).contains(&status) {
            if let Some(location) = header_value(&headers, "location") {
                current = join_url(&current, location);
                continue;
            }
        }
        return Ok(current);
    }
    Ok(current)
}

// ---------- 子进程 ----------
use std::os::windows::process::CommandExt;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 运行外部工具并收集 stdout+stderr（不弹控制台窗口）。
pub fn run_tool(exe: &Path, args: &[String]) -> Result<String, String> {
    let out = std::process::Command::new(exe)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法启动 {}: {}", exe.display(), e))?;
    let mut text = String::from_utf8_lossy(&out.stdout).to_string();
    let err = String::from_utf8_lossy(&out.stderr);
    if !err.trim().is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        text.push_str(&err);
    }
    Ok(text)
}

pub fn run_tool_str(exe: &Path, args: &[&str]) -> Result<String, String> {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    run_tool(exe, &owned)
}

/// 运行外部工具并返回原始 stdout 字节（用于 adb exec-out 二进制流）。
pub fn run_tool_bytes(exe: &Path, args: &[String]) -> Result<Vec<u8>, String> {
    let out = std::process::Command::new(exe)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("无法启动 {}: {}", exe.display(), e))?;
    if out.stdout.is_empty() {
        let err = String::from_utf8_lossy(&out.stderr);
        if !err.trim().is_empty() {
            return Err(err.trim().to_string());
        }
    }
    Ok(out.stdout)
}

// ---------- 路径 ----------

pub fn pack_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("All-FlashTool")
}

pub fn data_dir(name: &str) -> PathBuf {
    pack_dir().join(name)
}

/// 定位 adb/fastboot 等外部工具：配置包目录 → PATH。
pub fn tool_path(name: &str) -> Option<PathBuf> {
    let file = format!("{}.exe", name);
    for candidate in [
        pack_dir().join("platform-tools").join(&file),
        pack_dir().join(&file),
    ] {
        if candidate.exists() {
            return Some(candidate);
        }
    }
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(&file);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
}

pub fn adb_path() -> Result<PathBuf, String> {
    tool_path("adb").ok_or_else(|| "未找到 adb，请先完成环境部署".to_string())
}

pub fn fastboot_path() -> Result<PathBuf, String> {
    tool_path("fastboot").ok_or_else(|| "未找到 fastboot，请先完成环境部署".to_string())
}

// 用系统关联程序打开 URL/文件/目录。
#[link(name = "shell32")]
extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        params: *const u16,
        dir: *const u16,
        show: i32,
    ) -> isize;
}

pub fn shell_open(target: &str) -> Result<(), String> {
    let t = wide(target);
    let r = unsafe {
        ShellExecuteW(
            0,
            std::ptr::null(),
            t.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        )
    };
    if r > 32 {
        Ok(())
    } else {
        Err(format!("ShellExecuteW 失败: {}", r))
    }
}

/// 调用 Windows 自带 bsdtar 解压 zip（Win10+ 内置）。
pub fn unzip(archive: &Path, dest: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dest).map_err(|e| e.to_string())?;
    let tar = PathBuf::from(r"C:\Windows\System32\tar.exe");
    let out = run_tool_str(
        &tar,
        &[
            "-xf",
            &archive.to_string_lossy(),
            "-C",
            &dest.to_string_lossy(),
        ],
    )?;
    if dest.exists() {
        Ok(())
    } else {
        Err(format!("解压失败: {}", out))
    }
}

// ---------- 系统信息 ----------

#[repr(C)]
pub struct SystemTime {
    pub year: u16,
    pub month: u16,
    pub day_of_week: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub milliseconds: u16,
}

extern "system" {
    fn GetLocalTime(time: *mut SystemTime);
}

pub fn local_time() -> SystemTime {
    let mut t = SystemTime {
        year: 0,
        month: 0,
        day_of_week: 0,
        day: 0,
        hour: 0,
        minute: 0,
        second: 0,
        milliseconds: 0,
    };
    unsafe { GetLocalTime(&mut t) };
    t
}

#[repr(C)]
struct OsVersionInfoW {
    os_version_info_size: u32,
    major_version: u32,
    minor_version: u32,
    build_number: u32,
    platform_id: u32,
    csd_version: [u16; 128],
}

#[link(name = "ntdll")]
extern "system" {
    fn RtlGetVersion(info: *mut OsVersionInfoW) -> i32;
}

pub fn windows_version() -> String {
    let mut info = OsVersionInfoW {
        os_version_info_size: std::mem::size_of::<OsVersionInfoW>() as u32,
        major_version: 0,
        minor_version: 0,
        build_number: 0,
        platform_id: 0,
        csd_version: [0; 128],
    };
    unsafe { RtlGetVersion(&mut info) };
    let name = if info.major_version >= 10 && info.build_number >= 22000 {
        "Windows 11".to_string()
    } else if info.major_version >= 10 {
        "Windows 10".to_string()
    } else {
        format!("Windows {}.{}", info.major_version, info.minor_version)
    };
    format!("{} (Build {})", name, info.build_number)
}

// ---------- 地区（与前端登录页同一数据源） ----------

fn province_base(province: &str) -> String {
    for suffix in [
        "维吾尔自治区",
        "壮族自治区",
        "回族自治区",
        "自治区",
        "特别行政区",
        "省",
        "市",
    ] {
        if let Some(base) = province.strip_suffix(suffix) {
            return base.to_string();
        }
    }
    province.to_string()
}

fn normalize_province(raw: &str) -> String {
    let p = raw.trim();
    if p.is_empty() {
        return String::new();
    }
    if p.contains("自治区") || p.contains("特别行政区") || p.ends_with('省') || p.ends_with('市')
    {
        return p.to_string();
    }
    match p {
        "内蒙古" => "内蒙古自治区".to_string(),
        "广西" => "广西壮族自治区".to_string(),
        "西藏" => "西藏自治区".to_string(),
        "宁夏" => "宁夏回族自治区".to_string(),
        "新疆" => "新疆维吾尔自治区".to_string(),
        "香港" => "香港特别行政区".to_string(),
        "澳门" => "澳门特别行政区".to_string(),
        "北京" | "上海" | "天津" | "重庆" => format!("{}市", p),
        _ => format!("{}省", p),
    }
}

fn normalize_city(raw: &str, province: &str) -> String {
    let mut city = raw.trim().to_string();
    if city.is_empty() {
        return String::new();
    }
    let base = province_base(province);
    if !base.is_empty() {
        if let Some(rest) = city.strip_prefix(&base) {
            let rest =
                rest.trim_start_matches(|c: char| c == '省' || c == '市' || c.is_whitespace());
            if !rest.is_empty() {
                city = rest.to_string();
            }
        }
    }
    if city == province || city == base {
        return String::new();
    }
    if city.ends_with('市')
        || city.ends_with('盟')
        || city.ends_with("地区")
        || city.ends_with("自治州")
    {
        city
    } else {
        format!("{}市", city)
    }
}

pub fn format_region(province: &str, city: &str) -> String {
    let p = normalize_province(province);
    let c = normalize_city(city, &p);
    match (p.is_empty(), c.is_empty()) {
        (true, true) => String::new(),
        (false, true) => p,
        (true, false) => c,
        (false, false) => format!("{} · {}", p, c),
    }
}

fn region_cache_path() -> PathBuf {
    data_dir("region.json")
}

fn read_region_cache() -> Option<String> {
    let text = std::fs::read_to_string(region_cache_path()).ok()?;
    let v = serde_json::from_str::<Value>(&text).ok()?;
    let region = v
        .get("region")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if region.is_empty() {
        None
    } else {
        Some(region)
    }
}

fn write_region_cache(region: &str) {
    let path = region_cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(
        &path,
        serde_json::to_vec(&json!({"region": region})).unwrap_or_default(),
    );
}

fn fetch_zxinc_region() -> Option<String> {
    let body = http_get("https://ip.zxinc.org/api.php?type=json")?;
    let v = parse_json(&body)?;
    if v.get("code").and_then(Value::as_i64) != Some(0) {
        return None;
    }
    let country = v
        .get("data")?
        .get("country")
        .and_then(Value::as_str)
        .unwrap_or("");
    let parts: Vec<String> = country
        .replace('–', "-")
        .replace('—', "-")
        .split('-')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let skip = if parts.first().map(|s| s == "中国").unwrap_or(false) {
        1
    } else {
        0
    };
    let region = format_region(
        parts.get(skip).map(|s| s.as_str()).unwrap_or(""),
        parts.get(skip + 1).map(|s| s.as_str()).unwrap_or(""),
    );
    if region.is_empty() {
        None
    } else {
        Some(region)
    }
}

pub fn fetch_region() -> Option<String> {
    let mut result: Option<String> = None;
    // 主源：cz88
    if let Some(body) = http_get("https://cz88.net/api/cz88/ip/base?ip=") {
        if let Some(v) = parse_json(&body) {
            let ok = v.get("success").and_then(Value::as_bool).unwrap_or(false)
                || v.get("code").and_then(Value::as_i64) == Some(200);
            if ok {
                if let Some(data) = v.get("data") {
                    let region = format_region(
                        data.get("province").and_then(Value::as_str).unwrap_or(""),
                        data.get("city").and_then(Value::as_str).unwrap_or(""),
                    );
                    if !region.is_empty() {
                        result = Some(region);
                    }
                }
            }
        }
    }
    // 备用 1：pconline
    if result.is_none() {
        if let Some(body) = http_get("https://whois.pconline.com.cn/ipJson.jsp?json=true") {
            if let Some(v) = parse_json(&body) {
                let region = format_region(
                    v.get("pro").and_then(Value::as_str).unwrap_or(""),
                    v.get("city").and_then(Value::as_str).unwrap_or(""),
                );
                if !region.is_empty() {
                    result = Some(region);
                }
            }
        }
    }
    // 备用 2：zxinc
    if result.is_none() {
        result = fetch_zxinc_region();
    }
    if let Some(region) = &result {
        write_region_cache(region);
        return result;
    }
    // 全部在线源失败：回退到上次成功缓存
    read_region_cache()
}

// ---------- 文件对话框 ----------

#[repr(C)]
struct OpenFileNameW {
    struct_size: u32,
    hwnd_owner: isize,
    instance: isize,
    filter: *const u16,
    custom_filter: *mut u16,
    max_cust_filter: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_dir: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    def_ext: *const u16,
    cust_data: isize,
    hook: isize,
    template_name: *const u16,
    reserved: *mut c_void,
    reserved_dw: u32,
    flags_ex: u32,
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(ofn: *mut OpenFileNameW) -> i32;
}

/// 打开文件选择对话框，返回选中的文件路径（支持多选）。
pub fn browse_files(
    title: &str,
    filter_pairs: &[(&str, &str)],
    initial_dir: Option<&str>,
    multi: bool,
) -> Vec<String> {
    const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
    const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
    const OFN_ALLOWMULTISELECT: u32 = 0x0000_0200;
    const OFN_EXPLORER: u32 = 0x0008_0000;
    let mut filter = String::new();
    for (label, pattern) in filter_pairs {
        filter.push_str(label);
        filter.push('\0');
        filter.push_str(pattern);
        filter.push('\0');
    }
    filter.push('\0');
    let filter_w: Vec<u16> = filter.encode_utf16().chain(std::iter::once(0)).collect();
    let title_w = wide(title);
    let dir_w = initial_dir.map(wide);
    let mut buffer = vec![0u16; 32768];
    let mut ofn = OpenFileNameW {
        struct_size: std::mem::size_of::<OpenFileNameW>() as u32,
        hwnd_owner: 0,
        instance: 0,
        filter: filter_w.as_ptr(),
        custom_filter: std::ptr::null_mut(),
        max_cust_filter: 0,
        filter_index: 1,
        file: buffer.as_mut_ptr(),
        max_file: buffer.len() as u32,
        file_title: std::ptr::null_mut(),
        max_file_title: 0,
        initial_dir: dir_w
            .as_ref()
            .map(|d| d.as_ptr())
            .unwrap_or(std::ptr::null()),
        title: title_w.as_ptr(),
        flags: OFN_PATHMUSTEXIST
            | OFN_FILEMUSTEXIST
            | OFN_EXPLORER
            | if multi { OFN_ALLOWMULTISELECT } else { 0 },
        file_offset: 0,
        file_extension: 0,
        def_ext: std::ptr::null(),
        cust_data: 0,
        hook: 0,
        template_name: std::ptr::null(),
        reserved: std::ptr::null_mut(),
        reserved_dw: 0,
        flags_ex: 0,
    };
    let ok = unsafe { GetOpenFileNameW(&mut ofn) };
    if ok == 0 {
        return Vec::new();
    }
    // 单选：完整路径；多选：目录\0文件1\0文件2\0\0
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0usize;
    for i in 0..buffer.len() {
        if buffer[i] == 0 {
            if i == start {
                break;
            }
            parts.push(String::from_utf16_lossy(&buffer[start..i]));
            start = i + 1;
        }
    }
    if parts.is_empty() {
        return parts;
    }
    if parts.len() == 1 {
        return vec![parts[0].clone()];
    }
    let dir = &parts[0];
    parts[1..]
        .iter()
        .map(|name| {
            if name.contains(':') {
                name.clone()
            } else {
                PathBuf::from(dir).join(name).to_string_lossy().to_string()
            }
        })
        .collect()
}
