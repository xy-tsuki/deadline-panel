mod notifications;
mod panel_state;
use panel_state::{PanelState, PanelVisibility};
#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod material;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Size,
};
use tauri_plugin_dialog::DialogExt;

#[cfg(windows)]
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
    Globalization::GetUserDefaultUILanguage,
    Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON},
    UI::WindowsAndMessaging::{
        GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindow, GetWindowRect,
        GetWindowThreadProcessId, IsIconic, IsWindowVisible, SetWindowPos, GW_HWNDPREV,
        SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
    },
};

const PANEL_WIDTH: f64 = 372.0;
static SETTINGS_WINDOW_CREATION: Mutex<()> = Mutex::new(());

/// Reports the current no-blur renderer and native material request count.
#[tauri::command]
fn get_material_diagnostics()->serde_json::Value {
    #[cfg(windows)] {material::diagnostics()}
    #[cfg(not(windows))] {serde_json::json!({})}
}

#[tauri::command]
async fn set_window_material(window: tauri::WebviewWindow, dark: bool, low_power: Option<bool>) -> Result<String, String> {
    #[cfg(windows)]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let target = window.clone();
        window.run_on_main_thread(move || {
            // Keep native title bars and system controls in the selected theme.
            let result = target.set_theme(Some(if dark { tauri::Theme::Dark } else { tauri::Theme::Light }))
                .map_err(|error|error.to_string())
                .and_then(|_| material::apply(&target,dark,low_power.unwrap_or(false)));
            let _ = tx.send(result);
        }).map_err(|error| error.to_string())?;
        let mode = rx.recv_timeout(Duration::from_secs(5)).map_err(|error| error.to_string())??;
        Ok(mode.into())
    }
    #[cfg(not(windows))]
    { let _ = (window, dark); Ok("compatibility".into()) }
}

#[tauri::command]
async fn export_tasks_json(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<Option<String>, String> {
    // Take a snapshot, then release the DB lock before the user opens a dialog.
    let payload = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let tasks = query_tasks(&db).map_err(|error| error.to_string())?;
        let exported_at: String = db.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ', 'now')", [], |row| row.get(0)).map_err(|error| error.to_string())?;
        serde_json::to_string_pretty(&serde_json::json!({ "version": 1, "exportedAt": exported_at, "tasks": tasks })).map_err(|error| error.to_string())?
    };
    let Some(file) = app.dialog().file().add_filter("JSON", &["json"]).set_file_name("deadline-panel.json").blocking_save_file() else { return Ok(None); };
    let path = file.into_path().map_err(|error| error.to_string())?;
    fs::write(&path, payload).map_err(|error| error.to_string())?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[tauri::command]
async fn read_import_json(app: AppHandle) -> Result<Option<String>, String> {
    let Some(file) = app.dialog().file().add_filter("JSON", &["json"]).blocking_pick_file() else { return Ok(None); };
    let path = file.into_path().map_err(|error| error.to_string())?;
    if fs::metadata(&path).map_err(|error| error.to_string())?.len() > 8 * 1024 * 1024 { return Err("JSON exceeds the 8 MiB import limit".into()); }
    fs::read_to_string(path).map(Some).map_err(|error| error.to_string())
}
const PANEL_EXPANDED_HEIGHT: f64 = 600.0;
const PANEL_MIN_HEIGHT: f64 = 180.0;
const PANEL_COLLAPSED_HEIGHT: f64 = 44.0;
const PANEL_GAP: f64 = 10.0;
const PANEL_MARGIN: i32 = 18;
const PANEL_POSITION_SETTING_KEY: &str = "panel_position";
const PANEL_ANCHOR_POSITION_SETTING_KEY: &str = "panel_anchor_position_v2";
const SEED_TASKS_V2_SETTING_KEY: &str = "seed_tasks_v2";
const LONG_RUNTIME_RESTART_AFTER: Duration = Duration::from_secs(10 * 60 * 60);
const LONG_RUNTIME_RESTART_RETRY: Duration = Duration::from_secs(5 * 60);
#[cfg(windows)]
const AUTOSTART_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const AUTOSTART_REG_VALUE: &str = "Deadline Panel";

struct AppState {
    revision: AtomicU64,
    db: Mutex<Connection>,
    db_path: PathBuf,
    panel: Mutex<PanelState>,
    fullscreen_auto_hide: AtomicBool,
}

struct NativeMenuLabels {
    show_panel: &'static str,
    pause_panel: &'static str,
    hide_15: &'static str,
    hide_30: &'static str,
    hide_60: &'static str,
    reposition: &'static str,
    quit: &'static str,
}

struct SeedTaskSpec {
    id: &'static str,
    title: &'static str,
    day_offset: i64,
    due_time: &'static str,
    priority: &'static str,
    notes: &'static str,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeadlineTask {
    id: String,
    title: String,
    due_at: String,
    priority: String,
    status: String,
    notes: String,
    source: String,
    is_current: bool,
    created_at: String,
    updated_at: String,
    completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelPointerState {
    in_trigger: bool,
    in_window: bool,
    expand_direction: &'static str,
    left_down: bool,
    right_down: bool,
    cursor_x: i32,
    cursor_y: i32,
    window_x: i32,
    window_y: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PanelExpandResult {
    direction: &'static str,
    expanded: bool,
}

fn broadcast_revision(app: &AppHandle, state: &AppState) {
    let revision = state.revision.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = app.emit("deadline-state-changed", revision);
}

#[tauri::command]
fn pending_deletes(
    namespace: String,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let mut stmt = db
        .prepare("SELECT task_id FROM pending_deletes WHERE namespace = ?1")
        .map_err(|error| error.to_string())?;
    let rows = stmt
        .query_map(params![namespace], |row| row.get(0))
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn acknowledge_delete(
    namespace: String,
    id: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.execute(
        "DELETE FROM pending_deletes WHERE namespace = ?1 AND task_id = ?2",
        params![namespace, id],
    )
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn state_revision(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    let _db = state.db.lock().map_err(|error| error.to_string())?;
    Ok(state.revision.load(Ordering::SeqCst))
}

#[tauri::command]
fn list_tasks(state: tauri::State<'_, AppState>) -> Result<Vec<DeadlineTask>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    query_tasks(&db)
}

#[tauri::command]
fn load_snapshot(state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "tasks": query_tasks(&db)?,
        "focusLimit": get_app_setting_value(&db, "focus_limit").map_err(|error| error.to_string())?,
        "language": get_app_setting_value(&db, "app_language").map_err(|error| error.to_string())?,
        "appearance": get_app_setting_value(&db, "appearance_preferences").map_err(|error| error.to_string())?,
        "revision": state.revision.load(Ordering::SeqCst)
    }))
}

#[tauri::command]
fn get_app_settings(keys: Vec<String>, state: tauri::State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let mut values = serde_json::Map::new();
    for key in keys { values.insert(key.clone(), serde_json::json!(get_app_setting_value(&db, &key).map_err(|error| error.to_string())?)); }
    Ok(values.into())
}

#[tauri::command]
fn set_app_settings(values: std::collections::HashMap<String, String>, app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    // This batch is for sync configuration; unrelated preference side effects
    // remain on their dedicated commands.
    if values.keys().any(|key| !matches!(key.as_str(), "sync_supabase_url" | "sync_supabase_anon_key" | "sync_code")) { return Err("Unsupported batch setting".into()); }
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    let tx = db.transaction().map_err(|error| error.to_string())?;
    for (key, value) in values { set_app_setting_value(&tx, &key, &value).map_err(|error| error.to_string())?; }
    tx.commit().map_err(|error| error.to_string())?;
    broadcast_revision(&app, &state);
    Ok(())
}

fn query_tasks(db: &Connection) -> Result<Vec<DeadlineTask>, String> {
    let mut statement = db
        .prepare(
            "SELECT id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
             FROM tasks
             ORDER BY due_at ASC,
                      CASE priority
                        WHEN 'urgent' THEN 4
                        WHEN 'high' THEN 3
                        WHEN 'medium' THEN 2
                        ELSE 1
                      END DESC,
                      created_at ASC",
        )
        .map_err(|error| error.to_string())?;

    let rows = statement
        .query_map([], |row| {
            Ok(DeadlineTask {
                id: row.get(0)?,
                title: row.get(1)?,
                due_at: row.get(2)?,
                priority: row.get(3)?,
                status: row.get(4)?,
                notes: row.get(5)?,
                source: row.get(6)?,
                is_current: row.get::<_, i64>(7)? != 0,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
                completed_at: row.get(10)?,
            })
        })
        .map_err(|error| error.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn write_task(db: &Connection, task: &DeadlineTask) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO tasks (
            id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET
            title = excluded.title,
            due_at = excluded.due_at,
            priority = excluded.priority,
            status = excluded.status,
            notes = excluded.notes,
            source = excluded.source,
            is_current = excluded.is_current,
            updated_at = excluded.updated_at,
            completed_at = excluded.completed_at",
        params![
            &task.id,
            &task.title,
            &task.due_at,
            &task.priority,
            &task.status,
            &task.notes,
            &task.source,
            if task.is_current { 1 } else { 0 },
            &task.created_at,
            &task.updated_at,
            &task.completed_at
        ],
    )
    ?;

    Ok(())
}

fn persist_tasks(db: &mut Connection, tasks: &[DeadlineTask]) -> rusqlite::Result<()> {
    let tx = db.transaction()?;
    for task in tasks {
        write_task(&tx, task)?;
    }
    tx.commit()
}

#[tauri::command]
fn save_tasks(
    tasks: Vec<DeadlineTask>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    if tasks.is_empty() {
        return Ok(());
    }
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    persist_tasks(&mut db, &tasks).map_err(|error| error.to_string())?;
    refresh_scheduled_notifications(&app, &db);
    broadcast_revision(&app, &state);
    Ok(())
}

fn patch_existing_task(
    db: &Connection,
    id: &str,
    fields: serde_json::Value,
) -> Result<DeadlineTask, String> {
    let task = db.query_row("SELECT id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at FROM tasks WHERE id = ?1", params![id], |row| {
        Ok(DeadlineTask { id: row.get(0)?, title: row.get(1)?, due_at: row.get(2)?, priority: row.get(3)?, status: row.get(4)?, notes: row.get(5)?, source: row.get(6)?, is_current: row.get::<_, i64>(7)? != 0, created_at: row.get(8)?, updated_at: row.get(9)?, completed_at: row.get(10)? })
    }).map_err(|error| error.to_string())?;
    let mut value = serde_json::to_value(&task).map_err(|error| error.to_string())?;
    for (key, field) in fields.as_object().ok_or("invalid task patch")? {
        if ![
            "title",
            "dueAt",
            "priority",
            "status",
            "notes",
            "source",
            "isCurrent",
            "completedAt",
            "updatedAt",
        ]
        .contains(&key.as_str())
        {
            return Err(format!("unsupported task field: {key}"));
        }
        value[key] = field.clone();
    }
    let updated: DeadlineTask = serde_json::from_value(value).map_err(|error| error.to_string())?;
    if updated.title.trim().is_empty()
        || !["urgent", "high", "medium", "low"].contains(&updated.priority.as_str())
        || !["active", "completed", "postponed"].contains(&updated.status.as_str())
    {
        return Err("invalid task fields".into());
    }
    if updated.is_current && updated.status != "completed" && !task.is_current {
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE is_current = 1 AND status != 'completed'",
                [],
                |row| row.get(0),
            )
            .map_err(|error| error.to_string())?;
        if count >= 2 {
            return Err("current_task_limit".into());
        }
    }
    write_task(db, &updated).map_err(|error| error.to_string())?;
    Ok(updated)
}

#[tauri::command]
fn patch_task(
    id: String,
    fields: serde_json::Value,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<DeadlineTask, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let task = patch_existing_task(&db, &id, fields)?;
    refresh_scheduled_notifications(&app, &db);
    broadcast_revision(&app, &state);
    Ok(task)
}

#[tauri::command]
fn save_task(
    task: DeadlineTask,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<DeadlineTask, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    write_task(&db, &task).map_err(|error| error.to_string())?;
    refresh_scheduled_notifications(&app, &db);
    broadcast_revision(&app, &state);
    Ok(task)
}

fn persist_delete(db: &mut Connection, id: &str, namespace: Option<&str>) -> rusqlite::Result<()> {
    let tx = db.transaction()?;
    if let Some(namespace) = namespace {
        tx.execute(
            "INSERT OR IGNORE INTO pending_deletes(namespace, task_id) VALUES (?1, ?2)",
            params![namespace, &id],
        )?;
    }
    tx.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
fn delete_task(
    id: String,
    namespace: Option<String>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    persist_delete(&mut db, &id, namespace.as_deref()).map_err(|error| error.to_string())?;
    refresh_scheduled_notifications(&app, &db);
    broadcast_revision(&app, &state);
    Ok(())
}

#[tauri::command]
fn replace_tasks(
    tasks: Vec<DeadlineTask>,
    expected_revision: Option<u64>,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
    if expected_revision.is_some_and(|revision| revision != state.revision.load(Ordering::SeqCst)) {
        return Err("revision_conflict".into());
    }
    let tx = db.transaction().map_err(|error| error.to_string())?;
    tx.execute("DELETE FROM tasks", [])
        .map_err(|error| error.to_string())?;

    for task in tasks {
        tx.execute(
            "INSERT INTO tasks (
                id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                task.id,
                task.title,
                task.due_at,
                task.priority,
                task.status,
                task.notes,
                task.source,
                if task.is_current { 1 } else { 0 },
                task.created_at,
                task.updated_at,
                task.completed_at
            ],
        )
        .map_err(|error| error.to_string())?;
    }

    tx.commit().map_err(|error| error.to_string())?;
    refresh_scheduled_notifications(&app, &db);
    broadcast_revision(&app, &state);
    Ok(())
}

#[tauri::command]
fn get_app_setting(
    key: String,
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    let value = db
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok();
    Ok(value)
}

fn merge_appearance_preferences(
    db: &Connection,
    changes: serde_json::Value,
) -> Result<String, String> {
    let stored =
        get_app_setting_value(db, "appearance_preferences").map_err(|error| error.to_string())?;
    let mut current = stored
        .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
        .filter(|value| value.is_object())
        .unwrap_or_else(|| serde_json::json!({}));
    for (key, value) in changes.as_object().ok_or("invalid preferences")? {
        match key.as_str() {
            "theme" if matches!(value.as_str(), Some("system" | "light" | "dark")) => {}
            "lowPowerMode" | "fullscreenAutoHide" if value.is_boolean() => {}
            _ => return Err("invalid appearance preference".into()),
        }
        current[key] = value.clone();
    }
    let value = current.to_string();
    set_app_setting_value(db, "appearance_preferences", &value)
        .map_err(|error| error.to_string())?;
    Ok(value)
}

#[tauri::command]
fn set_appearance_preferences(
    changes: serde_json::Value,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let value = {
        let db = state.db.lock().map_err(|error| error.to_string())?;
        merge_appearance_preferences(&db, changes)?
    };
    update_fullscreen_auto_hide(&app, &state, &value)?;
    broadcast_revision(&app, &state);
    Ok(value)
}

#[tauri::command]
fn set_app_setting(
    key: String,
    value: String,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    set_app_setting_value(&db, &key, &value).map_err(|error| error.to_string())?;
    drop(db);

    if key == "appearance_preferences" {
        update_fullscreen_auto_hide(&app, &state, &value)?;
    }

    broadcast_revision(&app, &state);
    if key == "app_language" {
        refresh_tray_menu(&app);
        if let Some(window) = app.get_webview_window("settings") {
            let title = match resolved_native_language(&app).as_str() {
                "ja" => "設定とツール",
                "en" => "Settings and tools",
                _ => "设置与工具",
            };
            let _ = window.set_title(title);
        }
    }

    Ok(())
}

#[tauri::command]
fn schedule_notifications(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<usize, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    notifications::schedule(&db, &app.config().identifier)
}

fn refresh_scheduled_notifications(app: &AppHandle, db: &Connection) {
    if let Err(error) = notifications::refresh(db, &app.config().identifier) {
        let _ = app.emit("deadline-notification-error", error);
    } else {
        let _ = app.emit("deadline-notification-error", "");
    }
}

#[tauri::command]
fn system_language() -> String {
    system_native_language().to_string()
}

#[tauri::command]
async fn open_settings_window(app: AppHandle, title: String) -> Result<(), String> {
    // WebView2 creation waits on the UI thread. A synchronous IPC command runs
    // on that same thread and deadlocks; keep this command on Tauri's runtime.
    let _creation = SETTINGS_WINDOW_CREATION.lock().map_err(|error| error.to_string())?;
    if let Some(window) = app.get_webview_window("settings") {
        window
            .set_title(&title)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window.unminimize().map_err(|error| error.to_string())?;
        return window.set_focus().map_err(|error| error.to_string());
    }
    let monitor = app.get_webview_window("main")
        .and_then(|window| window.current_monitor().ok().flatten())
        .or(app.primary_monitor().map_err(|error| error.to_string())?);
    let (width, height) = monitor.map(|monitor| {
        let area = monitor.work_area();
        let scale = monitor.scale_factor();
        ((area.size.width as f64 / scale - 24.0).clamp(320.0, 600.0),
         (area.size.height as f64 / scale - 24.0).clamp(320.0, 900.0))
    }).unwrap_or((600.0, 900.0));
    tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html?window=settings".into()),
    )
    .title(title)
    .inner_size(width, height)
    .resizable(false)
    .maximizable(false)
    .decorations(false)
    .shadow(false)
    .transparent(true)
    .center()
    .build()
    .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn show_panel(app: AppHandle) -> Result<(), String> {
    show_panel_collapsed(&app, true, false)
}

#[tauri::command]
fn get_panel_visibility(state: tauri::State<'_, AppState>) -> Result<PanelVisibility, String> {
    Ok(state.panel.lock().map_err(|error| error.to_string())?.visibility())
}

fn emit_panel_visibility(app: &AppHandle) -> Result<(), String> {
    let visibility = get_panel_visibility(app.state::<AppState>())?;
    app.emit("deadline-panel-visibility", visibility).map_err(|error| error.to_string())
}

#[tauri::command]
async fn settings_window_action(window: tauri::WebviewWindow, action: String) -> Result<(), String> {
    if window.label() != "settings" { return Err("Settings controls belong to the settings window".into()); }
    match action.as_str() {
        "hide" => window.hide(),
        "minimize" => window.minimize(),
        "drag" => window.start_dragging(),
        _ => return Err("Unknown settings window action".into()),
    }.map_err(|error| error.to_string())
}

#[tauri::command]
fn data_file_path(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.db_path.to_string_lossy().to_string())
}

#[tauri::command]
fn open_data_dir(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let Some(dir) = state.db_path.parent() else {
        return Err("data directory not found".into());
    };

    #[cfg(windows)]
    {
        Command::new("explorer.exe")
            .arg(dir)
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    #[cfg(not(windows))]
    {
        let opener = if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        Command::new(opener)
            .arg(dir)
            .spawn()
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

#[tauri::command]
fn backup_database(state: tauri::State<'_, AppState>) -> Result<String, String> {
    backup_database_file(&state.db_path, "manual").map_err(|error| error.to_string())
}

#[tauri::command]
fn panel_expand_direction(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<PanelExpandResult, String> {
    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(PanelExpandResult { direction: "up", expanded: false });
    };
    let (open_down, expanded) = {
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        (panel.open_down, panel.expanded)
    };
    let anchor = current_panel_anchor(&strip_window, open_down, expanded)
        .map_err(|error| error.to_string())?;
    let should_open_down =
        should_expand_panel_down(&strip_window, anchor.1).map_err(|error| error.to_string())?;
    Ok(PanelExpandResult {
        direction: if should_open_down { "down" } else { "up" },
        expanded,
    })
}

#[tauri::command]
fn set_panel_expanded(
    expanded: bool,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<PanelExpandResult, String> {
    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(PanelExpandResult { direction: "up", expanded: false });
    };
    let Some(panel_window) = app.get_webview_window("panel") else {
        return Ok(PanelExpandResult { direction: "up", expanded: false });
    };

    let (previous_open_down, was_expanded, expanded) = {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        let previous_open_down = panel.open_down;
        let was_expanded = panel.expanded;
        // Reject a late hover request delivered after a hide or drag started.
        panel.expanded = expanded && panel.visibility().visible && !panel.dragging;
        (previous_open_down, was_expanded, panel.expanded)
    };

    let anchor = current_panel_anchor(&strip_window, previous_open_down, was_expanded)
        .map_err(|error| error.to_string())?;
    let open_down =
        should_expand_panel_down(&strip_window, anchor.1).map_err(|error| error.to_string())?;

    strip_window
        .set_focusable(false)
        .map_err(|error| error.to_string())?;
    strip_window
        .set_ignore_cursor_events(false)
        .map_err(|error| error.to_string())?;

    if expanded {
        panel_window
            .set_skip_taskbar(true)
            .map_err(|error| error.to_string())?;
        panel_window
            .set_shadow(false)
            .map_err(|error| error.to_string())?;
        position_expanded_panel(&strip_window, &panel_window, anchor.0, anchor.1, open_down)
            .map_err(|error| error.to_string())?;
        panel_window
            .set_focusable(true)
            .map_err(|error| error.to_string())?;
        panel_window
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        panel_window.show().map_err(|error| error.to_string())?;
    } else {
        panel_window.hide().map_err(|error| error.to_string())?;
        panel_window
            .set_focusable(false)
            .map_err(|error| error.to_string())?;
    }

    {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.open_down = open_down;
    }

    emit_panel_visibility(&app)?;
    Ok(PanelExpandResult {
        direction: if open_down { "down" } else { "up" },
        expanded,
    })
}

#[tauri::command]
fn hide_panel_temporarily(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.hide_manually();
    }
    sync_panel_visibility(&app)
}

#[tauri::command]
fn hide_panel_for_minutes(minutes: u64, app: AppHandle) -> Result<(), String> {
    hide_panel_for_duration(minutes, &app)
}

fn hide_panel_for_duration(minutes: u64, app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let hide_token = {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.hide_manually()
    };
    sync_panel_visibility(app)?;

    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(minutes.saturating_mul(60)));
        let handle = app.clone();
        // Window operations belong to the UI thread. Never wait for that thread
        // while holding the DB/state mutex from this timer thread.
        let _ = app.run_on_main_thread(move || {
            let state = handle.state::<AppState>();
            let expired = state.panel.lock().map(|mut panel| panel.expire_hide(hide_token)).unwrap_or(false);
            if expired { report_panel_result(&handle, sync_panel_visibility(&handle)); }
        });
    });

    Ok(())
}

#[tauri::command]
fn set_panel_auto_hidden(
    hidden: bool,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let changed = {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.observe_fullscreen(hidden)
    };
    if changed { sync_panel_visibility(&app)?; }
    Ok(())
}

fn fullscreen_auto_hide_enabled(value: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(value).ok()
        .and_then(|value| value.get("fullscreenAutoHide").and_then(|value| value.as_bool()))
        .unwrap_or(true)
}

fn update_fullscreen_auto_hide(app: &AppHandle, state: &AppState, value: &str) -> Result<(), String> {
    let enabled = fullscreen_auto_hide_enabled(value);
    state.fullscreen_auto_hide.store(enabled, Ordering::Relaxed);
    if !enabled { set_panel_auto_hidden(false, app.clone(), app.state::<AppState>())?; }
    Ok(())
}

fn start_fullscreen_watcher(app: AppHandle) {
    thread::spawn(move || {
        let mut previous = None;
        loop {
            let enabled = app.state::<AppState>().fullscreen_auto_hide.load(Ordering::Relaxed);
            // No foreground-window probes when the preference is disabled.
            let hidden = enabled && foreground_window_is_fullscreen(&app).unwrap_or(false);
            if previous != Some((enabled, hidden)) {
                previous = Some((enabled, hidden));
                let handle = app.clone();
                let _ = app.run_on_main_thread(move || {
                    let state = handle.state::<AppState>();
                    let enabled_now = state.fullscreen_auto_hide.load(Ordering::Relaxed);
                    report_panel_result(&handle, set_panel_auto_hidden(hidden && enabled_now, handle.clone(), state));
                });
            }
            thread::sleep(Duration::from_millis(1200));
        }
    });
}

fn report_panel_result(app: &AppHandle, result: Result<(), String>) {
    if let Err(error) = result {
        eprintln!("Panel recovery failed: {error}");
        let _ = app.emit("deadline-panel-error", error);
    }
}

#[tauri::command]
fn is_foreground_window_fullscreen(app: AppHandle) -> Result<bool, String> {
    foreground_window_is_fullscreen(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn cursor_in_panel_trigger(app: AppHandle) -> Result<bool, String> {
    cursor_is_in_panel_trigger(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn panel_pointer_state(app: AppHandle) -> Result<PanelPointerState, String> {
    get_panel_pointer_state(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_panel_accepts_input(accepts_input: bool, app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    window
        .set_ignore_cursor_events(false)
        .map_err(|error| error.to_string())?;
    if accepts_input {
        window
            .set_focusable(false)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn move_panel_window(x: i32, y: i32, app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn remember_panel_position(app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let state = app.state::<AppState>();
    let (open_down, expanded) = {
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        (panel.open_down, panel.expanded)
    };
    let (anchor_x, anchor_y) =
        current_panel_anchor(&window, open_down, expanded).map_err(|error| error.to_string())?;
    let db = state.db.lock().map_err(|error| error.to_string())?;
    set_app_setting_value(
        &db,
        PANEL_ANCHOR_POSITION_SETTING_KEY,
        &format!("{},{}", anchor_x, anchor_y),
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn reset_panel_position(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|error| error.to_string())?;
        db.execute(
            "DELETE FROM app_settings WHERE key IN (?1, ?2)",
            params![
                PANEL_POSITION_SETTING_KEY,
                PANEL_ANCHOR_POSITION_SETTING_KEY
            ],
        )
        .map_err(|error| error.to_string())?;
    }
    show_panel_collapsed(&app, true, true)
}

#[tauri::command]
fn show_panel_context_menu(app: AppHandle) -> Result<(), String> {
    let expanded = app
        .try_state::<AppState>()
        .and_then(|state| state.panel.lock().ok().map(|panel| panel.expanded))
        .unwrap_or(false);
    let window = if expanded {
        app.get_webview_window("panel")
            .or_else(|| app.get_webview_window("main"))
    } else {
        app.get_webview_window("main")
    };
    let Some(window) = window else {
        return Ok(());
    };
    let menu = build_panel_menu(&app).map_err(|error| error.to_string())?;
    window.popup_menu(&menu).map_err(|error| error.to_string())
}

#[tauri::command]
fn start_panel_drag(app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    {
        let state = app.state::<AppState>();
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        if panel.dragging {
            return Ok(());
        }
        panel.dragging = true;
        panel.expanded = false;
    }
    if let Some(panel_window) = app.get_webview_window("panel") {
        let _ = panel_window.hide();
    }

    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd_value = hwnd.0 as isize;
    let start_position = window.outer_position().map_err(|error| error.to_string())?;
    let mut start_cursor = POINT::default();
    unsafe { GetCursorPos(&mut start_cursor).map_err(|error| error.to_string())? };

    window
        .set_ignore_cursor_events(false)
        .map_err(|error| error.to_string())?;
    window
        .set_focusable(true)
        .map_err(|error| error.to_string())?;

    thread::spawn(move || {
        run_panel_drag_loop(
            app,
            hwnd_value,
            start_position.x,
            start_position.y,
            start_cursor.x,
            start_cursor.y,
        );
    });

    Ok(())
}

#[tauri::command]
fn finish_panel_drag(_app: AppHandle) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_autostart_enabled(app: AppHandle) -> Result<bool, String> {
    autostart_enabled(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<bool, String> {
    set_autostart_enabled_native(&app, enabled).map_err(|error| error.to_string())?;
    autostart_enabled(&app).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();
    builder = builder.plugin(tauri_plugin_dialog::init());

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_panel_collapsed(app, true, false);
        }));
        builder = builder.plugin(tauri_plugin_process::init());
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .on_window_event(|window, event| {
            if window.label() == "settings" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .on_menu_event(|app, event| {
            handle_panel_menu_event(app, event.id().as_ref());
        })
        .setup(|app| {
            let db_path = database_path(app.handle())?;
            let db = Connection::open(&db_path).map_err(|error| error.to_string())?;
            configure_database(&db).map_err(|error| error.to_string())?;
            initialize_database(&db).map_err(|error| error.to_string())?;
            position_main_window(app, &db)?;
            let fullscreen_auto_hide = get_app_setting_value(&db, "appearance_preferences")?
                .map(|value| fullscreen_auto_hide_enabled(&value)).unwrap_or(true);
            app.manage(AppState {
                revision: AtomicU64::new(0),
                db: Mutex::new(db),
                db_path,
                panel: Mutex::new(PanelState::default()),
                fullscreen_auto_hide: AtomicBool::new(fullscreen_auto_hide),
            });
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;
            setup_tray(app)?;
            show_panel_collapsed(app.handle(), false, false).map_err(|error| error.to_string())?;
            start_long_runtime_watchdog(app.handle().clone());
            start_fullscreen_watcher(app.handle().clone());
            #[cfg(debug_assertions)]
            if std::env::var_os("DEADLINE_PANEL_ACCEPTANCE_OPEN_SETTINGS").is_some()
                && std::env::var_os("DEADLINE_PANEL_ACCEPTANCE_DATA_DIR").is_some()
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = open_settings_window(handle, "设置与工具".into()).await {
                        eprintln!("Acceptance settings creation failed: {error}");
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_material_diagnostics,
            set_window_material,
            settings_window_action,
            load_snapshot,
            get_app_settings,
            set_app_settings,
            export_tasks_json,
            read_import_json,
            pending_deletes,
            acknowledge_delete,
            state_revision,
            list_tasks,
            patch_task,
            save_task,
            save_tasks,
            delete_task,
            replace_tasks,
            get_app_setting,
            set_app_setting,
            set_appearance_preferences,
            schedule_notifications,
            system_language,
            open_settings_window,
            show_panel,
            get_panel_visibility,
            restart_app,
            data_file_path,
            open_data_dir,
            backup_database,
            panel_expand_direction,
            set_panel_expanded,
            hide_panel_temporarily,
            hide_panel_for_minutes,
            set_panel_auto_hidden,
            is_foreground_window_fullscreen,
            cursor_in_panel_trigger,
            panel_pointer_state,
            set_panel_accepts_input,
            move_panel_window,
            remember_panel_position,
            reset_panel_position,
            show_panel_context_menu,
            start_panel_drag,
            finish_panel_drag,
            quit_app,
            get_autostart_enabled,
            set_autostart_enabled
        ])
        .run(tauri::generate_context!())
        .expect("error while running Deadline Panel");
}

fn position_main_window(
    app: &mut tauri::App,
    db: &Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(());
    };

    strip_window.set_shadow(false)?;
    strip_window.set_skip_taskbar(true)?;
    let anchor = get_saved_panel_anchor_position(db, collapsed_strip_offset(&strip_window)?)?;
    let _ = set_strip_bounds_from_saved_or_bottom_right(&strip_window, anchor)?;
    strip_window.set_focusable(false)?;
    strip_window.set_ignore_cursor_events(false)?;

    if let Some(panel_window) = app.get_webview_window("panel") {
        panel_window.set_shadow(false)?;
        panel_window.set_skip_taskbar(true)?;
        panel_window.set_focusable(false)?;
        panel_window.set_ignore_cursor_events(false)?;
        let _ = panel_window.hide();
    }
    Ok(())
}

fn show_panel_collapsed(
    app: &AppHandle,
    clear_hidden: bool,
    reset_position: bool,
) -> Result<(), String> {
    if clear_hidden {
        let state = app.state::<AppState>();
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.show();
    }

    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if let Some(panel_window) = app.get_webview_window("panel") {
        let _ = panel_window.hide();
        let _ = panel_window.set_focusable(false);
    }

    strip_window
        .set_skip_taskbar(true)
        .map_err(|error| error.to_string())?;
    strip_window
        .set_shadow(false)
        .map_err(|error| error.to_string())?;
    // Restore the normal rectangle before applying saved/clamped coordinates.
    // SetPosition while iconic moves the minimized icon, not that rectangle.
    strip_window.unminimize().map_err(|error| error.to_string())?;
    if reset_position {
        set_window_bounds_bottom_right(&strip_window, PANEL_WIDTH, PANEL_COLLAPSED_HEIGHT)
            .map_err(|error| error.to_string())?;
        let state = app.state::<AppState>();
        {
            if let Ok(mut panel) = state.panel.lock() {
                panel.open_down = false;
            }
        };
    } else {
        let open_down = restore_strip_bounds(app, &strip_window)?;
        let state = app.state::<AppState>();
        {
            if let Ok(mut panel) = state.panel.lock() {
                panel.open_down = open_down;
            }
        };
    }
    strip_window
        .set_focusable(false)
        .map_err(|error| error.to_string())?;
    strip_window
        .set_ignore_cursor_events(false)
        .map_err(|error| error.to_string())?;
    strip_window.set_always_on_top(true).map_err(|error| error.to_string())?;
    strip_window.show().map_err(|error| error.to_string())?;
    emit_panel_visibility(app)?;
    Ok(())
}

fn sync_panel_visibility(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (should_show, expanded) = {
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        (!panel.manual_hidden && !panel.auto_hidden, panel.expanded)
    };

    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(());
    };
    let panel_window = app.get_webview_window("panel");

    if should_show {
        strip_window
            .set_skip_taskbar(true)
            .map_err(|error| error.to_string())?;
        strip_window
            .set_shadow(false)
            .map_err(|error| error.to_string())?;
        strip_window.unminimize().map_err(|error| error.to_string())?;
        let open_down = restore_strip_bounds(app, &strip_window)?;
        if let Ok(mut panel) = state.panel.lock() {
            panel.open_down = open_down;
        }
        strip_window
            .set_focusable(false)
            .map_err(|error| error.to_string())?;
        strip_window
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        strip_window.set_always_on_top(true).map_err(|error| error.to_string())?;
        strip_window.show().map_err(|error| error.to_string())?;
        if let Some(panel_window) = panel_window {
            if expanded {
                let anchor = current_panel_anchor(&strip_window, open_down, expanded)
                    .map_err(|error| error.to_string())?;
                position_expanded_panel(
                    &strip_window,
                    &panel_window,
                    anchor.0,
                    anchor.1,
                    open_down,
                )
                .map_err(|error| error.to_string())?;
                panel_window.show().map_err(|error| error.to_string())?;
                panel_window
                    .set_focusable(true)
                    .map_err(|error| error.to_string())?;
            } else {
                panel_window.hide().map_err(|error| error.to_string())?;
                panel_window
                    .set_focusable(false)
                    .map_err(|error| error.to_string())?;
            }
        }
    } else {
        strip_window.hide().map_err(|error| error.to_string())?;
        if let Some(panel_window) = panel_window {
            panel_window.hide().map_err(|error| error.to_string())?;
        }
    }

    emit_panel_visibility(app)
}

fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_panel_menu(app.handle())?;
    let icon = app
        .default_window_icon()
        .cloned()
        .unwrap_or_else(|| Image::new(&[], 0, 0));

    TrayIconBuilder::with_id("main-tray")
        .tooltip("Deadline Panel")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        // Menu events already use Builder::on_menu_event. A second global
        // handler here executes each action twice, including restart.
        .on_tray_icon_event(|tray, event| {
            if matches!(event, TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. }) {
                let app = tray.app_handle();
                report_panel_result(app, show_panel_collapsed(app, true, false));
            }
        })
        .build(app)?;

    Ok(())
}

fn refresh_tray_menu(app: &AppHandle) {
    let Some(tray) = app.tray_by_id("main-tray") else {
        return;
    };
    if let Ok(menu) = build_panel_menu(app) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn build_panel_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let labels = native_menu_labels(app);
    let show = MenuItemBuilder::with_id("show_panel", labels.show_panel).build(app)?;
    let pause = MenuItemBuilder::with_id("pause_panel", labels.pause_panel).build(app)?;
    let hide_15 = MenuItemBuilder::with_id("hide_15", labels.hide_15).build(app)?;
    let hide_30 = MenuItemBuilder::with_id("hide_30", labels.hide_30).build(app)?;
    let hide_60 = MenuItemBuilder::with_id("hide_60", labels.hide_60).build(app)?;
    let reposition = MenuItemBuilder::with_id("reposition_panel", labels.reposition).build(app)?;
    let restart = MenuItemBuilder::with_id("restart", native_restart_label(app)).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", labels.quit).build(app)?;
    MenuBuilder::new(app)
        .item(&show)
        .item(&pause)
        .item(&hide_15)
        .item(&hide_30)
        .item(&hide_60)
        .item(&reposition)
        .separator()
        .item(&restart)
        .item(&quit)
        .build()
}

fn handle_panel_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show_panel" => {
            report_panel_result(app, show_panel_collapsed(app, true, false));
        }
        "hide_15" => {
            report_panel_result(app, hide_panel_for_duration(15, app));
        }
        "hide_30" => {
            report_panel_result(app, hide_panel_for_duration(30, app));
        }
        "hide_60" => {
            report_panel_result(app, hide_panel_for_duration(60, app));
        }
        "pause_panel" => {
            report_panel_result(app, hide_panel_temporarily(app.clone(), app.state::<AppState>()));
        }
        "reposition_panel" => {
            report_panel_result(app, reset_panel_position(app.clone()));
        }
        "restart" => restart_app(app.clone()),
        "quit" => app.exit(0),
        _ => {}
    }
}

#[tauri::command]
fn restart_app(app: AppHandle) {
    // Deliver RunEvent::Exit so the single-instance mutex/window is released
    // before Tauri starts the replacement process.
    app.request_restart();
}

fn start_long_runtime_watchdog(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(LONG_RUNTIME_RESTART_AFTER);
        loop {
            if should_restart_after_long_runtime(&app) {
                restart_app(app.clone());
                return;
            }
            thread::sleep(LONG_RUNTIME_RESTART_RETRY);
        }
    });
}

fn should_restart_after_long_runtime(app: &AppHandle) -> bool {
    let is_expanded = app
        .try_state::<AppState>()
        .and_then(|state| state.panel.lock().ok().map(|panel| panel.expanded))
        .unwrap_or(false);
    if is_expanded {
        return false;
    }

    #[cfg(windows)]
    if let Ok(pointer) = get_panel_pointer_state(app) {
        if pointer.in_window || pointer.left_down || pointer.right_down {
            return false;
        }
    }

    true
}

#[cfg(windows)]
fn normalized_windows_path(path: &std::path::Path) -> Option<String> {
    let text = path.to_str()?.to_string();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        Some(format!(r"\\{}", rest))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        Some(rest.to_string())
    } else {
        Some(text)
    }
}

#[cfg(windows)]
fn autostart_enabled(app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    let exe_path = current_exe_for_shell()?;
    Ok(autostart::read(AUTOSTART_REG_KEY, &autostart_value_name(app))?
        .is_some_and(|value| autostart::matches_executable(&value, &exe_path)))
}

#[cfg(not(windows))]
fn autostart_enabled(_app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(false)
}

#[cfg(windows)]
fn set_autostart_enabled_native(app: &AppHandle, enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    let value = enabled.then(|| current_exe_for_shell().map(|path| format!("\"{path}\""))).transpose()?;
    autostart::write(AUTOSTART_REG_KEY, &autostart_value_name(app), value.as_deref())
}

#[cfg(windows)]
fn autostart_value_name(app: &AppHandle) -> String {
    if app.config().identifier == "local.adhd-deadline-panel" { AUTOSTART_REG_VALUE.into() }
    else { format!("{} ({})", AUTOSTART_REG_VALUE, app.config().identifier) }
}

#[cfg(not(windows))]
fn set_autostart_enabled_native(_app: &AppHandle, _enabled: bool) -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}

#[cfg(windows)]
fn current_exe_for_shell() -> Result<String, Box<dyn std::error::Error>> {
    normalized_windows_path(&std::env::current_exe()?)
        .ok_or_else(|| "current executable path is not valid UTF-8".into())
}

fn native_menu_labels(app: &AppHandle) -> NativeMenuLabels {
    let language = resolved_native_language(app);

    match language.as_str() {
        "ja" => NativeMenuLabels {
            show_panel: "パネルを表示",
            pause_panel: "一時的に隠す",
            hide_15: "15分隠す",
            hide_30: "30分隠す",
            hide_60: "60分隠す",
            reposition: "右下に戻す",
            quit: "終了",
        },
        "en" => NativeMenuLabels {
            show_panel: "Show panel",
            pause_panel: "Hide temporarily",
            hide_15: "Hide 15 min",
            hide_30: "Hide 30 min",
            hide_60: "Hide 60 min",
            reposition: "Reset to lower-right",
            quit: "Quit",
        },
        _ => NativeMenuLabels {
            show_panel: "显示面板",
            pause_panel: "暂时隐藏",
            hide_15: "隐藏 15 分钟",
            hide_30: "隐藏 30 分钟",
            hide_60: "隐藏 60 分钟",
            reposition: "重新贴到右下角",
            quit: "退出",
        },
    }
}

fn native_restart_label(app: &AppHandle) -> &'static str {
    let language = resolved_native_language(app);

    match language.as_str() {
        "ja" => "再起動",
        "en" => "Restart",
        _ => "重启",
    }
}

fn resolved_native_language(app: &AppHandle) -> String {
    let language = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .db
                .lock()
                .ok()
                .and_then(|db| get_app_setting_value(&db, "app_language").ok().flatten())
        })
        .unwrap_or_else(|| "system".to_string());

    match language.as_str() {
        "zh" | "ja" | "en" => language,
        _ => system_native_language().to_string(),
    }
}

#[cfg(windows)]
fn system_native_language() -> &'static str {
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary_language = lang_id & 0x03ff;
    match primary_language {
        0x04 => "zh",
        0x11 => "ja",
        _ => "en",
    }
}

#[cfg(not(windows))]
fn system_native_language() -> &'static str {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    if locale.starts_with("zh") {
        "zh"
    } else if locale.starts_with("ja") {
        "ja"
    } else {
        "en"
    }
}

fn set_window_bounds_bottom_right(
    window: &tauri::WebviewWindow,
    logical_width: f64,
    logical_height: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };

    let work_area = monitor.work_area();
    let work_position = work_area.position;
    let work_size = work_area.size;
    let scale = monitor.scale_factor();
    let width = (logical_width * scale).round() as i32;
    let height = (logical_height * scale).round() as i32;
    let margin = (PANEL_MARGIN as f64 * scale).round() as i32;
    let x = work_position.x + work_size.width as i32 - width - margin;
    let y = work_position.y + work_size.height as i32 - height - margin;

    window.set_size(Size::Logical(LogicalSize {
        width: logical_width,
        height: logical_height,
    }))?;
    window.set_position(PhysicalPosition::new(x, y))?;
    Ok(())
}

fn set_strip_bounds_from_saved_or_bottom_right(
    window: &tauri::WebviewWindow,
    anchor: Option<(i32, i32)>,
) -> Result<bool, Box<dyn std::error::Error>> {
    if let Some((anchor_x, anchor_y)) = anchor {
        let monitors = window.available_monitors()?;
        // Saved coordinates can belong to a disconnected display. Only reuse
        // them on a current work area, keeping the complete strip on screen.
        if let Some(monitor) = monitors.iter().find(|monitor| {
            let area = monitor.work_area();
            anchor_x >= area.position.x && anchor_y >= area.position.y
                && i64::from(anchor_x) < i64::from(area.position.x) + i64::from(area.size.width)
                && i64::from(anchor_y) < i64::from(area.position.y) + i64::from(area.size.height)
        }) {
            let area = monitor.work_area();
            let (anchor_x, anchor_y) = clamp_strip_anchor(
                (anchor_x, anchor_y), (area.position.x, area.position.y),
                (area.size.width, area.size.height), monitor.scale_factor());
            window.set_size(Size::Logical(LogicalSize {
                width: PANEL_WIDTH,
                height: PANEL_COLLAPSED_HEIGHT,
            }))?;
            window.set_position(PhysicalPosition::new(anchor_x, anchor_y))?;
            return should_expand_panel_down(window, anchor_y);
        }
    }

    set_window_bounds_bottom_right(window, PANEL_WIDTH, PANEL_COLLAPSED_HEIGHT)?;
    Ok(false)
}

fn get_saved_panel_anchor_position(
    db: &Connection,
    legacy_offset: i32,
) -> Result<Option<(i32, i32)>, Box<dyn std::error::Error>> {
    if let Some(anchor) = get_saved_position_value(db, PANEL_ANCHOR_POSITION_SETTING_KEY)? {
        return Ok(Some(anchor));
    }

    let Some((x, y)) = get_saved_position_value(db, PANEL_POSITION_SETTING_KEY)? else {
        return Ok(None);
    };
    Ok(Some((x, y.saturating_add(legacy_offset))))
}

fn restore_strip_bounds(app: &AppHandle, window: &tauri::WebviewWindow) -> Result<bool, String> {
    let legacy_offset = collapsed_strip_offset(window).map_err(|error| error.to_string())?;
    let anchor = {
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|error| error.to_string())?;
        get_saved_panel_anchor_position(&db, legacy_offset).map_err(|error| error.to_string())?
    };
    // Release the database before any operation that dispatches to the UI.
    set_strip_bounds_from_saved_or_bottom_right(window, anchor).map_err(|error| error.to_string())
}

fn clamp_strip_anchor(anchor: (i32, i32), origin: (i32, i32), size: (u32, u32), scale: f64) -> (i32, i32) {
    let width = (PANEL_WIDTH * scale).round() as i64;
    let height = (PANEL_COLLAPSED_HEIGHT * scale).round() as i64;
    let max_x = (i64::from(origin.0) + i64::from(size.0) - width).max(i64::from(origin.0));
    let max_y = (i64::from(origin.1) + i64::from(size.1) - height).max(i64::from(origin.1));
    (i64::from(anchor.0).clamp(i64::from(origin.0), max_x) as i32,
     i64::from(anchor.1).clamp(i64::from(origin.1), max_y) as i32)
}

fn get_saved_position_value(
    db: &Connection,
    key: &str,
) -> Result<Option<(i32, i32)>, Box<dyn std::error::Error>> {
    let Some(value) = get_app_setting_value(db, key)? else {
        return Ok(None);
    };
    let Some((x_text, y_text)) = value.split_once(',') else {
        return Ok(None);
    };
    let Ok(x) = x_text.parse::<i32>() else {
        return Ok(None);
    };
    let Ok(y) = y_text.parse::<i32>() else {
        return Ok(None);
    };
    Ok(Some((x, y)))
}

fn collapsed_strip_offset(
    window: &tauri::WebviewWindow,
) -> Result<i32, Box<dyn std::error::Error>> {
    let scale = window.scale_factor()?;
    Ok(((PANEL_EXPANDED_HEIGHT - PANEL_COLLAPSED_HEIGHT) * scale).round() as i32)
}

fn collapsed_strip_height(
    window: &tauri::WebviewWindow,
) -> Result<i32, Box<dyn std::error::Error>> {
    let scale = window.scale_factor()?;
    Ok((PANEL_COLLAPSED_HEIGHT * scale).round() as i32)
}

fn expanded_panel_height(window: &tauri::WebviewWindow) -> Result<i32, Box<dyn std::error::Error>> {
    let scale = window.scale_factor()?;
    Ok((PANEL_EXPANDED_HEIGHT * scale).round() as i32)
}

fn min_panel_height(window: &tauri::WebviewWindow) -> Result<i32, Box<dyn std::error::Error>> {
    let scale = window.scale_factor()?;
    Ok((PANEL_MIN_HEIGHT * scale).round() as i32)
}

fn panel_gap(window: &tauri::WebviewWindow) -> Result<i32, Box<dyn std::error::Error>> {
    let scale = window.scale_factor()?;
    Ok((PANEL_GAP * scale).round() as i32)
}

fn current_panel_anchor(
    window: &tauri::WebviewWindow,
    _open_down: bool,
    _expanded: bool,
) -> Result<(i32, i32), Box<dyn std::error::Error>> {
    let position = window.outer_position()?;
    Ok((position.x, position.y))
}

fn position_expanded_panel(
    strip_window: &tauri::WebviewWindow,
    panel_window: &tauri::WebviewWindow,
    anchor_x: i32,
    anchor_y: i32,
    open_down: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(monitor) = strip_window.current_monitor()? else {
        return Ok(());
    };
    let scale = strip_window.scale_factor()?;
    let width = (PANEL_WIDTH * scale).round() as i32;
    let max_height = expanded_panel_height(strip_window)?;
    let min_height = min_panel_height(strip_window)?;
    let collapsed_height = collapsed_strip_height(strip_window)?;
    let gap = panel_gap(strip_window)?;
    let margin = (PANEL_MARGIN as f64 * scale).round() as i32;
    let work_area = monitor.work_area();
    let work_top = work_area.position.y + margin;
    let work_bottom = work_area.position.y + work_area.size.height as i32 - margin;
    let strip_top = anchor_y;
    let strip_bottom = anchor_y + collapsed_height;
    let down_y = strip_bottom + gap;
    let up_bottom = strip_top - gap;
    let available_down = (work_bottom - down_y).max(0);
    let available_up = (up_bottom - work_top).max(0);
    let available_height = if open_down {
        available_down
    } else {
        available_up
    };
    let height = max_height.min(available_height.max(min_height));
    let y = if open_down {
        down_y
    } else {
        up_bottom - height
    };

    #[cfg(windows)]
    {
        let flags = SWP_NOZORDER | SWP_NOACTIVATE;
        let hwnd = panel_window.hwnd()?;
        unsafe { SetWindowPos(hwnd, None, anchor_x, y, width, height, flags)? };
        return Ok(());
    }

    #[cfg(not(windows))]
    {
        panel_window.set_size(Size::Logical(LogicalSize {
            width: PANEL_WIDTH,
            height: height as f64 / scale,
        }))?;
        panel_window.set_position(PhysicalPosition::new(anchor_x, y))?;
        Ok(())
    }
}

fn should_expand_panel_down(
    window: &tauri::WebviewWindow,
    anchor_y: i32,
) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(false);
    };

    let work_area = monitor.work_area();
    let work_top = work_area.position.y;
    let work_bottom = work_area.position.y + work_area.size.height as i32;
    let scale = monitor.scale_factor();
    let margin = (PANEL_MARGIN as f64 * scale).round() as i32;
    let panel_height = expanded_panel_height(window)?;
    let collapsed_height = collapsed_strip_height(window)?;
    let gap = panel_gap(window)?;
    let required_above = panel_height + gap + margin;
    let required_below = panel_height + gap + margin;
    let available_above = anchor_y - work_top;
    let available_below = work_bottom - (anchor_y + collapsed_height);

    if available_above >= required_above {
        return Ok(false);
    }
    if available_below >= required_below {
        return Ok(true);
    }
    Ok(available_below > available_above)
}

#[cfg(windows)]
fn foreground_window_is_fullscreen(_app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    let foreground: HWND = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return Ok(false);
    }

    if unsafe { !IsWindowVisible(foreground).as_bool() || IsIconic(foreground).as_bool() } {
        return Ok(false);
    }

    if is_windows_shell_window(foreground) {
        return Ok(false);
    }

    if window_belongs_to_current_process(foreground) { return Ok(false); }

    if foreground_window_is_owned_by_explorer(foreground) {
        return Ok(false);
    }

    if window_is_desktop_assistant(foreground) {
        return Ok(false);
    }

    let monitor = unsafe { MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) };
    if monitor.0.is_null() {
        return Ok(false);
    }

    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { !GetMonitorInfoW(monitor, &mut monitor_info).as_bool() } {
        return Ok(false);
    }

    let mut window_rect = RECT::default();
    unsafe { GetWindowRect(foreground, &mut window_rect)? };

    let monitor_rect = monitor_info.rcMonitor;
    const TOLERANCE: i32 = 2;
    Ok(window_rect.left <= monitor_rect.left + TOLERANCE
        && window_rect.top <= monitor_rect.top + TOLERANCE
        && window_rect.right >= monitor_rect.right - TOLERANCE
        && window_rect.bottom >= monitor_rect.bottom - TOLERANCE)
}

#[cfg(windows)]
fn foreground_window_is_owned_by_explorer(hwnd: HWND) -> bool {
    window_process_name(hwnd).is_some_and(|name| name.eq_ignore_ascii_case("explorer.exe"))
}

#[cfg(windows)]
fn window_process_name(hwnd: HWND) -> Option<String> {
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    if process_id == 0 {
        return None;
    }

    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return None;
    };

    let mut buffer = [0u16; 260];
    let mut size = buffer.len() as u32;
    let result = unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_FORMAT(0),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut size,
        )
    };
    let _ = unsafe { windows::Win32::Foundation::CloseHandle(process) };

    if result.is_err() || size == 0 {
        return None;
    }

    let path = String::from_utf16_lossy(&buffer[..size as usize]);
    path.rsplit(['\\', '/']).next().map(ToOwned::to_owned)
}

#[cfg(windows)]
fn is_windows_shell_window(hwnd: HWND) -> bool {
    let Some(class_name) = window_class_name(hwnd) else {
        return false;
    };
    matches!(
        class_name.as_str(),
        "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd" | "Button"
    )
}

#[cfg(windows)]
fn window_class_name(hwnd: HWND) -> Option<String> {
    let mut buffer = [0u16; 256];
    let length = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if length <= 0 {
        return None;
    }

    Some(String::from_utf16_lossy(&buffer[..length as usize]))
}

#[cfg(windows)]
fn window_belongs_to_current_process(hwnd: HWND) -> bool {
    let mut process_id = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut process_id));
    }
    process_id != 0 && process_id == std::process::id()
}

#[cfg(windows)]
fn cursor_is_occluded_above_panel(panel_hwnd: HWND, cursor: POINT) -> bool {
    let mut hwnd = panel_hwnd;

    for _ in 0..128 {
        let Ok(previous) = (unsafe { GetWindow(hwnd, GW_HWNDPREV) }) else {
            return false;
        };
        hwnd = previous;

        if hwnd.0.is_null() || window_belongs_to_current_process(hwnd) {
            continue;
        }
        if should_ignore_window_occluder(hwnd) {
            continue;
        }
        if unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() } {
            continue;
        }

        let mut rect = RECT::default();
        if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() {
            continue;
        }
        if point_in_rect(cursor, rect) {
            return true;
        }
    }

    false
}

#[cfg(windows)]
fn should_ignore_window_occluder(hwnd: HWND) -> bool {
    is_windows_shell_window(hwnd)
        || window_is_desktop_assistant(hwnd)
        || (foreground_window_is_owned_by_explorer(hwnd) && !is_shell_overlay_window(hwnd))
}

#[cfg(windows)]
fn foreground_shell_overlay_covers_cursor(cursor: POINT) -> bool {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.0.is_null() || window_belongs_to_current_process(hwnd) {
        return false;
    }
    if unsafe { !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() } {
        return false;
    }

    let mut rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut rect) }.is_err() || !point_in_rect(cursor, rect) {
        return false;
    }

    is_shell_overlay_window(hwnd)
}

#[cfg(windows)]
fn is_shell_overlay_window(hwnd: HWND) -> bool {
    if is_windows_shell_window(hwnd) {
        return false;
    }

    let class_name = window_class_name(hwnd)
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();
    let process_name = window_process_name(hwnd)
        .map(|name| name.to_ascii_lowercase())
        .unwrap_or_default();

    matches!(
        process_name.as_str(),
        "shellexperiencehost.exe"
            | "startmenuexperiencehost.exe"
            | "searchhost.exe"
            | "textinputhost.exe"
    ) || (process_name == "explorer.exe"
        && (class_name.contains("notify")
            || class_name.contains("overflow")
            || class_name.contains("tray")
            || class_name.contains("xaml")
            || class_name.contains("corewindow")
            || class_name.contains("composition")
            || class_name.contains("flyout")
            || class_name.contains("popup")
            || class_name.contains("notification")
            || class_name.contains("clock")))
}

#[cfg(windows)]
fn window_is_desktop_assistant(hwnd: HWND) -> bool {
    window_process_name(hwnd).is_some_and(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "360desktoplite.exe"
                | "360desktoplite64.exe"
                | "360desktopservice.exe"
                | "360desktopservice64.exe"
        )
    })
}

#[cfg(windows)]
fn point_in_rect(point: POINT, rect: RECT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

#[cfg(not(windows))]
fn foreground_window_is_fullscreen(_app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(false)
}

#[cfg(windows)]
fn cursor_is_in_panel_trigger(app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(false);
    };

    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let panel_hwnd = window.hwnd()?;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };

    let left = position.x;
    let right = position.x + size.width as i32;
    let bottom = position.y + size.height as i32;
    let top = position.y;

    let raw_in_trigger =
        cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom;
    Ok(raw_in_trigger
        && !cursor_is_occluded_above_panel(panel_hwnd, cursor)
        && !foreground_shell_overlay_covers_cursor(cursor))
}

#[cfg(not(windows))]
fn cursor_is_in_panel_trigger(_app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(false)
}

#[cfg(windows)]
fn get_panel_pointer_state(
    app: &AppHandle,
) -> Result<PanelPointerState, Box<dyn std::error::Error>> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(PanelPointerState {
            in_trigger: false,
            in_window: false,
            expand_direction: "up",
            left_down: false,
            right_down: false,
            cursor_x: 0,
            cursor_y: 0,
            window_x: 0,
            window_y: 0,
        });
    };

    let open_down = app
        .try_state::<AppState>()
        .and_then(|state| state.panel.lock().ok().map(|panel| panel.open_down))
        .unwrap_or(false);
    let panel_hwnd = window.hwnd()?;
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };

    let left = position.x;
    let right = position.x + size.width as i32;
    let bottom = position.y + size.height as i32;
    let window_top = position.y;
    let in_strip =
        cursor.x >= left && cursor.x <= right && cursor.y >= window_top && cursor.y <= bottom;
    let in_panel = app
        .get_webview_window("panel")
        .and_then(|panel_window| cursor_in_window_rect(&panel_window, cursor).ok())
        .unwrap_or(false);
    let in_window = in_strip || in_panel;
    let raw_in_trigger =
        cursor.x >= left && cursor.x <= right && cursor.y >= window_top && cursor.y <= bottom;
    let in_trigger = raw_in_trigger
        && !cursor_is_occluded_above_panel(panel_hwnd, cursor)
        && !foreground_shell_overlay_covers_cursor(cursor);
    let left_down = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } < 0;
    let right_down = unsafe { GetAsyncKeyState(VK_RBUTTON.0 as i32) } < 0;

    Ok(PanelPointerState {
        in_trigger,
        in_window,
        expand_direction: if open_down { "down" } else { "up" },
        left_down,
        right_down,
        cursor_x: cursor.x,
        cursor_y: cursor.y,
        window_x: position.x,
        window_y: position.y,
    })
}

#[cfg(windows)]
fn cursor_in_window_rect(
    window: &tauri::WebviewWindow,
    cursor: POINT,
) -> Result<bool, Box<dyn std::error::Error>> {
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    Ok(cursor.x >= position.x
        && cursor.x <= position.x + size.width as i32
        && cursor.y >= position.y
        && cursor.y <= position.y + size.height as i32)
}

#[cfg(not(windows))]
fn get_panel_pointer_state(
    _app: &AppHandle,
) -> Result<PanelPointerState, Box<dyn std::error::Error>> {
    Ok(PanelPointerState {
        in_trigger: false,
        in_window: false,
        expand_direction: "up",
        left_down: false,
        right_down: false,
        cursor_x: 0,
        cursor_y: 0,
        window_x: 0,
        window_y: 0,
    })
}

#[cfg(windows)]
fn run_panel_drag_loop(
    app: AppHandle,
    hwnd_value: isize,
    start_window_x: i32,
    start_window_y: i32,
    start_cursor_x: i32,
    start_cursor_y: i32,
) {
    let hwnd = HWND(hwnd_value as *mut _);
    let mut final_x = start_window_x;
    let mut final_y = start_window_y;
    let flags = SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE;

    loop {
        let left_down = unsafe { GetAsyncKeyState(VK_LBUTTON.0 as i32) } < 0;
        if !left_down {
            break;
        }

        let mut cursor = POINT::default();
        if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
            final_x = start_window_x + (cursor.x - start_cursor_x);
            final_y = start_window_y + (cursor.y - start_cursor_y);
            let _ = unsafe { SetWindowPos(hwnd, None, final_x, final_y, 0, 0, flags) };
        }

        thread::sleep(Duration::from_millis(8));
    }

    let main_app = app.clone();
    let _ = app.run_on_main_thread(move || {
        let state = main_app.state::<AppState>();
        if let Ok(db) = state.db.lock() {
            let _ = set_app_setting_value(
                &db,
                PANEL_ANCHOR_POSITION_SETTING_KEY,
                &format!("{},{}", final_x, final_y),
            );
        }

        let next_open_down = if let Some(window) = main_app.get_webview_window("main") {
            should_expand_panel_down(&window, final_y).unwrap_or(false)
        } else {
            false
        };
        if let Some(panel_window) = main_app.get_webview_window("panel") {
            let _ = panel_window.hide();
            let _ = panel_window.set_focusable(false);
        }

        if let Ok(mut panel) = state.panel.lock() {
            panel.dragging = false;
            panel.open_down = next_open_down;
            panel.expanded = false;
        }

        if let Some(window) = main_app.get_webview_window("main") {
            if let Ok(current_position) = window.outer_position() {
                if current_position.x != final_x || current_position.y != final_y {
                    let _ = window.set_position(PhysicalPosition::new(final_x, final_y));
                }
            }
        }

        if let Some(window) = main_app.get_webview_window("main") {
            let _ = window.set_focusable(false);
            let _ = window.set_ignore_cursor_events(false);
        }
        let _ = emit_panel_visibility(&main_app);
    });
}

fn database_path(app: &AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Debug-only acceptance profile: never redirect a distributed build's data.
    #[cfg(debug_assertions)]
    if let Some(dir) = std::env::var_os("DEADLINE_PANEL_ACCEPTANCE_DATA_DIR") {
        let dir = PathBuf::from(dir);
        fs::create_dir_all(&dir)?;
        return Ok(dir.join("deadline-panel.sqlite3"));
    }
    let dir = app.path().app_data_dir()?;
    fs::create_dir_all(&dir)?;
    Ok(dir.join("deadline-panel.sqlite3"))
}

fn backup_database_file(
    db_path: &PathBuf,
    label: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let parent = db_path
        .parent()
        .ok_or_else(|| "database parent directory not found".to_string())?;
    let backup_dir = parent.join("backups");
    fs::create_dir_all(&backup_dir)?;
    let backup_path = backup_dir.join(format!(
        "deadline-panel-{}-{}.sqlite3",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos(),
        label
    ));
    let source = Connection::open(db_path)?;
    source.execute(
        "VACUUM INTO ?1",
        params![backup_path.to_string_lossy().to_string()],
    )?;
    Ok(backup_path.to_string_lossy().to_string())
}

fn chrono_like_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    seconds.to_string()
}

fn get_app_setting_value(db: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    Ok(db
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .ok())
}

fn set_app_setting_value(db: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    let now = chrono_like_now();
    db.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params![key, value, now],
    )?;
    Ok(())
}

fn configure_database(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA busy_timeout = 5000;
        PRAGMA foreign_keys = ON;
        ",
    )?;
    Ok(())
}

fn initialize_database(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS pending_deletes (namespace TEXT NOT NULL, task_id TEXT NOT NULL, PRIMARY KEY(namespace, task_id));")?;
    db.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS tasks (
          id TEXT PRIMARY KEY,
          title TEXT NOT NULL,
          due_at TEXT NOT NULL,
          priority TEXT NOT NULL CHECK (priority IN ('low', 'medium', 'high', 'urgent')),
          status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'postponed')),
          notes TEXT NOT NULL DEFAULT '',
          source TEXT NOT NULL DEFAULT 'manual',
          is_current INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL,
          updated_at TEXT NOT NULL,
          completed_at TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_status_due_at
        ON tasks (status, due_at);

        CREATE INDEX IF NOT EXISTS idx_tasks_due_priority
        ON tasks (due_at, priority);

        CREATE TABLE IF NOT EXISTS app_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL
        );
        ",
    )?;

    ensure_task_columns(db)?;
    seed_initial_tasks(db)?;
    migrate_seed_tasks_v2(db)
}

fn ensure_task_columns(db: &Connection) -> rusqlite::Result<()> {
    let mut statement = db.prepare("PRAGMA table_info(tasks)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut has_is_current = false;

    for row in rows {
        if row? == "is_current" {
            has_is_current = true;
            break;
        }
    }

    if !has_is_current {
        db.execute(
            "ALTER TABLE tasks ADD COLUMN is_current INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }

    Ok(())
}

fn seed_initial_tasks(db: &Connection) -> rusqlite::Result<()> {
    let seed_flag: Option<String> = db
        .query_row(
            "SELECT value FROM app_settings WHERE key = 'seed_tasks_v1'",
            [],
            |row| row.get(0),
        )
        .ok();

    if seed_flag.is_some() {
        return Ok(());
    }

    let task_count: i64 = db.query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))?;
    if task_count == 0 {
        for task in seed_task_specs() {
            let day_modifier = format!("+{} days", task.day_offset);
            db.execute(
                "INSERT INTO tasks (
                    id, title, due_at, priority, status, notes, source, is_current, created_at, updated_at, completed_at
                 ) VALUES (
                    ?1,
                    ?2,
                    date('now', 'localtime', ?3) || 'T' || ?4 || ':00',
                    ?5,
                    'active',
                    ?6,
                    'seed',
                    0,
                    strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime'),
                    strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime'),
                    NULL
                 )",
                params![
                    task.id,
                    task.title,
                    day_modifier,
                    task.due_time,
                    task.priority,
                    task.notes,
                ],
            )?;
        }
    }

    db.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        params!["seed_tasks_v1", "done", chrono_like_now()],
    )?;

    Ok(())
}

fn migrate_seed_tasks_v2(db: &Connection) -> rusqlite::Result<()> {
    if get_app_setting_value(db, SEED_TASKS_V2_SETTING_KEY)?.is_some() {
        return Ok(());
    }

    for task in seed_task_specs() {
        let day_modifier = format!("+{} days", task.day_offset);
        db.execute(
            "UPDATE tasks
             SET title = ?1,
                 due_at = date('now', 'localtime', ?2) || 'T' || ?3 || ':00',
                 priority = ?4,
                 notes = ?5,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%S', 'now', 'localtime')
             WHERE id = ?6 AND source = 'seed'",
            params![
                task.title,
                day_modifier,
                task.due_time,
                task.priority,
                task.notes,
                task.id
            ],
        )?;
    }

    set_app_setting_value(db, SEED_TASKS_V2_SETTING_KEY, "done")?;
    Ok(())
}

fn seed_task_specs() -> Vec<SeedTaskSpec> {
    vec![
        SeedTaskSpec {
            id: "seed-graph-mining-quiz",
            title: "示例：课程小测",
            day_offset: 1,
            due_time: "23:59",
            priority: "high",
            notes: "示例任务，可直接修改或删除",
        },
        SeedTaskSpec {
            id: "seed-lab-report",
            title: "示例：提交报告",
            day_offset: 3,
            due_time: "18:00",
            priority: "medium",
            notes: "示例任务，可直接修改或删除",
        },
        SeedTaskSpec {
            id: "seed-paper-reading",
            title: "示例：阅读材料",
            day_offset: 7,
            due_time: "23:59",
            priority: "medium",
            notes: "示例任务，可直接修改或删除",
        },
    ]
}

#[allow(dead_code)]
fn default_tasks() -> Vec<DeadlineTask> {
    vec![
        DeadlineTask {
            id: "seed-graph-mining-quiz".into(),
            title: "示例：课程小测".into(),
            due_at: "2026-06-23T23:59:00+09:00".into(),
            priority: "high".into(),
            status: "active".into(),
            notes: "示例任务，可直接修改或删除".into(),
            source: "seed".into(),
            is_current: false,
            created_at: "2026-06-21T09:00:00+09:00".into(),
            updated_at: "2026-06-21T09:00:00+09:00".into(),
            completed_at: None,
        },
        DeadlineTask {
            id: "seed-lab-report".into(),
            title: "示例：提交报告".into(),
            due_at: "2026-06-25T18:00:00+09:00".into(),
            priority: "medium".into(),
            status: "active".into(),
            notes: "示例任务，可直接修改或删除".into(),
            source: "seed".into(),
            is_current: false,
            created_at: "2026-06-21T09:05:00+09:00".into(),
            updated_at: "2026-06-21T09:05:00+09:00".into(),
            completed_at: None,
        },
        DeadlineTask {
            id: "seed-paper-reading".into(),
            title: "示例：阅读材料".into(),
            due_at: "2026-06-28T23:59:00+09:00".into(),
            priority: "medium".into(),
            status: "active".into(),
            notes: "示例任务，可直接修改或删除".into(),
            source: "seed".into(),
            is_current: false,
            created_at: "2026-06-21T09:10:00+09:00".into(),
            updated_at: "2026-06-21T09:10:00+09:00".into(),
            completed_at: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_anchor_stays_inside_current_work_area() {
        assert_eq!(clamp_strip_anchor((1900, 1030), (0, 0), (1920, 1040), 1.5), (1362, 974));
        assert_eq!(clamp_strip_anchor((-5, -5), (0, 0), (1920, 1040), 1.0), (0, 0));
        assert_eq!(clamp_strip_anchor((-20, 999), (-1920, 0), (1920, 1080), 1.0), (-372, 999));
        assert_eq!(clamp_strip_anchor((100, 100), (0, 0), (200, 30), 1.0), (0, 0));
    }

    #[test]
    fn fullscreen_preference_defaults_and_explicit_disable() {
        assert!(fullscreen_auto_hide_enabled("{}"));
        assert!(fullscreen_auto_hide_enabled("invalid"));
        assert!(!fullscreen_auto_hide_enabled(r#"{"fullscreenAutoHide":false}"#));
        assert!(fullscreen_auto_hide_enabled(r#"{"fullscreenAutoHide":true}"#));
    }

    #[test]
    fn initialize_database_seeds_once() {
        let db = Connection::open_in_memory().expect("open in-memory database");

        initialize_database(&db).expect("initialize database");
        let first_count: i64 = db
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .expect("count seeded tasks");
        assert_eq!(first_count, 3);

        initialize_database(&db).expect("initialize database again");
        let second_count: i64 = db
            .query_row("SELECT COUNT(*) FROM tasks", [], |row| row.get(0))
            .expect("count seeded tasks after second init");
        assert_eq!(second_count, 3);
    }
    #[test]
    fn deletion_outbox_survives_reopen_and_is_atomic() {
        let path = std::env::temp_dir().join(format!(
            "deadline-outbox-test-{}-{}.sqlite3",
            std::process::id(),
            chrono_like_now()
        ));
        {
            let mut db = Connection::open(&path).unwrap();
            initialize_database(&db).unwrap();
            let id: String = db
                .query_row("SELECT id FROM tasks LIMIT 1", [], |row| row.get(0))
                .unwrap();
            db.execute_batch("CREATE TRIGGER block_delete BEFORE DELETE ON tasks BEGIN SELECT RAISE(ABORT, 'test failure'); END;").unwrap();
            assert!(persist_delete(&mut db, &id, Some("account-a")).is_err());
            let count: i64 = db
                .query_row("SELECT COUNT(*) FROM pending_deletes", [], |row| row.get(0))
                .unwrap();
            assert_eq!(count, 0, "failed delete must roll back the outbox insert");
            db.execute_batch("DROP TRIGGER block_delete").unwrap();
            persist_delete(&mut db, &id, Some("account-a")).unwrap();
        }
        {
            let db = Connection::open(&path).unwrap();
            let count: i64 = db
                .query_row(
                    "SELECT COUNT(*) FROM pending_deletes WHERE namespace = 'account-a'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);
            let count: i64 = db
                .query_row(
                    "SELECT COUNT(*) FROM pending_deletes WHERE namespace = 'account-b'",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 0);
        }
        std::fs::remove_file(path).unwrap();
    }
    fn synthetic_task(id: &str, title: &str) -> DeadlineTask {
        serde_json::from_value(serde_json::json!({"id":id,"title":title,"dueAt":"2026-12-01T00:00:00Z","priority":"medium","status":"active","notes":"","source":"manual","isCurrent":false,"createdAt":"2026-10-03T00:00:00Z","updatedAt":"2026-10-03T00:00:00Z","completedAt":null})).unwrap()
    }

    #[test]
    fn batch_failure_rolls_back_all_tasks() {
        let mut db = Connection::open_in_memory().unwrap();
        initialize_database(&db).unwrap();
        db.execute_batch("CREATE TRIGGER reject_synthetic BEFORE INSERT ON tasks WHEN NEW.title = 'reject' BEGIN SELECT RAISE(ABORT, 'synthetic failure'); END;").unwrap();
        let tasks = vec![
            synthetic_task("batch-first", "accept"),
            synthetic_task("batch-second", "reject"),
        ];
        assert!(persist_tasks(&mut db, &tasks).is_err());
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM tasks WHERE id LIKE 'batch-%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn backup_includes_uncheckpointed_wal() {
        let dir = std::env::temp_dir().join(format!(
            "deadline-backup-test-{}-{}",
            std::process::id(),
            chrono_like_now()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("source.sqlite3");
        let db = Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0; CREATE TABLE evidence(value TEXT); INSERT INTO evidence VALUES ('in-wal');").unwrap();
        let backup = backup_database_file(&path, "test").unwrap();
        {
            let copied = Connection::open(&backup).unwrap();
            let value: String = copied
                .query_row("SELECT value FROM evidence", [], |row| row.get(0))
                .unwrap();
            assert_eq!(value, "in-wal");
        }
        drop(db);
        std::fs::remove_file(backup).unwrap();
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir.join("backups")).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn independent_patches_preserve_other_task_fields() {
        let db = Connection::open_in_memory().unwrap();
        initialize_database(&db).unwrap();
        write_task(&db, &synthetic_task("partial", "initial")).unwrap();
        patch_existing_task(&db, "partial", serde_json::json!({"title":"new title"})).unwrap();
        let updated =
            patch_existing_task(&db, "partial", serde_json::json!({"notes":"new notes"})).unwrap();
        assert_eq!(updated.title, "new title");
        assert_eq!(updated.notes, "new notes");
        assert!(
            patch_existing_task(&db, "partial", serde_json::json!({"id":"different"})).is_err()
        );
    }

    #[test]
    fn appearance_changes_preserve_other_preferences() {
        let db = Connection::open_in_memory().unwrap();
        initialize_database(&db).unwrap();
        merge_appearance_preferences(&db, serde_json::json!({"theme":"light"})).unwrap();
        let value =
            merge_appearance_preferences(&db, serde_json::json!({"lowPowerMode":true})).unwrap();
        let current: serde_json::Value = serde_json::from_str(&value).unwrap();
        assert_eq!(current["theme"], "light");
        assert_eq!(current["lowPowerMode"], true);
        assert!(merge_appearance_preferences(&db, serde_json::json!({"theme":"invalid"})).is_err());
    }
}
