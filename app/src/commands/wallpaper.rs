//! 本地壁纸、背景偏好与登录页壁纸缓存。

use super::util::*;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn opt_string(value: &Option<Value>) -> Option<String> {
    value
        .as_ref()
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn required_string(value: &Option<Value>, name: &str) -> Result<String, String> {
    let text = opt_string(value).ok_or_else(|| format!("缺少 {}", name))?;
    if text.trim().is_empty() {
        Err(format!("{} 为空", name))
    } else {
        Ok(text)
    }
}

fn static_wallpaper_dir() -> PathBuf {
    data_dir("wallpaper").join("static")
}

fn dynamic_wallpaper_dir() -> PathBuf {
    data_dir("wallpaper").join("dynamic")
}

fn matches_kind(path: &Path, kind: &str) -> bool {
    let ext = path
        .extension()
        .and_then(|v| v.to_str())
        .map(|v| v.to_lowercase())
        .unwrap_or_default();
    match kind {
        "image" => matches!(
            ext.as_str(),
            "jpg" | "jpeg" | "png" | "bmp" | "webp" | "gif"
        ),
        "video" => ext == "mp4",
        _ => false,
    }
}

fn wallpaper_entry(path: &Path, kind: &str) -> Value {
    let name = path
        .file_name()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_default();
    let display = path
        .file_stem()
        .map(|v| v.to_string_lossy().to_string())
        .unwrap_or_else(|| name.clone());
    json!({
        "name": name,
        "display": display,
        "local_path": path.to_string_lossy().to_string(),
        "kind": kind
    })
}

#[tauri::command]
pub fn wallpaper_list_local() -> Result<Value, String> {
    let mut items = Vec::new();
    for (dir, kind) in [
        (static_wallpaper_dir(), "image"),
        (dynamic_wallpaper_dir(), "video"),
    ] {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(format!("读取壁纸目录失败 {}: {}", dir.display(), e)),
        };
        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                continue;
            }
            let path = entry.path();
            if matches_kind(&path, kind) {
                items.push(wallpaper_entry(&path, kind));
            }
        }
    }
    items.sort_by_key(|item| {
        item.get("display")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_lowercase()
    });
    Ok(Value::Array(items))
}

#[tauri::command]
pub fn wallpaper_list_cloud_static() -> Value {
    // 尚未确认云端清单协议，不构造虚假条目。
    json!([])
}

#[tauri::command]
pub fn wallpaper_list_dynamic() -> Value {
    json!([])
}

#[tauri::command]
pub fn wallpaper_download_static(file_name: Option<Value>) -> Result<Value, String> {
    let _ = required_string(&file_name, "fileName")?;
    Err("未配置云端静态壁纸下载源".to_string())
}

#[tauri::command]
pub fn wallpaper_download_dynamic(file_name: Option<Value>) -> Result<Value, String> {
    let _ = required_string(&file_name, "fileName")?;
    Err("未配置云端动态壁纸下载源".to_string())
}

#[tauri::command]
pub fn wallpaper_exists(path: Option<Value>) -> Value {
    Value::Bool(
        opt_string(&path)
            .map(|p| Path::new(&p).is_file())
            .unwrap_or(false),
    )
}

#[tauri::command]
pub fn wallpaper_default_path() -> Value {
    Value::String(static_wallpaper_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn wallpaper_dirs() -> Value {
    json!({
        "static_dir": static_wallpaper_dir().to_string_lossy().to_string(),
        "dynamic_dir": dynamic_wallpaper_dir().to_string_lossy().to_string()
    })
}

fn same_path(left: &Path, right: &Path) -> bool {
    if let (Ok(left), Ok(right)) = (left.canonicalize(), right.canonicalize()) {
        return left == right;
    }
    let normalize = |path: &Path| {
        path.to_string_lossy()
            .replace('/', "\\")
            .trim_end_matches('\\')
            .to_lowercase()
    };
    normalize(left) == normalize(right)
}

fn import_wallpaper(source_path: Option<Value>, kind: &str) -> Result<Value, String> {
    let source = PathBuf::from(required_string(&source_path, "sourcePath")?);
    if !source.is_file() {
        return Err(format!("源文件不存在: {}", source.display()));
    }
    if !matches_kind(&source, kind) {
        return Err(if kind == "video" {
            "动态壁纸仅支持 mp4 文件".to_string()
        } else {
            "静态壁纸仅支持 jpg、jpeg、png、bmp、webp、gif 文件".to_string()
        });
    }
    let file_name = source
        .file_name()
        .ok_or_else(|| "源文件名无效".to_string())?;
    let dir = if kind == "video" {
        dynamic_wallpaper_dir()
    } else {
        static_wallpaper_dir()
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建壁纸目录失败: {}", e))?;
    let dest = dir.join(file_name);
    if !same_path(&source, &dest) {
        std::fs::copy(&source, &dest).map_err(|e| format!("导入壁纸失败: {}", e))?;
    }
    Ok(wallpaper_entry(&dest, kind))
}

#[tauri::command]
pub fn wallpaper_import_static(source_path: Option<Value>) -> Result<Value, String> {
    import_wallpaper(source_path, "image")
}

#[tauri::command]
pub fn wallpaper_import_dynamic(source_path: Option<Value>) -> Result<Value, String> {
    import_wallpaper(source_path, "video")
}

fn write_json(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let data = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(path, data).map_err(|e| format!("写入 {} 失败: {}", path.display(), e))
}

fn background_preference_path() -> PathBuf {
    pack_dir().join("background_pref.json")
}

fn save_background_preference(preference: &str, path: Option<&str>) -> Result<(), String> {
    write_json(
        &background_preference_path(),
        &json!({"preference": preference, "path": path}),
    )
}

fn clear_wallpapers(dir: PathBuf, kind: &str, active_path: Option<&str>) -> Result<Value, String> {
    let mut removed = 0u64;
    let mut active_removed = false;
    match std::fs::read_dir(&dir) {
        Ok(entries) => {
            for entry in entries {
                let entry = entry.map_err(|e| e.to_string())?;
                if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
                    continue;
                }
                let path = entry.path();
                if !matches_kind(&path, kind) {
                    continue;
                }
                let is_active = active_path
                    .map(|active| same_path(Path::new(active), &path))
                    .unwrap_or(false);
                std::fs::remove_file(&path)
                    .map_err(|e| format!("删除壁纸失败 {}: {}", path.display(), e))?;
                removed += 1;
                active_removed |= is_active;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("读取壁纸目录失败 {}: {}", dir.display(), e)),
    }
    if active_removed {
        save_background_preference("default", None)?;
    }
    Ok(json!({"removed": removed, "active_removed": active_removed}))
}

#[tauri::command]
pub fn wallpaper_clear_static(active_path: Option<Value>) -> Result<Value, String> {
    let active_path = opt_string(&active_path);
    clear_wallpapers(static_wallpaper_dir(), "image", active_path.as_deref())
}

#[tauri::command]
pub fn wallpaper_clear_dynamic(active_path: Option<Value>) -> Result<Value, String> {
    let active_path = opt_string(&active_path);
    clear_wallpapers(dynamic_wallpaper_dir(), "video", active_path.as_deref())
}

#[tauri::command]
pub fn background_preference_load() -> Value {
    let data = match std::fs::read(background_preference_path()) {
        Ok(data) => data,
        Err(_) => return Value::Null,
    };
    serde_json::from_slice(&data).unwrap_or(Value::Null)
}

#[tauri::command]
pub fn background_preference_save(
    path: Option<Value>,
    preference: Option<Value>,
) -> Result<Value, String> {
    let preference = required_string(&preference, "preference")?;
    let path = opt_string(&path);
    save_background_preference(&preference, path.as_deref())?;
    Ok(json!({"ok": true}))
}

fn login_wallpaper_dir() -> PathBuf {
    data_dir("login_wallpapers")
}

fn login_wallpaper_cache_path() -> PathBuf {
    login_wallpaper_dir().join("current.json")
}

fn read_login_wallpaper_cache() -> Option<Value> {
    let data = std::fs::read(login_wallpaper_cache_path()).ok()?;
    let cache: Value = serde_json::from_slice(&data).ok()?;
    let path = cache.get("path").and_then(Value::as_str)?;
    let kind = cache.get("kind").and_then(Value::as_str)?;
    if matches!(kind, "image" | "video") && Path::new(path).is_file() {
        Some(cache)
    } else {
        None
    }
}

fn remove_file_if_exists(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("删除 {} 失败: {}", path.display(), e)),
    }
}

fn clear_login_wallpaper_cache() -> Result<(), String> {
    remove_file_if_exists(&login_wallpaper_cache_path())?;
    for ext in ["jpg", "jpeg", "png", "bmp", "webp", "gif", "mp4"] {
        remove_file_if_exists(&login_wallpaper_dir().join(format!("current.{}", ext)))?;
    }
    Ok(())
}

#[tauri::command]
pub fn load_login_wallpaper_cache() -> Value {
    read_login_wallpaper_cache().unwrap_or(Value::Null)
}

fn login_wallpaper_extension(url: &str, kind: &str) -> String {
    let clean = url.split(|c| c == '?' || c == '#').next().unwrap_or(url);
    let raw_ext = clean
        .rsplit('/')
        .next()
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_lowercase())
        .unwrap_or_default();
    let ext = match (kind, raw_ext.as_str()) {
        ("video", "mp4") => "mp4",
        ("image", "jpg") => "jpg",
        ("image", "jpeg") => "jpeg",
        ("image", "png") => "png",
        ("image", "bmp") => "bmp",
        ("image", "webp") => "webp",
        ("image", "gif") => "gif",
        ("video", _) => "mp4",
        _ => "jpg",
    };
    ext.to_string()
}

#[tauri::command]
pub fn sync_login_wallpaper(
    download_ticket: Option<Value>,
    exists: Option<Value>,
    kind: Option<Value>,
    updated_at: Option<Value>,
    url: Option<Value>,
    username: Option<Value>,
) -> Result<Value, String> {
    if !exists.as_ref().and_then(Value::as_bool).unwrap_or(false) {
        clear_login_wallpaper_cache()?;
        return Ok(Value::Null);
    }

    let username = required_string(&username, "username")?;
    let url = required_string(&url, "url")?;
    let kind = opt_string(&kind).unwrap_or_default().to_lowercase();
    if !matches!(kind.as_str(), "image" | "video") {
        return Err("登录壁纸 kind 必须为 image 或 video".to_string());
    }
    let updated_at = updated_at.as_ref().and_then(Value::as_u64).unwrap_or(0);
    let download_ticket = opt_string(&download_ticket).unwrap_or_default();
    let media_path = login_wallpaper_dir().join(format!(
        "current.{}",
        login_wallpaper_extension(&url, &kind)
    ));

    if let Some(cache) = read_login_wallpaper_cache() {
        let unchanged = cache.get("username").and_then(Value::as_str) == Some(username.as_str())
            && cache.get("sourceUrl").and_then(Value::as_str) == Some(url.as_str())
            && cache.get("kind").and_then(Value::as_str) == Some(kind.as_str())
            && cache.get("updatedAt").and_then(Value::as_u64) == Some(updated_at)
            && cache
                .get("downloadTicket")
                .and_then(Value::as_str)
                .unwrap_or("")
                == download_ticket.as_str();
        if unchanged {
            return Ok(cache);
        }
    }

    let bytes = download_file(&url, &media_path).map_err(|e| format!("下载登录壁纸失败: {}", e))?;
    if bytes == 0 {
        let _ = std::fs::remove_file(&media_path);
        return Err("下载的登录壁纸为空".to_string());
    }
    let cache = json!({
        "username": username,
        "path": media_path.to_string_lossy().to_string(),
        "kind": kind,
        "sourceUrl": url,
        "updatedAt": updated_at,
        "downloadTicket": download_ticket
    });
    write_json(&login_wallpaper_cache_path(), &cache)?;
    for ext in ["jpg", "jpeg", "png", "bmp", "webp", "gif", "mp4"] {
        let old_path = login_wallpaper_dir().join(format!("current.{}", ext));
        if old_path != media_path {
            let _ = std::fs::remove_file(old_path);
        }
    }
    Ok(cache)
}

#[tauri::command]
pub fn refresh_login_wallpaper(username: Option<Value>) -> Value {
    let _ = username;
    // 缺少服务器查询与会话协议时不伪造刷新结果。
    Value::Null
}

#[tauri::command]
pub fn refresh_login_wallpaper_with_creds(
    login_key: Option<Value>,
    password: Option<Value>,
    username: Option<Value>,
) -> Value {
    let _ = (login_key, password, username);
    Value::Null
}
