//! ROM 固件索引命令。
//! rom_resolve_redirect 为真实实现（重定向链解析）。
//! 小米/小越索引在原应用中经私有服务器中转，协议未恢复，返回空列表而非编造数据。

use super::util::*;
use serde_json::{json, Value};

fn opt_str(v: &Option<Value>) -> Option<String> {
    v.as_ref().and_then(|x| x.as_str()).map(|s| s.to_string())
}

#[tauri::command]
pub fn rom_resolve_redirect(url: Option<Value>) -> Result<Value, String> {
    let url = opt_str(&url).ok_or_else(|| "缺少 url".to_string())?;
    let final_url = resolve_redirect(&url, 8)?;
    Ok(Value::String(final_url))
}

#[tauri::command]
pub fn rom_load_scheme(force_refresh: Option<Value>, scheme: Option<Value>) -> Value {
    json!({"schemes": []})
}

#[tauri::command]
pub fn rom_list_xiaoyue_brands() -> Value {
    json!([])
}

#[tauri::command]
pub fn rom_load_xiaoyue_devices(brand: Option<Value>) -> Value {
    json!([])
}

#[tauri::command]
pub fn rom_load_xiaoyue_pro_devices(brand: Option<Value>, pro_kind: Option<Value>) -> Value {
    json!([])
}

#[tauri::command]
pub fn rom_load_xiaoyue_majors(brand: Option<Value>, device: Option<Value>) -> Value {
    json!([])
}

#[tauri::command]
pub fn rom_load_xiaoyue_versions(
    brand: Option<Value>,
    device: Option<Value>,
    major: Option<Value>,
) -> Value {
    json!([])
}

#[tauri::command]
pub fn rom_load_xiaomi_versions(flash_type: Option<Value>, rom_page_url: Option<Value>) -> Value {
    // xiaomirom.com 索引协议未恢复：返回空列表（前端显示「暂无稳定版」）
    json!([])
}

#[tauri::command]
pub fn rom_load_firmware_link(
    brand: Option<Value>,
    file: Option<Value>,
    name: Option<Value>,
    version: Option<Value>,
) -> Result<Value, String> {
    Err("固件索引服务未接入".to_string())
}

#[tauri::command]
pub fn rom_load_server_device(
    brand: Option<Value>,
    file: Option<Value>,
    name: Option<Value>,
) -> Result<Value, String> {
    Err("固件索引服务未接入".to_string())
}

#[tauri::command]
pub fn rom_resolve_xiaomi_link(
    flash_type: Option<Value>,
    page_url: Option<Value>,
) -> Result<Value, String> {
    let url = opt_str(&page_url).ok_or_else(|| "缺少 pageUrl".to_string())?;
    let final_url = resolve_redirect(&url, 8)?;
    Ok(Value::String(final_url))
}

#[tauri::command]
pub fn rom_resolve_xiaoyue_link(meta: Option<Value>) -> Result<Value, String> {
    Err("小越链接解析服务未接入".to_string())
}
