use super::util::*;
use serde_json::{json, Value};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tauri::Emitter;

static STORE_ACTIVE: AtomicBool = AtomicBool::new(false);
static STORE_CANCEL: AtomicBool = AtomicBool::new(false);

struct StoreDownloadStopped;

fn opt_string(value: &Option<Value>) -> Option<String> {
    value
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn desktop_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(pack_dir);
    let desktop = home.join("Desktop");
    if desktop.exists() {
        return desktop;
    }
    if let Some(onedrive) = std::env::var_os("OneDrive") {
        let onedrive_desktop = PathBuf::from(onedrive).join("Desktop");
        if onedrive_desktop.exists() {
            return onedrive_desktop;
        }
    }
    desktop
}

fn safe_filename(name: &str) -> Result<String, String> {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        Err("缺少有效的文件名".to_string())
    } else {
        Ok(cleaned.to_string())
    }
}

fn emit_status(app: &tauri::AppHandle, phase: &str, title: &str) {
    let _ = app.emit("store:status", json!({"phase": phase, "title": title}));
}

fn install_apk(serial: Option<&str>, path: &Path) -> Result<(), String> {
    let adb = adb_path()?;
    let mut args = Vec::<String>::new();
    if let Some(serial) = serial.filter(|s| !s.is_empty()) {
        args.push("-s".to_string());
        args.push(serial.to_string());
    }
    args.push("install".to_string());
    args.push("-r".to_string());
    args.push(path.to_string_lossy().to_string());
    let output = run_tool(&adb, &args)?;
    let lower = output.to_lowercase();
    if lower.contains("success") && !lower.contains("failure") && !lower.contains("error") {
        Ok(())
    } else {
        Err(format!("安装失败: {}", output.trim()))
    }
}

fn run_store_job(
    app: tauri::AppHandle,
    display_name: String,
    remote_path: String,
    temp_path: PathBuf,
    final_path: PathBuf,
    serial: Option<String>,
) {
    emit_status(&app, "download", &display_name);
    let started = Instant::now();
    let app_progress = app.clone();
    let title = display_name.clone();
    let download = catch_unwind(AssertUnwindSafe(|| {
        download_with_progress(&remote_path, &temp_path, move |downloaded, total| {
            if STORE_CANCEL.load(Ordering::SeqCst) {
                std::panic::resume_unwind(Box::new(StoreDownloadStopped));
            }
            let percent = if total > 0 {
                (downloaded as f64 * 100.0 / total as f64).min(100.0)
            } else {
                0.0
            };
            let speed = downloaded as f64 / 1048576.0 / started.elapsed().as_secs_f64().max(0.001);
            let _ = app_progress.emit(
                "store:download-progress",
                json!({
                    "title": title,
                    "percent": percent,
                    "downloaded": downloaded,
                    "total": total,
                    "speed_mbps": speed,
                }),
            );
        })
    }));

    let outcome = match download {
        Ok(Ok(_)) if STORE_CANCEL.load(Ordering::SeqCst) => Err("已取消".to_string()),
        Ok(Ok(_)) => (|| -> Result<String, String> {
            if final_path.exists() {
                std::fs::remove_file(&final_path).map_err(|e| format!("替换旧文件失败: {}", e))?;
            }
            std::fs::rename(&temp_path, &final_path).map_err(|e| format!("保存文件失败: {}", e))?;
            if final_path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("apk"))
                .unwrap_or(false)
                && serial.as_deref().is_some()
            {
                emit_status(&app, "install", &display_name);
                install_apk(serial.as_deref(), &final_path)
                    .map(|_| format!("安装成功: {}", display_name))
            } else {
                Ok(format!("下载完成: {}", final_path.to_string_lossy()))
            }
        })(),
        Ok(Err(e)) => Err(format!("下载失败: {}", e)),
        Err(_) if STORE_CANCEL.load(Ordering::SeqCst) => Err("已取消".to_string()),
        Err(_) => Err("下载已中断".to_string()),
    };

    match outcome {
        Ok(message) => {
            emit_status(&app, "done", &display_name);
            let _ = app.emit(
                "store:completed",
                json!({"success": true, "message": message}),
            );
        }
        Err(message) => {
            if message == "已取消" || message.starts_with("下载失败") {
                let _ = std::fs::remove_file(&temp_path);
            }
            emit_status(&app, "error", &display_name);
            let _ = app.emit(
                "store:completed",
                json!({"success": false, "message": message}),
            );
        }
    }
    STORE_ACTIVE.store(false, Ordering::SeqCst);
}

#[tauri::command]
pub fn store_list_catalog(refresh: Option<Value>) -> Value {
    let _ = refresh;
    // 云端目录源尚未接入，不生成虚假目录。
    json!({"categories": [], "apps": []})
}

#[tauri::command]
pub fn store_list_categories(refresh: Option<Value>) -> Value {
    let _ = refresh;
    Value::Array(Vec::new())
}

#[tauri::command]
pub fn store_list_apps(category: Option<Value>, refresh: Option<Value>) -> Value {
    let _ = (category, refresh);
    Value::Array(Vec::new())
}

#[tauri::command]
pub fn store_default_download_dir() -> Value {
    Value::String(desktop_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn store_download_quota_status() -> Result<Value, String> {
    Err("商店下载额度服务尚未接入".to_string())
}

#[tauri::command]
pub fn store_start_download_install(
    app: tauri::AppHandle,
    display_name: Option<Value>,
    download_dir: Option<Value>,
    file_name: Option<Value>,
    remote_path: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let remote_path = opt_string(&remote_path).ok_or_else(|| "缺少 remotePath".to_string())?;
    if !remote_path.starts_with("https://") {
        return Err("商店云端下载源尚未接入；remotePath 目前仅接受完整 https URL".to_string());
    }
    let file_name =
        safe_filename(&opt_string(&file_name).ok_or_else(|| "缺少 fileName".to_string())?)?;
    let display_name = opt_string(&display_name).unwrap_or_else(|| file_name.clone());
    let download_dir =
        opt_string(&download_dir).unwrap_or_else(|| desktop_dir().to_string_lossy().to_string());
    std::fs::create_dir_all(&download_dir).map_err(|e| format!("创建下载目录失败: {}", e))?;
    let final_path = Path::new(&download_dir).join(&file_name);
    let temp_path = Path::new(&download_dir).join(format!(".{}.storepart", file_name));
    let serial = opt_string(&serial);

    if STORE_ACTIVE
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("已有商店下载任务正在进行".to_string());
    }
    STORE_CANCEL.store(false, Ordering::SeqCst);
    std::thread::spawn(move || {
        run_store_job(
            app,
            display_name,
            remote_path,
            temp_path,
            final_path,
            serial,
        )
    });
    Ok(json!({"ok": true, "started": true}))
}

#[tauri::command]
pub fn store_cancel_download() -> Value {
    let active = STORE_ACTIVE.load(Ordering::SeqCst);
    if active {
        STORE_CANCEL.store(true, Ordering::SeqCst);
    }
    json!({"ok": true, "cancelled": active})
}
