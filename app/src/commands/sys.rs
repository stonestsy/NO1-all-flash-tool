//! 系统级命令：输入法、剪贴板、打开程序/链接、进程退出、登录环境信息。

use super::util::*;
use serde_json::{json, Value};
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};

#[link(name = "user32")]
extern "system" {
    fn GetKeyboardLayout(thread: u32) -> isize;
    fn ActivateKeyboardLayout(hkl: isize, flags: u32) -> isize;
    fn LoadKeyboardLayoutW(name: *const u16, flags: u32) -> isize;
    fn OpenClipboard(hwnd: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn SetClipboardData(format: u32, mem: isize) -> isize;
    fn GetClipboardData(format: u32) -> isize;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(mem: isize) -> *mut c_void;
    fn GlobalUnlock(mem: isize) -> i32;
    fn GlobalSize(mem: isize) -> usize;
}

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;
const KLF_ACTIVATE: u32 = 0x0000_0001;

static SAVED_HKL: AtomicIsize = AtomicIsize::new(0);

#[tauri::command]
pub fn force_english_input() -> Value {
    unsafe {
        let current = GetKeyboardLayout(0);
        SAVED_HKL.store(current, Ordering::SeqCst);
        let name = wide_null("00000409");
        let hkl = LoadKeyboardLayoutW(name.as_ptr(), KLF_ACTIVATE);
        if hkl != 0 {
            ActivateKeyboardLayout(hkl, 0);
        }
    }
    json!({"ok": true})
}

#[tauri::command]
pub fn restore_input_method() -> Value {
    let saved = SAVED_HKL.load(Ordering::SeqCst);
    if saved != 0 {
        unsafe { ActivateKeyboardLayout(saved, 0) };
    }
    json!({"ok": true})
}

#[tauri::command]
pub fn health_check() -> Value {
    json!({"ok": true, "version": env!("CARGO_PKG_VERSION"), "os": windows_version()})
}

#[tauri::command]
pub fn app_shutdown_cleanup() -> Value {
    // 不主动结束外部 adb/fastboot 进程（避免影响其他工具占用）
    json!({"ok": true})
}

#[tauri::command]
pub fn app_exit() -> Value {
    std::process::exit(0);
}

#[tauri::command]
pub fn open_device_manager() -> Result<Value, String> {
    shell_open("devmgmt.msc")?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn open_url(url: Option<Value>) -> Result<Value, String> {
    let url = url
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    if url.is_empty() {
        return Err("url 为空".to_string());
    }
    shell_open(&url)?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn open_official_site() -> Result<Value, String> {
    shell_open("https://www.all-flashtool.online")?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn open_cmd() -> Result<Value, String> {
    std::process::Command::new("cmd.exe")
        .current_dir(pack_dir())
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn clipboard_write_text(text: Option<Value>) -> Result<Value, String> {
    let text = text
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let mut data: Vec<u16> = text.encode_utf16().collect();
    data.push(0);
    unsafe {
        if OpenClipboard(0) == 0 {
            return Err("无法打开剪贴板".to_string());
        }
        let result = (|| {
            EmptyClipboard();
            let bytes = data.len() * 2;
            let mem = GlobalAlloc(GMEM_MOVEABLE, bytes);
            if mem == 0 {
                return Err("GlobalAlloc 失败".to_string());
            }
            let ptr = GlobalLock(mem) as *mut u16;
            if ptr.is_null() {
                return Err("GlobalLock 失败".to_string());
            }
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
            GlobalUnlock(mem);
            if SetClipboardData(CF_UNICODETEXT, mem) == 0 {
                return Err("SetClipboardData 失败".to_string());
            }
            Ok(())
        })();
        CloseClipboard();
        result?;
    }
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn clipboard_read_text() -> Value {
    unsafe {
        if OpenClipboard(0) == 0 {
            return Value::String(String::new());
        }
        let mut out = String::new();
        let mem = GetClipboardData(CF_UNICODETEXT);
        if mem != 0 {
            let ptr = GlobalLock(mem) as *const u16;
            if !ptr.is_null() {
                let max = GlobalSize(mem) / 2;
                let mut len = 0usize;
                while len < max && *ptr.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                out = String::from_utf16_lossy(slice);
                GlobalUnlock(mem);
            }
        }
        CloseClipboard();
        Value::String(out)
    }
}

// ---------- 登录环境信息（region/weather/isp/network） ----------

fn normalize_isp(raw: &str) -> String {
    let s = raw.trim();
    if s.contains("移动") {
        "中国移动".to_string()
    } else if s.contains("联通") {
        "中国联通".to_string()
    } else if s.contains("电信") {
        "中国电信".to_string()
    } else if s.contains("广电") {
        "中国广电".to_string()
    } else {
        s.to_string()
    }
}

fn weather_text(code: i64) -> &'static str {
    match code {
        0 => "晴",
        1 | 2 => "少云",
        3 => "多云",
        45 | 48 => "雾",
        51 | 53 | 55 | 56 | 57 => "毛毛雨",
        61 => "小雨",
        63 => "中雨",
        65 => "大雨",
        66 | 67 => "冻雨",
        71 | 73 | 75 | 77 => "雪",
        80 | 81 | 82 => "阵雨",
        85 | 86 => "阵雪",
        95 | 96 | 99 => "雷阵雨",
        _ => "天气",
    }
}

fn fetch_weather(lat: f64, lon: f64) -> Option<String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,weather_code&timezone=auto",
        lat, lon
    );
    let body = http_get(&url)?;
    let v = parse_json(&body)?;
    let current = v.get("current")?;
    let temp = current.get("temperature_2m").and_then(Value::as_f64);
    let code = current
        .get("weather_code")
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    let text = weather_text(code);
    match temp {
        Some(t) => Some(format!("{} · {}°C", text, t.round() as i64)),
        None => Some(text.to_string()),
    }
}

fn fetch_env_meta() -> Value {
    let mut region = String::new();
    let mut isp = String::new();
    let mut network = String::new();
    let mut weather = String::new();
    let mut lat_lon: Option<(f64, f64)> = None;

    if let Some(body) = http_get("https://cz88.net/api/cz88/ip/base?ip=") {
        if let Some(v) = parse_json(&body) {
            if let Some(data) = v.get("data") {
                region = format_region(
                    data.get("province").and_then(Value::as_str).unwrap_or(""),
                    data.get("city").and_then(Value::as_str).unwrap_or(""),
                );
                isp = normalize_isp(data.get("isp").and_then(Value::as_str).unwrap_or(""));
                let nt = data
                    .get("netWorkType")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim();
                if !nt.is_empty() && nt != "暂未发现" && nt != "未知" {
                    network = nt.to_string();
                }
                if let Some(locations) = data.get("locations").and_then(Value::as_array) {
                    let mut sorted: Vec<&Value> = locations.iter().collect();
                    sorted.sort_by_key(|l| {
                        l.get("radius").and_then(Value::as_f64).unwrap_or(1e9) as i64
                    });
                    if let Some(first) = sorted.first() {
                        let lat = first.get("latitude").and_then(Value::as_f64);
                        let lon = first.get("longitude").and_then(Value::as_f64);
                        if let (Some(a), Some(b)) = (lat, lon) {
                            lat_lon = Some((a, b));
                        }
                    }
                }
            }
        }
    }
    if region.is_empty() {
        if let Some(body) = http_get("https://whois.pconline.com.cn/ipJson.jsp?json=true") {
            if let Some(v) = parse_json(&body) {
                region = format_region(
                    v.get("pro").and_then(Value::as_str).unwrap_or(""),
                    v.get("city").and_then(Value::as_str).unwrap_or(""),
                );
                if isp.is_empty() {
                    isp = normalize_isp(v.get("addr").and_then(Value::as_str).unwrap_or(""));
                }
            }
        }
    }
    if let Some((lat, lon)) = lat_lon {
        if let Some(w) = fetch_weather(lat, lon) {
            weather = w;
        }
    }
    json!({"region": region, "weather": weather, "isp": isp, "network": network})
}

#[tauri::command]
pub async fn get_login_env_meta() -> Value {
    tauri::async_runtime::spawn_blocking(fetch_env_meta)
        .await
        .unwrap_or_else(|_| json!({"region": "", "weather": "", "isp": "", "network": ""}))
}
