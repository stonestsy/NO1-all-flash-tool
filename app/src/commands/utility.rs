//! 实用工具命令：本地镜像识别、GPT 预览、LUN 枚举与 Fastboot 回锁。

use super::util::*;
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

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

fn header_hex(data: &[u8]) -> String {
    data.iter()
        .take(16)
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- 尚未接入的专有功能 ----------

#[tauri::command]
pub fn utility_xiaomi_acc_repair(
    modem_path: Option<Value>,
    persist_path: Option<Value>,
) -> Result<Value, String> {
    let _ = (modem_path, persist_path);
    Err("小米 ACC 修复算法尚未实现".to_string())
}

#[tauri::command]
pub fn utility_xiaomi_grant_applist() -> Result<Value, String> {
    Err("小米授权服务尚未接入".to_string())
}

#[tauri::command]
pub fn utility_xiaomi_usb_security() -> Result<Value, String> {
    Err("小米 USB 安全服务尚未接入".to_string())
}

#[tauri::command]
pub fn utility_oneplus_unlock_qcom() -> Result<Value, String> {
    Err("一加高通解锁尚未实现".to_string())
}

#[tauri::command]
pub fn utility_oneplus_unlock_dimensity() -> Result<Value, String> {
    Err("一加天玑解锁尚未实现".to_string())
}

#[tauri::command]
pub fn utility_oneplus_repair_baseband() -> Result<Value, String> {
    Err("一加基带修复算法尚未实现".to_string())
}

#[tauri::command]
pub fn utility_xiaomi_gen3_nv() -> Result<Value, String> {
    Err("小米三代 NV 修复算法尚未实现".to_string())
}

#[tauri::command]
pub fn utility_lk_sign_unlock() -> Result<Value, String> {
    Err("LK 签名解锁尚未实现".to_string())
}

// ---------- LK 本地运行时 ----------

fn lk_runtime_info(unlock: bool) -> Value {
    let root = data_dir("lk");
    let file_name = if unlock {
        "lk_unlock_runtime.bin"
    } else {
        "lk_runtime.bin"
    };
    let path = root.join(file_name);
    let ready = path.is_file();
    json!({
        "ready": ready,
        "installed": ready,
        "kind": if unlock { "unlock" } else { "relock" },
        "path": path.to_string_lossy(),
        "root": root.to_string_lossy()
    })
}

#[tauri::command]
pub fn lk_runtime_status() -> Value {
    lk_runtime_info(false)
}

#[tauri::command]
pub fn lk_runtime_ensure() -> Value {
    // 本地运行时由配置包流程部署，此处只报告真实存在性。
    lk_runtime_info(false)
}

#[tauri::command]
pub fn lk_unlock_runtime_ensure() -> Value {
    lk_runtime_info(true)
}

#[tauri::command]
pub fn lk_quota_status(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("LK 配额服务尚未接入".to_string())
}

#[tauri::command]
pub fn lk_patch_run(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("LK 伪回锁修补算法尚未实现".to_string())
}

#[tauri::command]
pub fn lk_unlock_patch_run(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("LK 普通解锁修补算法尚未实现".to_string())
}

// ---------- Fastboot 回锁 ----------

#[tauri::command]
pub fn utility_android_relock_bl() -> Result<Value, String> {
    let exe = fastboot_path()?;
    let devices = run_tool_str(&exe, &["devices"])?;
    let serial = devices
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == "fastboot" {
                Some(parts[0].to_string())
            } else {
                None
            }
        })
        .next()
        .ok_or_else(|| "未检测到 Fastboot 设备，已取消回锁".to_string())?;
    let args = vec![
        "-s".to_string(),
        serial.clone(),
        "flashing".to_string(),
        "lock".to_string(),
    ];
    let output = run_tool(&exe, &args)?;
    let lower = output.to_ascii_lowercase();
    if lower.contains("failed") || lower.contains("error:") || lower.contains("not allowed") {
        return Err(format!("Fastboot 回锁失败: {}", output.trim()));
    }
    Ok(json!({"ok": true, "serial": serial, "output": output}))
}

#[tauri::command]
pub fn utility_nv_repair(input_path: Option<Value>) -> Result<Value, String> {
    let _ = input_path;
    Err("NV 修复算法尚未实现".to_string())
}

#[tauri::command]
pub fn utility_persist_acc_repair(input_path: Option<Value>) -> Result<Value, String> {
    let _ = input_path;
    Err("Persist ACC 修复算法尚未实现".to_string())
}

// ---------- 镜像头识别 ----------

fn inspect_image(path: &Path) -> Result<Value, String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("无法读取镜像信息 {}: {}", path.display(), e))?
        .len();
    if !path.is_file() {
        return Err(format!("不是镜像文件: {}", path.display()));
    }
    let header = read_prefix(path, 1082)?;
    let format = if u32_le(&header, 0) == Some(0x3aff26ed) {
        "android_sparse"
    } else if header.get(..8) == Some(b"ANDROID!") {
        "android_boot"
    } else if header.len() >= 1082 && u16_le(&header, 1024 + 56) == Some(0x53af) {
        "ext4"
    } else {
        "unknown"
    };
    let valid = format != "unknown";
    let original_size = if format == "android_sparse" {
        match (u32_le(&header, 12), u32_le(&header, 16)) {
            (Some(block_size), Some(total_blocks)) => {
                Some(u64::from(block_size) * u64::from(total_blocks))
            }
            _ => None,
        }
    } else {
        None
    };
    let error = if valid {
        None
    } else {
        Some("未识别的镜像格式")
    };
    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy())
        .unwrap_or_default();
    Ok(json!({
        "valid": valid,
        "format": format,
        "size": size,
        "fileName": file_name,
        "fileSize": size,
        "imageSize": if valid { Some(size) } else { None },
        "originalImageSize": original_size,
        "signed": false,
        "signatureChecked": false,
        "algorithm": Value::Null,
        "rollbackIndex": Value::Null,
        "flags": Value::Null,
        "hashPartitions": [],
        "props": [],
        "headerHex": header_hex(&header),
        "error": error
    }))
}

#[tauri::command]
pub fn utility_vab_inspect_image(path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&path, "path")?;
    inspect_image(&path)
}

#[tauri::command]
pub fn utility_vab_analyze(path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&path, "path")?;
    let mut info = inspect_image(&path)?;
    if let Value::Object(ref mut map) = info {
        // 仅凭通用镜像头不能可靠区分旧版与 AOSP 签名方案。
        map.insert("scheme".to_string(), Value::String("unknown".to_string()));
    }
    Ok(info)
}

#[tauri::command]
pub fn utility_vab_sign(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("VAB 签名流程尚未实现".to_string())
}

#[tauri::command]
pub fn utility_mtk_gpt_generate(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("MTK GPT 生成算法尚未实现".to_string())
}

// ---------- GPT 预览 ----------

fn gpt_preview(path: &Path) -> Result<Value, String> {
    const SECTOR_SIZE: u64 = 512;
    let mut file = File::open(path).map_err(|e| format!("无法打开 GPT 镜像: {}", e))?;
    let file_size = file.metadata().map_err(|e| e.to_string())?.len();
    let mut header = [0u8; 512];
    file.seek(SeekFrom::Start(SECTOR_SIZE))
        .map_err(|e| e.to_string())?;
    file.read_exact(&mut header)
        .map_err(|_| "GPT 头不完整（需要偏移 512 的 EFI PART 头）".to_string())?;
    if &header[..8] != b"EFI PART" {
        return Err("不是有效的 GPT 镜像（缺少 EFI PART 签名）".to_string());
    }
    let entry_lba = u64_le(&header, 72).ok_or_else(|| "GPT 分区表 LBA 字段不完整".to_string())?;
    let entry_count =
        u32_le(&header, 80).ok_or_else(|| "GPT 分区数量字段不完整".to_string())? as usize;
    let entry_size =
        u32_le(&header, 84).ok_or_else(|| "GPT 分区项大小字段不完整".to_string())? as usize;
    if entry_count == 0 || entry_count > 16384 {
        return Err(format!("GPT 分区项数量异常: {}", entry_count));
    }
    if entry_size < 128 || entry_size > 4096 {
        return Err(format!("不支持的 GPT 分区项大小: {}", entry_size));
    }
    let table_offset = entry_lba
        .checked_mul(SECTOR_SIZE)
        .ok_or_else(|| "GPT 分区表偏移溢出".to_string())?;
    let table_size = entry_count
        .checked_mul(entry_size)
        .ok_or_else(|| "GPT 分区表大小溢出".to_string())?;
    let table_end = table_offset
        .checked_add(table_size as u64)
        .ok_or_else(|| "GPT 分区表结束位置溢出".to_string())?;
    if table_end > file_size {
        return Err("GPT 分区表超出文件长度".to_string());
    }
    file.seek(SeekFrom::Start(table_offset))
        .map_err(|e| e.to_string())?;
    let mut table = vec![0u8; table_size];
    file.read_exact(&mut table)
        .map_err(|e| format!("读取 GPT 分区表失败: {}", e))?;

    let mut partitions = Vec::new();
    for entry in table.chunks_exact(entry_size) {
        if entry[..16].iter().all(|b| *b == 0) {
            continue;
        }
        let start = u64_le(entry, 32).unwrap_or(0);
        let end = u64_le(entry, 40).unwrap_or(0);
        if end < start {
            continue;
        }
        let name_units: Vec<u16> = entry[56..128]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .take_while(|unit| *unit != 0)
            .collect();
        let name = String::from_utf16_lossy(&name_units);
        let sectors = end.saturating_sub(start).saturating_add(1);
        partitions.push(json!({
            "name": name,
            "start_sector": start,
            "end_sector": end,
            "first_lba": start,
            "last_lba": end,
            "num_sectors": sectors,
            "size_bytes": sectors.saturating_mul(SECTOR_SIZE)
        }));
    }
    Ok(json!({
        "valid": true,
        "format": "gpt",
        "size": file_size,
        "sector_size": SECTOR_SIZE,
        "partition_entry_lba": entry_lba,
        "entry_size": entry_size,
        "partitions": partitions
    }))
}

#[tauri::command]
pub fn utility_xiaomi_perfect_nv_gpt_preview(path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&path, "path")?;
    gpt_preview(&path)
}

#[tauri::command]
pub fn utility_xiaomi_perfect_nv_gpt(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("小米完美 NV GPT 生成算法尚未实现".to_string())
}

// ---------- LUN 目录枚举 ----------

fn lun_file_index(stem: &str, extension: &str) -> Option<u32> {
    let lower = stem.to_ascii_lowercase();
    if matches!(extension, "bin" | "img") {
        if let Some(index) = lower.strip_prefix("lun") {
            if !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()) {
                return index.parse().ok();
            }
        }
    }
    if extension == "img" && lower.starts_with("super") {
        let suffix = lower
            .trim_start_matches("super")
            .trim_start_matches(|c| c == '_' || c == '-');
        return if suffix.is_empty() {
            Some(0)
        } else {
            suffix.parse().ok().or(Some(0))
        };
    }
    None
}

#[tauri::command]
pub fn lun_extract_parse(source_dir: Option<Value>) -> Result<Value, String> {
    let source_dir = required_path(&source_dir, "sourceDir")?;
    if !source_dir.is_dir() {
        return Err(format!("源目录不存在: {}", source_dir.display()));
    }
    let mut files: Vec<(u32, String, PathBuf)> = Vec::new();
    for item in std::fs::read_dir(&source_dir).map_err(|e| format!("读取源目录失败: {}", e))?
    {
        let item = item.map_err(|e| e.to_string())?;
        let path = item.path();
        if !path.is_file() {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_default();
        let extension = path
            .extension()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if let Some(index) = lun_file_index(&stem, &extension) {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            files.push((index, name, path));
        }
    }
    files.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.to_ascii_lowercase().cmp(&b.1.to_ascii_lowercase()))
    });

    let mut luns = Vec::new();
    let mut images = Vec::new();
    for (index, name, path) in files {
        let size = std::fs::metadata(&path)
            .map_err(|e| format!("读取 {} 信息失败: {}", name, e))?
            .len();
        let prefix = read_prefix(&path, 520)?;
        let has_gpt = prefix.get(512..520) == Some(b"EFI PART");
        let sectors = size.saturating_add(511) / 512;
        images.push(json!({"path": path.to_string_lossy(), "fileName": name, "size": size}));
        luns.push(json!({
            "index": index,
            "fileName": name,
            "path": path.to_string_lossy(),
            "dumpBytes": size,
            "diskBytes": size,
            "sectorSize": 512,
            "hasGpt": has_gpt,
            "label": Value::Null,
            "partitions": [{
                "name": path.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default(),
                "kind": "whole",
                "sizeBytes": size,
                "firstLba": 0,
                "numSectors": sectors,
                "partial": false
            }]
        }));
    }
    Ok(json!({"luns": luns, "images": images}))
}

#[tauri::command]
pub fn lun_extract_start(payload: Option<Value>) -> Result<Value, String> {
    let _ = payload;
    Err("LUN 解包流程尚未实现".to_string())
}

#[tauri::command]
pub fn lun_extract_cancel() -> Value {
    json!({"ok": true})
}

// ---------- 联想 SN ----------

fn consider_sn_candidate(run: &[u8], best: &mut Option<String>) {
    let text = String::from_utf8_lossy(run);
    let text = text.trim();
    if (4..=64).contains(&text.len())
        && text.bytes().all(|b| b.is_ascii_alphanumeric())
        && best
            .as_ref()
            .map(|old| text.len() > old.len())
            .unwrap_or(true)
    {
        *best = Some(text.to_string());
    }
}

#[tauri::command]
pub fn lenovo_sn_read_image(path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&path, "path")?;
    let data = std::fs::read(&path).map_err(|e| format!("读取 SN 镜像失败: {}", e))?;
    let mut best = None;
    let mut run = Vec::new();
    for byte in data.iter().copied().chain(std::iter::once(0)) {
        if byte == b' ' || byte.is_ascii_graphic() {
            run.push(byte);
        } else {
            consider_sn_candidate(&run, &mut best);
            run.clear();
        }
    }
    let serial = best.unwrap_or_default();
    let valid = !serial.is_empty();
    let detail = if valid {
        "扫描到可打印 ASCII 序列号"
    } else {
        "未找到可打印 ASCII 序列号"
    };
    Ok(json!({
        "valid": valid,
        "sn": serial,
        "serial": serial,
        "size": data.len() as u64,
        "max_length": 64,
        "generation": Value::Null,
        "detail": detail
    }))
}

#[tauri::command]
pub fn lenovo_sn_write_image(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("联想 SN 镜像写入算法尚未实现".to_string())
}

// ---------- UFS DES / Oplus DA ----------

#[tauri::command]
pub fn ufs_des_inspect(path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&path, "path")?;
    let mut info = inspect_image(&path)?;
    let recognized = info.get("valid").and_then(Value::as_bool).unwrap_or(false);
    let format = info
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let detail = if recognized {
        format!("识别为 {}；DES 专有记录尚未解析", format)
    } else {
        "未识别的 DES 或镜像格式".to_string()
    };
    if let Value::Object(ref mut map) = info {
        map.insert("recognized".to_string(), Value::Bool(recognized));
        map.insert("will_restore".to_string(), Value::Bool(false));
        map.insert("needs_target_choice".to_string(), Value::Bool(false));
        map.insert("detail".to_string(), Value::String(detail));
    }
    Ok(info)
}

#[tauri::command]
pub fn ufs_des_patch(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("UFS DES 修补算法尚未实现".to_string())
}

#[tauri::command]
pub fn da_patch_run(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("Oplus DA 修补算法尚未实现".to_string())
}

// ---------- oplus_custom ----------

fn find_oplus_custom(path: &Path) -> Result<(PathBuf, PathBuf), String> {
    if path.is_file() {
        let root = path.parent().unwrap_or(path).to_path_buf();
        return Ok((path.to_path_buf(), root));
    }
    if !path.is_dir() {
        return Err(format!("路径不存在: {}", path.display()));
    }
    let mut candidates = Vec::new();
    for item in std::fs::read_dir(path).map_err(|e| format!("读取 IMAGES 目录失败: {}", e))? {
        let item = item.map_err(|e| e.to_string())?;
        let candidate = item.path();
        if !candidate.is_file() {
            continue;
        }
        let stem = candidate
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        let extension = candidate
            .extension()
            .map(|s| s.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if stem == "oplus_custom" && matches!(extension.as_str(), "img" | "bin") {
            candidates.push(candidate);
        }
    }
    candidates.sort();
    let image = candidates
        .into_iter()
        .next()
        .ok_or_else(|| "目录中未找到 oplus_custom.img/bin".to_string())?;
    Ok((image, path.to_path_buf()))
}

#[tauri::command]
pub fn oplus_custom_inspect(path: Option<Value>) -> Result<Value, String> {
    let path = required_path(&path, "path")?;
    let (image, root) = find_oplus_custom(&path)?;
    let mut info = inspect_image(&image)?;
    let partition_size = info.get("size").cloned().unwrap_or(Value::Null);
    if let Value::Object(ref mut map) = info {
        map.insert(
            "path".to_string(),
            Value::String(image.to_string_lossy().into_owned()),
        );
        map.insert(
            "ofp_dir".to_string(),
            Value::String(root.to_string_lossy().into_owned()),
        );
        map.insert("partition_size".to_string(), partition_size);
        map.insert("entries".to_string(), Value::Array(Vec::new()));
        map.insert(
            "detail".to_string(),
            Value::String("已读取镜像头；NV 条目尚未解析".to_string()),
        );
    }
    Ok(info)
}

#[tauri::command]
pub fn oplus_custom_generate(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("oplus_custom 生成算法尚未实现".to_string())
}

#[tauri::command]
pub fn utility_lenovo_unlock_bl(args: Option<Value>) -> Result<Value, String> {
    let _ = args;
    Err("联想 BL 解锁算法尚未实现".to_string())
}

#[tauri::command]
pub fn ocdt_generate(platform: Option<Value>, project_id: Option<Value>) -> Result<Value, String> {
    let _ = (platform, project_id);
    Err("OCDT 生成算法尚未实现".to_string())
}

#[tauri::command]
pub fn utility_cancel() -> Value {
    json!({"ok": true})
}
