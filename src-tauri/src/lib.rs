use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{
    image::Image,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    ActivationPolicy, AppHandle, LogicalSize, Manager, PhysicalPosition, Size,
};

#[cfg(target_os = "macos")]
use core_foundation::{
    base::{CFType, TCFType},
    dictionary::{CFDictionary, CFDictionaryRef},
    number::CFNumber,
    string::CFStringRef,
};
#[cfg(target_os = "macos")]
use core_graphics::{
    display::CGDisplay,
    geometry::CGRect,
    window::{
        copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowLayer,
        kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerPID,
    },
};
#[cfg(target_os = "macos")]
use tauri_plugin_autostart::ManagerExt;

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
const PANEL_EXPANDED_HEIGHT: f64 = 600.0;
const PANEL_MIN_HEIGHT: f64 = 180.0;
const PANEL_COLLAPSED_HEIGHT: f64 = 44.0;
const PANEL_GAP: f64 = 10.0;
const PANEL_MARGIN: i32 = 18;
const PANEL_POSITION_SETTING_KEY: &str = "panel_position";
const PANEL_ANCHOR_POSITION_SETTING_KEY: &str = "panel_anchor_position_v2";
const SEED_TASKS_V2_SETTING_KEY: &str = "seed_tasks_v2";
#[cfg(windows)]
const AUTOSTART_REG_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const AUTOSTART_REG_VALUE: &str = "Deadline Panel";

struct AppState {
    db: Mutex<Connection>,
    db_path: PathBuf,
    panel: Mutex<PanelState>,
}

struct PanelState {
    manual_hidden: bool,
    auto_hidden: bool,
    expanded: bool,
    dragging: bool,
    open_down: bool,
    hide_token: u64,
    dock_visible: bool,
}

struct NativeMenuLabels {
    show_panel: &'static str,
    pause_panel: &'static str,
    hide_15: &'static str,
    hide_30: &'static str,
    hide_60: &'static str,
    reposition: &'static str,
    show_dock: &'static str,
    hide_dock: &'static str,
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
}

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct WindowRect {
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
}

#[tauri::command]
fn list_tasks(state: tauri::State<'_, AppState>) -> Result<Vec<DeadlineTask>, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
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

#[tauri::command]
fn save_task(
    task: DeadlineTask,
    state: tauri::State<'_, AppState>,
) -> Result<DeadlineTask, String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
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
    .map_err(|error| error.to_string())?;

    Ok(task)
}

#[tauri::command]
fn delete_task(id: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let db = state.db.lock().map_err(|error| error.to_string())?;
    db.execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
fn replace_tasks(
    tasks: Vec<DeadlineTask>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let mut db = state.db.lock().map_err(|error| error.to_string())?;
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

    tx.commit().map_err(|error| error.to_string())
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

    if key == "app_language" {
        refresh_tray_menu(&app);
    }

    Ok(())
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
        return Ok(PanelExpandResult { direction: "up" });
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
    })
}

#[tauri::command]
fn set_panel_expanded(
    expanded: bool,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<PanelExpandResult, String> {
    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(PanelExpandResult { direction: "up" });
    };
    let Some(panel_window) = app.get_webview_window("panel") else {
        return Ok(PanelExpandResult { direction: "up" });
    };

    let (previous_open_down, was_expanded) = {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        let previous_open_down = panel.open_down;
        let was_expanded = panel.expanded;
        panel.expanded = expanded;
        (previous_open_down, was_expanded)
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
        apply_panel_workspace_behavior(&panel_window).map_err(|error| error.to_string())?;
        panel_window
            .set_shadow(panel_window_shadow_enabled())
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

    Ok(PanelExpandResult {
        direction: if open_down { "down" } else { "up" },
    })
}

#[tauri::command]
fn hide_panel_temporarily(app: AppHandle, state: tauri::State<'_, AppState>) -> Result<(), String> {
    {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.hide_token = panel.hide_token.wrapping_add(1);
        panel.manual_hidden = true;
        panel.expanded = false;
        panel.open_down = false;
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
        panel.hide_token = panel.hide_token.wrapping_add(1);
        panel.manual_hidden = true;
        panel.expanded = false;
        panel.open_down = false;
        panel.hide_token
    };
    sync_panel_visibility(app)?;

    let app = app.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(minutes.saturating_mul(60)));
        let state = app.state::<AppState>();
        if let Ok(mut panel) = state.panel.lock() {
            if panel.hide_token != hide_token {
                return;
            }
            panel.manual_hidden = false;
            panel.expanded = false;
            panel.open_down = false;
        }
        let _ = sync_panel_visibility(&app);
    });

    Ok(())
}

#[tauri::command]
fn set_panel_auto_hidden(
    hidden: bool,
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.auto_hidden = hidden;
        if hidden {
            panel.expanded = false;
            panel.open_down = false;
        }
    }
    sync_panel_visibility(&app)
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
fn panel_hover_polling_enabled() -> bool {
    cfg!(target_os = "macos")
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

#[cfg(windows)]
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

#[cfg(target_os = "macos")]
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
        let _ = panel_window.set_focusable(false);
    }

    window
        .set_ignore_cursor_events(false)
        .map_err(|error| error.to_string())?;
    window
        .set_focusable(true)
        .map_err(|error| error.to_string())?;
    window.start_dragging().map_err(|error| error.to_string())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
#[tauri::command]
fn start_panel_drag(app: AppHandle) -> Result<(), String> {
    finish_panel_drag_stub(&app)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn finish_panel_drag_stub(app: &AppHandle) -> Result<(), String> {
    if let Some(panel_window) = app.get_webview_window("panel") {
        let _ = panel_window.hide();
        let _ = panel_window.set_focusable(false);
    }

    let state = app.state::<AppState>();
    let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
    panel.dragging = false;
    panel.expanded = false;
    Ok(())
}

#[cfg(target_os = "macos")]
fn finish_panel_drag_macos(app: &AppHandle) -> Result<(), String> {
    if let Some(panel_window) = app.get_webview_window("panel") {
        let _ = panel_window.hide();
        let _ = panel_window.set_focusable(false);
    }

    let Some(window) = app.get_webview_window("main") else {
        let state = app.state::<AppState>();
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.dragging = false;
        panel.expanded = false;
        return Ok(());
    };

    let position = window.outer_position().map_err(|error| error.to_string())?;
    let open_down = should_expand_panel_down(&window, position.y).unwrap_or(false);

    let state = app.state::<AppState>();
    if let Ok(db) = state.db.lock() {
        let _ = set_app_setting_value(
            &db,
            PANEL_ANCHOR_POSITION_SETTING_KEY,
            &format!("{},{}", position.x, position.y),
        );
    }
    if let Ok(mut panel) = state.panel.lock() {
        panel.dragging = false;
        panel.open_down = open_down;
        panel.expanded = false;
    }

    let _ = window.set_focusable(false);
    let _ = window.set_ignore_cursor_events(false);
    Ok(())
}

#[tauri::command]
fn finish_panel_drag(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        finish_panel_drag_macos(&app)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(())
    }
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
fn set_autostart_enabled(enabled: bool, app: AppHandle) -> Result<bool, String> {
    set_autostart_enabled_native(&app, enabled).map_err(|error| error.to_string())?;
    autostart_enabled(&app).map_err(|error| error.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(desktop)]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = show_panel_collapsed(app, true, false);
        }));
        builder = builder.plugin(tauri_plugin_process::init());
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .on_menu_event(|app, event| {
            handle_panel_menu_event(app, event.id().as_ref());
        })
        .setup(|app| {
            let db_path = database_path(app.handle())?;
            let db = Connection::open(&db_path).map_err(|error| error.to_string())?;
            configure_database(&db).map_err(|error| error.to_string())?;
            initialize_database(&db).map_err(|error| error.to_string())?;
            position_main_window(app, &db)?;
            app.manage(AppState {
                db: Mutex::new(db),
                db_path,
                panel: Mutex::new(PanelState {
                    manual_hidden: false,
                    auto_hidden: false,
                    expanded: false,
                    dragging: false,
                    open_down: false,
                    hide_token: 0,
                    dock_visible: true,
                }),
            });
            #[cfg(desktop)]
            app.handle().plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ))?;
            setup_tray(app)?;
            show_panel_collapsed(app.handle(), false, false).map_err(|error| error.to_string())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_tasks,
            save_task,
            delete_task,
            replace_tasks,
            get_app_setting,
            set_app_setting,
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
            panel_hover_polling_enabled,
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

    strip_window.set_shadow(panel_window_shadow_enabled())?;
    strip_window.set_skip_taskbar(true)?;
    apply_panel_workspace_behavior(&strip_window)?;
    let _ = set_strip_bounds_from_saved_or_bottom_right(&strip_window, db)?;
    strip_window.set_focusable(false)?;
    strip_window.set_ignore_cursor_events(false)?;

    if let Some(panel_window) = app.get_webview_window("panel") {
        panel_window.set_shadow(panel_window_shadow_enabled())?;
        panel_window.set_skip_taskbar(true)?;
        apply_panel_workspace_behavior(&panel_window)?;
        panel_window.set_focusable(false)?;
        panel_window.set_ignore_cursor_events(false)?;
        let _ = panel_window.hide();
    }
    Ok(())
}

fn panel_window_shadow_enabled() -> bool {
    cfg!(target_os = "macos")
}

fn apply_panel_workspace_behavior(
    window: &tauri::WebviewWindow,
) -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "macos")]
    {
        window.set_visible_on_all_workspaces(true)?;
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
        panel.manual_hidden = false;
        panel.auto_hidden = false;
        panel.expanded = false;
        panel.open_down = false;
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
    apply_panel_workspace_behavior(&strip_window).map_err(|error| error.to_string())?;
    strip_window.show().map_err(|error| error.to_string())?;
    strip_window
        .set_shadow(panel_window_shadow_enabled())
        .map_err(|error| error.to_string())?;
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
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let open_down = set_strip_bounds_from_saved_or_bottom_right(&strip_window, &db)
            .map_err(|error| error.to_string())?;
        drop(db);
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
        apply_panel_workspace_behavior(&strip_window).map_err(|error| error.to_string())?;
        strip_window.show().map_err(|error| error.to_string())?;
        strip_window
            .set_shadow(panel_window_shadow_enabled())
            .map_err(|error| error.to_string())?;
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let open_down = set_strip_bounds_from_saved_or_bottom_right(&strip_window, &db)
            .map_err(|error| error.to_string())?;
        drop(db);
        if let Ok(mut panel) = state.panel.lock() {
            panel.open_down = open_down;
        }
        strip_window
            .set_focusable(false)
            .map_err(|error| error.to_string())?;
        strip_window
            .set_ignore_cursor_events(false)
            .map_err(|error| error.to_string())?;
        if let Some(panel_window) = panel_window {
            if expanded {
                apply_panel_workspace_behavior(&panel_window).map_err(|error| error.to_string())?;
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

    Ok(())
}

fn setup_tray(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let menu = build_panel_menu(app.handle())?;
    let icon = load_menu_bar_icon().unwrap_or_else(|_| {
        app.default_window_icon()
            .cloned()
            .unwrap_or_else(|| Image::new(&[], 0, 0))
    });

    TrayIconBuilder::with_id("main-tray")
        .tooltip("Deadline Panel")
        .icon(icon)
        .menu(&menu)
        .icon_as_template(cfg!(target_os = "macos"))
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| {
            handle_panel_menu_event(app, event.id().as_ref());
        })
        .build(app)?;

    Ok(())
}

fn load_menu_bar_icon() -> Result<Image<'static>, Box<dyn std::error::Error>> {
    let bytes = include_bytes!("../icons/menuBarIcon.png");
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer)?;
    let pixels = &buffer[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => pixels
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect(),
        png::ColorType::Grayscale => pixels
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect(),
        png::ColorType::Indexed => return Err("indexed menu bar icon was not expanded".into()),
    };
    Ok(Image::new_owned(rgba, info.width, info.height))
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
    #[cfg(target_os = "macos")]
    let dock = {
        let dock_visible = app
            .try_state::<AppState>()
            .and_then(|state| state.panel.lock().ok().map(|panel| panel.dock_visible))
            .unwrap_or(true);
        let label = if dock_visible {
            labels.hide_dock
        } else {
            labels.show_dock
        };
        MenuItemBuilder::with_id("toggle_dock_icon", label).build(app)?
    };
    let restart = MenuItemBuilder::with_id("restart", native_restart_label(app)).build(app)?;
    let quit = MenuItemBuilder::with_id("quit", labels.quit).build(app)?;
    let mut builder = MenuBuilder::new(app)
        .item(&show)
        .item(&pause)
        .item(&hide_15)
        .item(&hide_30)
        .item(&hide_60)
        .item(&reposition);
    #[cfg(target_os = "macos")]
    {
        builder = builder.item(&dock);
    }
    builder.separator().item(&restart).item(&quit).build()
}

fn handle_panel_menu_event(app: &AppHandle, id: &str) {
    match id {
        "show_panel" => {
            let _ = show_panel_collapsed(app, true, false);
        }
        "hide_15" => {
            let _ = hide_panel_for_duration(15, app);
        }
        "hide_30" => {
            let _ = hide_panel_for_duration(30, app);
        }
        "hide_60" => {
            let _ = hide_panel_for_duration(60, app);
        }
        "pause_panel" => {
            let state = app.state::<AppState>();
            if let Ok(mut panel) = state.panel.lock() {
                panel.hide_token = panel.hide_token.wrapping_add(1);
                panel.manual_hidden = true;
                panel.expanded = false;
            }
            let _ = sync_panel_visibility(app);
        }
        "reposition_panel" => {
            let _ = reset_panel_position(app.clone());
        }
        "toggle_dock_icon" => {
            let _ = toggle_dock_icon(app);
        }
        "restart" => restart_app(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

#[cfg(target_os = "macos")]
fn toggle_dock_icon(app: &AppHandle) -> Result<(), String> {
    let next_visible = {
        let state = app.state::<AppState>();
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        !panel.dock_visible
    };

    set_macos_dock_icon_visible(app, next_visible)?;

    {
        let state = app.state::<AppState>();
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.dock_visible = next_visible;
    }
    refresh_tray_menu(app);
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_macos_dock_icon_visible(app: &AppHandle, visible: bool) -> Result<(), String> {
    let policy = if visible {
        ActivationPolicy::Regular
    } else {
        ActivationPolicy::Accessory
    };
    app.set_activation_policy(policy)
        .map_err(|error| error.to_string())?;
    app.set_dock_visibility(visible)
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn toggle_dock_icon(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}

fn restart_app(app: &AppHandle) {
    app.restart();
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
fn autostart_enabled(_app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    let exe_path = current_exe_for_shell()?;
    let output = Command::new("reg")
        .args(["query", AUTOSTART_REG_KEY, "/v", AUTOSTART_REG_VALUE])
        .output()?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    Ok(stdout.contains(&exe_path.to_ascii_lowercase()))
}

#[cfg(target_os = "macos")]
fn autostart_enabled(app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(app.autolaunch().is_enabled()?)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn autostart_enabled(_app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(false)
}

#[cfg(windows)]
fn set_autostart_enabled_native(
    app: &AppHandle,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if enabled {
        let value = format!("\"{}\"", current_exe_for_shell()?);
        let status = Command::new("reg")
            .args([
                "add",
                AUTOSTART_REG_KEY,
                "/v",
                AUTOSTART_REG_VALUE,
                "/t",
                "REG_SZ",
                "/d",
                value.as_str(),
                "/f",
            ])
            .status()?;

        if !status.success() {
            return Err("failed to write startup registry value".into());
        }
    } else {
        let status = Command::new("reg")
            .args(["delete", AUTOSTART_REG_KEY, "/v", AUTOSTART_REG_VALUE, "/f"])
            .status()?;

        if !status.success() && autostart_enabled(app)? {
            return Err("failed to remove startup registry value".into());
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn set_autostart_enabled_native(
    app: &AppHandle,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if enabled {
        app.autolaunch().enable()?;
    } else {
        app.autolaunch().disable()?;
    }
    Ok(())
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn set_autostart_enabled_native(
    _app: &AppHandle,
    _enabled: bool,
) -> Result<(), Box<dyn std::error::Error>> {
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
            show_dock: "Dockに表示",
            hide_dock: "Dockから隠す",
            quit: "終了",
        },
        "en" => NativeMenuLabels {
            show_panel: "Show panel",
            pause_panel: "Hide temporarily",
            hide_15: "Hide 15 min",
            hide_30: "Hide 30 min",
            hide_60: "Hide 60 min",
            reposition: "Reset to lower-right",
            show_dock: "Show in Dock",
            hide_dock: "Hide from Dock",
            quit: "Quit",
        },
        _ => NativeMenuLabels {
            show_panel: "显示面板",
            pause_panel: "暂时隐藏",
            hide_15: "隐藏 15 分钟",
            hide_30: "隐藏 30 分钟",
            hide_60: "隐藏 60 分钟",
            reposition: "重新贴到右下角",
            show_dock: "在程序坞显示",
            hide_dock: "从程序坞隐藏",
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
        _ => system_native_language(),
    }
}

#[cfg(windows)]
fn system_native_language() -> String {
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary_language = lang_id & 0x03ff;
    match primary_language {
        0x04 => "zh",
        0x11 => "ja",
        _ => "en",
    }
    .to_string()
}

#[cfg(target_os = "macos")]
fn system_native_language() -> String {
    let apple_languages = Command::new("defaults")
        .args(["read", "-g", "AppleLanguages"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
        .unwrap_or_default();

    macos_language_from_defaults_output(&apple_languages).unwrap_or_else(fallback_native_language)
}

#[cfg(target_os = "macos")]
fn macos_language_from_defaults_output(output: &str) -> Option<String> {
    output
        .lines()
        .map(|line| {
            line.trim()
                .trim_matches(',')
                .trim_matches('"')
                .trim_matches('\'')
                .to_ascii_lowercase()
        })
        .find_map(|locale| normalized_native_language(&locale))
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn system_native_language() -> String {
    fallback_native_language()
}

#[cfg(not(windows))]
fn fallback_native_language() -> String {
    let locale = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default()
        .to_ascii_lowercase();
    normalized_native_language(&locale).unwrap_or_else(|| "en".to_string())
}

#[cfg(not(windows))]
fn normalized_native_language(locale: &str) -> Option<String> {
    if locale.starts_with("zh") {
        Some("zh".to_string())
    } else if locale.starts_with("ja") {
        Some("ja".to_string())
    } else if locale.starts_with("en") {
        Some("en".to_string())
    } else {
        None
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
    db: &Connection,
) -> Result<bool, Box<dyn std::error::Error>> {
    if let Some((anchor_x, anchor_y)) = get_saved_panel_anchor_position(window, db)? {
        let open_down = should_expand_panel_down(window, anchor_y)?;
        window.set_size(Size::Logical(LogicalSize {
            width: PANEL_WIDTH,
            height: PANEL_COLLAPSED_HEIGHT,
        }))?;
        window.set_position(PhysicalPosition::new(anchor_x, anchor_y))?;
        return Ok(open_down);
    }

    set_window_bounds_bottom_right(window, PANEL_WIDTH, PANEL_COLLAPSED_HEIGHT)?;
    Ok(false)
}

fn get_saved_panel_anchor_position(
    window: &tauri::WebviewWindow,
    db: &Connection,
) -> Result<Option<(i32, i32)>, Box<dyn std::error::Error>> {
    if let Some(anchor) = get_saved_position_value(db, PANEL_ANCHOR_POSITION_SETTING_KEY)? {
        return Ok(Some(anchor));
    }

    let Some((x, y)) = get_saved_position_value(db, PANEL_POSITION_SETTING_KEY)? else {
        return Ok(None);
    };
    Ok(Some((x, y + collapsed_strip_offset(window)?)))
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
        let width = (PANEL_WIDTH * scale).round() as i32;
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
fn foreground_window_is_fullscreen(app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
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

    if foreground_window_is_owned_by_explorer(foreground) {
        return Ok(false);
    }

    if window_is_desktop_assistant(foreground) {
        return Ok(false);
    }

    if let Some(window) = app.get_webview_window("main") {
        if let Ok(app_hwnd) = window.hwnd() {
            if app_hwnd == foreground {
                return Ok(false);
            }
        }
    }
    if let Some(window) = app.get_webview_window("panel") {
        if let Ok(app_hwnd) = window.hwnd() {
            if app_hwnd == foreground {
                return Ok(false);
            }
        }
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

#[cfg(target_os = "macos")]
fn foreground_window_is_fullscreen(app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    let Some(window_info) = copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
    ) else {
        return Ok(false);
    };

    let display_bounds = macos_display_bounds();
    if display_bounds.is_empty() {
        return Ok(false);
    }

    let current_pid = std::process::id() as i32;
    let own_window_rects = macos_own_window_rects(app);

    for value in window_info.get_all_values() {
        let dict: CFDictionary<*const std::ffi::c_void, CFType> =
            unsafe { TCFType::wrap_under_get_rule(value as CFDictionaryRef) };
        let layer = macos_cf_number_i32(&dict, unsafe { kCGWindowLayer }).unwrap_or(0);
        if layer != 0 {
            continue;
        }

        let owner_pid = macos_cf_number_i32(&dict, unsafe { kCGWindowOwnerPID }).unwrap_or_default();
        if owner_pid == current_pid {
            continue;
        }

        let Some(rect) = macos_cf_rect(&dict, unsafe { kCGWindowBounds }) else {
            continue;
        };
        if macos_rect_is_too_small_for_foreground(rect)
            || macos_rect_matches_any(rect, &own_window_rects)
        {
            continue;
        }

        return Ok(macos_rect_matches_any_display(rect, &display_bounds));
    }

    Ok(false)
}

#[cfg(target_os = "macos")]
fn macos_display_bounds() -> Vec<CGRect> {
    CGDisplay::active_displays()
        .map(|displays| {
            displays
                .into_iter()
                .map(|display_id| CGDisplay::new(display_id).bounds())
                .collect()
        })
        .unwrap_or_else(|_| vec![CGDisplay::main().bounds()])
}

#[cfg(target_os = "macos")]
fn macos_own_window_rects(app: &AppHandle) -> Vec<CGRect> {
    ["main", "panel"]
        .iter()
        .filter_map(|label| app.get_webview_window(label))
        .filter_map(|window| webview_window_rect(&window).ok())
        .map(|rect| {
            CGRect::new(
                &core_graphics::geometry::CGPoint::new(rect.left, rect.top),
                &core_graphics::geometry::CGSize::new(
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                ),
            )
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn macos_cf_number_i32(
    dict: &CFDictionary<*const std::ffi::c_void, CFType>,
    key: CFStringRef,
) -> Option<i32> {
    dict.find(key as *const std::ffi::c_void)
        .and_then(|value| value.downcast::<CFNumber>())
        .and_then(|value| value.to_i32())
}

#[cfg(target_os = "macos")]
fn macos_cf_rect(
    dict: &CFDictionary<*const std::ffi::c_void, CFType>,
    key: CFStringRef,
) -> Option<CGRect> {
    dict.find(key as *const std::ffi::c_void)
        .and_then(|value| value.downcast::<CFDictionary>())
        .and_then(|value| CGRect::from_dict_representation(&value))
}

#[cfg(target_os = "macos")]
fn macos_rect_matches_any_display(rect: CGRect, displays: &[CGRect]) -> bool {
    displays
        .iter()
        .any(|display| macos_rect_covers_rect(rect, *display, 2.0))
}

#[cfg(target_os = "macos")]
fn macos_rect_matches_any(rect: CGRect, others: &[CGRect]) -> bool {
    others.iter().any(|other| {
        macos_rect_covers_rect(rect, *other, 2.0)
            && macos_rect_covers_rect(*other, rect, 2.0)
    })
}

#[cfg(target_os = "macos")]
fn macos_rect_covers_rect(rect: CGRect, target: CGRect, tolerance: f64) -> bool {
    rect.origin.x <= target.origin.x + tolerance
        && rect.origin.y <= target.origin.y + tolerance
        && rect.origin.x + rect.size.width >= target.origin.x + target.size.width - tolerance
        && rect.origin.y + rect.size.height >= target.origin.y + target.size.height - tolerance
}

#[cfg(target_os = "macos")]
fn macos_rect_is_too_small_for_foreground(rect: CGRect) -> bool {
    rect.size.width < 80.0 || rect.size.height < 80.0
}

#[cfg(all(not(windows), not(target_os = "macos")))]
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

#[cfg(target_os = "macos")]
fn cursor_is_in_panel_trigger(app: &AppHandle) -> Result<bool, Box<dyn std::error::Error>> {
    Ok(macos_panel_pointer_state(app)?.in_trigger)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
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

#[cfg(target_os = "macos")]
fn get_panel_pointer_state(
    app: &AppHandle,
) -> Result<PanelPointerState, Box<dyn std::error::Error>> {
    macos_panel_pointer_state(app)
}

#[cfg(all(not(windows), not(target_os = "macos")))]
fn get_panel_pointer_state(
    _app: &AppHandle,
) -> Result<PanelPointerState, Box<dyn std::error::Error>> {
    Ok(default_panel_pointer_state())
}

#[cfg(not(windows))]
fn default_panel_pointer_state() -> PanelPointerState {
    PanelPointerState {
        in_trigger: false,
        in_window: false,
        expand_direction: "up",
        left_down: false,
        right_down: false,
        cursor_x: 0,
        cursor_y: 0,
        window_x: 0,
        window_y: 0,
    }
}

#[cfg(target_os = "macos")]
fn macos_panel_pointer_state(
    app: &AppHandle,
) -> Result<PanelPointerState, Box<dyn std::error::Error>> {
    let Some(strip_window) = app.get_webview_window("main") else {
        return Ok(default_panel_pointer_state());
    };

    let cursor = strip_window.cursor_position()?;
    let strip_rect = webview_window_rect(&strip_window)?;
    let (open_down, expanded, dragging) = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .panel
                .lock()
                .ok()
                .map(|panel| (panel.open_down, panel.expanded, panel.dragging))
        })
        .unwrap_or((false, false, false));
    let in_strip = strip_rect.contains(cursor.x, cursor.y);
    let in_panel_or_bridge = if expanded {
        app.get_webview_window("panel")
            .and_then(|panel_window| webview_window_rect(&panel_window).ok())
            .is_some_and(|panel_rect| {
                panel_rect.contains(cursor.x, cursor.y)
                    || bridge_between_windows_contains(strip_rect, panel_rect, cursor.x, cursor.y)
            })
    } else {
        false
    };

    Ok(PanelPointerState {
        in_trigger: in_strip,
        in_window: in_strip || in_panel_or_bridge,
        expand_direction: if open_down { "down" } else { "up" },
        left_down: dragging,
        right_down: false,
        cursor_x: cursor.x.round() as i32,
        cursor_y: cursor.y.round() as i32,
        window_x: strip_rect.left.round() as i32,
        window_y: strip_rect.top.round() as i32,
    })
}

#[cfg(target_os = "macos")]
fn webview_window_rect(
    window: &tauri::WebviewWindow,
) -> Result<WindowRect, Box<dyn std::error::Error>> {
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    Ok(WindowRect {
        left: position.x as f64,
        top: position.y as f64,
        right: position.x as f64 + size.width as f64,
        bottom: position.y as f64 + size.height as f64,
    })
}

#[cfg(target_os = "macos")]
fn bridge_between_windows_contains(
    strip: WindowRect,
    panel: WindowRect,
    cursor_x: f64,
    cursor_y: f64,
) -> bool {
    let left = strip.left.max(panel.left);
    let right = strip.right.min(panel.right);
    if left > right || cursor_x < left || cursor_x > right {
        return false;
    }

    let (top, bottom) = if panel.top >= strip.bottom {
        (strip.bottom, panel.top)
    } else if strip.top >= panel.bottom {
        (panel.bottom, strip.top)
    } else {
        (strip.top.max(panel.top), strip.bottom.min(panel.bottom))
    };

    cursor_y >= top && cursor_y <= bottom
}

#[cfg(target_os = "macos")]
impl WindowRect {
    fn contains(self, x: f64, y: f64) -> bool {
        x >= self.left && x <= self.right && y >= self.top && y <= self.bottom
    }
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
    });
}

fn database_path(app: &AppHandle) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
        chrono_like_now(),
        label
    ));
    fs::copy(db_path, &backup_path)?;
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
}
