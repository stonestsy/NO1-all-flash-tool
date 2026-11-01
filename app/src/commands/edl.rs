//! 高通 EDL 命令：本地 XML/配置可用，Sahara/Firehose 协议操作明确返回未实现。

use super::util::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn opt_str(value: &Option<Value>) -> Option<String> {
    value.as_ref().and_then(Value::as_str).map(str::to_string)
}

fn opt_strs(value: &Option<Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::to_string))
            .collect(),
        Some(Value::String(item)) => vec![item.clone()],
        _ => Vec::new(),
    }
}

fn xml_attr(line: &str, key: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!("{}={}", key, quote);
        if let Some(start) = line.find(&pattern) {
            let rest = &line[start + pattern.len()..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }
    None
}

fn parse_u64(raw: &str) -> Option<u64> {
    let value = raw.trim().trim_end_matches('.').trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.2} GiB", bytes as f64 / 1073741824.0)
    } else if bytes >= 1024 * 1024 {
        format!("{:.1} MiB", bytes as f64 / 1048576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn xml_kind(path: &Path) -> Option<&'static str> {
    if !path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
    {
        return None;
    }
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name.starts_with("rawprogram") {
        Some("rawprogram")
    } else if name.starts_with("patch") {
        Some("patch")
    } else {
        None
    }
}

fn push_unique(items: &mut Vec<String>, value: String) {
    if !items.iter().any(|item| item.eq_ignore_ascii_case(&value)) {
        items.push(value);
    }
}

fn dialog_result(files: Vec<String>, warnings: Vec<String>, patch_added: Vec<String>) -> Value {
    let last_dir = files
        .first()
        .and_then(|file| Path::new(file).parent())
        .map(|dir| dir.to_string_lossy().to_string());
    json!({
        "files": files,
        "last_dir": last_dir,
        "warnings": warnings,
        "patch_added": patch_added
    })
}

fn collect_schemes(value: Value, output: &mut Vec<Value>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_schemes(item, output);
            }
        }
        Value::Object(mut object) => {
            if let Some(schemes) = object.remove("schemes") {
                collect_schemes(schemes, output);
            } else if object.contains_key("scheme")
                || object.contains_key("name")
                || object.contains_key("value")
            {
                output.push(Value::Object(object));
            }
        }
        _ => {}
    }
}

#[tauri::command]
pub fn edl_list_schemes() -> Value {
    // 本地配置可为 JSON 数组、{ schemes: [...] } 或单个方案对象。
    let root = data_dir("edl");
    let mut files: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("json"))
                .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }
    files.sort();
    let mut schemes = Vec::new();
    for file in files {
        if let Ok(text) = std::fs::read_to_string(file) {
            if let Ok(value) = serde_json::from_str(&text) {
                collect_schemes(value, &mut schemes);
            }
        }
    }
    json!(schemes)
}

fn disconnected_status() -> Value {
    json!({
        "connected": false,
        "configured": false,
        "port": null,
        "scheme": null,
        "sector_size": 4096
    })
}

#[tauri::command]
pub fn edl_session_status() -> Value {
    disconnected_status()
}

#[tauri::command]
pub fn edl_connect(port: Option<Value>) -> Result<Value, String> {
    let _ = port;
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_disconnect() -> Result<Value, String> {
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_identify_sahara(scheme: Option<Value>) -> Result<Value, String> {
    let _ = scheme;
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_reboot(mode: Option<Value>) -> Result<Value, String> {
    let _ = mode;
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_cancel() -> Result<Value, String> {
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_list_rawprogram_in_dir(dir: Option<Value>) -> Result<Value, String> {
    let dir = opt_str(&dir)
        .filter(|dir| !dir.trim().is_empty())
        .ok_or_else(|| "缺少目录路径".to_string())?;
    let entries =
        std::fs::read_dir(&dir).map_err(|error| format!("无法读取目录 {}: {}", dir, error))?;
    let mut rawprogram = Vec::new();
    let mut patches = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        match xml_kind(&path) {
            Some("rawprogram") => rawprogram.push(path.to_string_lossy().to_string()),
            Some("patch") => patches.push(path.to_string_lossy().to_string()),
            _ => {}
        }
    }
    rawprogram.sort_by_key(|path| path.to_lowercase());
    patches.sort_by_key(|path| path.to_lowercase());
    rawprogram.extend(patches);
    Ok(json!(rawprogram))
}

#[tauri::command]
pub fn edl_browse_patch_xml(default_dir: Option<Value>) -> Value {
    let dir = opt_str(&default_dir);
    let files = browse_files(
        "选择 patch XML",
        &[
            ("patch XML", "patch*.xml"),
            ("XML 文件", "*.xml"),
            ("所有文件", "*.*"),
        ],
        dir.as_deref(),
        true,
    );
    dialog_result(files, Vec::new(), Vec::new())
}

#[tauri::command]
pub fn edl_browse_raw_xml(default_dir: Option<Value>) -> Result<Value, String> {
    let dir = opt_str(&default_dir);
    let files = browse_files(
        "选择 rawprogram XML",
        &[
            ("rawprogram XML", "rawprogram*.xml"),
            ("patch XML", "patch*.xml"),
            ("XML 文件", "*.xml"),
            ("所有文件", "*.*"),
        ],
        dir.as_deref(),
        true,
    );
    if files.is_empty() {
        return Ok(dialog_result(files, Vec::new(), Vec::new()));
    }
    normalize_rawprogram(&files)
}

#[tauri::command]
pub fn edl_pick_rawprogram_files(default_dir: Option<Value>) -> Value {
    let dir = opt_str(&default_dir);
    let files = browse_files(
        "选择 rawprogram 文件",
        &[
            ("rawprogram XML", "rawprogram*.xml"),
            ("XML 文件", "*.xml"),
            ("所有文件", "*.*"),
        ],
        dir.as_deref(),
        true,
    );
    json!(files)
}

fn numeric_suffix(file_name: &str) -> &str {
    let stem = file_name
        .rsplit_once('.')
        .map(|parts| parts.0)
        .unwrap_or(file_name);
    let prefix = stem.trim_end_matches(|character: char| character.is_ascii_digit());
    &stem[prefix.len()..]
}

fn matching_patch(rawprogram: &Path) -> Option<PathBuf> {
    let parent = rawprogram.parent().unwrap_or(Path::new("."));
    let raw_name = rawprogram.file_name()?.to_string_lossy().to_lowercase();
    let expected = format!("patch{}", &raw_name["rawprogram".len()..]);
    let mut patches: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| xml_kind(path) == Some("patch"))
        .collect();
    patches.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    if let Some(path) = patches.iter().find(|path| {
        path.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .eq_ignore_ascii_case(&expected)
    }) {
        return Some(path.clone());
    }
    let suffix = numeric_suffix(&raw_name);
    if suffix.is_empty() {
        return None;
    }
    patches.into_iter().find(|path| {
        path.file_name()
            .map(|name| numeric_suffix(&name.to_string_lossy()) == suffix)
            .unwrap_or(false)
    })
}

fn normalize_rawprogram(paths: &[String]) -> Result<Value, String> {
    let mut rawprogram = Vec::new();
    let mut selected_patches = Vec::new();
    let mut directories = Vec::new();
    for file in paths {
        let path = PathBuf::from(file);
        if !path.is_file() {
            return Err(format!("XML 文件不存在: {}", file));
        }
        let full_path = path.to_string_lossy().to_string();
        match xml_kind(&path) {
            Some("rawprogram") => push_unique(&mut rawprogram, full_path),
            Some("patch") => push_unique(&mut selected_patches, full_path),
            _ => return Err(format!("不是 rawprogram/patch XML: {}", file)),
        }
        if let Some(parent) = path.parent() {
            push_unique(&mut directories, parent.to_string_lossy().to_string());
        }
    }
    if rawprogram.is_empty() {
        return Err("未选择 rawprogram XML".to_string());
    }
    rawprogram.sort_by_key(|path| path.to_lowercase());

    let mut warnings = Vec::new();
    let mut patch_added = selected_patches;
    if directories.len() > 1 {
        warnings.push("所选 XML 来自多个目录，请确认它们属于同一固件包".to_string());
    }
    for raw in &rawprogram {
        let path = PathBuf::from(raw);
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let contains_patch = std::fs::read(&path)
            .map(|data| String::from_utf8_lossy(&data).contains("<patch"))
            .unwrap_or(false);
        if contains_patch {
            warnings.push(format!("{} 已包含 patch 指令，免合并独立 patch 文件", name));
            continue;
        }
        if let Some(patch) = matching_patch(&path) {
            push_unique(&mut patch_added, patch.to_string_lossy().to_string());
        } else {
            warnings.push(format!("未找到与 {} 对应的 patch XML", name));
        }
    }
    Ok(dialog_result(rawprogram, warnings, patch_added))
}

#[tauri::command]
pub fn edl_normalize_rawprogram(paths: Option<Value>) -> Result<Value, String> {
    let paths = opt_strs(&paths);
    if paths.is_empty() {
        return Ok(dialog_result(Vec::new(), Vec::new(), Vec::new()));
    }
    normalize_rawprogram(&paths)
}

#[tauri::command]
pub fn edl_parse_rawprogram(
    paths: Option<Value>,
    sector_size: Option<Value>,
) -> Result<Value, String> {
    let paths = opt_strs(&paths);
    let sector_size = match sector_size {
        Some(Value::Number(number)) => number.as_u64().unwrap_or(0),
        Some(Value::String(text)) => parse_u64(&text).unwrap_or(0),
        None | Some(Value::Null) => 4096,
        _ => 0,
    };
    if sector_size == 0 {
        return Err("sectorSize 必须大于 0".to_string());
    }

    let mut output = Vec::new();
    for file in paths {
        let path = PathBuf::from(file);
        let data = std::fs::read(&path)
            .map_err(|error| format!("无法读取 {}: {}", path.display(), error))?;
        let content = String::from_utf8_lossy(&data);
        let directory = path.parent().unwrap_or(Path::new("."));
        for line in content.lines() {
            if !line.contains("<program") {
                continue;
            }
            let name = xml_attr(line, "label").unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let filename = xml_attr(line, "filename").unwrap_or_default();
            let has_filename = !filename.trim().is_empty();
            let image_path = if has_filename {
                directory
                    .join(filename.trim())
                    .to_string_lossy()
                    .to_string()
            } else {
                String::new()
            };
            let exists = has_filename && Path::new(&image_path).is_file();
            let lun = xml_attr(line, "physical_partition_number")
                .and_then(|value| parse_u64(&value))
                .unwrap_or(0);
            let start_raw = xml_attr(line, "start_sector").unwrap_or_else(|| "0".to_string());
            let start_sector = parse_u64(&start_raw);
            let start_sector_expr = if start_sector.is_none() && !start_raw.trim().is_empty() {
                Some(start_raw.clone())
            } else {
                None
            };
            let num_sectors = xml_attr(line, "num_partition_sectors")
                .and_then(|value| parse_u64(&value))
                .unwrap_or(0);
            let image_size = if exists {
                std::fs::metadata(&image_path)
                    .map(|meta| meta.len())
                    .unwrap_or(0)
            } else {
                0
            };
            let size_bytes = if exists {
                image_size
            } else {
                num_sectors.saturating_mul(sector_size)
            };
            output.push(json!({
                "name": name,
                "lun": lun,
                "start_sector": start_sector.unwrap_or(0),
                "start_sector_expr": start_sector_expr,
                "num_sectors": num_sectors,
                "size": format_size(size_bytes),
                "size_bytes": size_bytes,
                "kind": if !has_filename { "skip" } else if exists { "image" } else { "missing" },
                "selected": exists,
                "image_path": image_path,
                "image_size": image_size,
                "exists": exists,
                "xml_has_filename": has_filename
            }));
        }
    }
    Ok(json!(output))
}

#[tauri::command]
pub fn edl_will_set_boot_partition_after_xml(paths: Option<Value>) -> Result<Value, String> {
    for file in opt_strs(&paths) {
        let path = PathBuf::from(&file);
        let data = std::fs::read(&path).map_err(|error| format!("无法读取 {}: {}", file, error))?;
        let content = String::from_utf8_lossy(&data);
        for line in content.lines() {
            if !line.contains("<program")
                || xml_attr(line, "filename")
                    .unwrap_or_default()
                    .trim()
                    .is_empty()
            {
                continue;
            }
            let label = xml_attr(line, "label")
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            if label == "boot" || label == "boot_a" || label == "boot_b" {
                return Ok(Value::Bool(true));
            }
        }
    }
    Ok(Value::Bool(false))
}

#[tauri::command]
pub fn edl_ensure_cloud_loader(
    brand: Option<Value>,
    name: Option<Value>,
    op: Option<Value>,
) -> Result<Value, String> {
    let _ = (brand, name, op);
    Err("EDL 云端引导下载源未知".to_string())
}

#[tauri::command]
pub fn edl_resolve_oplus_loader(loader_path: Option<Value>) -> Result<Value, String> {
    let _ = loader_path;
    Err("OPlus 云端引导解析源未知".to_string())
}

#[tauri::command]
pub fn edl_read_gpt(
    generate_xml: Option<Value>,
    loader_path: Option<Value>,
    memory: Option<Value>,
    output_dir: Option<Value>,
    scheme: Option<Value>,
) -> Result<Value, String> {
    let _ = (generate_xml, loader_path, memory, output_dir, scheme);
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_peek_super_info(
    loader_path: Option<Value>,
    memory: Option<Value>,
    scheme: Option<Value>,
) -> Result<Value, String> {
    let _ = (loader_path, memory, scheme);
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_read_partitions(
    generate_xml: Option<Value>,
    loader_path: Option<Value>,
    memory: Option<Value>,
    output_dir: Option<Value>,
    scheme: Option<Value>,
    tasks: Option<Value>,
) -> Result<Value, String> {
    let _ = (generate_xml, loader_path, memory, output_dir, scheme, tasks);
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_write_partitions(
    loader_path: Option<Value>,
    memory: Option<Value>,
    protect_lun5: Option<Value>,
    raw_xml_paths: Option<Value>,
    scheme: Option<Value>,
    tasks: Option<Value>,
) -> Result<Value, String> {
    let _ = (
        loader_path,
        memory,
        protect_lun5,
        raw_xml_paths,
        scheme,
        tasks,
    );
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_erase_partitions(
    erase_mode: Option<Value>,
    loader_path: Option<Value>,
    memory: Option<Value>,
    protect_lun5: Option<Value>,
    scheme: Option<Value>,
    tasks: Option<Value>,
) -> Result<Value, String> {
    let _ = (erase_mode, loader_path, memory, protect_lun5, scheme, tasks);
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_flash_raw_xml_package(
    loader_path: Option<Value>,
    memory: Option<Value>,
    protect_lun5: Option<Value>,
    raw_xml_paths: Option<Value>,
    scheme: Option<Value>,
    tasks: Option<Value>,
) -> Result<Value, String> {
    let _ = (
        loader_path,
        memory,
        protect_lun5,
        raw_xml_paths,
        scheme,
        tasks,
    );
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_xiaomi_oneclick_acc(
    loader_path: Option<Value>,
    memory: Option<Value>,
    scheme: Option<Value>,
) -> Result<Value, String> {
    let _ = (loader_path, memory, scheme);
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_oplus_ocdt_verify(
    loader_path: Option<Value>,
    memory: Option<Value>,
    raw_xml_paths: Option<Value>,
    scheme: Option<Value>,
) -> Result<Value, String> {
    let _ = (loader_path, memory, raw_xml_paths, scheme);
    Err("EDL 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn edl_run_tool(
    action: Option<Value>,
    loader_path: Option<Value>,
    lun: Option<Value>,
    memory: Option<Value>,
    output_dir: Option<Value>,
    patch_paths: Option<Value>,
    scheme: Option<Value>,
) -> Result<Value, String> {
    let _ = (
        action,
        loader_path,
        lun,
        memory,
        output_dir,
        patch_paths,
        scheme,
    );
    Err("EDL 协议栈尚未实现".to_string())
}
