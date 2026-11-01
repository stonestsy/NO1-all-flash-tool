//! 应用更新：版本检查、更新包下载（SHA256 校验）、安装。

use super::util::*;
use serde_json::{json, Value};
use std::path::PathBuf;

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

/// 版本对象契约：前端 Kn() 读 .blocked，Ns() 读 show_update_popup/latest_version。
#[tauri::command]
pub fn check_update() -> Value {
    json!({
        "blocked": false,
        "show_update_popup": false,
        "latest_version": env!("CARGO_PKG_VERSION"),
        "current_version": env!("CARGO_PKG_VERSION"),
        "update_content": "",
        "update_time": "",
        "download_url": ""
    })
}

#[tauri::command]
pub fn check_version_prelogin() -> Value {
    json!({"blocked": false, "latest_version": env!("CARGO_PKG_VERSION"), "show_update_popup": false})
}

fn sha256_of(path: &PathBuf) -> Result<String, String> {
    let certutil = PathBuf::from(r"C:\Windows\System32\certutil.exe");
    let out = run_tool_str(&certutil, &["-hashfile", &path.to_string_lossy(), "SHA256"])?;
    for line in out.lines() {
        let compact: String = line.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if compact.len() == 64 {
            return Ok(compact.to_lowercase());
        }
    }
    Err("无法计算 SHA256".to_string())
}

#[tauri::command]
pub async fn download_app_update(
    expected_sha256: Option<Value>,
    url: Option<Value>,
) -> Result<Value, String> {
    let url = opt_str(&url).ok_or_else(|| "缺少 url".to_string())?;
    let expected = opt_str(&expected_sha256).unwrap_or_default().to_lowercase();
    let dest = data_dir("update").join("update-installer.exe");
    let dest2 = dest.clone();
    tauri::async_runtime::spawn_blocking(move || download_file(&url, &dest2))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| format!("下载更新失败: {}", e))?;
    if !expected.is_empty() {
        let actual = sha256_of(&dest)?;
        if actual != expected {
            let _ = std::fs::remove_file(&dest);
            return Err(format!(
                "更新包 SHA256 校验失败（期望 {}，实际 {}）",
                expected, actual
            ));
        }
    }
    Ok(json!({"ok": true, "file_path": dest.to_string_lossy()}))
}

#[tauri::command]
pub fn apply_app_update(installer_path: Option<Value>) -> Result<Value, String> {
    let path = opt_str(&installer_path).ok_or_else(|| "缺少 installerPath".to_string())?;
    if !PathBuf::from(&path).exists() {
        return Err("安装包不存在".to_string());
    }
    shell_open(&path)?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn server_offline_age_secs() -> Value {
    json!(0)
}
