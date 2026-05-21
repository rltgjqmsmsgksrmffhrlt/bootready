#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

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
        win.set_size(tauri::LogicalSize::new(360.0, h as f64))
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
fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn get_file_icon(_exe_path: String) -> Result<Option<String>, String> {
    // TODO: implement Windows SHGetFileInfoW icon extraction
    Ok(None)
}

// ── Background tasks ──────────────────────────────────────────────────────────

fn position_bottom_right(win: &tauri::WebviewWindow) {
    if let Ok(Some(monitor)) = win.primary_monitor() {
        let screen = monitor.size();
        let scale = monitor.scale_factor();
        let win_w = (360.0 * scale) as i32;
        let win_h = (480.0 * scale) as i32;
        let x = screen.width as i32 - win_w - 20;
        let y = screen.height as i32 - win_h - 60;
        let _ = win.set_position(tauri::PhysicalPosition::new(x, y));
    }
}

fn start_signal_watcher(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(500));

        let sig = signal_path();
        if sig.exists() {
            let _ = std::fs::remove_file(&sig);
            if let Some(win) = app.get_webview_window("main") {
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

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
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
            get_file_icon,
        ])
        .setup(|app| {
            let win = app.get_webview_window("main").expect("no main window");

            // Hide on focus lost
            let win_clone = win.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::Focused(false) = event {
                    let _ = win_clone.hide();
                }
            });

            // Background watchers
            start_signal_watcher(app.handle().clone());
            start_session_watcher(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|_app, event| {
            // Keep alive in background — prevent exit when window closes
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
