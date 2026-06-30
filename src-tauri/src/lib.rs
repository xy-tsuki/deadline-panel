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
    AppHandle, LogicalSize, Manager, PhysicalPosition, Size,
};

#[cfg(windows)]
use windows::Win32::{
    Foundation::{HWND, POINT, RECT},
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
const PANEL_EXPANDED_HEIGHT: f64 = 642.0;
const PANEL_COLLAPSED_HEIGHT: f64 = 44.0;
const PANEL_MARGIN: i32 = 18;
const PANEL_TRIGGER_EXTRA: i32 = 8;
const PANEL_POSITION_SETTING_KEY: &str = "panel_position";
const PANEL_ANCHOR_POSITION_SETTING_KEY: &str = "panel_anchor_position_v2";
const SEED_TASKS_V2_SETTING_KEY: &str = "seed_tasks_v2";

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
    let Some(window) = app.get_webview_window("main") else {
        return Ok(PanelExpandResult { direction: "up" });
    };
    let open_down = {
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.open_down
    };
    let anchor = current_panel_anchor(&window, open_down).map_err(|error| error.to_string())?;
    let should_open_down =
        should_expand_panel_down(&window, anchor.1).map_err(|error| error.to_string())?;
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
    let Some(window) = app.get_webview_window("main") else {
        return Ok(PanelExpandResult { direction: "up" });
    };

    let previous_open_down = {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        let previous_open_down = panel.open_down;
        panel.expanded = expanded;
        previous_open_down
    };

    let anchor =
        current_panel_anchor(&window, previous_open_down).map_err(|error| error.to_string())?;
    let open_down =
        should_expand_panel_down(&window, anchor.1).map_err(|error| error.to_string())?;
    position_window_for_anchor(&window, anchor.0, anchor.1, open_down)
        .map_err(|error| error.to_string())?;

    {
        let mut panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.open_down = open_down;
    }

    window
        .set_focusable(expanded)
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(!expanded)
        .map_err(|error| error.to_string())?;
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
fn set_panel_accepts_input(accepts_input: bool, app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    window
        .set_ignore_cursor_events(!accepts_input)
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
    let open_down = state
        .panel
        .lock()
        .map_err(|error| error.to_string())?
        .open_down;
    let (anchor_x, anchor_y) =
        current_panel_anchor(&window, open_down).map_err(|error| error.to_string())?;
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
    let Some(window) = app.get_webview_window("main") else {
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
    }

    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let hwnd_value = hwnd.0 as isize;
    let start_position = window.outer_position().map_err(|error| error.to_string())?;
    let collapsed_offset = collapsed_strip_offset(&window).map_err(|error| error.to_string())?;
    let start_open_down = {
        let state = app.state::<AppState>();
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        panel.open_down
    };
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
            collapsed_offset,
            start_open_down,
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
            set_panel_accepts_input,
            move_panel_window,
            remember_panel_position,
            reset_panel_position,
            show_panel_context_menu,
            start_panel_drag,
            finish_panel_drag,
            quit_app
        ])
        .run(tauri::generate_context!())
        .expect("error while running Deadline Panel");
}

fn position_main_window(
    app: &mut tauri::App,
    db: &Connection,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    window.set_shadow(false)?;
    window.set_skip_taskbar(true)?;
    let _ = set_window_bounds_from_saved_or_bottom_right(&window, db)?;
    window.set_focusable(false)?;
    window.set_ignore_cursor_events(true)?;
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

    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    window
        .set_skip_taskbar(true)
        .map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window
        .set_shadow(false)
        .map_err(|error| error.to_string())?;
    if reset_position {
        set_window_bounds_bottom_right(&window, PANEL_WIDTH, PANEL_EXPANDED_HEIGHT)
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
        let open_down = set_window_bounds_from_saved_or_bottom_right(&window, &db)
            .map_err(|error| error.to_string())?;
        drop(db);
        {
            if let Ok(mut panel) = state.panel.lock() {
                panel.open_down = open_down;
            }
        };
    }
    window
        .set_focusable(false)
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn sync_panel_visibility(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (should_show, expanded) = {
        let panel = state.panel.lock().map_err(|error| error.to_string())?;
        (!panel.manual_hidden && !panel.auto_hidden, panel.expanded)
    };

    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    if should_show {
        window
            .set_skip_taskbar(true)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        window
            .set_shadow(false)
            .map_err(|error| error.to_string())?;
        let state = app.state::<AppState>();
        let db = state.db.lock().map_err(|error| error.to_string())?;
        let open_down = set_window_bounds_from_saved_or_bottom_right(&window, &db)
            .map_err(|error| error.to_string())?;
        drop(db);
        if let Ok(mut panel) = state.panel.lock() {
            panel.open_down = open_down;
        }
        window
            .set_focusable(expanded)
            .map_err(|error| error.to_string())?;
        window
            .set_ignore_cursor_events(!expanded)
            .map_err(|error| error.to_string())?;
    } else {
        window.hide().map_err(|error| error.to_string())?;
    }

    Ok(())
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
        .on_menu_event(|app, event| {
            handle_panel_menu_event(app, event.id().as_ref());
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
        "restart" => restart_app(app),
        "quit" => app.exit(0),
        _ => {}
    }
}

fn restart_app(app: &AppHandle) {
    if let Ok(exe_path) = std::env::current_exe() {
        if Command::new(exe_path).spawn().is_ok() {
            app.exit(0);
        }
    }
}

fn native_menu_labels(app: &AppHandle) -> NativeMenuLabels {
    let language = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .db
                .lock()
                .ok()
                .and_then(|db| get_app_setting_value(&db, "app_language").ok().flatten())
        })
        .unwrap_or_else(|| "zh".to_string());

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
    let language = app
        .try_state::<AppState>()
        .and_then(|state| {
            state
                .db
                .lock()
                .ok()
                .and_then(|db| get_app_setting_value(&db, "app_language").ok().flatten())
        })
        .unwrap_or_else(|| "zh".to_string());

    match language.as_str() {
        "ja" => "再起動",
        "en" => "Restart",
        _ => "重启",
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

fn set_window_bounds_from_saved_or_bottom_right(
    window: &tauri::WebviewWindow,
    db: &Connection,
) -> Result<bool, Box<dyn std::error::Error>> {
    window.set_size(Size::Logical(LogicalSize {
        width: PANEL_WIDTH,
        height: PANEL_EXPANDED_HEIGHT,
    }))?;

    if let Some((anchor_x, anchor_y)) = get_saved_panel_anchor_position(window, db)? {
        let open_down = should_expand_panel_down(window, anchor_y)?;
        position_window_for_anchor(window, anchor_x, anchor_y, open_down)?;
        return Ok(open_down);
    }

    set_window_bounds_bottom_right(window, PANEL_WIDTH, PANEL_EXPANDED_HEIGHT)?;
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

fn current_panel_anchor(
    window: &tauri::WebviewWindow,
    open_down: bool,
) -> Result<(i32, i32), Box<dyn std::error::Error>> {
    let position = window.outer_position()?;
    let anchor_y = if open_down {
        position.y
    } else {
        position.y + collapsed_strip_offset(window)?
    };
    Ok((position.x, anchor_y))
}

fn position_window_for_anchor(
    window: &tauri::WebviewWindow,
    anchor_x: i32,
    anchor_y: i32,
    open_down: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let y = if open_down {
        anchor_y
    } else {
        anchor_y - collapsed_strip_offset(window)?
    };
    window.set_position(PhysicalPosition::new(anchor_x, y))?;
    Ok(())
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
    let full_height = (PANEL_EXPANDED_HEIGHT * scale).round() as i32;
    let collapsed_height = (PANEL_COLLAPSED_HEIGHT * scale).round() as i32;
    let required_above = full_height - collapsed_height + margin;
    let required_below = full_height + margin;
    let available_above = anchor_y - work_top;
    let available_below = work_bottom - anchor_y;

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

    let open_down = app
        .try_state::<AppState>()
        .and_then(|state| state.panel.lock().ok().map(|panel| panel.open_down))
        .unwrap_or(false);
    let position = window.outer_position()?;
    let size = window.outer_size()?;
    let scale = window.scale_factor()?;
    let trigger_height = (PANEL_COLLAPSED_HEIGHT * scale).round() as i32 + PANEL_TRIGGER_EXTRA;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };

    let left = position.x;
    let right = position.x + size.width as i32;
    let bottom = position.y + size.height as i32;
    let window_top = position.y;
    let (top, bottom) = if open_down {
        (window_top, window_top + trigger_height)
    } else {
        (bottom - trigger_height, bottom)
    };

    Ok(cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= bottom)
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
    let scale = window.scale_factor()?;
    let trigger_height = (PANEL_COLLAPSED_HEIGHT * scale).round() as i32 + PANEL_TRIGGER_EXTRA;
    let mut cursor = POINT::default();
    unsafe { GetCursorPos(&mut cursor)? };

    let left = position.x;
    let right = position.x + size.width as i32;
    let bottom = position.y + size.height as i32;
    let window_top = position.y;
    let (top, trigger_bottom) = if open_down {
        (window_top, window_top + trigger_height)
    } else {
        (bottom - trigger_height, bottom)
    };
    let in_window =
        cursor.x >= left && cursor.x <= right && cursor.y >= window_top && cursor.y <= bottom;
    let raw_in_trigger =
        cursor.x >= left && cursor.x <= right && cursor.y >= top && cursor.y <= trigger_bottom;
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
    collapsed_offset: i32,
    start_open_down: bool,
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
            let anchor_y = if start_open_down {
                final_y
            } else {
                final_y + collapsed_offset
            };
            let _ = set_app_setting_value(
                &db,
                PANEL_ANCHOR_POSITION_SETTING_KEY,
                &format!("{},{}", final_x, anchor_y),
            );
        }

        let mut next_open_down = start_open_down;
        if let Some(window) = main_app.get_webview_window("main") {
            let anchor_y = if start_open_down {
                final_y
            } else {
                final_y + collapsed_offset
            };
            if let Ok(open_down) = should_expand_panel_down(&window, anchor_y) {
                next_open_down = open_down;
                let _ = position_window_for_anchor(&window, final_x, anchor_y, open_down);
            }
        }

        let expanded = if let Ok(mut panel) = state.panel.lock() {
            panel.dragging = false;
            panel.open_down = next_open_down;
            panel.expanded
        } else {
            false
        };

        if let Some(window) = main_app.get_webview_window("main") {
            let _ = window.set_focusable(expanded);
            let _ = window.set_ignore_cursor_events(!expanded);
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
