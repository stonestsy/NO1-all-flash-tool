//! 联发科 MTK 命令：真实解析本地配置，BROM/DA 会话操作明确返回未实现。

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn opt_str(value: &Option<Value>) -> Option<String> {
    value.as_ref().and_then(Value::as_str).map(str::to_string)
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

fn parse_bool(raw: &str) -> Option<bool> {
    match raw.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Some(true),
        "false" | "no" | "0" => Some(false),
        _ => None,
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(boolean) => boolean.to_string(),
        _ => String::new(),
    }
}

fn value_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => parse_u64(text),
        _ => None,
    }
}

fn value_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(boolean) => Some(*boolean),
        Value::String(text) => parse_bool(text),
        Value::Number(number) => number.as_i64().map(|number| number != 0),
        _ => None,
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

fn xml_tag(content: &str, key: &str) -> Option<String> {
    let open = format!("<{}>", key);
    let close = format!("</{}>", key);
    let start = content.find(&open)? + open.len();
    let end = content[start..].find(&close)? + start;
    Some(content[start..end].trim().to_string())
}

fn first_xml_tag(content: &str, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| xml_tag(content, key))
        .unwrap_or_default()
}

fn xml_blocks(content: &str, tag: &str) -> Vec<(String, String)> {
    let needle = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let mut blocks = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = content[offset..].find(&needle) {
        let start = offset + relative;
        let after_name = start + needle.len();
        let boundary = content[after_name..].chars().next();
        let valid_boundary = match boundary {
            Some('>') | Some('/') => true,
            Some(character) => character.is_whitespace(),
            None => false,
        };
        if !valid_boundary {
            offset = after_name;
            continue;
        }
        let Some(open_relative) = content[after_name..].find('>') else {
            break;
        };
        let open_end = after_name + open_relative;
        let opening = &content[start..=open_end];
        if opening.trim_end().ends_with("/>") {
            blocks.push((opening.to_string(), String::new()));
            offset = open_end + 1;
            continue;
        }
        let body_start = open_end + 1;
        let Some(close_relative) = content[body_start..].find(&close) else {
            break;
        };
        let body_end = body_start + close_relative;
        blocks.push((
            opening.to_string(),
            content[body_start..body_end].to_string(),
        ));
        offset = body_end + close.len();
    }
    blocks
}

fn xml_field(opening: &str, body: &str, keys: &[&str]) -> String {
    for key in keys {
        if let Some(value) = xml_tag(body, key).or_else(|| xml_attr(opening, key)) {
            return value.trim().to_string();
        }
    }
    String::new()
}

#[derive(Default)]
struct MtkPartition {
    name: String,
    image: String,
    address: u64,
    size: u64,
    storage: String,
    region: String,
    download: Option<bool>,
}

#[derive(Default)]
struct ConfigDetails {
    format: String,
    platform: String,
    scatter_path: String,
    da_path: String,
    preloader_path: String,
    auth_path: String,
    readback_backup: bool,
    partitions: Vec<Value>,
    warnings: Vec<String>,
}

fn resolve_path(directory: &Path, value: &str) -> String {
    let value = value.trim();
    if value.is_empty() || value.eq_ignore_ascii_case("NONE") {
        return String::new();
    }
    let path = PathBuf::from(value);
    let full_path = if path.is_absolute() {
        path
    } else {
        directory.join(path)
    };
    full_path.to_string_lossy().to_string()
}

fn partition_json(partition: MtkPartition, directory: &Path) -> Value {
    let has_image =
        !partition.image.trim().is_empty() && !partition.image.trim().eq_ignore_ascii_case("NONE");
    let image_path = if has_image {
        resolve_path(directory, &partition.image)
    } else {
        String::new()
    };
    let image_size = if has_image {
        std::fs::metadata(&image_path).ok().map(|meta| meta.len())
    } else {
        None
    };
    let download = partition.download.unwrap_or(has_image);
    json!({
        "name": partition.name,
        "address": partition.address,
        "size": partition.size,
        "storage": partition.storage,
        "region": partition.region,
        "file_name": partition.image,
        "image_path": image_path,
        "image_size": image_size,
        "download": download,
        "exists": image_size.is_some()
    })
}

fn object_text(object: &serde_json::Map<String, Value>, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| object.get(*key).map(value_text))
        .unwrap_or_default()
}

fn object_u64(object: &serde_json::Map<String, Value>, keys: &[&str]) -> u64 {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(value_u64))
        .unwrap_or(0)
}

fn object_bool(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(value_bool))
}

fn inspect_json(content: &str, directory: &Path) -> Result<ConfigDetails, String> {
    let root: Value =
        serde_json::from_str(content).map_err(|error| format!("JSON 配置解析失败: {}", error))?;
    let (object, array_only) = match &root {
        Value::Object(object) => (Some(object), None),
        Value::Array(partitions) => (None, Some(partitions)),
        _ => return Err("JSON 配置根节点必须是对象或数组".to_string()),
    };
    let empty = serde_json::Map::new();
    let object = object.unwrap_or(&empty);
    let mut details = ConfigDetails {
        format: if array_only.is_some() {
            "json".to_string()
        } else {
            object_text(object, &["format"]).trim().to_string()
        },
        platform: object_text(object, &["platform", "platform_name"]),
        scatter_path: resolve_path(
            directory,
            &object_text(object, &["scatter_path", "scatter"]),
        ),
        da_path: resolve_path(
            directory,
            &object_text(object, &["da_path", "da", "download_agent"]),
        ),
        preloader_path: resolve_path(
            directory,
            &object_text(object, &["preloader_path", "preloader"]),
        ),
        auth_path: resolve_path(directory, &object_text(object, &["auth_path", "auth"])),
        readback_backup: object_bool(object, &["readback_backup"]).unwrap_or(false),
        ..ConfigDetails::default()
    };
    if details.format.is_empty() {
        details.format = "json".to_string();
    }
    if let Some(warnings) = object.get("warnings").and_then(Value::as_array) {
        details.warnings = warnings
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
    }
    let partitions = array_only.or_else(|| object.get("partitions").and_then(Value::as_array));
    if let Some(partitions) = partitions {
        for value in partitions {
            let Some(partition) = value.as_object() else {
                details.warnings.push("忽略非对象分区配置".to_string());
                continue;
            };
            let item = MtkPartition {
                name: object_text(partition, &["name", "partition_name", "partition"]),
                image: object_text(partition, &["image_path", "file_name", "filename", "path"]),
                address: object_u64(
                    partition,
                    &[
                        "address",
                        "linear_start_addr",
                        "physical_start_addr",
                        "start_addr",
                    ],
                ),
                size: object_u64(partition, &["size", "partition_size"]),
                storage: object_text(partition, &["storage"]),
                region: object_text(partition, &["region"]),
                download: object_bool(partition, &["download", "is_download"]),
            };
            if item.name.trim().is_empty() {
                details
                    .warnings
                    .push("忽略缺少 name 的分区配置".to_string());
                continue;
            }
            details.partitions.push(partition_json(item, directory));
        }
    }
    if details.partitions.is_empty() {
        details
            .warnings
            .push("配置中没有 partitions 数组".to_string());
    }
    Ok(details)
}

fn text_key_value(line: &str) -> Option<(String, String)> {
    let colon = line.find(':');
    let equals = line.find('=');
    let split = match (colon, equals) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(index), None) | (None, Some(index)) => Some(index),
        (None, None) => None,
    }?;
    let key = line[..split]
        .trim()
        .trim_start_matches('-')
        .trim()
        .to_lowercase();
    let value = line[split + 1..].trim().to_string();
    Some((key, value))
}

fn inspect_text(content: &str, directory: &Path, source: &Path) -> ConfigDetails {
    let mut details = ConfigDetails {
        format: if content.contains("partition_name") {
            "scatter".to_string()
        } else {
            "text".to_string()
        },
        ..ConfigDetails::default()
    };
    let mut current: Option<MtkPartition> = None;
    let mut parsed = Vec::new();
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some((key, value)) = text_key_value(line) else {
            continue;
        };
        if key == "partition_name" {
            if let Some(partition) = current.take() {
                parsed.push(partition);
            }
            current = Some(MtkPartition {
                name: value,
                ..MtkPartition::default()
            });
            continue;
        }
        if let Some(partition) = current.as_mut() {
            match key.as_str() {
                "file_name" | "filename" | "image_path" => partition.image = value,
                "linear_start_addr" | "physical_start_addr" | "address" | "start_addr" => {
                    if key == "linear_start_addr" || partition.address == 0 {
                        partition.address = parse_u64(&value).unwrap_or(0);
                    }
                }
                "partition_size" | "size" => partition.size = parse_u64(&value).unwrap_or(0),
                "storage" => partition.storage = value,
                "region" => partition.region = value,
                "is_download" | "download" => partition.download = parse_bool(&value),
                "platform" if details.platform.is_empty() => details.platform = value,
                _ => {}
            }
            continue;
        }
        match key.as_str() {
            "platform" | "platform_name" => details.platform = value,
            "scatter_path" | "scatter" => details.scatter_path = resolve_path(directory, &value),
            "da_path" | "da" | "download_agent" => {
                details.da_path = resolve_path(directory, &value)
            }
            "preloader_path" | "preloader" => {
                details.preloader_path = resolve_path(directory, &value)
            }
            "auth_path" | "auth" => details.auth_path = resolve_path(directory, &value),
            "readback_backup" => details.readback_backup = parse_bool(&value).unwrap_or(false),
            _ => {}
        }
    }
    if let Some(partition) = current.take() {
        parsed.push(partition);
    }
    details.partitions = parsed
        .into_iter()
        .filter(|partition| !partition.name.trim().is_empty())
        .map(|partition| partition_json(partition, directory))
        .collect();
    if details.format == "scatter" && details.scatter_path.is_empty() {
        details.scatter_path = source.to_string_lossy().to_string();
    }
    if details.partitions.is_empty() {
        details.warnings.push("未解析到分区项".to_string());
    }
    details
}

fn inspect_xml(content: &str, directory: &Path) -> ConfigDetails {
    let mut details = ConfigDetails {
        format: "flash.xml".to_string(),
        platform: first_xml_tag(content, &["platform", "platform_name"]),
        scatter_path: resolve_path(
            directory,
            &first_xml_tag(content, &["scatter_path", "scatter", "scatter_file"]),
        ),
        da_path: resolve_path(
            directory,
            &first_xml_tag(content, &["da_path", "da", "da_file", "download_agent"]),
        ),
        preloader_path: resolve_path(
            directory,
            &first_xml_tag(content, &["preloader_path", "preloader", "preloader_file"]),
        ),
        auth_path: resolve_path(
            directory,
            &first_xml_tag(content, &["auth_path", "auth", "auth_file"]),
        ),
        readback_backup: first_xml_tag(content, &["readback_backup"])
            .as_str()
            .trim()
            .to_lowercase()
            == "true",
        ..ConfigDetails::default()
    };
    for (opening, body) in xml_blocks(content, "partition") {
        let item = MtkPartition {
            name: xml_field(&opening, &body, &["name", "partition_name"]),
            image: xml_field(&opening, &body, &["image_path", "file_name", "filename"]),
            address: parse_u64(&xml_field(
                &opening,
                &body,
                &[
                    "linear_start_addr",
                    "address",
                    "physical_start_addr",
                    "start_addr",
                ],
            ))
            .unwrap_or(0),
            size: parse_u64(&xml_field(&opening, &body, &["partition_size", "size"])).unwrap_or(0),
            storage: xml_field(&opening, &body, &["storage"]),
            region: xml_field(&opening, &body, &["region"]),
            download: parse_bool(&xml_field(&opening, &body, &["is_download", "download"])),
        };
        if item.name.trim().is_empty() {
            details
                .warnings
                .push("忽略缺少 name 的 XML 分区项".to_string());
            continue;
        }
        details.partitions.push(partition_json(item, directory));
    }
    if details.partitions.is_empty() {
        details.warnings.push("未解析到 XML 分区项".to_string());
    }
    details
}

fn discover_scatter(source: &Path) -> Option<String> {
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(source.parent().unwrap_or(Path::new(".")))
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            path.is_file()
                && name.contains("scatter")
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("txt"))
                    .unwrap_or(false)
        })
        .collect();
    candidates.sort();
    if candidates.len() == 1 {
        candidates
            .pop()
            .map(|path| path.to_string_lossy().to_string())
    } else {
        None
    }
}

fn preloader_from_partitions(partitions: &[Value]) -> String {
    partitions
        .iter()
        .find(|partition| {
            partition
                .get("name")
                .and_then(Value::as_str)
                .map(|name| name.eq_ignore_ascii_case("preloader"))
                .unwrap_or(false)
        })
        .and_then(|partition| partition.get("image_path"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

#[tauri::command]
pub fn mtk_inspect_config(path: Option<Value>) -> Result<Value, String> {
    let source = opt_str(&path)
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| "缺少配置文件路径".to_string())?;
    let source = PathBuf::from(source);
    let metadata = std::fs::metadata(&source)
        .map_err(|error| format!("无法访问 {}: {}", source.display(), error))?;
    if !metadata.is_file() {
        return Err(format!("配置路径不是文件: {}", source.display()));
    }
    let data = std::fs::read(&source)
        .map_err(|error| format!("无法读取 {}: {}", source.display(), error))?;
    let content = String::from_utf8_lossy(&data);
    if content.trim().is_empty() {
        return Err("配置文件为空".to_string());
    }
    let directory = source.parent().unwrap_or(Path::new("."));
    let trimmed = content.trim_start();
    let mut details = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        inspect_json(&content, directory)?
    } else if trimmed.starts_with('<') {
        inspect_xml(&content, directory)
    } else {
        inspect_text(&content, directory, &source)
    };
    if details.format == "flash.xml" && details.scatter_path.is_empty() {
        if let Some(scatter) = discover_scatter(&source) {
            details.scatter_path = scatter;
        } else {
            details
                .warnings
                .push("flash.xml 未声明唯一的 scatter 文件".to_string());
        }
    }
    if details.preloader_path.is_empty() {
        details.preloader_path = preloader_from_partitions(&details.partitions);
    }
    Ok(json!({
        "path": source.to_string_lossy(),
        "file_name": source.file_name().unwrap_or_default().to_string_lossy(),
        "file_size": metadata.len(),
        "format": details.format,
        "platform": details.platform,
        "scatter_path": details.scatter_path,
        "da_path": details.da_path,
        "preloader_path": details.preloader_path,
        "auth_path": details.auth_path,
        "readback_backup": details.readback_backup,
        "partitions": details.partitions,
        "warnings": details.warnings
    }))
}

#[tauri::command]
pub fn mtk_list_cloud_da() -> Value {
    json!([])
}

#[tauri::command]
pub fn mtk_ensure_cloud_da(brand: Option<Value>, name: Option<Value>) -> Result<Value, String> {
    let _ = (brand, name);
    Err("MTK 云端 DA 下载源未知".to_string())
}

#[tauri::command]
pub fn mtk_ensure_cloud_auth() -> Result<Value, String> {
    Err("MTK 云端 Auth 下载源未知".to_string())
}

#[tauri::command]
pub fn mtk_connect(
    auth_path: Option<Value>,
    da_path: Option<Value>,
    preloader_path: Option<Value>,
) -> Result<Value, String> {
    let _ = (auth_path, da_path, preloader_path);
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_disconnect() -> Result<Value, String> {
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_reboot() -> Result<Value, String> {
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_read_partition_table() -> Result<Value, String> {
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_peek_super() -> Result<Value, String> {
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_flash_package(
    mode: Option<Value>,
    scatter_path: Option<Value>,
) -> Result<Value, String> {
    let _ = (mode, scatter_path);
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_read_partition(
    output_path: Option<Value>,
    partition: Option<Value>,
) -> Result<Value, String> {
    let _ = (output_path, partition);
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_write_partition(
    input_path: Option<Value>,
    partition: Option<Value>,
) -> Result<Value, String> {
    let _ = (input_path, partition);
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_erase_partition(partition: Option<Value>) -> Result<Value, String> {
    let _ = partition;
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_submit_signature(signature: Option<Value>) -> Result<Value, String> {
    let _ = signature;
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_request_signature(blob: Option<Value>, kami: Option<Value>) -> Result<Value, String> {
    let _ = (blob, kami);
    Err("MTK BROM 协议栈尚未实现".to_string())
}

#[tauri::command]
pub fn mtk_cancel_auth() -> Result<Value, String> {
    Err("MTK BROM 协议栈尚未实现".to_string())
}
