//! Tauri 命令聚合与启动链路实现。
//! 命令名/参数由前端反混淆代码恢复；各功能组在子模块中实现，本文件负责注册与应用级命令。
#![allow(dead_code, non_snake_case, unused_variables)]

use serde_json::{json, Value};

pub mod adb;
pub mod creds;
pub mod downloads;
pub mod edl;
pub mod fastboot;
pub mod fastboot_script;
pub mod formats;
pub mod lib_backup;
pub mod misc;
pub mod mtk;
pub mod pack;
pub mod pan123;
pub mod qcn;
pub mod rom;
pub mod root;
pub mod store;
pub mod sys;
pub mod update;
pub mod util;
pub mod utility;
pub mod wallpaper;
pub mod xiaomi_unlock;

pub use adb::*;
pub use creds::*;
pub use downloads::*;
pub use edl::*;
pub use fastboot::*;
pub use fastboot_script::*;
pub use formats::*;
pub use lib_backup::*;
pub use misc::*;
pub use mtk::*;
pub use pack::*;
pub use pan123::*;
pub use qcn::*;
pub use rom::*;
pub use root::*;
pub use store::*;
pub use sys::*;
pub use update::*;
use util::*;
pub use utility::*;
pub use wallpaper::*;
pub use xiaomi_unlock::*;

/// 启动链路命令按前端真实契约返回。

#[tauri::command]
pub fn app_version() -> Value {
    Value::String(env!("CARGO_PKG_VERSION").to_string())
}

#[tauri::command]
pub fn login_bootstrap(
    force_reconnect: Option<Value>,
    skip_time_calibrate: Option<Value>,
) -> Value {
    // 本地模式：不连接鉴权服务器，通道视为可用、无版本拦截
    json!({
        "ok": true,
        "security_ok": true,
        "server_ok": true,
        "status_message": "",
        "diag": {},
        "version": {
            "ok": true,
            "blocked": false,
            "latest_version": env!("CARGO_PKG_VERSION"),
            "show_update_popup": false
        }
    })
}

#[tauri::command]
pub fn calibrate_system_time_cmd() -> Value {
    let t = local_time();
    json!({
        "ok": true,
        "message": "",
        "calibrated_hm": format!("{:02}:{:02}", t.hour, t.minute)
    })
}

/// 本地账号对象（主界面 _u() 直接读这些字段；已去除登录校验，任意账号可进入）
fn local_account(username: &str) -> Value {
    json!({
        "ok": true,
        "username": username,
        "level": "永久会员",
        "days_left": -1,
        "account_expires": "",
        "software_title": "All-FlashTool",
        "software_title_color": "",
        "login_wallpaper_exists": false,
        "login_wallpaper_kind": "",
        "login_wallpaper_updated_at": 0,
        "login_wallpaper_download_ticket": ""
    })
}

#[tauri::command]
pub fn login(password: Option<Value>, username: Option<Value>) -> Value {
    local_account(
        username
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or("local"),
    )
}

#[tauri::command]
pub fn login_by_key(login_key: Option<Value>) -> Value {
    local_account(
        login_key
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or("local"),
    )
}

#[tauri::command]
pub fn server_status() -> Value {
    json!("正常")
}

#[tauri::command]
pub async fn status_bar_info() -> Value {
    // 网络查询放到阻塞线程池，避免卡住 IPC 主线程
    let region = tauri::async_runtime::spawn_blocking(fetch_region)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "未知地区".to_string());
    json!({"system_version": windows_version(), "region": region})
}

#[tauri::command]
pub fn marquee_messages() -> Value {
    json!([])
}
