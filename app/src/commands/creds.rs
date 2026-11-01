//! 登录凭据本地存储：DPAPI 加密落盘（CryptProtectData，仅当前 Windows 用户可解）。

use super::util::*;
use serde_json::{json, Value};
use std::ffi::c_void;
use std::path::PathBuf;

#[repr(C)]
struct DataBlob {
    cb_data: u32,
    pb_data: *mut u8,
}

#[link(name = "crypt32")]
extern "system" {
    fn CryptProtectData(
        input: *const DataBlob,
        descr: *const u16,
        entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *const c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        input: *const DataBlob,
        descr: *mut *mut u16,
        entropy: *const DataBlob,
        reserved: *mut c_void,
        prompt: *const c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(mem: isize) -> isize;
}

fn credentials_path() -> PathBuf {
    data_dir("account").join("credentials.bin")
}

fn dpapi_protect(data: &[u8]) -> Option<Vec<u8>> {
    let mut input = DataBlob {
        cb_data: data.len() as u32,
        pb_data: data.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    unsafe {
        if CryptProtectData(
            &mut input,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            &mut output,
        ) == 0
        {
            return None;
        }
        let slice = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize);
        let out = slice.to_vec();
        LocalFree(output.pb_data as isize);
        Some(out)
    }
}

fn dpapi_unprotect(data: &[u8]) -> Option<Vec<u8>> {
    let mut input = DataBlob {
        cb_data: data.len() as u32,
        pb_data: data.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        cb_data: 0,
        pb_data: std::ptr::null_mut(),
    };
    unsafe {
        if CryptUnprotectData(
            &mut input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null(),
            0,
            &mut output,
        ) == 0
        {
            return None;
        }
        let slice = std::slice::from_raw_parts(output.pb_data, output.cb_data as usize);
        let out = slice.to_vec();
        LocalFree(output.pb_data as isize);
        Some(out)
    }
}

#[tauri::command]
pub fn load_login_credentials() -> Value {
    let path = credentials_path();
    let empty = json!({"remember": false, "username": "", "password": ""});
    let raw = match std::fs::read(&path) {
        Ok(r) => r,
        Err(_) => return empty,
    };
    let plain = match dpapi_unprotect(&raw) {
        Some(p) => p,
        None => return empty,
    };
    match serde_json::from_slice::<Value>(&plain) {
        Ok(v) => v,
        Err(_) => empty,
    }
}

#[tauri::command]
pub fn save_login_credentials(
    password: Option<Value>,
    username: Option<Value>,
) -> Result<Value, String> {
    let username = username
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let password = password
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_default();
    let payload = json!({"remember": true, "username": username, "password": password});
    let plain = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let encrypted = dpapi_protect(&plain).ok_or_else(|| "DPAPI 加密失败".to_string())?;
    let path = credentials_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, encrypted).map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn clear_login_credentials() -> Value {
    let _ = std::fs::remove_file(credentials_path());
    json!({"ok": true})
}

#[tauri::command]
pub fn logout() -> Value {
    json!({"ok": true})
}
