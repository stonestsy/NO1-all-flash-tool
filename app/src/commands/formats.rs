//! 镜像与固件格式解析：payload、boot、super、OFP 及常用分区镜像。

use super::util::*;
use serde_json::{json, Value};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static SUPER_MERGE_CANCEL: AtomicBool = AtomicBool::new(false);
static OFP_DECRYPT_CANCEL: AtomicBool = AtomicBool::new(false);

fn required_text(value: &Option<Value>, name: &str) -> Result<String, String> {
    value
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("缺少参数 {}", name))
}

fn required_path(value: &Option<Value>, name: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(required_text(value, name)?))
}

fn read_prefix(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("无法打开 {}: {}", path.display(), e))?;
    let mut data = Vec::with_capacity(limit);
    file.take(limit as u64)
        .read_to_end(&mut data)
        .map_err(|e| format!("读取文件失败: {}", e))?;
    Ok(data)
}

fn u16_le(data: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        data.get(offset..offset + 2)?.try_into().ok()?,
    ))
}

fn u32_le(data: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(offset..offset + 4)?.try_into().ok()?,
    ))
}

fn u64_le(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
}

fn u64_be(data: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_be_bytes(
        data.get(offset..offset + 8)?.try_into().ok()?,
    ))
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

// ---------- payload.bin ----------

#[derive(Debug)]
struct PayloadPartition {
    name: String,
    size: u64,
}

#[derive(Debug)]
struct PayloadMetadata {
    version: u64,
    manifest_size: u64,
    metadata_signature_size: u32,
    data_offset: u64,
    block_size: u64,
    minor_version: Option<u64>,
    max_timestamp: Option<u64>,
    partitions: Vec<PayloadPartition>,
}

struct TempDownload {
    path: PathBuf,
}

impl Drop for TempDownload {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn is_http_url(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    lower.starts_with("https://") || lower.starts_with("http://")
}

fn payload_source_path(source: &str) -> Result<(PathBuf, Option<TempDownload>), String> {
    if is_http_url(source) {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let path = data_dir("tmp").join(format!("payload-{}-{}.bin", std::process::id(), stamp));
        download_file(source, &path).map_err(|e| format!("下载 payload.bin 失败: {}", e))?;
        Ok((path.clone(), Some(TempDownload { path })))
    } else {
        let path = PathBuf::from(source);
        if !path.is_file() {
            return Err(format!("payload.bin 不存在: {}", path.display()));
        }
        Ok((path, None))
    }
}

fn read_varint(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut value = 0u64;
    for index in 0..10u32 {
        let byte = *data
            .get(*pos)
            .ok_or_else(|| "protobuf varint 意外结束".to_string())?;
        *pos += 1;
        if index == 9 && byte > 1 {
            return Err("protobuf varint 溢出".to_string());
        }
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err("protobuf varint 过长".to_string())
}

fn protobuf_bytes<'a>(data: &'a [u8], pos: &mut usize) -> Result<&'a [u8], String> {
    let length = read_varint(data, pos)?;
    let length = usize::try_from(length).map_err(|_| "protobuf 字段长度溢出".to_string())?;
    let end = pos
        .checked_add(length)
        .ok_or_else(|| "protobuf 字段长度溢出".to_string())?;
    let bytes = data
        .get(*pos..end)
        .ok_or_else(|| "protobuf 字段超出数据边界".to_string())?;
    *pos = end;
    Ok(bytes)
}

fn skip_bytes(data: &[u8], pos: &mut usize, count: usize) -> Result<(), String> {
    let end = pos
        .checked_add(count)
        .ok_or_else(|| "protobuf 字段长度溢出".to_string())?;
    if end > data.len() {
        return Err("protobuf 字段超出数据边界".to_string());
    }
    *pos = end;
    Ok(())
}

fn skip_protobuf_field(data: &[u8], pos: &mut usize, wire_type: u64) -> Result<(), String> {
    match wire_type {
        0 => {
            let _ = read_varint(data, pos)?;
            Ok(())
        }
        1 => skip_bytes(data, pos, 8),
        2 => {
            let _ = protobuf_bytes(data, pos)?;
            Ok(())
        }
        5 => skip_bytes(data, pos, 4),
        _ => Err(format!("不支持的 protobuf wire type: {}", wire_type)),
    }
}

fn parse_partition_info(data: &[u8]) -> Result<u64, String> {
    let mut pos = 0usize;
    let mut size = 0u64;
    while pos < data.len() {
        let tag = read_varint(data, &mut pos)?;
        let field = tag >> 3;
        let wire_type = tag & 7;
        if field == 0 {
            return Err("protobuf 字段编号不能为 0".to_string());
        }
        if field == 2 && wire_type == 0 {
            size = read_varint(data, &mut pos)?;
        } else {
            skip_protobuf_field(data, &mut pos, wire_type)?;
        }
    }
    Ok(size)
}

fn parse_payload_partition(data: &[u8]) -> Result<PayloadPartition, String> {
    let mut pos = 0usize;
    let mut name = String::new();
    let mut size = 0u64;
    while pos < data.len() {
        let tag = read_varint(data, &mut pos)?;
        let field = tag >> 3;
        let wire_type = tag & 7;
        if field == 0 {
            return Err("protobuf 字段编号不能为 0".to_string());
        }
        match (field, wire_type) {
            (1, 2) => {
                name = std::str::from_utf8(protobuf_bytes(data, &mut pos)?)
                    .map_err(|_| "分区名不是合法 UTF-8".to_string())?
                    .to_string();
            }
            (3, 2) => size = parse_partition_info(protobuf_bytes(data, &mut pos)?)?,
            _ => skip_protobuf_field(data, &mut pos, wire_type)?,
        }
    }
    if name.is_empty() {
        return Err("payload 分区缺少 partition_name".to_string());
    }
    Ok(PayloadPartition { name, size })
}

fn parse_payload_manifest(
    data: &[u8],
) -> Result<(u64, Option<u64>, Option<u64>, Vec<PayloadPartition>), String> {
    let mut pos = 0usize;
    let mut block_size = 4096u64;
    let mut minor_version = None;
    let mut max_timestamp = None;
    let mut partitions = Vec::new();
    while pos < data.len() {
        let tag = read_varint(data, &mut pos)?;
        let field = tag >> 3;
        let wire_type = tag & 7;
        if field == 0 {
            return Err("protobuf 字段编号不能为 0".to_string());
        }
        match (field, wire_type) {
            (2, 2) => partitions.push(parse_payload_partition(protobuf_bytes(data, &mut pos)?)?),
            (3, 0) => block_size = read_varint(data, &mut pos)?,
            (4, 0) => minor_version = Some(read_varint(data, &mut pos)?),
            (5, 0) => max_timestamp = Some(read_varint(data, &mut pos)?),
            _ => skip_protobuf_field(data, &mut pos, wire_type)?,
        }
    }
    if partitions.is_empty() {
        return Err("payload manifest 中没有分区".to_string());
    }
    Ok((block_size, minor_version, max_timestamp, partitions))
}

fn read_payload_metadata(source: &str) -> Result<PayloadMetadata, String> {
    let (path, _temporary) = payload_source_path(source)?;
    let mut file = File::open(&path).map_err(|e| format!("无法打开 payload.bin: {}", e))?;
    let file_size = file.metadata().map_err(|e| e.to_string())?.len();
    if file_size < 20 {
        return Err("payload.bin 文件头不完整".to_string());
    }

    let mut header = [0u8; 20];
    file.read_exact(&mut header)
        .map_err(|e| format!("读取 payload 文件头失败: {}", e))?;
    if &header[..4] != b"CrAU" {
        return Err("不是有效的 payload.bin（缺少 CrAU magic）".to_string());
    }
    let version = u64_be(&header, 4).ok_or_else(|| "payload 版本字段不完整".to_string())?;
    let manifest_size =
        u64_be(&header, 12).ok_or_else(|| "payload manifest 长度不完整".to_string())?;
    if version == 0 {
        return Err("不支持的 payload 主版本 0".to_string());
    }

    let mut header_size = 20u64;
    let mut metadata_signature_size = 0u32;
    if version >= 2 {
        let mut signature = [0u8; 4];
        file.read_exact(&mut signature)
            .map_err(|e| format!("读取 payload 签名长度失败: {}", e))?;
        metadata_signature_size = u32::from_be_bytes(signature);
        header_size = 24;
    }
    if manifest_size == 0 || manifest_size > 64 * 1024 * 1024 {
        return Err(format!("payload manifest 大小异常: {}", manifest_size));
    }
    let data_offset = header_size
        .checked_add(manifest_size)
        .and_then(|v| v.checked_add(u64::from(metadata_signature_size)))
        .ok_or_else(|| "payload 数据偏移溢出".to_string())?;
    if data_offset > file_size {
        return Err("payload manifest 或签名超出文件长度".to_string());
    }

    let mut manifest = vec![0u8; manifest_size as usize];
    file.read_exact(&mut manifest)
        .map_err(|e| format!("读取 payload manifest 失败: {}", e))?;
    let (block_size, minor_version, max_timestamp, partitions) = parse_payload_manifest(&manifest)?;
    Ok(PayloadMetadata {
        version,
        manifest_size,
        metadata_signature_size,
        data_offset,
        block_size,
        minor_version,
        max_timestamp,
        partitions,
    })
}

fn payload_url_error(url: &str) -> Option<String> {
    if url.is_empty() {
        return Some("URL 不能为空".to_string());
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Some("URL 不能包含空白或控制字符".to_string());
    }
    let lower = url.to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &url[8..]
    } else if lower.starts_with("http://") {
        &url[7..]
    } else {
        return Some("仅支持 http:// 或 https:// URL".to_string());
    };
    let authority_end = rest
        .find(|c: char| matches!(c, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    if authority.is_empty() || authority.contains('@') {
        return Some("URL 主机为空或包含凭据".to_string());
    }

    let (host, port) = if let Some(bracket_end) = authority.find(']') {
        if !authority.starts_with('[') {
            return Some("URL 主机格式无效".to_string());
        }
        let host = &authority[1..bracket_end];
        let suffix = &authority[bracket_end + 1..];
        if !suffix.is_empty() && !suffix.starts_with(':') {
            return Some("URL 主机格式无效".to_string());
        }
        (host, suffix.strip_prefix(':'))
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        if host.contains(':') {
            return Some("IPv6 主机必须使用方括号".to_string());
        }
        (host, Some(port))
    } else {
        (authority, None)
    };
    if host.is_empty() {
        return Some("URL 主机不能为空".to_string());
    }
    if let Some(port) = port {
        if port.is_empty()
            || !port.bytes().all(|b| b.is_ascii_digit())
            || port.parse::<u16>().ok().filter(|p| *p > 0).is_none()
        {
            return Some("URL 端口无效".to_string());
        }
    }
    None
}

#[tauri::command]
pub fn payload_validate_url(url: Option<Value>) -> Value {
    let url = url
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    match payload_url_error(url) {
        Some(error) => json!({"valid": false, "url": url, "status": Value::Null, "error": error}),
        None => json!({"valid": true, "url": url, "status": Value::Null, "error": Value::Null}),
    }
}

#[tauri::command]
pub fn payload_list_partitions(source: Option<Value>) -> Result<Value, String> {
    let source = required_text(&source, "source")?;
    let metadata = read_payload_metadata(&source)?;
    let partitions: Vec<Value> = metadata
        .partitions
        .iter()
        .map(|partition| {
            json!({
                "name": partition.name,
                "size": format_size(partition.size),
                "size_bytes": partition.size,
                "kind": "payload"
            })
        })
        .collect();
    Ok(json!({
        "partitions": partitions,
        "mode": "payload",
        "version": metadata.version,
        "block_size": metadata.block_size
    }))
}

#[tauri::command]
pub fn payload_parse_metadata(source: Option<Value>) -> Result<Value, String> {
    let source = required_text(&source, "source")?;
    let metadata = read_payload_metadata(&source)?;
    let total_size = metadata
        .partitions
        .iter()
        .fold(0u64, |sum, p| sum.saturating_add(p.size));
    let mut lines = vec![
        json!({"key": "Payload 版本", "value": metadata.version.to_string()}),
        json!({"key": "Manifest 大小", "value": format_size(metadata.manifest_size)}),
        json!({"key": "数据偏移", "value": format!("0x{:x}", metadata.data_offset)}),
        json!({"key": "块大小", "value": metadata.block_size.to_string()}),
        json!({"key": "分区数量", "value": metadata.partitions.len().to_string()}),
        json!({"key": "分区总大小", "value": format_size(total_size)}),
    ];
    if metadata.version >= 2 {
        lines.push(json!({"key": "元数据签名", "value": format_size(u64::from(metadata.metadata_signature_size))}));
    }
    if let Some(minor) = metadata.minor_version {
        lines.push(json!({"key": "次版本", "value": minor.to_string()}));
    }
    if let Some(timestamp) = metadata.max_timestamp {
        lines.push(json!({"key": "最大时间戳", "value": timestamp.to_string()}));
    }
    Ok(json!({"success": true, "lines": lines, "mode": "payload"}))
}

#[tauri::command]
pub fn payload_extract(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    // 压缩数据块需要 brotli，当前不伪造解包结果。
    Err("payload 解包尚未实现".to_string())
}

#[tauri::command]
pub fn payload_default_output_dir() -> Value {
    Value::String(data_dir("payload_extract").to_string_lossy().into_owned())
}

// ---------- Android boot.img ----------

fn boot_c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).trim().to_string()
}

fn decode_boot_os_version(value: u32) -> (String, String) {
    if value == 0 {
        return (String::new(), String::new());
    }
    let major = (value >> 25) & 0x7f;
    let minor = (value >> 18) & 0x7f;
    let patch = (value >> 11) & 0x7f;
    let year = 2000 + ((value >> 4) & 0x7f);
    let month = value & 0x0f;
    let android = if patch > 0 {
        format!("{}.{}.{}", major, minor, patch)
    } else {
        format!("{}.{}", major, minor)
    };
    let security_patch = if (1..=12).contains(&month) {
        format!("{}-{:02}", year, month)
    } else {
        String::new()
    };
    (android, security_patch)
}

#[tauri::command]
pub fn boot_info_parse(image_path: Option<Value>) -> Result<Value, String> {
    let started = Instant::now();
    let path = required_path(&image_path, "imagePath")?;
    let file_size = std::fs::metadata(&path)
        .map_err(|e| format!("无法读取镜像信息: {}", e))?
        .len();
    let header = read_prefix(&path, 4096)?;
    if header.len() < 44 || &header[..8] != b"ANDROID!" {
        return Err("不是有效的 Android boot.img（缺少 ANDROID! magic）".to_string());
    }

    let kernel_size;
    let ramdisk_size;
    let os_version_raw;
    let header_version;
    let name;
    let cmdline;
    let mut kernel_addr = 0u32;
    let mut ramdisk_addr = 0u32;
    let mut second_size = 0u32;
    let mut second_addr = 0u32;
    let mut tags_addr = 0u32;
    let mut page_size = 0u32;
    let mut dt_size = 0u32;
    let mut dtb_size = 0u32;
    let mut dtb_addr = 0u64;
    let mut recovery_dtbo_size = 0u32;
    let mut signature_size = 0u32;
    let header_size;

    let version_word =
        u32_le(&header, 40).ok_or_else(|| "boot header_version 字段不完整".to_string())?;
    if version_word == 3 || version_word == 4 {
        header_version = version_word;
        let expected_size = if header_version == 3 {
            1580usize
        } else {
            1584usize
        };
        if header.len() < expected_size {
            return Err("boot v3/v4 文件头不完整".to_string());
        }
        kernel_size = u32_le(&header, 8).ok_or_else(|| "kernel_size 字段不完整".to_string())?;
        ramdisk_size = u32_le(&header, 12).ok_or_else(|| "ramdisk_size 字段不完整".to_string())?;
        os_version_raw = u32_le(&header, 16).ok_or_else(|| "os_version 字段不完整".to_string())?;
        let declared = u32_le(&header, 20).ok_or_else(|| "header_size 字段不完整".to_string())?;
        header_size = if declared == 0 {
            expected_size as u32
        } else {
            declared
        };
        name = String::new();
        cmdline = boot_c_string(&header[44..1580]);
        if header_version == 4 {
            signature_size =
                u32_le(&header, 1580).ok_or_else(|| "signature_size 字段不完整".to_string())?;
        }
    } else {
        header_version = if version_word <= 2 { version_word } else { 0 };
        let expected_size = match header_version {
            0 => 1632usize,
            1 => 1648usize,
            2 => 1660usize,
            _ => unreachable!(),
        };
        if header.len() < expected_size {
            return Err("boot v0/v1/v2 文件头不完整".to_string());
        }
        kernel_size = u32_le(&header, 8).ok_or_else(|| "kernel_size 字段不完整".to_string())?;
        kernel_addr = u32_le(&header, 12).unwrap_or(0);
        ramdisk_size = u32_le(&header, 16).ok_or_else(|| "ramdisk_size 字段不完整".to_string())?;
        ramdisk_addr = u32_le(&header, 20).unwrap_or(0);
        second_size = u32_le(&header, 24).unwrap_or(0);
        second_addr = u32_le(&header, 28).unwrap_or(0);
        tags_addr = u32_le(&header, 32).unwrap_or(0);
        page_size = u32_le(&header, 36).unwrap_or(0);
        os_version_raw = u32_le(&header, 44).ok_or_else(|| "os_version 字段不完整".to_string())?;
        name = boot_c_string(&header[48..64]);
        cmdline = boot_c_string(&header[64..576]);
        if header_version == 0 {
            dt_size = version_word;
            header_size = expected_size as u32;
        } else {
            recovery_dtbo_size = u32_le(&header, 1632).unwrap_or(0);
            let declared = u32_le(&header, 1644).unwrap_or(0);
            header_size = if declared == 0 {
                expected_size as u32
            } else {
                declared
            };
        }
        if header_version == 2 {
            dtb_size = u32_le(&header, 1648).ok_or_else(|| "dtb_size 字段不完整".to_string())?;
            dtb_addr = u64_le(&header, 1652).ok_or_else(|| "dtb_addr 字段不完整".to_string())?;
        }
    }
    if u64::from(header_size) > file_size {
        return Err("boot 声明的文件头大小超过镜像长度".to_string());
    }

    let (android_version, security_patch) = decode_boot_os_version(os_version_raw);
    let os_version = if android_version.is_empty() && security_patch.is_empty() {
        String::new()
    } else if security_patch.is_empty() {
        android_version.clone()
    } else {
        format!("{} ({})", android_version, security_patch)
    };
    let image_kind = path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_ascii_lowercase())
        .filter(|name| name.starts_with("init_boot"))
        .map(|_| "init_boot")
        .unwrap_or("boot");

    Ok(json!({
        "valid": true,
        "image_kind": image_kind,
        "kernel_size": kernel_size,
        "kernel_addr": kernel_addr,
        "ramdisk_size": ramdisk_size,
        "ramdisk_addr": ramdisk_addr,
        "second_size": second_size,
        "second_addr": second_addr,
        "tags_addr": tags_addr,
        "page_size": page_size,
        "dt_size": dt_size,
        "dtb_size": dtb_size,
        "dtb_addr": dtb_addr,
        "recovery_dtbo_size": recovery_dtbo_size,
        "signature_size": signature_size,
        "os_version": os_version,
        "os_version_raw": os_version_raw,
        "android_version": android_version,
        "security_patch": security_patch,
        "header_version": header_version,
        "header_size": header_size,
        "name": name,
        "cmdline": cmdline,
        "size": file_size,
        "parse_ms": started.elapsed().as_millis() as u64,
        "errors": []
    }))
}

// ---------- super 镜像与散包 ----------

struct SuperInspection {
    format: &'static str,
    size: u64,
    raw_size: u64,
    image_count: u64,
    metadata_path: Option<PathBuf>,
}

fn is_ext4_prefix(data: &[u8]) -> bool {
    if data.len() < 1082 || u16_le(data, 1080) != Some(0xef53) {
        return false;
    }
    let inodes = u32_le(data, 1024).unwrap_or(0);
    let blocks = u32_le(data, 1028).unwrap_or(0);
    let log_block_size = u32_le(data, 1048).unwrap_or(u32::MAX);
    inodes > 0 && blocks > 0 && log_block_size <= 6
}

fn scan_super_directory(
    directory: &Path,
    depth: u8,
    metadata_path: &mut Option<PathBuf>,
    image_bytes: &mut u64,
    image_count: &mut u64,
) -> Result<(), String> {
    for entry in std::fs::read_dir(directory).map_err(|e| format!("读取散包目录失败: {}", e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if file_type.is_dir() {
            if depth > 0 {
                scan_super_directory(&path, depth - 1, metadata_path, image_bytes, image_count)?;
            }
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if metadata_path.is_none()
            && (name == "super_def.json"
                || (name.starts_with("super_meta.") && name.ends_with(".raw")))
        {
            *metadata_path = Some(path.clone());
        }
        if name.ends_with(".img") && name != "super.img" && name != "super_empty.img" {
            *image_bytes =
                image_bytes.saturating_add(entry.metadata().map_err(|e| e.to_string())?.len());
            *image_count += 1;
        }
    }
    Ok(())
}

fn inspect_super_package(path: &Path) -> Result<SuperInspection, String> {
    let metadata = std::fs::metadata(path).map_err(|e| format!("路径不存在或不可访问: {}", e))?;
    if metadata.is_dir() {
        let mut metadata_path = None;
        let mut image_bytes = 0u64;
        let mut image_count = 0u64;
        scan_super_directory(
            path,
            2,
            &mut metadata_path,
            &mut image_bytes,
            &mut image_count,
        )?;
        if metadata_path.is_none() {
            return Err(
                "未找到 super_meta.*.raw 或 super_def.json，请选择已解压的散包根目录。".to_string(),
            );
        }
        if image_count == 0 {
            return Err("散包中未找到可合并的 .img 子镜像".to_string());
        }
        return Ok(SuperInspection {
            format: "package",
            size: image_bytes,
            raw_size: image_bytes,
            image_count,
            metadata_path,
        });
    }
    if !metadata.is_file() {
        return Err("super 路径既不是文件也不是目录".to_string());
    }

    let size = metadata.len();
    let prefix = read_prefix(path, 1082)?;
    // sparse magic 0x3aff26ed 在文件中按小端存储，读取值为 0xed26ff3a。
    if prefix.len() >= 28 && u32_le(&prefix, 0) == Some(0xed26ff3a) {
        let major_version =
            u16_le(&prefix, 4).ok_or_else(|| "sparse 主版本字段不完整".to_string())?;
        let file_header_size = u16_le(&prefix, 8).unwrap_or(0);
        let chunk_header_size = u16_le(&prefix, 10).unwrap_or(0);
        let block_size = u32_le(&prefix, 12).unwrap_or(0);
        let total_blocks = u32_le(&prefix, 16).unwrap_or(0);
        let total_chunks = u32_le(&prefix, 20).unwrap_or(0);
        if major_version != 1
            || file_header_size < 28
            || chunk_header_size < 12
            || block_size == 0
            || total_chunks == 0
        {
            return Err("Android sparse 文件头结构无效".to_string());
        }
        let raw_size = u64::from(total_blocks)
            .checked_mul(u64::from(block_size))
            .ok_or_else(|| "sparse 展开大小溢出".to_string())?;
        return Ok(SuperInspection {
            format: "sparse",
            size,
            raw_size,
            image_count: 1,
            metadata_path: None,
        });
    }
    if is_ext4_prefix(&prefix) {
        return Ok(SuperInspection {
            format: "ext4",
            size,
            raw_size: size,
            image_count: 1,
            metadata_path: None,
        });
    }
    Err("无法识别 super 镜像：不是 Android sparse 或 raw ext4".to_string())
}

#[link(name = "kernel32")]
extern "system" {
    fn GetDiskFreeSpaceExW(
        directory: *const u16,
        free_available: *mut u64,
        total_bytes: *mut u64,
        total_free: *mut u64,
    ) -> i32;
}

fn disk_free_bytes(path: &Path) -> Result<u64, String> {
    let directory = if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    };
    let wide = wide_null(&directory.to_string_lossy());
    let mut available = 0u64;
    let mut total = 0u64;
    let mut free = 0u64;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut available, &mut total, &mut free) };
    if ok == 0 {
        Err(format!("查询磁盘空间失败: {}", last_error()))
    } else {
        Ok(available)
    }
}

#[tauri::command]
pub fn super_merge_validate_package(package_path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&package_path, "packagePath")?;
    let info = inspect_super_package(&path)?;
    Ok(json!({
        "valid": true,
        "path": path.to_string_lossy(),
        "format": info.format,
        "size": info.size,
        "size_bytes": info.size,
        "raw_size": info.raw_size,
        "image_count": info.image_count,
        "metadata_path": info.metadata_path.as_ref().map(|p| p.to_string_lossy().to_string())
    }))
}

#[tauri::command]
pub fn super_merge_preflight(package_path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&package_path, "packagePath")?;
    let info = inspect_super_package(&path)?;
    let free_bytes = disk_free_bytes(&path)?;
    let device_size_bytes = info.raw_size;
    let ready = device_size_bytes > 0 && free_bytes >= device_size_bytes;
    let message = if ready {
        format!(
            "可用 {}，合并约需 {}，可以合并。",
            format_size(free_bytes),
            format_size(device_size_bytes)
        )
    } else {
        format!(
            "可用 {}，合并约需 {}，磁盘空间不足。",
            format_size(free_bytes),
            format_size(device_size_bytes)
        )
    };
    Ok(json!({
        "valid": true,
        "ready": ready,
        "free_bytes": free_bytes,
        "device_size_bytes": device_size_bytes,
        "format": info.format,
        "message": message
    }))
}

#[derive(Default)]
struct MergeResidue {
    files: Vec<PathBuf>,
    directories: Vec<PathBuf>,
    total_bytes: u64,
}

fn find_merge_residue(package_path: &Path) -> Result<MergeResidue, String> {
    if !package_path.exists() {
        return Err(format!("路径不存在: {}", package_path.display()));
    }
    let directory = if package_path.is_dir() {
        package_path
    } else {
        package_path
            .parent()
            .ok_or_else(|| "无法确定散包目录".to_string())?
    };
    let mut residue = MergeResidue::default();
    for entry in std::fs::read_dir(directory).map_err(|e| format!("读取散包目录失败: {}", e))?
    {
        let entry = entry.map_err(|e| e.to_string())?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        let file_type = entry.file_type().map_err(|e| e.to_string())?;
        if file_type.is_file()
            && matches!(
                name.as_str(),
                "super.img.tmp" | "super.img.partial" | "super.img.part" | "super_merge.tmp"
            )
        {
            residue.total_bytes = residue
                .total_bytes
                .saturating_add(entry.metadata().map_err(|e| e.to_string())?.len());
            residue.files.push(entry.path());
        } else if file_type.is_dir()
            && matches!(
                name.as_str(),
                ".aft-super-merge" | ".super_merge_tmp" | "super_merge_tmp"
            )
        {
            residue.directories.push(entry.path());
        }
    }
    residue.files.sort();
    residue.directories.sort();
    Ok(residue)
}

#[tauri::command]
pub fn super_merge_residue_preview(package_path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&package_path, "packagePath")?;
    let residue = find_merge_residue(&path)?;
    let files: Vec<String> = residue
        .files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let directories: Vec<String> = residue
        .directories
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    Ok(json!({
        "file_count": files.len(),
        "dir_count": directories.len(),
        "total_bytes": residue.total_bytes,
        "files": files,
        "dirs": directories
    }))
}

#[tauri::command]
pub fn super_merge_cleanup(package_path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&package_path, "packagePath")?;
    let residue = find_merge_residue(&path)?;
    let file_count = residue.files.len();
    let dir_count = residue.directories.len();
    for file in &residue.files {
        std::fs::remove_file(file).map_err(|e| format!("删除 {} 失败: {}", file.display(), e))?;
    }
    for directory in &residue.directories {
        std::fs::remove_dir_all(directory)
            .map_err(|e| format!("删除 {} 失败: {}", directory.display(), e))?;
    }
    let message = if file_count == 0 && dir_count == 0 {
        "未发现可清理的合并残留".to_string()
    } else {
        format!("已清理 {} 个文件、{} 个目录", file_count, dir_count)
    };
    Ok(json!({"ok": true, "file_count": file_count, "dir_count": dir_count, "message": message}))
}

#[tauri::command]
pub fn super_merge_cancel() -> Value {
    SUPER_MERGE_CANCEL.store(true, Ordering::SeqCst);
    json!({"ok": true})
}

#[tauri::command]
pub fn super_merge_run(package_path: Option<Value>) -> Result<Value, String> {
    let _ = package_path;
    Err("super 合并尚未实现".to_string())
}

// ---------- OFP ----------

fn ofp_project_name(header: &[u8]) -> Option<String> {
    let length = u32_le(header, 16)? as usize;
    if length == 0 || length > 128 {
        return None;
    }
    let bytes = header.get(20..20 + length)?;
    let text = std::str::from_utf8(bytes)
        .ok()?
        .trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if text.is_empty() || text.chars().any(char::is_control) {
        None
    } else {
        Some(text.to_string())
    }
}

#[tauri::command]
pub fn ofp_default_output_dir() -> Value {
    Value::String(data_dir("all-OFP解包").to_string_lossy().into_owned())
}

#[tauri::command]
pub fn ofp_probe(chipset: Option<Value>, input_path: Option<Value>) -> Result<Value, String> {
    let selected = chipset
        .as_ref()
        .and_then(Value::as_str)
        .unwrap_or("auto")
        .to_ascii_lowercase();
    if !matches!(selected.as_str(), "auto" | "qc" | "mtk") {
        return Err(format!("不支持的 OFP 平台: {}", selected));
    }
    let path = required_path(&input_path, "inputPath")?;
    let size = std::fs::metadata(&path)
        .map_err(|e| format!("无法读取 OFP 文件: {}", e))?
        .len();
    if size == 0 {
        return Err("OFP 文件为空".to_string());
    }
    let header = read_prefix(&path, 4096)?;
    let project = ofp_project_name(&header);
    let detected = if header.starts_with(b"PK\x03\x04") {
        "zip"
    } else if selected == "qc" || selected == "mtk" {
        selected.as_str()
    } else if let Some(project_name) = &project {
        let upper = project_name.to_ascii_uppercase();
        if upper.contains("DIMENSITY") || upper.contains("MT") {
            "mtk"
        } else if ["SNAPDRAGON", "MSM", "SDM", "QCM", "QCS", "SM"]
            .iter()
            .any(|marker| upper.contains(marker))
        {
            "qc"
        } else {
            "unknown"
        }
    } else {
        "unknown"
    };
    Ok(json!({
        "valid": detected != "unknown",
        "format": detected,
        "prjname": project,
        "size": size,
        "chipset": selected
    }))
}

#[tauri::command]
pub fn ofp_decrypt_run(
    chipset: Option<Value>,
    input_path: Option<Value>,
    merge_super: Option<Value>,
    output_dir: Option<Value>,
) -> Result<Value, String> {
    let _ = (chipset, input_path, merge_super, output_dir);
    Err("OFP 解密尚未实现".to_string())
}

#[tauri::command]
pub fn ofp_decrypt_cancel() -> Value {
    OFP_DECRYPT_CANCEL.store(true, Ordering::SeqCst);
    json!({"ok": true})
}

// ---------- NV / persist / modem 镜像校验 ----------

fn image_result(path: &Path, valid: bool, size: u64, format: &str, detail: String) -> Value {
    json!({
        "valid": valid,
        "path": path.to_string_lossy(),
        "size": size,
        "format": format,
        "detail": detail
    })
}

fn fat_format(data: &[u8]) -> Option<&'static str> {
    if data.len() < 512 || data[510] != 0x55 || data[511] != 0xaa {
        return None;
    }
    let bytes_per_sector = u16_le(data, 11)?;
    let sectors_per_cluster = *data.get(13)?;
    if !(512..=4096).contains(&bytes_per_sector)
        || !bytes_per_sector.is_power_of_two()
        || sectors_per_cluster == 0
        || !sectors_per_cluster.is_power_of_two()
    {
        return None;
    }
    if data.get(54..62) == Some(b"FAT16   ") {
        Some("fat16")
    } else if data.get(54..62) == Some(b"FAT12   ") {
        Some("fat12")
    } else if data.get(82..90) == Some(b"FAT32   ") {
        Some("fat32")
    } else {
        None
    }
}

fn is_mbn(data: &[u8], file_size: u64) -> bool {
    if data.len() < 40 {
        return false;
    }
    let image_id = u32_le(data, 0).unwrap_or(0);
    let header_version = u32_le(data, 4).unwrap_or(0);
    let image_size = u32_le(data, 16).unwrap_or(0);
    let code_size = u32_le(data, 20).unwrap_or(0);
    image_id != 0
        && matches!(header_version, 3 | 6)
        && code_size > 0
        && image_size >= code_size
        && u64::from(code_size) <= file_size
}

fn validate_modem_like(path: &Path, label: &str) -> Value {
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return image_result(path, false, 0, "unknown", "所选路径不是文件".to_string()),
        Err(e) => return image_result(path, false, 0, "unknown", format!("镜像不可访问: {}", e)),
    };
    let size = metadata.len();
    if size == 0 {
        return image_result(path, false, size, "unknown", "镜像文件为空".to_string());
    }
    let data = match read_prefix(path, 1082) {
        Ok(data) => data,
        Err(e) => return image_result(path, false, size, "unknown", e),
    };

    if data.starts_with(b"\x7fELF") && data.len() >= 16 {
        let class = data[4];
        let endian = data[5];
        let minimum = if class == 1 {
            52
        } else if class == 2 {
            64
        } else {
            usize::MAX
        };
        if matches!(endian, 1 | 2) && size >= minimum as u64 {
            let format = if class == 1 { "elf32" } else { "elf64" };
            return image_result(
                path,
                true,
                size,
                format,
                format!(
                    "有效 {} 镜像（{}，{}）",
                    label,
                    format.to_ascii_uppercase(),
                    format_size(size)
                ),
            );
        }
    }
    if is_mbn(&data, size) {
        return image_result(
            path,
            true,
            size,
            "mbn",
            format!("有效 {} 镜像（MBN，{}）", label, format_size(size)),
        );
    }
    if let Some(format) = fat_format(&data) {
        return image_result(
            path,
            true,
            size,
            format,
            format!(
                "有效 {} 镜像（{}，{}）",
                label,
                format.to_ascii_uppercase(),
                format_size(size)
            ),
        );
    }
    image_result(
        path,
        false,
        size,
        "unknown",
        format!("{} 镜像结构无效：未识别 ELF、MBN 或 FAT 文件系统", label),
    )
}

#[tauri::command]
pub fn nv_validate_image(path: Option<Value>) -> Value {
    let raw = path
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if raw.is_empty() {
        return image_result(
            Path::new(""),
            false,
            0,
            "unknown",
            "未选择 NON-HLOS 镜像".to_string(),
        );
    }
    validate_modem_like(Path::new(raw), "NON-HLOS")
}

#[tauri::command]
pub fn persist_validate_image(path: Option<Value>) -> Value {
    let raw = path
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let path = Path::new(raw);
    if raw.is_empty() {
        return image_result(path, false, 0, "unknown", "未选择 persist 镜像".to_string());
    }
    let metadata = match std::fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => metadata,
        Ok(_) => return image_result(path, false, 0, "unknown", "所选路径不是文件".to_string()),
        Err(e) => return image_result(path, false, 0, "unknown", format!("镜像不可访问: {}", e)),
    };
    let size = metadata.len();
    if size < 1082 {
        return image_result(
            path,
            false,
            size,
            "unknown",
            "persist 镜像过小，缺少 ext4 superblock".to_string(),
        );
    }
    match read_prefix(path, 1082) {
        Ok(data) if is_ext4_prefix(&data) => image_result(
            path,
            true,
            size,
            "ext4",
            format!("有效 persist 镜像（ext4，{}）", format_size(size)),
        ),
        Ok(_) => image_result(
            path,
            false,
            size,
            "unknown",
            "persist 镜像不是有效的 raw ext4（superblock magic/结构不匹配）".to_string(),
        ),
        Err(e) => image_result(path, false, size, "unknown", e),
    }
}

#[tauri::command]
pub fn modem_validate_image(path: Option<Value>) -> Value {
    let raw = path
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if raw.is_empty() {
        return image_result(
            Path::new(""),
            false,
            0,
            "unknown",
            "未选择 modem 镜像".to_string(),
        );
    }
    validate_modem_like(Path::new(raw), "modem")
}
