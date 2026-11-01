//! 分区库备份：枚举设备分区并批量读回（基于 ADB root）。

use super::util::*;
use serde_json::{json, Value};
use std::path::PathBuf;

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

const BY_NAME: &str = "/dev/block/bootdevice/by-name";

#[tauri::command]
pub fn lib_backup_enumerate(serial: Option<Value>) -> Result<Value, String> {
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
            &format!("ls {}", BY_NAME),
        ],
    )?;
    if out.contains("uid=0") || out.to_lowercase().contains("permission") {
        return Err("无 ROOT 权限".to_string());
    }
    let items: Vec<Value> = out
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|name| json!({"name": name, "path": format!("{}/{}", BY_NAME, name)}))
        .collect();
    Ok(json!(items))
}

#[tauri::command]
pub fn lib_backup_start(payload: Option<Value>) -> Result<Value, String> {
    let p = payload.clone().unwrap_or(json!({}));
    let serial = p
        .get("serial")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if serial.is_empty() {
        return Err("缺少 serial".to_string());
    }
    let output_dir = p
        .get("output_dir")
        .or_else(|| p.get("outputDir"))
        .and_then(Value::as_str)
        .map(|s| s.to_string())
        .unwrap_or_else(|| data_dir("lib-backup").to_string_lossy().to_string());
    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;
    let exe = adb_path()?;
    // 仅备份指定的 luns/分区；缺省备份全部分区
    let names: Vec<String> = match p.get("partitions").or_else(|| p.get("luns")) {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|x| match x {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => o.get("name").and_then(Value::as_str).map(|s| s.to_string()),
                _ => None,
            })
            .collect(),
        _ => {
            let out = run_tool_str(
                &exe,
                &[
                    "-s",
                    &serial,
                    "shell",
                    "su",
                    "-c",
                    &format!("ls {}", BY_NAME),
                ],
            )?;
            out.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        }
    };
    let mut failed: Vec<String> = Vec::new();
    let mut saved = 0usize;
    for name in &names {
        let script = format!("dd if={}/{}", BY_NAME, name);
        let bytes = run_tool_bytes(
            &exe,
            &[
                "-s".into(),
                serial.clone(),
                "exec-out".into(),
                "su".into(),
                "-c".into(),
                script,
            ],
        );
        match bytes {
            Ok(data) if !data.is_empty() => {
                let dest = PathBuf::from(&output_dir).join(format!("{}.img", name));
                if std::fs::write(&dest, &data).is_ok() {
                    saved += 1;
                } else {
                    failed.push(name.clone());
                }
            }
            _ => failed.push(name.clone()),
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "saved": saved, "dir": output_dir}))
}

#[tauri::command]
pub fn lib_backup_cancel() -> Value {
    json!({"ok": true})
}
