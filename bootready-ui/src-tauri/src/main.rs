#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Emitter, Manager};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

static FORCE_QUIT: AtomicBool = AtomicBool::new(false);
static LAST_SHOWN_MS: AtomicI64 = AtomicI64::new(0);

fn mark_shown() {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    LAST_SHOWN_MS.store(ms, Ordering::Relaxed);
}

fn recently_shown() -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    now - LAST_SHOWN_MS.load(Ordering::Relaxed) < 400
}

// ── Data types ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BootSession {
    pub id: i64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub total_duration_ms: Option<i64>,
    pub score: Option<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProgramEvent {
    pub id: i64,
    pub session_id: i64,
    pub name: String,
    pub exe_path: Option<String>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SessionWithEvents {
    pub session: BootSession,
    pub events: Vec<ProgramEvent>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BootStatus {
    pub is_complete: bool,
    pub total_programs: i64,
    pub active_programs: i64,
    pub total_ms: Option<i64>,
    pub score: Option<i64>,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

fn appdata_dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("BootReady")
}

fn db_path() -> PathBuf {
    appdata_dir().join("data.db")
}

fn config_path() -> PathBuf {
    appdata_dir().join("config.json")
}

fn signal_path() -> PathBuf {
    appdata_dir().join("show.signal")
}

fn open_db() -> Result<Connection, String> {
    Connection::open(db_path()).map_err(|e| e.to_string())
}

// ── Commands ──────────────────────────────────────────────────────────────────

#[tauri::command]
fn get_latest_session() -> Result<Option<SessionWithEvents>, String> {
    let conn = open_db()?;

    let session = conn.query_row(
        "SELECT id, started_at, completed_at, total_duration_ms, score \
         FROM boot_session ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok(BootSession {
                id: row.get(0)?,
                started_at: row.get(1)?,
                completed_at: row.get(2)?,
                total_duration_ms: row.get(3)?,
                score: row.get(4)?,
            })
        },
    );

    let session = match session {
        Ok(s) => s,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };

    let events = fetch_events(&conn, session.id)?;
    Ok(Some(SessionWithEvents { session, events }))
}

#[tauri::command]
fn get_boot_status() -> Result<BootStatus, String> {
    let conn = open_db()?;

    let result = conn.query_row(
        "SELECT id, total_duration_ms, score FROM boot_session ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        },
    );

    match result {
        Ok((session_id, total_ms, score)) => {
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM program_event WHERE session_id = ?",
                    [session_id],
                    |row| row.get(0),
                )
                .unwrap_or(0);
            Ok(BootStatus {
                is_complete: total_ms.is_some(),
                total_programs: count,
                active_programs: count,
                total_ms,
                score,
            })
        }
        Err(_) => Ok(BootStatus {
            is_complete: false,
            total_programs: 0,
            active_programs: 0,
            total_ms: None,
            score: None,
        }),
    }
}

#[tauri::command]
fn get_recent_sessions(limit: Option<i64>) -> Result<Vec<BootSession>, String> {
    let conn = open_db()?;
    let limit = limit.unwrap_or(20);

    let mut stmt = conn
        .prepare(
            "SELECT id, started_at, completed_at, total_duration_ms, score \
             FROM boot_session ORDER BY id DESC LIMIT ?",
        )
        .map_err(|e| e.to_string())?;

    let sessions = stmt
        .query_map([limit], |row| {
            Ok(BootSession {
                id: row.get(0)?,
                started_at: row.get(1)?,
                completed_at: row.get(2)?,
                total_duration_ms: row.get(3)?,
                score: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sessions)
}

#[tauri::command]
fn get_session_by_id(id: i64) -> Result<Option<SessionWithEvents>, String> {
    let conn = open_db()?;

    let session = conn.query_row(
        "SELECT id, started_at, completed_at, total_duration_ms, score \
         FROM boot_session WHERE id = ?",
        [id],
        |row| {
            Ok(BootSession {
                id: row.get(0)?,
                started_at: row.get(1)?,
                completed_at: row.get(2)?,
                total_duration_ms: row.get(3)?,
                score: row.get(4)?,
            })
        },
    );

    let session = match session {
        Ok(s) => s,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };

    let events = fetch_events(&conn, id)?;
    Ok(Some(SessionWithEvents { session, events }))
}

fn fetch_events(conn: &Connection, session_id: i64) -> Result<Vec<ProgramEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, name, exe_path, start_ms, end_ms, status \
             FROM program_event WHERE session_id = ? ORDER BY start_ms ASC",
        )
        .map_err(|e| e.to_string())?;

    let events = stmt
        .query_map([session_id], |row| {
            Ok(ProgramEvent {
                id: row.get(0)?,
                session_id: row.get(1)?,
                name: row.get(2)?,
                exe_path: row.get(3)?,
                start_ms: row.get(4)?,
                end_ms: row.get(5)?,
                status: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(events)
}

#[tauri::command]
fn close_window(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_window_height(app: AppHandle, h: u32) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        win.set_size(tauri::LogicalSize::new(420.0, h as f64))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_autostart() -> Result<bool, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| e.to_string())?;
    Ok(run.get_value::<String, _>("BootReady").is_ok())
}

#[tauri::command]
fn set_autostart(enable: bool) -> Result<bool, String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let run = hkcu
        .open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            winreg::enums::KEY_SET_VALUE,
        )
        .map_err(|e| e.to_string())?;

    if enable {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        run.set_value("BootReady", &exe.to_string_lossy().as_ref())
            .map_err(|e| e.to_string())?;
    } else {
        run.delete_value("BootReady").map_err(|e| e.to_string())?;
    }

    Ok(enable)
}

#[tauri::command]
fn save_config(config: serde_json::Value) -> Result<bool, String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
fn load_config() -> Result<Option<serde_json::Value>, String> {
    let path = config_path();
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    Ok(Some(value))
}

#[tauri::command]
fn quit_app(_app: AppHandle) {
    std::process::exit(0);
}

// ── Background tasks ──────────────────────────────────────────────────────────

fn position_bottom_right(win: &tauri::WebviewWindow) {
    // Try primary monitor first, fall back to current monitor
    let monitor = win.primary_monitor().ok().flatten()
        .or_else(|| win.current_monitor().ok().flatten());

    if let Some(monitor) = monitor {
        let screen = monitor.size();
        let pos = monitor.position(); // monitor origin (multi-monitor support)
        let scale = monitor.scale_factor();

        // Window size in physical pixels
        let win_size = win.outer_size().unwrap_or(tauri::PhysicalSize::new(
            (420.0 * scale) as u32,
            (560.0 * scale) as u32,
        ));

        // Taskbar height: scale 48px logical with DPI
        let taskbar_h = (48.0 * scale) as i32 + 8;

        let x = pos.x + screen.width as i32 - win_size.width as i32 - 20;
        let y = pos.y + screen.height as i32 - win_size.height as i32 - taskbar_h;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn ensure_boot_core(app: &tauri::App) {
    let dest = appdata_dir().join("boot-core.exe");

    // Always try to update boot-core.exe from bundle (ignore errors if file is in use)
    if let Ok(src) = app.path().resource_dir() {
        let src = src.join("boot-core.exe");
        if src.exists() {
            std::fs::create_dir_all(&appdata_dir()).ok();
            std::fs::copy(&src, &dest).ok();
        }
    }

    // Register via Task Scheduler (starts earlier at logon than HKCU Run)
    if dest.exists() {
        let dest_str = dest.to_string_lossy();
        let task_exists = std::process::Command::new("schtasks")
            .args(["/query", "/tn", "BootReadyCore"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !task_exists {
            std::process::Command::new("schtasks")
                .args([
                    "/create",
                    "/tn", "BootReadyCore",
                    "/tr", dest_str.as_ref(),
                    "/sc", "ONLOGON",
                    "/delay", "0000:00",
                    "/f",
                ])
                .creation_flags(0x08000000) // CREATE_NO_WINDOW
                .spawn()
                .ok();
        }

        // Launch boot-core if not running
        let running = std::process::Command::new("tasklist")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains("boot-core.exe"))
            .unwrap_or(false);
        if !running {
            std::process::Command::new(&dest)
                .spawn()
                .ok();
        }
    }
}

fn start_signal_watcher(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));

        let sig = signal_path();
        if sig.exists() {
            let _ = std::fs::remove_file(&sig);
            if let Some(win) = app.get_webview_window("main") {
                mark_shown();
                position_bottom_right(&win);
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
    });
}

fn start_session_watcher(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_id: Option<i64> = None;

        loop {
            std::thread::sleep(std::time::Duration::from_secs(5));

            if let Ok(conn) = open_db() {
                if let Ok(current_id) = conn.query_row(
                    "SELECT id FROM boot_session ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get::<_, i64>(0),
                ) {
                    if last_id.map_or(false, |prev| current_id > prev) {
                        let _ = app.emit("session-updated", ());
                    }
                    last_id = Some(current_id);
                }
            }
        }
    });
}

// ── Tray ─────────────────────────────────────────────────────────────────────

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{TrayIconBuilder, TrayIconEvent};

    let show_i = MenuItem::with_id(app, "show", "BootReady 열기", true, None::<&str>)?;
    let quit_i = MenuItem::with_id(app, "quit", "종료", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

    TrayIconBuilder::new()
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
        .tooltip("BootReady")
        .on_tray_icon_event(|tray, event| {
            use tauri::tray::{MouseButton, MouseButtonState};
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window("main") {
                    mark_shown();
                    position_bottom_right(&win);
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(win) = app.get_webview_window("main") {
                    mark_shown();
                    position_bottom_right(&win);
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
            "quit" => std::process::exit(0),
            _ => {}
        })
        .build(app)?;

    Ok(())
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                mark_shown();
                position_bottom_right(&win);
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            get_latest_session,
            get_boot_status,
            get_recent_sessions,
            get_session_by_id,
            close_window,
            set_window_height,
            get_autostart,
            set_autostart,
            save_config,
            load_config,
            quit_app,
        ])
        .setup(|app| {
            let win = app.get_webview_window("main").expect("no main window");

            // Hide on focus lost — debounced to avoid race with show/set_focus
            let win_clone = win.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(false) = event {
                    if !recently_shown() {
                        let _ = win_clone.hide();
                    }
                }
            });

            // Pre-position window before first show
            position_bottom_right(&win);

            // Ensure boot-core is present and running
            ensure_boot_core(app);

            // Tray icon
            setup_tray(app)?;

            // Background watchers
            start_signal_watcher(app.handle().clone());
            start_session_watcher(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !FORCE_QUIT.load(Ordering::SeqCst) {
                    api.prevent_exit();
                }
            }
        });
}
