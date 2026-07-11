#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod watcher;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::os::windows::process::CommandExt;
use tauri::{AppHandle, Manager};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

use tauri::Emitter;
use crate::watcher::MonitorState;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_RELEASES_URL: &str =
    "https://github.com/rltgjqmsmsgksrmffhrlt/bootready/releases/latest";
const GITHUB_API_LATEST: &str =
    "https://api.github.com/repos/rltgjqmsmsgksrmffhrlt/bootready/releases/latest";

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
    pub settled_duration_ms: Option<i64>,
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
    pub settled_ms: Option<i64>,
    pub score: Option<i64>,
}

// ── Paths ─────────────────────────────────────────────────────────────────────

pub(crate) fn appdata_dir() -> PathBuf {
    PathBuf::from(std::env::var("APPDATA").unwrap_or_default()).join("BootReady")
}

pub(crate) fn db_path() -> PathBuf {
    appdata_dir().join("data.db")
}

pub(crate) fn config_path() -> PathBuf {
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
        "SELECT id, started_at, completed_at, total_duration_ms, settled_duration_ms, score \
         FROM boot_session ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok(BootSession {
                id: row.get(0)?,
                started_at: row.get(1)?,
                completed_at: row.get(2)?,
                total_duration_ms: row.get(3)?,
                settled_duration_ms: row.get(4)?,
                score: row.get(5)?,
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
        "SELECT id, total_duration_ms, settled_duration_ms, score FROM boot_session ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
                row.get::<_, Option<i64>>(3)?,
            ))
        },
    );

    match result {
        Ok((session_id, total_ms, settled_ms, score)) => {
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
                settled_ms,
                score,
            })
        }
        Err(_) => Ok(BootStatus {
            is_complete: false,
            total_programs: 0,
            active_programs: 0,
            total_ms: None,
            settled_ms: None,
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
            "SELECT id, started_at, completed_at, total_duration_ms, settled_duration_ms, score \
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
                settled_duration_ms: row.get(4)?,
                score: row.get(5)?,
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
        "SELECT id, started_at, completed_at, total_duration_ms, settled_duration_ms, score \
         FROM boot_session WHERE id = ?",
        [id],
        |row| {
            Ok(BootSession {
                id: row.get(0)?,
                started_at: row.get(1)?,
                completed_at: row.get(2)?,
                total_duration_ms: row.get(3)?,
                settled_duration_ms: row.get(4)?,
                score: row.get(5)?,
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
        // 이미 없으면 무시 — 사용자 의도는 "off"
        let _ = run.delete_value("BootReady");
    }

    // 사용자 의도를 config에 기록 → ensure_autostart_synced가 다음 시작 시 참고
    let cfg_path = config_path();
    let mut json: serde_json::Value = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    json["autostart_disabled_by_user"] = serde_json::Value::Bool(!enable);
    if let Some(parent) = cfg_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(content) = serde_json::to_string_pretty(&json) {
        let _ = std::fs::write(&cfg_path, content);
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

#[tauri::command]
fn open_release_page() {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    let op = HSTRING::from("open");
    let url = HSTRING::from(GITHUB_RELEASES_URL);
    unsafe {
        ShellExecuteW(
            HWND(std::ptr::null_mut()),
            windows::core::PCWSTR(op.as_ptr()),
            windows::core::PCWSTR(url.as_ptr()),
            windows::core::PCWSTR::null(),
            windows::core::PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
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

/// 이전 버전(1.0.x)의 boot-core 잔재 제거 — 별도 프로세스/스케줄러 작업/exe 파일/Run 키.
fn cleanup_legacy_boot_core() {
    std::fs::create_dir_all(appdata_dir()).ok();

    std::process::Command::new("schtasks")
        .args(["/delete", "/tn", "BootReadyCore", "/f"])
        .creation_flags(0x08000000)
        .output()
        .ok();

    std::process::Command::new("taskkill")
        .args(["/f", "/im", "boot-core.exe"])
        .creation_flags(0x08000000)
        .output()
        .ok();

    let _ = std::fs::remove_file(appdata_dir().join("boot-core.exe"));
    let _ = std::fs::remove_file(signal_path());

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        winreg::enums::KEY_SET_VALUE,
    ) {
        let _ = run.delete_value("BootReadyCore");
    }
}

/// 매 시작 시 HKCU\Run\BootReady를 현재 exe로 동기화.
/// 사용자가 Settings에서 명시적으로 끈 경우(config.autostart_disabled_by_user=true)에만 skip.
/// → 키가 사라지거나 경로가 바뀌어도 다음 실행에서 자가복구.
fn ensure_autostart_synced() {
    let cfg_path = config_path();
    let json: serde_json::Value = std::fs::read_to_string(&cfg_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if json.get("autostart_disabled_by_user").and_then(|v| v.as_bool()) == Some(true) {
        return;
    }

    let Ok(exe) = std::env::current_exe() else { return };
    let exe_str = exe.to_string_lossy().to_string();

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(run) = hkcu.open_subkey_with_flags(
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        winreg::enums::KEY_READ | winreg::enums::KEY_SET_VALUE,
    ) else { return };

    if let Ok(existing) = run.get_value::<String, _>("BootReady") {
        if existing == exe_str {
            return;
        }
    }

    let _ = run.set_value("BootReady", &exe_str.as_str());
}

/// GitHub Releases API로 최신 버전 확인. 새 버전이면 "update-available" 이벤트 emit.
/// 앱 시작 30초 후 1회, 이후 12시간마다 반복.
fn start_update_checker(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(30));
        if let Some(latest) = fetch_latest_version() {
            if is_newer(&latest, CURRENT_VERSION) {
                let _ = app.emit("update-available", latest);
            }
        }
    });
}

fn fetch_latest_version() -> Option<String> {
    let response = ureq::get(GITHUB_API_LATEST)
        .set("User-Agent", "BootReady")
        .call()
        .ok()?;
    let json: serde_json::Value = response.into_json().ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

/// "1.2.0" > "1.1.1" 처럼 semver 숫자 비교 (major.minor.patch 형식 가정).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> (u32, u32, u32) {
        let mut parts = s.splitn(3, '.').map(|p| p.parse::<u32>().unwrap_or(0));
        (parts.next().unwrap_or(0), parts.next().unwrap_or(0), parts.next().unwrap_or(0))
    };
    parse(latest) > parse(current)
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
            open_release_page,
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

            // Cleanup legacy boot-core artifacts (schtasks, exe, signal) from 1.0.x
            cleanup_legacy_boot_core();

            // Keep HKCU\Run\BootReady in sync with current exe (unless user opted out)
            ensure_autostart_synced();

            // Tray icon
            setup_tray(app)?;

            // Shared monitor state + startup watcher (replaces former boot-core process)
            let monitor_state: Arc<Mutex<MonitorState>> =
                Arc::new(Mutex::new(MonitorState::default()));
            app.manage(monitor_state.clone());
            watcher::spawn(app.handle().clone(), monitor_state);

            // Show-signal watcher still works as an external popup trigger
            start_signal_watcher(app.handle().clone());

            // Update checker — polls GitHub Releases in background
            start_update_checker(app.handle().clone());

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
