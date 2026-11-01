//! 配置包（运行环境）部署：下载 Google platform-tools 并解压，发射进度事件。

use super::util::*;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use tauri::Emitter;

const PLATFORM_TOOLS_URL: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";

static PACK_CANCEL: AtomicBool = AtomicBool::new(false);
static DOWNLOAD_SOURCE: Mutex<String> = Mutex::new(String::new());

fn platform_tools_dir() -> PathBuf {
    pack_dir().join("platform-tools")
}

fn pack_ready() -> bool {
    platform_tools_dir().join("adb.exe").exists()
        && platform_tools_dir().join("fastboot.exe").exists()
}

#[tauri::command]
pub fn config_pack_check() -> Value {
    json!({"ready": pack_ready()})
}

#[tauri::command]
pub fn verify_pack_access() -> Value {
    json!({"ok": pack_ready(), "dir": platform_tools_dir().to_string_lossy()})
}

#[tauri::command]
pub fn refresh_tool_paths_cmd() -> Value {
    json!({
        "adb": tool_path("adb").map(|p| p.to_string_lossy().to_string()),
        "fastboot": tool_path("fastboot").map(|p| p.to_string_lossy().to_string()),
        "dir": platform_tools_dir().to_string_lossy()
    })
}

#[tauri::command]
pub fn set_pack_download_source(source: Option<Value>) -> Value {
    let source = source
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "google".to_string());
    if let Ok(mut slot) = DOWNLOAD_SOURCE.lock() {
        *slot = source;
    }
    json!({"ok": true})
}

#[tauri::command]
pub fn get_pack_download_source() -> Value {
    let source = DOWNLOAD_SOURCE
        .lock()
        .map(|s| s.clone())
        .unwrap_or_default();
    Value::String(if source.is_empty() {
        "google".to_string()
    } else {
        source
    })
}

#[tauri::command]
pub fn probe_pack_download_lines() -> Value {
    let start = Instant::now();
    let ok = http_get("https://dl.google.com/android/repository/platform-tools-latest-windows.zip")
        .is_some();
    json!([{
        "name": "google",
        "url": PLATFORM_TOOLS_URL,
        "ok": ok,
        "latency_ms": start.elapsed().as_millis() as u64
    }])
}

#[tauri::command]
pub fn config_pack_cancel() -> Value {
    PACK_CANCEL.store(true, Ordering::SeqCst);
    json!({"ok": true})
}

fn deploy(app: tauri::AppHandle) -> Result<(), String> {
    PACK_CANCEL.store(false, Ordering::SeqCst);
    let emit = |payload: Value| {
        let _ = app.emit("config-pack:progress", payload);
    };
    emit(json!({"phase": "check", "percent": 0, "label": "检测运行环境"}));
    if pack_ready() {
        emit(json!({"phase": "done", "percent": 100, "label": "环境已就绪"}));
        return Ok(());
    }
    let zip_path = pack_dir().join("platform-tools.zip");
    let start = Instant::now();
    let app_dl = app.clone();
    let download = download_with_progress(
        PLATFORM_TOOLS_URL,
        &zip_path,
        move |transferred, total| {
            if PACK_CANCEL.load(Ordering::SeqCst) {
                return;
            }
            let elapsed = start.elapsed().as_secs_f64().max(0.001);
            let speed = transferred as f64 / 1048576.0 / elapsed;
            let percent = if total > 0 {
                (transferred as f64 / total as f64 * 85.0).min(85.0)
            } else {
                0.0
            };
            let _ = app_dl.emit(
            "config-pack:progress",
            json!({
                "phase": "download",
                "percent": percent,
                "label": "下载 ADB 与刷机运行环境",
                "transferred_mb": format!("{:.1}", transferred as f64 / 1048576.0),
                "total_mb": if total > 0 { format!("{:.1}", total as f64 / 1048576.0) } else { String::new() },
                "speed_mbps": format!("{:.2}", speed)
            }),
        );
        },
    );
    if PACK_CANCEL.load(Ordering::SeqCst) {
        let _ = std::fs::remove_file(&zip_path);
        return Err("操作已停止".to_string());
    }
    let bytes = download.map_err(|e| format!("下载失败: {}", e))?;
    if bytes < 100_000 {
        return Err("下载内容不完整".to_string());
    }
    emit(json!({"phase": "extract", "percent": 90, "label": "解压运行环境"}));
    unzip(&zip_path, &pack_dir()).map_err(|e| format!("解压失败: {}", e))?;
    let _ = std::fs::remove_file(&zip_path);
    if !pack_ready() {
        return Err("部署后未找到 adb/fastboot".to_string());
    }
    emit(json!({"phase": "done", "percent": 100, "label": "部署完成"}));
    Ok(())
}

#[tauri::command]
pub async fn config_pack_sync(app: tauri::AppHandle) -> Value {
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || deploy(app2)).await;
    let (success, message) = match result {
        Ok(Ok(())) => (true, String::new()),
        Ok(Err(e)) => (false, e),
        Err(e) => (false, e.to_string()),
    };
    let _ = app.emit(
        "config-pack:completed",
        json!({"success": success, "message": message}),
    );
    json!({"ok": success, "message": message})
}
