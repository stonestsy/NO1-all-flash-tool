//! Fastboot 命令：fastboot 子进程封装（getvar/flash/erase/槽位/重启）。

use super::util::*;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

static FB_CANCEL: AtomicBool = AtomicBool::new(false);

fn fastboot(serial: Option<&str>, args: &[&str]) -> Result<String, String> {
    let exe = fastboot_path()?;
    let mut full: Vec<String> = Vec::new();
    if let Some(s) = serial {
        full.push("-s".to_string());
        full.push(s.to_string());
    }
    full.extend(args.iter().map(|a| a.to_string()));
    run_tool(&exe, &full)
}

fn fastboot_devices() -> Vec<Value> {
    let mut items = Vec::new();
    if let Ok(exe) = fastboot_path() {
        if let Ok(out) = run_tool_str(&exe, &["devices"]) {
            for line in out.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    items.push(json!({"serial": parts[0], "mode": parts[1]}));
                }
            }
        }
    }
    items
}

/// 解析 `fastboot getvar all` 输出：(bootloader) key: value
fn getvars(serial: &str) -> Result<Vec<(String, String)>, String> {
    let out = fastboot(Some(serial), &["getvar", "all"])?;
    let mut vars = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        let rest = line
            .strip_prefix("(bootloader)")
            .or_else(|| line.strip_prefix("(bootloader) "))
            .unwrap_or(line)
            .trim();
        if let Some((key, value)) = rest.split_once(':') {
            let key = key.trim();
            if !key.is_empty() && !key.contains(' ') {
                vars.push((key.to_string(), value.trim().to_string()));
            }
        }
    }
    Ok(vars)
}

fn var<'a>(vars: &'a [(String, String)], key: &str) -> &'a str {
    vars.iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
        .unwrap_or("")
}

fn summary_of(vars: &[(String, String)]) -> Value {
    let slot = match var(vars, "current-slot") {
        "a" | "_a" => "A".to_string(),
        "b" | "_b" => "B".to_string(),
        other => {
            if other.is_empty() {
                if var(vars, "slot-count") == "2" {
                    "未知".to_string()
                } else {
                    "单分区".to_string()
                }
            } else {
                other.to_uppercase()
            }
        }
    };
    let product = var(vars, "product");
    let android = var(vars, "version");
    json!([
        {"key": "品牌", "value": var(vars, "manufacturer")},
        {"key": "型号", "value": product},
        {"key": "安卓版本", "value": android},
        {"key": "当前槽位", "value": slot}
    ])
}

#[tauri::command]
pub fn list_fastboot_devices() -> Value {
    Value::Array(fastboot_devices())
}

#[tauri::command]
pub fn fastboot_reboot(mode: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let mode = opt_str(&mode).unwrap_or_default();
    let out = if mode.is_empty() {
        fastboot(Some(&serial), &["reboot"])?
    } else {
        fastboot(Some(&serial), &["reboot", &mode])?
    };
    Ok(json!({"ok": true, "output": out}))
}

#[tauri::command]
pub fn fastboot_vars(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let vars = getvars(&serial)?;
    let items: Vec<Value> = vars
        .iter()
        .map(|(k, v)| json!({"key": k, "value": v}))
        .collect();
    Ok(json!({"vars": items}))
}

#[tauri::command]
pub fn fastboot_device_info_cmd(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let vars = getvars(&serial)?;
    Ok(summary_of(&vars))
}

#[tauri::command]
pub fn fastboot_read_device_info_cmd(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let vars = getvars(&serial)?;
    let items: Vec<Value> = vars
        .iter()
        .map(|(k, v)| json!({"key": k, "value": v}))
        .collect();
    Ok(json!({"summary": summary_of(&vars), "vars": items}))
}

#[tauri::command]
pub fn fastboot_partitions_cmd(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let vars = getvars(&serial)?;
    let mut parts: Vec<Value> = Vec::new();
    for (key, value) in &vars {
        if let Some(name) = key.strip_prefix("partition-size:") {
            let size: u64 = value
                .trim_start_matches("0x")
                .parse::<u64>()
                .unwrap_or_else(|_| {
                    u64::from_str_radix(value.trim_start_matches("0x"), 16).unwrap_or(0)
                });
            parts.push(json!({"name": name, "size": size, "kind": var(&vars, &format!("partition-type:{}", name))}));
        }
    }
    Ok(Value::Array(parts))
}

#[tauri::command]
pub fn fastboot_flash(
    image: Option<Value>,
    partition: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let partition = opt_str(&partition).ok_or_else(|| "缺少 partition".to_string())?;
    let image = opt_str(&image).ok_or_else(|| "缺少 image".to_string())?;
    FB_CANCEL.store(false, Ordering::SeqCst);
    let out = fastboot(Some(&serial), &["flash", &partition, &image])?;
    Ok(json!({"ok": true, "output": out}))
}

#[tauri::command]
pub fn fastboot_erase(partition: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let partition = opt_str(&partition).ok_or_else(|| "缺少 partition".to_string())?;
    let out = fastboot(Some(&serial), &["erase", &partition])?;
    Ok(json!({"ok": true, "output": out}))
}

#[tauri::command]
pub fn fastboot_set_active(serial: Option<Value>, slot: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let slot = opt_str(&slot).unwrap_or_default();
    let slot = slot.trim_start_matches('_').to_lowercase();
    if slot != "a" && slot != "b" {
        return Err("slot 必须为 a 或 b".to_string());
    }
    let out = fastboot(Some(&serial), &["set_active", &slot])?;
    Ok(json!({"ok": true, "output": out}))
}

#[tauri::command]
pub fn fastboot_cancel() -> Value {
    FB_CANCEL.store(true, Ordering::SeqCst);
    json!({"ok": true})
}

#[tauri::command]
pub fn fastboot_clear_cancel() -> Value {
    FB_CANCEL.store(false, Ordering::SeqCst);
    json!({"ok": true})
}

// ---------- 智能操作（自动识别当前连接模式） ----------

fn any_adb_serial() -> Option<String> {
    let exe = adb_path().ok()?;
    let out = run_tool_str(&exe, &["devices"]).ok()?;
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "device" {
            return Some(parts[0].to_string());
        }
    }
    None
}

#[tauri::command]
pub fn smart_reboot(target: Option<Value>) -> Result<Value, String> {
    let target = opt_str(&target).unwrap_or_default();
    if let Some(serial) = any_adb_serial() {
        let out = if target.is_empty() {
            crate::commands::adb::adb_reboot(None, Some(Value::String(serial)))?
        } else {
            crate::commands::adb::adb_reboot(
                Some(Value::String(target.clone())),
                Some(Value::String(serial)),
            )?
        };
        return Ok(out);
    }
    let fb = fastboot_devices();
    if let Some(first) = fb.first() {
        let serial = first
            .get("serial")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let mode = if target.is_empty() {
            None
        } else {
            Some(Value::String(target))
        };
        return fastboot_reboot(mode, Some(Value::String(serial)));
    }
    Err("未检测到 ADB 或 Fastboot 设备".to_string())
}

#[tauri::command]
pub fn smart_set_active(slot: Option<Value>) -> Result<Value, String> {
    let slot_value = slot.clone();
    let slot_str = opt_str(&slot).unwrap_or_default();
    let fb = fastboot_devices();
    if let Some(first) = fb.first() {
        let serial = first
            .get("serial")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return fastboot_set_active(Some(Value::String(serial)), slot_value);
    }
    if let Some(serial) = any_adb_serial() {
        let suffix = format!("_{}", slot_str.trim_start_matches('_').to_lowercase());
        let exe = adb_path()?;
        let out = run_tool_str(
            &exe,
            &[
                "-s",
                &serial,
                "shell",
                &format!("setprop ro.boot.slot_suffix {}", suffix),
            ],
        )?;
        return Ok(json!({"ok": true, "output": out}));
    }
    Err("未检测到设备".to_string())
}
