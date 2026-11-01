//! ADB 命令：通过 adb 子进程与设备交互（枚举/getprop/文件/分区 dd/投屏按键）。

use super::util::*;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::Emitter;

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

fn adb(serial: Option<&str>, args: &[&str]) -> Result<String, String> {
    let exe = adb_path()?;
    let mut full: Vec<String> = Vec::new();
    if let Some(s) = serial {
        full.push("-s".to_string());
        full.push(s.to_string());
    }
    full.extend(args.iter().map(|a| a.to_string()));
    run_tool(&exe, &full)
}

fn adb_bytes(serial: Option<&str>, args: &[&str]) -> Result<Vec<u8>, String> {
    let exe = adb_path()?;
    let mut full: Vec<String> = Vec::new();
    if let Some(s) = serial {
        full.push("-s".to_string());
        full.push(s.to_string());
    }
    full.extend(args.iter().map(|a| a.to_string()));
    run_tool_bytes(&exe, &full)
}

fn getprops(serial: &str) -> Result<std::collections::HashMap<String, String>, String> {
    let out = adb(Some(serial), &["shell", "getprop"])?;
    let mut map = std::collections::HashMap::new();
    for line in out.lines() {
        // 格式: [ro.product.brand]: [Xiaomi]
        if let Some(rest) = line.strip_prefix('[') {
            if let Some((key, tail)) = rest.split_once("]: [") {
                let value = tail.trim_end_matches(']').trim_end();
                map.insert(key.to_string(), value.to_string());
            }
        }
    }
    Ok(map)
}

fn prop(map: &std::collections::HashMap<String, String>, key: &str) -> String {
    map.get(key).cloned().unwrap_or_default()
}

#[tauri::command]
pub fn list_devices() -> Value {
    let mut adb_items: Vec<Value> = Vec::new();
    let mut fb_items: Vec<Value> = Vec::new();
    let mut adb_enum = "ok";
    match adb_path() {
        Ok(exe) => match run_tool_str(&exe, &["devices"]) {
            Ok(out) => {
                for line in out.lines() {
                    let line = line.trim();
                    if line.is_empty()
                        || line.starts_with("List of devices")
                        || line.starts_with('*')
                    {
                        continue;
                    }
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let serial = parts[0].to_string();
                        let raw_state = parts[1];
                        let mut state = raw_state.to_string();
                        if let Some(first) = state.get(..1) {
                            state = first.to_uppercase() + &state[1..];
                        }
                        adb_items.push(json!({"serial": serial, "state": state}));
                    }
                }
            }
            Err(_) => adb_enum = "cmd_fail",
        },
        Err(_) => adb_enum = "cmd_fail",
    }
    if let Ok(exe) = fastboot_path() {
        if let Ok(out) = run_tool_str(&exe, &["devices"]) {
            for line in out.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    fb_items.push(json!({"serial": parts[0], "mode": parts[1]}));
                }
            }
        }
    }
    json!({"adb": adb_items, "fastboot": fb_items, "adb_enum": adb_enum})
}

#[tauri::command]
pub fn adb_device_info(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let map = getprops(&serial)?;
    let slot = match prop(&map, "ro.boot.slot_suffix").as_str() {
        "_a" => "A".to_string(),
        "_b" => "B".to_string(),
        "" => "单分区".to_string(),
        other => other.trim_start_matches('_').to_uppercase(),
    };
    Ok(json!([
        {"key": "品牌", "value": prop(&map, "ro.product.brand")},
        {"key": "型号", "value": prop(&map, "ro.product.model")},
        {"key": "安卓版本", "value": prop(&map, "ro.build.version.release")},
        {"key": "当前槽位", "value": slot}
    ]))
}

#[tauri::command]
pub fn adb_read_device_detail(serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let map = getprops(&serial)?;
    let fields = [
        ("品牌", "ro.product.brand"),
        ("型号", "ro.product.model"),
        ("设备代号", "ro.product.device"),
        ("安卓版本", "ro.build.version.release"),
        ("SDK", "ro.build.version.sdk"),
        ("安全补丁", "ro.build.version.security_patch"),
        ("软件版本", "ro.build.display.id"),
        ("指纹", "ro.build.fingerprint"),
        ("CPU", "ro.product.cpu.abi"),
        ("基带", "gsm.version.baseband"),
        ("序列号", "ro.serialno"),
        ("当前槽位", "ro.boot.slot_suffix"),
    ];
    let items: Vec<Value> = fields
        .iter()
        .map(|(label, key)| json!({"key": label, "value": prop(&map, key)}))
        .collect();
    Ok(Value::Array(items))
}

#[tauri::command]
pub fn adb_reboot(mode: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let mode = opt_str(&mode).unwrap_or_default();
    let out = if mode.is_empty() {
        adb(Some(&serial), &["reboot"])?
    } else {
        adb(Some(&serial), &["reboot", &mode])?
    };
    Ok(json!({"ok": true, "output": out}))
}

// ---------- ROOT / 分区 ----------

fn require_root(serial: &str) -> Result<(), String> {
    let out = adb(Some(serial), &["shell", "su", "-c", "id"])?;
    if out.contains("uid=0") {
        Ok(())
    } else {
        Err("无 ROOT 权限".to_string())
    }
}

const BY_NAME: &str = "/dev/block/bootdevice/by-name";

#[tauri::command]
pub fn adb_partitions(app: tauri::AppHandle, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    require_root(&serial)?;
    let script = format!(
        "for f in {}/*; do n=$(basename $f); s=$(blockdev --getsize64 $f 2>/dev/null); echo \"$n $s\"; done",
        BY_NAME
    );
    let out = adb(Some(&serial), &["shell", "su", "-c", &script])?;
    let mut count = 0;
    for line in out.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let name = parts[0].to_string();
        let size: u64 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        let item = json!({"name": name, "size": size, "kind": "raw", "path": format!("{}/{}", BY_NAME, name)});
        let _ = app.emit("partition:item", item);
        count += 1;
    }
    Ok(json!({"ok": true, "count": count}))
}

fn partition_names(v: &Option<Value>) -> Vec<String> {
    match v {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|x| match x {
                Value::String(s) => Some(s.clone()),
                Value::Object(o) => o
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[tauri::command]
pub fn adb_erase_partitions(
    partitions: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    require_root(&serial)?;
    let names = partition_names(&partitions);
    let mut failed: Vec<String> = Vec::new();
    for name in &names {
        let path = format!("{}/{}", BY_NAME, name);
        let script = format!("dd if=/dev/zero of={} bs=4096", path);
        match adb(Some(&serial), &["shell", "su", "-c", &script]) {
            Ok(out) => {
                if out.to_lowercase().contains("error") || out.contains("No such") {
                    failed.push(name.clone());
                }
            }
            Err(_) => failed.push(name.clone()),
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "warnings": []}))
}

#[tauri::command]
pub fn adb_readback_partitions(
    partitions: Option<Value>,
    save_dir: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let save_dir =
        opt_str(&save_dir).unwrap_or_else(|| data_dir("readback").to_string_lossy().to_string());
    require_root(&serial)?;
    let names = partition_names(&partitions);
    std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
    let mut failed: Vec<String> = Vec::new();
    let mut saved: Vec<String> = Vec::new();
    for name in &names {
        let path = format!("{}/{}", BY_NAME, name);
        let script = format!("dd if={}", path);
        match adb_bytes(Some(&serial), &["exec-out", "su", "-c", &script]) {
            Ok(bytes) if !bytes.is_empty() => {
                let dest = PathBuf::from(&save_dir).join(format!("{}.img", name));
                if std::fs::write(&dest, &bytes).is_ok() {
                    saved.push(dest.to_string_lossy().to_string());
                } else {
                    failed.push(name.clone());
                }
            }
            _ => failed.push(name.clone()),
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "saved": saved, "warnings": []}))
}

#[tauri::command]
pub fn adb_write_partitions(items: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    require_root(&serial)?;
    let list = match items {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    };
    let mut failed: Vec<String> = Vec::new();
    for item in &list {
        let name = item
            .get("name")
            .or_else(|| item.get("partition"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let image = item
            .get("image")
            .or_else(|| item.get("path"))
            .or_else(|| item.get("file"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() || image.is_empty() {
            failed.push(name.clone());
            continue;
        }
        let remote = "/data/local/tmp/aft_write.img";
        let target = format!("{}/{}", BY_NAME, name);
        let result = (|| {
            adb(Some(&serial), &["push", &image, remote])?;
            let script = format!("dd if={} of={} bs=4096 && rm -f {}", remote, target, remote);
            adb(Some(&serial), &["shell", "su", "-c", &script])
        })();
        if result.is_err() {
            failed.push(name);
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "warnings": []}))
}

fn backup_all(save_dir: Option<Value>, serial: Option<Value>, sub: &str) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    require_root(&serial)?;
    let save_dir =
        opt_str(&save_dir).unwrap_or_else(|| data_dir(sub).to_string_lossy().to_string());
    let out = adb(
        Some(&serial),
        &["shell", "su", "-c", &format!("ls {}", BY_NAME)],
    )?;
    let names: Vec<String> = out
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();
    std::fs::create_dir_all(&save_dir).map_err(|e| e.to_string())?;
    let mut failed: Vec<String> = Vec::new();
    let mut count = 0;
    for name in &names {
        let script = format!("dd if={}/{}", BY_NAME, name);
        match adb_bytes(Some(&serial), &["exec-out", "su", "-c", &script]) {
            Ok(bytes) if !bytes.is_empty() => {
                let dest = PathBuf::from(&save_dir).join(format!("{}.img", name));
                if std::fs::write(&dest, &bytes).is_ok() {
                    count += 1;
                } else {
                    failed.push(name.clone());
                }
            }
            _ => failed.push(name.clone()),
        }
    }
    Ok(
        json!({"ok": failed.is_empty(), "failed": failed, "count": count, "dir": save_dir, "warnings": []}),
    )
}

#[tauri::command]
pub fn adb_qc_backup(save_dir: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    backup_all(save_dir, serial, "qc-backup")
}

#[tauri::command]
pub fn adb_mtk_backup(save_dir: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    backup_all(save_dir, serial, "mtk-backup")
}

// ---------- 安装 / 文件 ----------

#[tauri::command]
pub fn adb_install(apk: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let apk = opt_str(&apk).ok_or_else(|| "缺少 apk 路径".to_string())?;
    let out = adb(Some(&serial), &["install", "-r", &apk])?;
    let ok = out.contains("Success");
    Ok(json!({"ok": ok, "output": out}))
}

#[tauri::command]
pub fn adb_push(
    local: Option<Value>,
    remote: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let local = opt_str(&local).ok_or_else(|| "缺少 local".to_string())?;
    let remote = opt_str(&remote).ok_or_else(|| "缺少 remote".to_string())?;
    let out = adb(Some(&serial), &["push", &local, &remote])?;
    Ok(json!({"ok": true, "output": out}))
}

#[tauri::command]
pub fn adb_list_files(path: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let path = opt_str(&path).unwrap_or_else(|| "/sdcard".to_string());
    let out = adb(Some(&serial), &["shell", "ls", "-l", &path])?;
    let mut entries: Vec<Value> = Vec::new();
    for line in out.lines() {
        let line = line.trim_end();
        if line.is_empty()
            || line.starts_with("total")
            || line.contains("No such")
            || line.contains("Permission denied")
        {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let is_dir = parts[0].starts_with('d');
        let name = parts[parts.len() - 1].to_string();
        if name == "." || name == ".." {
            continue;
        }
        let size: u64 = parts
            .iter()
            .find_map(|p| p.parse::<u64>().ok())
            .unwrap_or(0);
        entries.push(json!({"name": name, "is_dir": is_dir, "size": size}));
    }
    let parent = match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => path[..i].to_string(),
        None => "/".to_string(),
    };
    Ok(json!({"entries": entries, "path": path, "parent": parent}))
}

#[tauri::command]
pub fn adb_pull_files(
    local_dir: Option<Value>,
    remote_paths: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let local_dir =
        opt_str(&local_dir).unwrap_or_else(|| data_dir("Downloads").to_string_lossy().to_string());
    let paths = opt_strs(&remote_paths);
    std::fs::create_dir_all(&local_dir).map_err(|e| e.to_string())?;
    let mut failed: Vec<String> = Vec::new();
    for remote in &paths {
        if adb(Some(&serial), &["pull", remote, &local_dir]).is_err() {
            failed.push(remote.clone());
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "dir": local_dir}))
}

#[tauri::command]
pub fn adb_push_files(
    local_paths: Option<Value>,
    remote_dir: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let remote_dir = opt_str(&remote_dir).unwrap_or_else(|| "/sdcard".to_string());
    let paths = opt_strs(&local_paths);
    let mut failed: Vec<String> = Vec::new();
    for local in &paths {
        if adb(Some(&serial), &["push", local, &remote_dir]).is_err() {
            failed.push(local.clone());
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed}))
}

#[tauri::command]
pub fn adb_drag_files_out(
    remote_paths: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let dir = data_dir("Downloads").to_string_lossy().to_string();
    adb_pull_files(Some(Value::String(dir)), remote_paths, serial)
}

// ---------- 投屏 ----------

#[tauri::command]
pub fn mirror_enable() -> Result<Value, String> {
    let exe = tool_path("scrcpy").ok_or_else(|| "未部署 scrcpy，无法投屏".to_string())?;
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn mirror_disable() -> Value {
    json!({"ok": true})
}

#[tauri::command]
pub fn mirror_keyevent(code: Option<Value>, serial: Option<Value>) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let code = match code {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => s,
        _ => return Err("缺少 keycode".to_string()),
    };
    adb(Some(&serial), &["shell", "input", "keyevent", &code])?;
    Ok(json!({"ok": true}))
}
