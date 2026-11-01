//! 杂项命令：loader 扫描、GPT 校验、网络安全预检、弹幕状态等。

use super::util::*;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

// ---------- EDL loader 本地扫描 ----------

fn scan_loaders(dir: &PathBuf) -> Vec<Value> {
    let mut items = Vec::new();
    if let Ok(read) = std::fs::read_dir(dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if ext != "mbn" && ext != "elf" && ext != "bin" {
                continue;
            }
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            items.push(json!({
                "name": path.file_name().unwrap_or_default().to_string_lossy(),
                "path": path.to_string_lossy(),
                "size": size
            }));
        }
    }
    items
}

#[tauri::command]
pub fn edl_list_cached_loaders() -> Value {
    json!(scan_loaders(&data_dir("loaders")))
}

#[tauri::command]
pub fn edl_list_local_loaders() -> Value {
    json!(scan_loaders(&data_dir("loaders")))
}

#[tauri::command]
pub fn loader_download_start() -> Result<Value, String> {
    Err("loader 云端下载源未接入".to_string())
}

#[tauri::command]
pub fn loader_download_cancel() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn ocdt_list_models() -> Value {
    json!([])
}

// ---------- GPT 校验 ----------

#[tauri::command]
pub fn qc_gpt_validate(path: Option<Value>) -> Result<Value, String> {
    let path = opt_str(&path).ok_or_else(|| "缺少 path".to_string())?;
    let bytes = std::fs::read(&path).map_err(|e| format!("读取失败: {}", e))?;
    // GPT 头位于 LBA1（偏移 512），magic "EFI PART"
    let valid = bytes.len() > 1024 && &bytes[512..520] == b"EFI PART";
    Ok(json!({"valid": valid, "size": bytes.len(), "path": path}))
}

// ---------- 网络安全预检（代理/抓包进程检测） ----------

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        key: isize,
        sub: *const u16,
        options: u32,
        access: u32,
        result: *mut isize,
    ) -> i32;
    fn RegQueryValueExW(
        key: isize,
        name: *const u16,
        reserved: *mut u32,
        ty: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    fn RegCloseKey(key: isize) -> i32;
}

const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as isize;
const KEY_READ: u32 = 0x0002_0019;

fn system_proxy_enabled() -> bool {
    let sub = wide_null(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings");
    let name = wide_null("ProxyEnable");
    let mut key: isize = 0;
    unsafe {
        if RegOpenKeyExW(HKEY_CURRENT_USER, sub.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return false;
        }
        let mut value: u32 = 0;
        let mut len = 4u32;
        let rc = RegQueryValueExW(
            key,
            name.as_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut value as *mut u32 as *mut u8,
            &mut len,
        );
        RegCloseKey(key);
        rc == 0 && value != 0
    }
}

fn sniffer_processes() -> Vec<String> {
    let tasklist = PathBuf::from(r"C:\Windows\System32\tasklist.exe");
    let mut found = Vec::new();
    if let Ok(out) = run_tool_str(&tasklist, &["/FO", "CSV", "/NH"]) {
        let lower = out.to_lowercase();
        for name in [
            "fiddler",
            "charles",
            "mitmproxy",
            "wireshark",
            "httpdebugger",
            "burp",
        ] {
            if lower.contains(name) {
                found.push(name.to_string());
            }
        }
    }
    found
}

#[tauri::command]
pub fn network_security_preflight() -> Value {
    let proxy = system_proxy_enabled();
    let sniffers = sniffer_processes();
    json!({"ok": !proxy && sniffers.is_empty(), "proxy_enabled": proxy, "sniffers": sniffers})
}

// ---------- 爱心弹幕（娱乐功能，本地状态） ----------

static DANMAKU_CONSUMED: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn love_danmaku_show() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn love_danmaku_consume() -> Value {
    // 未消费过则返回一次有效载荷，之后返回 null
    if DANMAKU_CONSUMED.swap(true, Ordering::SeqCst) {
        Value::Null
    } else {
        json!({"ok": true})
    }
}

#[tauri::command]
pub fn love_danmaku_overlay_show() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn love_danmaku_finished() -> Value {
    DANMAKU_CONSUMED.store(true, Ordering::SeqCst);
    json!({"ok": true})
}

#[tauri::command]
pub fn love_danmaku_prepare() -> Value {
    DANMAKU_CONSUMED.store(false, Ordering::SeqCst);
    json!({"ok": true})
}

// ---------- 账号/云服务（私有服务器协议未恢复） ----------

#[tauri::command]
pub fn refresh_auth() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn security_login_anomaly() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn one_click_call() -> Result<Value, String> {
    Err("一键呼叫服务未接入".to_string())
}

#[tauri::command]
pub fn device_op_redeem(op: Option<Value>) -> Result<Value, String> {
    Err("设备操作兑换服务未接入".to_string())
}

#[tauri::command]
pub fn fdl_authorize(kami: Option<Value>) -> Result<Value, String> {
    Err("FDL 授权服务未接入".to_string())
}

#[tauri::command]
pub fn cloud_gate(op: Option<Value>, path: Option<Value>) -> Result<Value, String> {
    Err("云端网关未接入".to_string())
}

#[tauri::command]
pub fn cloud_driver_run(kind: Option<Value>) -> Result<Value, String> {
    Err("云驱动服务未接入".to_string())
}

#[tauri::command]
pub fn cloud_driver_download(kind: Option<Value>) -> Result<Value, String> {
    Err("云驱动下载源未接入".to_string())
}

#[tauri::command]
pub fn cloud_driver_install(session_id: Option<Value>) -> Result<Value, String> {
    Err("云驱动安装服务未接入".to_string())
}

#[tauri::command]
pub fn cloud_driver_dismiss(session_id: Option<Value>) -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn cloud_driver_cancel() -> Value {
    json!({"ok": true})
}

// ---------- 库信息读取 ----------

#[tauri::command]
pub fn utility_adb_read_lib_info() -> Result<Value, String> {
    let exe = adb_path()?;
    let out = run_tool_str(&exe, &["shell", "getprop", "ro.product.cpu.abi"])?;
    Ok(json!({"ok": true, "abi": out.trim()}))
}
