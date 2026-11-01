//! ROOT/环境隐藏/越狱相关：ADB 可实现的部分真实实现，内核漏洞链返回明确错误。

use super::util::*;
use serde_json::{json, Value};

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

fn opt_strs(v: &Option<Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect(),
        Some(Value::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn adb_shell(serial: &str, args: &[&str]) -> Result<String, String> {
    let exe = adb_path()?;
    let mut full: Vec<String> = vec!["-s".into(), serial.into(), "shell".into()];
    full.extend(args.iter().map(|a| a.to_string()));
    run_tool(&exe, &full)
}

fn list_packages(serial: &str) -> Result<Vec<String>, String> {
    let out = adb_shell(serial, &["pm", "list", "packages"])?;
    Ok(out
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:").map(|s| s.to_string()))
        .collect())
}

#[tauri::command]
pub fn env_hide_detect(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let packages = list_packages(&serial)?;
    let known = [
        "com.topjohnwu.magisk",
        "io.github.vvb2060.magisk",
        "com.byyoung.setting",
        "com.rs.full",
    ];
    let found: Vec<&str> = packages
        .iter()
        .filter(|p| known.contains(&p.as_str()))
        .map(|p| p.as_str())
        .collect();
    Ok(json!({"detected": !found.is_empty(), "packages": found, "total": packages.len()}))
}

#[tauri::command]
pub fn env_hide_list_apks(scheme: Option<Value>) -> Result<Value, String> {
    let exe = adb_path()?;
    let out = run_tool_str(&exe, &["shell", "pm", "list", "packages"])?;
    let items: Vec<Value> = out
        .lines()
        .filter_map(|l| l.trim().strip_prefix("package:"))
        .map(|p| json!({"package": p, "name": p}))
        .collect();
    Ok(json!(items))
}

#[tauri::command]
pub fn env_hide_run(
    scheme: Option<Value>,
    selected_apks: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let packages = opt_strs(&selected_apks);
    let mut failed: Vec<String> = Vec::new();
    for pkg in &packages {
        // pm hide 需要 root；失败时尝试 disable-user
        let result = adb_shell(&serial, &["su", "-c", &format!("pm hide {}", pkg)])
            .or_else(|_| adb_shell(&serial, &["pm", "disable-user", pkg]));
        if result.is_err() {
            failed.push(pkg.clone());
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "count": packages.len()}))
}

#[tauri::command]
pub fn env_hide_cancel() -> Value {
    json!({"ok": true})
}

// ---------- ROOT 修补 ----------

#[tauri::command]
pub fn root_patch_preflight() -> Value {
    let has_adb = adb_path().is_ok();
    json!({"ready": has_adb, "adb": has_adb})
}

#[tauri::command]
pub fn root_patch_run(
    flash_type: Option<Value>,
    local_apk: Option<Value>,
    local_image: Option<Value>,
    ota_url: Option<Value>,
    partition: Option<Value>,
    scheme: Option<Value>,
    use_cloud: Option<Value>,
    use_oplus_rom: Option<Value>,
) -> Result<Value, String> {
    Err("boot 镜像修补流程尚未实现".to_string())
}

#[tauri::command]
pub fn root_patch_cancel() -> Value {
    json!({"ok": true})
}

// ---------- 越狱（内核漏洞链不可离线重建） ----------

#[tauri::command]
pub fn jailbreak_detect() -> Value {
    json!({"available": false, "reason": "内核漏洞利用链未恢复"})
}

#[tauri::command]
pub fn jailbreak_offsets_summary() -> Value {
    let path = data_dir("jailbreak").join("offsets.json");
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| json!({"entries": 0})),
        Err(_) => json!({"entries": 0, "path": path.to_string_lossy()}),
    }
}

#[tauri::command]
pub fn jailbreak_run(custom_so: Option<Value>, ota_url: Option<Value>) -> Result<Value, String> {
    Err("越狱利用链未恢复".to_string())
}

#[tauri::command]
pub fn jailbreak_import_url(url: Option<Value>) -> Result<Value, String> {
    let url = opt_str(&url).ok_or_else(|| "缺少 url".to_string())?;
    let dest = data_dir("jailbreak").join("imported.json");
    download_file(&url, &dest).map_err(|e| format!("导入失败: {}", e))?;
    Ok(json!({"ok": true, "path": dest.to_string_lossy()}))
}

#[tauri::command]
pub fn jailbreak_parse_boot(
    boot_path: Option<Value>,
    xbl_path: Option<Value>,
) -> Result<Value, String> {
    let boot = opt_str(&boot_path).unwrap_or_default();
    if boot.is_empty() {
        return Err("缺少 bootPath".to_string());
    }
    let bytes = std::fs::read(&boot).map_err(|e| format!("读取失败: {}", e))?;
    let is_android = bytes.len() > 8 && &bytes[..8] == b"ANDROID!";
    Ok(json!({"valid": is_android, "size": bytes.len()}))
}

#[tauri::command]
pub fn jailbreak_import_offsets(path: Option<Value>) -> Result<Value, String> {
    let src = opt_str(&path).ok_or_else(|| "缺少 path".to_string())?;
    let dest = data_dir("jailbreak").join("offsets.json");
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::copy(&src, &dest).map_err(|e| format!("导入失败: {}", e))?;
    Ok(json!({"ok": true, "path": dest.to_string_lossy()}))
}

#[tauri::command]
pub fn jailbreak_export_offsets(path: Option<Value>) -> Result<Value, String> {
    let dest = opt_str(&path).ok_or_else(|| "缺少 path".to_string())?;
    let src = data_dir("jailbreak").join("offsets.json");
    if !src.exists() {
        return Err("尚无 offsets 可导出".to_string());
    }
    std::fs::copy(&src, &dest).map_err(|e| format!("导出失败: {}", e))?;
    Ok(json!({"ok": true, "path": dest}))
}

#[tauri::command]
pub fn jailbreak_cancel() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn jailbreak_deploy_ksu() -> Result<Value, String> {
    Err("KSU 部署流程尚未实现".to_string())
}

// ---------- 镜像上传 ----------

#[tauri::command]
pub fn image_upload_run(
    file_path: Option<Value>,
    partition: Option<Value>,
) -> Result<Value, String> {
    let path = opt_str(&file_path).ok_or_else(|| "缺少 filePath".to_string())?;
    if !std::path::Path::new(&path).exists() {
        return Err("镜像文件不存在".to_string());
    }
    Err("镜像上传通道（9008）尚未实现".to_string())
}

#[tauri::command]
pub fn image_upload_cancel() -> Value {
    json!({"ok": true})
}
