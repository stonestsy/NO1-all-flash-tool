//! QCN（高通 NV 备份）相关：串口枚举、QCN 文件解析、DIAG 开启。
//! 真正的备份/写回依赖 QPST，未安装时返回明确错误。

use super::util::*;
use serde_json::{json, Value};
use std::path::PathBuf;

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

// ---------- 串口枚举（注册表 SERIALCOMM） ----------

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        key: isize,
        sub: *const u16,
        options: u32,
        access: u32,
        result: *mut isize,
    ) -> i32;
    fn RegEnumValueW(
        key: isize,
        index: u32,
        name: *mut u16,
        name_len: *mut u32,
        reserved: *mut u32,
        ty: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    fn RegCloseKey(key: isize) -> i32;
}

const HKEY_LOCAL_MACHINE: isize = 0x8000_0002u32 as isize;
const KEY_READ: u32 = 0x0002_0019;

fn enum_com_ports() -> Vec<String> {
    let sub = wide_null(r"HARDWARE\DEVICEMAP\SERIALCOMM");
    let mut key: isize = 0;
    let mut ports = Vec::new();
    unsafe {
        if RegOpenKeyExW(HKEY_LOCAL_MACHINE, sub.as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return ports;
        }
        let mut index = 0u32;
        loop {
            let mut name = [0u16; 256];
            let mut name_len = 256u32;
            let mut data = [0u8; 512];
            let mut data_len = 512u32;
            let mut ty = 0u32;
            let rc = RegEnumValueW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                &mut ty,
                data.as_mut_ptr(),
                &mut data_len,
            );
            if rc != 0 {
                break;
            }
            let len = data
                .iter()
                .position(|b| *b == 0)
                .unwrap_or(data_len as usize);
            let port = String::from_utf8_lossy(&data[..len]).to_string();
            if port.starts_with("COM") {
                ports.push(port);
            }
            index += 1;
        }
        RegCloseKey(key);
    }
    ports.sort_by_key(|p| p.trim_start_matches("COM").parse::<u32>().unwrap_or(0));
    ports
}

#[tauri::command]
pub fn qcn_list_ports() -> Value {
    json!(enum_com_ports())
}

#[tauri::command]
pub fn qcn_detect_port() -> Value {
    match enum_com_ports().first() {
        Some(p) => Value::String(p.clone()),
        None => Value::Null,
    }
}

#[tauri::command]
pub fn qcn_check_tool() -> Value {
    let candidates = [
        r"C:\Program Files (x86)\Qualcomm\QPST\bin\QPSTConfig.exe",
        r"C:\Program Files\Qualcomm\QPST\bin\QPSTConfig.exe",
    ];
    for c in candidates {
        if PathBuf::from(c).exists() {
            return json!({"available": true, "path": c});
        }
    }
    json!({"available": false, "path": ""})
}

// ---------- QCN 文件解析 ----------

fn read_qcn_text(path: &str) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("读取失败: {}", e))?;
    if bytes.iter().take(64).any(|b| *b == 0) {
        return Err("暂不支持二进制 QCN（仅支持文本/XML 格式）".to_string());
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

#[tauri::command]
pub fn qcn_parse_imei(qcn_path: Option<Value>) -> Result<Value, String> {
    let path = opt_str(&qcn_path).ok_or_else(|| "缺少 qcnPath".to_string())?;
    let text = read_qcn_text(&path)?;
    // 文本 QCN 中 IMEI 以 15 位数字形式出现
    for token in text.split(|c: char| !c.is_ascii_digit()) {
        if token.len() == 15 && token.starts_with(|c: char| c.is_ascii_digit()) {
            if !token.chars().all(|c| c == '0') {
                return Ok(json!({"imei": token}));
            }
        }
    }
    Err("未在 QCN 中找到 IMEI".to_string())
}

#[tauri::command]
pub fn qcn_parse_identity(qcn_path: Option<Value>) -> Result<Value, String> {
    let path = opt_str(&qcn_path).ok_or_else(|| "缺少 qcnPath".to_string())?;
    let text = read_qcn_text(&path)?;
    let imei = qcn_parse_imei(Some(Value::String(path.clone())))
        .ok()
        .and_then(|v| v.get("imei").and_then(Value::as_str).map(|s| s.to_string()))
        .unwrap_or_default();
    Ok(json!({"imei": imei, "size": text.len()}))
}

#[tauri::command]
pub fn qcn_enable_diag(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let exe = adb_path()?;
    let out = run_tool_str(
        &exe,
        &[
            "-s",
            &serial,
            "shell",
            "su",
            "-c",
            "setprop persist.sys.usb.config diag,adb",
        ],
    )?;
    Ok(json!({"ok": true, "output": out}))
}

#[tauri::command]
pub fn qcn_backup(payload: Option<Value>) -> Result<Value, String> {
    let tool = qcn_check_tool();
    if tool.get("available").and_then(Value::as_bool) != Some(true) {
        return Err("未检测到 QPST，无法进行 QCN 备份".to_string());
    }
    Err("QPST 备份流程尚未实现".to_string())
}

#[tauri::command]
pub fn qcn_restore(payload: Option<Value>) -> Result<Value, String> {
    let tool = qcn_check_tool();
    if tool.get("available").and_then(Value::as_bool) != Some(true) {
        return Err("未检测到 QPST，无法进行 QCN 写回".to_string());
    }
    Err("QPST 写回流程尚未实现".to_string())
}

#[tauri::command]
pub fn qcn_cancel() -> Value {
    json!({"ok": true})
}

// ---------- EDL 端口复用 ----------

#[tauri::command]
pub fn edl_list_ports() -> Value {
    json!(enum_com_ports())
}
