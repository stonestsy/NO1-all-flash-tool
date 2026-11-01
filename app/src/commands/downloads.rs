use super::util::*;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Emitter;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static MAX_ACTIVE: AtomicUsize = AtomicUsize::new(1);
static TASKS: LazyLock<Mutex<HashMap<u64, Task>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

struct DownloadStopped;

#[derive(Clone)]
struct Task {
    id: u64,
    url: String,
    filename: String,
    folder: String,
    status: String,
    downloaded: u64,
    total: u64,
    speed: f64,
    error: String,
    connections: u32,
    created_at: u64,
    temp_path: PathBuf,
    final_path: PathBuf,
    generation: u64,
    app: tauri::AppHandle,
}

impl Task {
    fn to_json(&self) -> Value {
        let percent = if self.status == "done" {
            100.0
        } else if self.total > 0 {
            (self.downloaded as f64 * 100.0 / self.total as f64).min(100.0)
        } else {
            0.0
        };
        json!({
            // 前端会把 DOM 字符串 id 与任务 id 直接比较，因此序列化为字符串。
            "id": self.id.to_string(),
            "url": self.url,
            "filename": self.filename,
            "folder": self.folder,
            "folderpath": self.folder,
            "status": self.status,
            "downloaded": self.downloaded,
            "transferred": self.downloaded,
            "total": self.total,
            "filesize": self.total,
            "speed": self.speed,
            "speed_mbps": self.speed,
            "error": self.error,
            "errortext": self.error,
            "percent": percent,
            "connections": self.connections,
            "created_at": self.created_at,
            "resumable": true,
            "temppath": self.temp_path.to_string_lossy(),
            "finalpath": self.final_path.to_string_lossy(),
        })
    }
}

fn tasks() -> MutexGuard<'static, HashMap<u64, Task>> {
    TASKS.lock().unwrap_or_else(|e| e.into_inner())
}

fn emit_progress(task: &Task) {
    let _ = task.app.emit("downloads:progress", task.to_json());
}

fn opt_string(value: &Option<Value>) -> Option<String> {
    value
        .as_ref()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn opt_u64(value: &Option<Value>) -> Option<u64> {
    value.as_ref().and_then(|v| match v {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

fn task_id(value: &Option<Value>) -> Result<u64, String> {
    opt_u64(value).ok_or_else(|| "无效的下载任务 id".to_string())
}

fn checked_url(value: &Option<Value>) -> Result<String, String> {
    let url = opt_string(value).ok_or_else(|| "缺少下载链接".to_string())?;
    if !url.starts_with("https://") {
        return Err("当前下载器仅支持 https URL".to_string());
    }
    Ok(url)
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    if cleaned.is_empty() {
        "download.bin".to_string()
    } else {
        cleaned.to_string()
    }
}

fn filename_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or("");
    safe_filename(name)
}

fn default_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(pack_dir)
        .join("Downloads")
}

fn temp_path(folder: &Path, filename: &str, id: u64, generation: u64) -> PathBuf {
    folder.join(format!(".{}.{}.{}.aftpart", filename, id, generation))
}

fn queue_task(task: &mut Task) {
    task.generation = task.generation.wrapping_add(1);
    task.status = "queued".to_string();
    task.downloaded = 0;
    task.speed = 0.0;
    task.error.clear();
    task.temp_path = temp_path(
        Path::new(&task.folder),
        &task.filename,
        task.id,
        task.generation,
    );
}

// 按 created_at/id 顺序补足正在运行的任务数。
fn pump() {
    loop {
        let next = {
            let mut map = tasks();
            let active = map.values().filter(|t| t.status == "downloading").count();
            if active >= MAX_ACTIVE.load(Ordering::SeqCst) {
                break;
            }
            let next_id = map
                .values()
                .filter(|t| t.status == "queued")
                .map(|t| (t.created_at, t.id))
                .min()
                .map(|(_, id)| id);
            let Some(id) = next_id else { break };
            let Some(task) = map.get_mut(&id) else { break };
            task.status = "downloading".to_string();
            task.speed = 0.0;
            task.clone()
        };
        let generation = next.generation;
        let id = next.id;
        emit_progress(&next);
        std::thread::spawn(move || run_download(id, generation));
    }
}

fn run_download(id: u64, generation: u64) {
    let snapshot = {
        let map = tasks();
        map.get(&id).cloned()
    };
    let Some(snapshot) = snapshot else {
        pump();
        return;
    };
    if snapshot.generation != generation || snapshot.status != "downloading" {
        pump();
        return;
    }

    let url = snapshot.url.clone();
    let part = snapshot.temp_path.clone();
    let started = Instant::now();
    let mut last_emit = Instant::now() - Duration::from_millis(200);
    let result = catch_unwind(AssertUnwindSafe(|| {
        download_with_progress(&url, &part, move |transferred, total| {
            let stopped = {
                let map = tasks();
                map.get(&id)
                    .map(|t| t.generation != generation || t.status != "downloading")
                    .unwrap_or(true)
            };
            if stopped {
                // 共享下载器没有取消回调，用局部 unwind 退出，catch_unwind 在本线程接住。
                std::panic::resume_unwind(Box::new(DownloadStopped));
            }

            let speed = transferred as f64 / 1048576.0 / started.elapsed().as_secs_f64().max(0.001);
            let event = {
                let mut map = tasks();
                let Some(task) = map.get_mut(&id) else {
                    std::panic::resume_unwind(Box::new(DownloadStopped));
                };
                if task.generation != generation || task.status != "downloading" {
                    std::panic::resume_unwind(Box::new(DownloadStopped));
                }
                task.downloaded = transferred;
                task.total = total;
                task.speed = speed;
                if last_emit.elapsed() >= Duration::from_millis(100) {
                    last_emit = Instant::now();
                    Some(task.clone())
                } else {
                    None
                }
            };
            if let Some(task) = event {
                emit_progress(&task);
            }
        })
    }));

    let event = {
        let mut map = tasks();
        let Some(task) = map.get_mut(&id) else {
            drop(map);
            let _ = std::fs::remove_file(&part);
            pump();
            return;
        };
        if task.generation != generation || task.status != "downloading" {
            None
        } else {
            match result {
                Ok(Ok(bytes)) => match std::fs::rename(&part, &task.final_path) {
                    Ok(()) => {
                        task.status = "done".to_string();
                        task.downloaded = bytes;
                        if task.total == 0 {
                            task.total = bytes;
                        }
                        task.speed = 0.0;
                        task.error.clear();
                        Some(task.clone())
                    }
                    Err(e) => {
                        task.status = "error".to_string();
                        task.speed = 0.0;
                        task.error = format!("保存文件失败: {}", e);
                        Some(task.clone())
                    }
                },
                Ok(Err(e)) => {
                    task.status = "error".to_string();
                    task.speed = 0.0;
                    task.error = format!("下载失败: {}", e);
                    Some(task.clone())
                }
                Err(_) => {
                    task.status = "error".to_string();
                    task.speed = 0.0;
                    task.error = "下载已中断".to_string();
                    Some(task.clone())
                }
            }
        }
    };
    if let Some(task) = event {
        emit_progress(&task);
    }
    pump();
}

#[tauri::command]
pub fn downloads_list() -> Value {
    Value::Array(tasks().values().map(Task::to_json).collect())
}

#[tauri::command]
pub fn downloads_default_dir() -> Value {
    Value::String(default_dir().to_string_lossy().to_string())
}

#[tauri::command]
pub fn downloads_sweep_orphans() -> Value {
    let (dirs, keep) = {
        let map = tasks();
        let mut dirs: HashSet<PathBuf> = map.values().map(|t| PathBuf::from(&t.folder)).collect();
        dirs.insert(default_dir());
        let keep: HashSet<PathBuf> = map.values().map(|t| t.temp_path.clone()).collect();
        (dirs, keep)
    };
    let mut removed = 0u64;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("aftpart")
                && !keep.contains(&path)
                && std::fs::remove_file(path).is_ok()
            {
                removed += 1;
            }
        }
    }
    json!(removed)
}

#[tauri::command]
pub fn downloads_get_max_active() -> Value {
    json!(MAX_ACTIVE.load(Ordering::SeqCst))
}

#[tauri::command]
pub fn downloads_set_max_active(max_active: Option<Value>) -> Result<Value, String> {
    let value = opt_u64(&max_active).ok_or_else(|| "缺少 maxActive".to_string())?;
    if !(1..=64).contains(&value) {
        return Err("同时下载数必须在 1 到 64 之间".to_string());
    }
    MAX_ACTIVE.store(value as usize, Ordering::SeqCst);
    pump();
    Ok(json!(value))
}

#[tauri::command]
pub fn downloads_add(
    app: tauri::AppHandle,
    connections: Option<Value>,
    filename: Option<Value>,
    folder: Option<Value>,
    url: Option<Value>,
) -> Result<Value, String> {
    let url = checked_url(&url)?;
    let connections = opt_u64(&connections).unwrap_or(8);
    if !(1..=64).contains(&connections) {
        return Err("下载线程数必须在 1 到 64 之间".to_string());
    }
    let folder = opt_string(&folder).unwrap_or_else(|| default_dir().to_string_lossy().to_string());
    std::fs::create_dir_all(&folder).map_err(|e| format!("创建下载目录失败: {}", e))?;
    let filename = opt_string(&filename)
        .map(|name| safe_filename(&name))
        .unwrap_or_else(|| filename_from_url(&url));
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let final_path = Path::new(&folder).join(&filename);
    if final_path.exists() {
        return Err("目标文件已存在".to_string());
    }
    let task = Task {
        id,
        url,
        filename: filename.clone(),
        folder: folder.clone(),
        status: "queued".to_string(),
        downloaded: 0,
        total: 0,
        speed: 0.0,
        error: String::new(),
        connections: connections as u32,
        created_at: now_seconds(),
        temp_path: temp_path(Path::new(&folder), &filename, id, 0),
        final_path,
        generation: 0,
        app,
    };
    let result = task.to_json();
    tasks().insert(id, task);
    pump();
    Ok(result)
}

#[tauri::command]
pub fn downloads_pause(id: Option<Value>) -> Result<Value, String> {
    let id = task_id(&id)?;
    let event = {
        let mut map = tasks();
        let task = map
            .get_mut(&id)
            .ok_or_else(|| "下载任务不存在".to_string())?;
        match task.status.as_str() {
            "downloading" | "queued" => {
                task.generation = task.generation.wrapping_add(1);
                task.status = "paused".to_string();
                task.speed = 0.0;
            }
            "paused" => {}
            _ => return Err("当前状态不能暂停".to_string()),
        }
        task.clone()
    };
    emit_progress(&event);
    let result = event.to_json();
    pump();
    Ok(result)
}

#[tauri::command]
pub fn downloads_resume(id: Option<Value>) -> Result<Value, String> {
    let id = task_id(&id)?;
    let event = {
        let mut map = tasks();
        let task = map
            .get_mut(&id)
            .ok_or_else(|| "下载任务不存在".to_string())?;
        if !matches!(task.status.as_str(), "paused" | "error" | "cancelled") {
            return Err("当前状态不能继续".to_string());
        }
        queue_task(task);
        task.clone()
    };
    emit_progress(&event);
    let result = event.to_json();
    pump();
    Ok(result)
}

#[tauri::command]
pub fn downloads_replace_url(id: Option<Value>, url: Option<Value>) -> Result<Value, String> {
    let id = task_id(&id)?;
    let url = checked_url(&url)?;
    let event = {
        let mut map = tasks();
        let task = map
            .get_mut(&id)
            .ok_or_else(|| "下载任务不存在".to_string())?;
        if !matches!(task.status.as_str(), "paused" | "error" | "cancelled") {
            return Err("请先暂停任务再更换链接".to_string());
        }
        task.url = url;
        queue_task(task);
        task.clone()
    };
    emit_progress(&event);
    let result = event.to_json();
    pump();
    Ok(result)
}

#[tauri::command]
pub fn downloads_cancel(id: Option<Value>) -> Result<Value, String> {
    let id = task_id(&id)?;
    let event = {
        let mut map = tasks();
        let task = map
            .get_mut(&id)
            .ok_or_else(|| "下载任务不存在".to_string())?;
        if task.status == "done" {
            return Err("已完成任务不能取消".to_string());
        }
        if task.status != "cancelled" {
            task.generation = task.generation.wrapping_add(1);
            task.status = "cancelled".to_string();
            task.speed = 0.0;
            task.error = "已取消".to_string();
        }
        task.clone()
    };
    emit_progress(&event);
    let result = event.to_json();
    pump();
    Ok(result)
}

#[tauri::command]
pub fn downloads_remove(id: Option<Value>) -> Result<Value, String> {
    let id = task_id(&id)?;
    let removed = tasks()
        .remove(&id)
        .ok_or_else(|| "下载任务不存在".to_string())?;
    let _ = std::fs::remove_file(removed.temp_path);
    pump();
    Ok(json!(true))
}

#[tauri::command]
pub fn downloads_clear_finished() -> Value {
    let ids: Vec<u64> = tasks()
        .values()
        .filter(|t| matches!(t.status.as_str(), "done" | "error" | "cancelled"))
        .map(|t| t.id)
        .collect();
    let mut removed = 0u64;
    for id in ids {
        if let Some(task) = tasks().remove(&id) {
            let _ = std::fs::remove_file(task.temp_path);
            removed += 1;
        }
    }
    json!(removed)
}

#[tauri::command]
pub fn downloads_set_connections(
    connections: Option<Value>,
    id: Option<Value>,
) -> Result<Value, String> {
    let id = task_id(&id)?;
    let connections = opt_u64(&connections).ok_or_else(|| "缺少 connections".to_string())?;
    if !(1..=64).contains(&connections) {
        return Err("下载线程数必须在 1 到 64 之间".to_string());
    }
    let mut map = tasks();
    let task = map
        .get_mut(&id)
        .ok_or_else(|| "下载任务不存在".to_string())?;
    if !matches!(task.status.as_str(), "paused" | "error") {
        return Err("仅暂停或失败的任务可修改线程数".to_string());
    }
    task.connections = connections as u32;
    Ok(task.to_json())
}

#[tauri::command]
pub fn downloads_open_folder(path: Option<Value>) -> Result<Value, String> {
    let path = opt_string(&path).ok_or_else(|| "缺少目录路径".to_string())?;
    let path = PathBuf::from(path);
    let target = if path.is_file() {
        path.parent().unwrap_or(path.as_path())
    } else {
        path.as_path()
    };
    shell_open(&target.to_string_lossy())?;
    Ok(json!(true))
}
