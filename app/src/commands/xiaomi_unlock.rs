//! 小米 BL 解锁：fastboot 真实解锁命令 + 本地会话管理。
//! 小米账号登录（扫码/密码/验证码）协议未恢复，返回明确错误。

use super::util::*;
use serde_json::{json, Value};

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn session_path() -> std::path::PathBuf {
    data_dir("xiaomi-unlock").join("session.json")
}

fn fastboot_devices() -> Vec<String> {
    let mut serials = Vec::new();
    if let Ok(exe) = fastboot_path() {
        if let Ok(out) = run_tool_str(&exe, &["devices"]) {
            for line in out.lines() {
                if let Some(serial) = line.split_whitespace().next() {
                    if !serial.is_empty() {
                        serials.push(serial.to_string());
                    }
                }
            }
        }
    }
    serials
}

#[tauri::command]
pub fn xiaomi_unlock_session_reset() -> Value {
    let _ = std::fs::remove_file(session_path());
    json!({"ok": true})
}

#[tauri::command]
pub fn xiaomi_unlock_restore_session() -> Value {
    match std::fs::read_to_string(session_path()) {
        Ok(text) => serde_json::from_str(&text).unwrap_or(Value::Null),
        Err(_) => Value::Null,
    }
}

#[tauri::command]
pub fn xiaomi_unlock_logout() -> Value {
    let _ = std::fs::remove_file(session_path());
    json!({"ok": true})
}

#[tauri::command]
pub fn xiaomi_unlock_preview() -> Result<Value, String> {
    let serials = fastboot_devices();
    if serials.is_empty() {
        return Err("未检测到 Fastboot 设备，请先进入 fastboot 模式".to_string());
    }
    let exe = fastboot_path()?;
    let serial = &serials[0];
    let out = run_tool_str(&exe, &["-s", serial, "getvar", "unlocked"])?;
    let unlocked = out.to_lowercase().contains("unlocked: yes");
    Ok(json!({"serial": serial, "unlocked": unlocked, "output": out}))
}

#[tauri::command]
pub fn xiaomi_unlock_bl() -> Result<Value, String> {
    let serials = fastboot_devices();
    if serials.is_empty() {
        return Err("未检测到 Fastboot 设备".to_string());
    }
    let exe = fastboot_path()?;
    let serial = &serials[0];
    let out = run_tool_str(&exe, &["-s", serial, "flashing", "unlock"])?;
    Ok(json!({"ok": true, "serial": serial, "output": out}))
}

// ---------- 小米账号协议（未恢复） ----------

#[tauri::command]
pub fn xiaomi_unlock_qr_start() -> Result<Value, String> {
    Err("小米账号扫码登录协议未恢复".to_string())
}

#[tauri::command]
pub fn xiaomi_unlock_qr_poll(args: Option<Value>) -> Result<Value, String> {
    Err("小米账号扫码登录协议未恢复".to_string())
}

#[tauri::command]
pub fn xiaomi_unlock_password_login(args: Option<Value>) -> Result<Value, String> {
    Err("小米账号密码登录协议未恢复".to_string())
}

#[tauri::command]
pub fn xiaomi_unlock_submit_captcha(args: Option<Value>) -> Result<Value, String> {
    Err("小米账号验证码协议未恢复".to_string())
}

#[tauri::command]
pub fn xiaomi_unlock_2fa_send(args: Option<Value>) -> Result<Value, String> {
    Err("小米账号二次验证协议未恢复".to_string())
}

#[tauri::command]
pub fn xiaomi_unlock_2fa_verify(args: Option<Value>) -> Result<Value, String> {
    Err("小米账号二次验证协议未恢复".to_string())
}

#[tauri::command]
pub fn xiaomi_unlock_set_zone(args: Option<Value>) -> Result<Value, String> {
    Err("小米账号区域设置协议未恢复".to_string())
}
