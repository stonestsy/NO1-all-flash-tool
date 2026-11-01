//! Fastboot 刷机脚本解析（bat / rawprogram.xml / scatter.txt）与批量刷写。

use super::util::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

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

#[tauri::command]
pub fn fastboot_browse_scripts(default_dir: Option<Value>) -> Value {
    let dir = opt_str(&default_dir);
    let files = browse_files(
        "选择刷机脚本",
        &[
            ("刷机脚本", "*.bat;*.xml;*.txt"),
            ("批处理脚本", "*.bat"),
            ("rawprogram", "*.xml"),
            ("scatter", "*.txt"),
            ("所有文件", "*.*"),
        ],
        dir.as_deref(),
        true,
    );
    json!(files)
}

fn xml_attr(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=\"", key);
    let start = line.find(&pattern)? + pattern.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn parse_bat(path: &Path, content: &str) -> Vec<Value> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut items = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.starts_with("rem") || line.starts_with("::") || line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        if !lower.contains("fastboot") {
            continue;
        }
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let flash_pos = tokens.iter().position(|t| *t == "flash");
        if let Some(i) = flash_pos {
            if tokens.len() > i + 2 {
                let partition = tokens[i + 1].to_string();
                let image = tokens[i + 2].trim_matches('"');
                let img_path = dir.join(image);
                let size = std::fs::metadata(&img_path).map(|m| m.len()).unwrap_or(0);
                items.push(json!({
                    "name": partition,
                    "path": img_path.to_string_lossy(),
                    "size": size,
                    "kind": "image",
                    "op": "flash",
                    "target": "",
                    "lun": 0
                }));
            }
            continue;
        }
        if let Some(i) = tokens.iter().position(|t| *t == "erase") {
            if tokens.len() > i + 1 {
                items.push(json!({"name": tokens[i + 1], "path": "", "size": 0, "kind": "", "op": "erase", "target": "", "lun": 0}));
            }
            continue;
        }
        if tokens.iter().any(|t| *t == "reboot") {
            items.push(json!({"name": "reboot", "path": "", "size": 0, "kind": "", "op": "reboot", "target": "", "lun": 0}));
        }
    }
    items
}

fn parse_rawprogram(path: &Path, content: &str) -> Vec<Value> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut items = Vec::new();
    for line in content.lines() {
        if !line.contains("<program") {
            continue;
        }
        let label = xml_attr(line, "label").unwrap_or_default();
        let filename = xml_attr(line, "filename").unwrap_or_default();
        if label.is_empty() {
            continue;
        }
        let img_path = if filename.is_empty() {
            PathBuf::new()
        } else {
            dir.join(&filename)
        };
        let size = std::fs::metadata(&img_path).map(|m| m.len()).unwrap_or(0);
        let lun = xml_attr(line, "physical_partition_number")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        items.push(json!({
            "name": label,
            "path": img_path.to_string_lossy(),
            "size": size,
            "kind": "image",
            "op": if filename.is_empty() { "skip" } else { "flash" },
            "target": "",
            "lun": lun
        }));
    }
    items
}

fn parse_scatter(path: &Path, content: &str) -> Vec<Value> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut items = Vec::new();
    let mut name = String::new();
    let mut file = String::new();
    let flush = |name: &mut String, file: &mut String, items: &mut Vec<Value>| {
        if !name.is_empty() {
            let img_path = if file.is_empty() || file == "NONE" {
                PathBuf::new()
            } else {
                dir.join(&*file)
            };
            let size = std::fs::metadata(&img_path).map(|m| m.len()).unwrap_or(0);
            items.push(json!({
                "name": name.clone(),
                "path": img_path.to_string_lossy(),
                "size": size,
                "kind": "image",
                "op": "flash",
                "target": "",
                "lun": 0
            }));
        }
        name.clear();
        file.clear();
    };
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("partition_name:") {
            flush(&mut name, &mut file, &mut items);
            name = line["partition_name:".len()..].trim().to_string();
        } else if line.starts_with("file_name:") {
            file = line["file_name:".len()..].trim().to_string();
        }
    }
    flush(&mut name, &mut file, &mut items);
    items
}

fn parse_one(path: &Path) -> (Vec<Value>, &'static str) {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let lower = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    if lower.contains("rawprogram") || path.extension().map(|e| e == "xml").unwrap_or(false) {
        (parse_rawprogram(path, &content), "rawprogram")
    } else if lower.contains("scatter") {
        (parse_scatter(path, &content), "scatter")
    } else {
        (parse_bat(path, &content), "bat")
    }
}

#[tauri::command]
pub fn fastboot_parse_scripts(script_paths: Option<Value>) -> Value {
    let paths = opt_strs(&script_paths);
    let mut items: Vec<Value> = Vec::new();
    let mut kind = "bat";
    let mut dirs: Vec<String> = Vec::new();
    for p in &paths {
        let path = PathBuf::from(p);
        if !path.exists() {
            continue;
        }
        let (mut parsed, k) = parse_one(&path);
        kind = k;
        items.append(&mut parsed);
        if let Some(dir) = path.parent() {
            dirs.push(dir.to_string_lossy().to_string());
        }
    }
    json!({
        "items": items,
        "meta": {"dirs": dirs, "files": paths},
        "script_kind": kind
    })
}

#[tauri::command]
pub fn fastboot_parse_script(script_path: Option<Value>) -> Value {
    let path = opt_str(&script_path).unwrap_or_default();
    fastboot_parse_scripts(Some(json!([path])))
}

#[tauri::command]
pub fn fastboot_verify_model_cmd(
    args: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let exe = fastboot_path()?;
    let out = run_tool_str(&exe, &["-s", &serial, "getvar", "product"])?;
    let product = out
        .lines()
        .filter_map(|l| {
            l.strip_prefix("(bootloader)")?
                .split_once(':')
                .map(|(_, v)| v.trim().to_string())
        })
        .next()
        .unwrap_or_default();
    let expected = match &args {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Object(o)) => o
            .get("model")
            .or_else(|| o.get("product"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    };
    let matched = expected.is_empty() || product.eq_ignore_ascii_case(&expected);
    Ok(json!({"ok": matched, "product": product, "expected": expected, "match": matched}))
}

#[tauri::command]
pub fn fastboot_batch_write(
    options: Option<Value>,
    serial: Option<Value>,
    tasks: Option<Value>,
) -> Result<Value, String> {
    let serial = opt_str(&serial).ok_or_else(|| "缺少 serial".to_string())?;
    let exe = fastboot_path()?;
    let list = match tasks {
        Some(Value::Array(a)) => a,
        _ => Vec::new(),
    };
    let mut failed: Vec<String> = Vec::new();
    let mut done = 0usize;
    for task in &list {
        let op = task.get("op").and_then(Value::as_str).unwrap_or("flash");
        let name = task
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let image = task
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let result = match op {
            "flash" => run_tool_str(&exe, &["-s", &serial, "flash", &name, &image]),
            "erase" => run_tool_str(&exe, &["-s", &serial, "erase", &name]),
            "reboot" => run_tool_str(&exe, &["-s", &serial, "reboot"]),
            _ => continue,
        };
        match result {
            Ok(_) => done += 1,
            Err(_) => failed.push(name),
        }
    }
    Ok(json!({"ok": failed.is_empty(), "failed": failed, "done": done, "total": list.len()}))
}

#[tauri::command]
pub fn oneplus_scan_images(directory: Option<Value>) -> Value {
    let dir = opt_str(&directory).unwrap_or_default();
    let mut items = Vec::new();
    if let Ok(read) = std::fs::read_dir(&dir) {
        for entry in read.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "img").unwrap_or(false) {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                items.push(json!({"name": name, "path": path.to_string_lossy(), "size": size, "kind": "image"}));
            }
        }
    }
    json!(items)
}

#[tauri::command]
pub fn oneplus_fix_ftd_cmd(
    partitions: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    Err("一加 FTD 修复流程尚未实现".to_string())
}

#[tauri::command]
pub fn oneplus_write_partitions_cmd(
    options: Option<Value>,
    partitions: Option<Value>,
    selected: Option<Value>,
    serial: Option<Value>,
) -> Result<Value, String> {
    // 与批量刷写同语义：partitions 为任务列表
    fastboot_batch_write(options, serial, partitions)
}
