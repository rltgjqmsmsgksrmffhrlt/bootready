// Startup-program watcher absorbed from the former boot-core process.
// Runs as a background thread under the Tauri runtime, sharing MonitorState
// via Arc<Mutex> with the rest of the app.

use chrono::Utc;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, HWND};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
    TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_READ, REG_SZ,
};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::{appdata_dir, config_path, db_path};

#[derive(Debug, Default)]
pub struct MonitorState {
    pub session_id: Option<i64>,
    pub is_complete: bool,
    pub total_programs: usize,
    pub active_programs: usize,
    pub initial_running_count: usize,
}

#[derive(Debug, Clone)]
struct StartupEntry {
    name: String,
    exe_path: String,
    exe_name: String,
}

pub fn spawn(app: AppHandle, state: Arc<Mutex<MonitorState>>) {
    std::thread::spawn(move || {
        if let Err(e) = run(app, state) {
            eprintln!("watcher error: {e}");
        }
    });
}

fn run(app: AppHandle, state: Arc<Mutex<MonitorState>>) -> Result<(), String> {
    let mut conn = open_db()?;
    init_schema(&conn)?;

    let session_id = begin_session(&mut conn)?;
    {
        let mut s = state.lock().unwrap();
        s.session_id = Some(session_id);
    }

    let entries = collect_startup_entries();
    {
        let mut s = state.lock().unwrap();
        s.total_programs = entries.len();
    }

    let boot_start = Instant::now();
    let mut tracked: HashMap<String, (i64, Option<i64>, String)> = HashMap::new();

    for entry in &entries {
        let start_ms = boot_start.elapsed().as_millis() as i64;
        tracked.insert(entry.name.clone(), (start_ms, None, "ok".to_string()));
        let _ = upsert_program_event(
            &mut conn,
            session_id,
            &entry.name,
            Some(&entry.exe_path),
            Some(start_ms),
            None,
            "ok",
        );
    }

    const POLL_MS: u64 = 500;
    const IDLE_SECS: u64 = 5;
    const MAX_WAIT_SECS: u64 = 300;

    let mut completed_names: Vec<String> = Vec::new();
    let mut last_new_process = Instant::now();
    let mut initial_running_set = false;
    let mut initial_running_count = 0usize;

    loop {
        std::thread::sleep(Duration::from_millis(POLL_MS));
        let elapsed = boot_start.elapsed();

        let running = running_exe_names();

        let mut any_new = false;
        for entry in &entries {
            if completed_names.contains(&entry.name) {
                continue;
            }
            let exe_lower = entry.exe_name.to_lowercase();
            if running.iter().any(|r| r.to_lowercase() == exe_lower) {
                let end_ms = elapsed.as_millis() as i64;
                let start_ms = tracked
                    .get(&entry.name)
                    .map(|(s, _, _)| *s)
                    .unwrap_or(0);
                let status = classify_status(end_ms - start_ms);
                tracked.insert(
                    entry.name.clone(),
                    (start_ms, Some(end_ms), status.clone()),
                );
                let _ = upsert_program_event(
                    &mut conn,
                    session_id,
                    &entry.name,
                    Some(&entry.exe_path),
                    Some(start_ms),
                    Some(end_ms),
                    &status,
                );
                completed_names.push(entry.name.clone());
                any_new = true;
                let mut s = state.lock().unwrap();
                s.active_programs = completed_names.len();
            }
        }

        if !initial_running_set {
            initial_running_count = completed_names.len();
            initial_running_set = true;
            let mut s = state.lock().unwrap();
            s.initial_running_count = initial_running_count;
        }

        if any_new {
            last_new_process = Instant::now();
        }

        if elapsed.as_secs() > MAX_WAIT_SECS {
            finalize(
                &mut conn,
                session_id,
                elapsed.as_millis() as i64,
                entries.len(),
                initial_running_count,
                &state,
                &app,
            )?;
            break;
        }

        let all_detected = completed_names.len() >= entries.len();
        let idle_expired = last_new_process.elapsed().as_secs() >= IDLE_SECS;

        if elapsed.as_secs() >= 3 && (all_detected || idle_expired) {
            finalize(
                &mut conn,
                session_id,
                elapsed.as_millis() as i64,
                entries.len(),
                initial_running_count,
                &state,
                &app,
            )?;
            break;
        }
    }

    Ok(())
}

fn finalize(
    conn: &mut Connection,
    session_id: i64,
    total_ms: i64,
    entry_count: usize,
    initial_running_count: usize,
    state: &Arc<Mutex<MonitorState>>,
    app: &AppHandle,
) -> Result<(), String> {
    let score = calculate_score(total_ms, entry_count);
    complete_session(conn, session_id, total_ms, score)?;

    {
        let mut s = state.lock().unwrap();
        s.is_complete = true;
    }

    let _ = app.emit("session-updated", ());

    if should_show_popup(entry_count, initial_running_count) {
        let _ = std::fs::write(appdata_dir().join("show.signal"), b"show");
    }

    if should_open_urls(entry_count, initial_running_count) {
        open_startup_urls();
    }

    Ok(())
}

/// 부팅 직후라고 판단될 때만 popup 자동 노출 — URL 가드와 같은 기준.
fn should_show_popup(entry_count: usize, initial_running_count: usize) -> bool {
    should_open_urls(entry_count, initial_running_count)
}

/// URL을 여는 조건 — 진짜 부팅 직후라고 판단될 때만.
/// 1) 시작프로그램이 1개 이상이어야 함 (의미 있는 부팅 감지)
/// 2) 시스템 uptime이 임계값 안 (사용자가 한참 뒤에 실행한 게 아닌)
/// 3) 첫 폴링에서 절반 이상이 이미 실행 중이 아니어야 함 (즉, 늦게 시작된 게 아님)
fn should_open_urls(entry_count: usize, initial_running_count: usize) -> bool {
    if entry_count == 0 {
        return false;
    }

    const REAL_BOOT_WINDOW_MS: u64 = 10 * 60 * 1000; // 10 minutes
    let uptime_ms = unsafe { GetTickCount64() };
    if uptime_ms > REAL_BOOT_WINDOW_MS {
        return false;
    }

    if initial_running_count * 2 >= entry_count {
        return false;
    }

    true
}

// ── Startup entries ──────────────────────────────────────────────────────────

fn collect_startup_entries() -> Vec<StartupEntry> {
    let mut entries: Vec<StartupEntry> = Vec::new();

    let reg_paths: &[(HKEY, &str)] = &[
        (HKEY_CURRENT_USER, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run"),
        (HKEY_CURRENT_USER, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run"),
        (HKEY_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce"),
    ];

    for (hive, path) in reg_paths {
        if let Ok(mut found) = read_registry_run_key(*hive, path) {
            entries.append(&mut found);
        }
    }

    for folder_path in startup_folder_paths() {
        if let Ok(mut found) = read_startup_folder(&folder_path) {
            entries.append(&mut found);
        }
    }

    // Drop ourselves so we don't count BootReady.exe as a startup program.
    if let Ok(self_exe) = std::env::current_exe() {
        let self_lower = self_exe.to_string_lossy().to_lowercase();
        entries.retain(|e| e.exe_path.to_lowercase() != self_lower);
    }

    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.exe_path.to_lowercase()));
    entries
}

fn read_registry_run_key(hive: HKEY, subkey: &str) -> Result<Vec<StartupEntry>, String> {
    let mut entries = Vec::new();
    let subkey_wide: Vec<u16> = subkey.encode_utf16().chain([0]).collect();
    let mut hkey = HKEY::default();

    unsafe {
        let result = RegOpenKeyExW(hive, PCWSTR(subkey_wide.as_ptr()), 0, KEY_READ, &mut hkey);
        if result.is_err() {
            return Ok(entries);
        }
    }

    let mut index = 0u32;
    loop {
        let mut name_buf = vec![0u16; 256];
        let mut name_len = name_buf.len() as u32;
        let mut data_buf = vec![0u8; 1024];
        let mut data_len = data_buf.len() as u32;
        let mut reg_type = 0u32;

        let result = unsafe {
            RegEnumValueW(
                hkey,
                index,
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                Some(&mut reg_type),
                Some(data_buf.as_mut_ptr()),
                Some(&mut data_len),
            )
        };

        if result == ERROR_NO_MORE_ITEMS {
            break;
        }
        if result.is_err() {
            index += 1;
            continue;
        }

        if reg_type == REG_SZ.0 || reg_type == 2 {
            let name = String::from_utf16_lossy(&name_buf[..name_len as usize])
                .trim_end_matches('\0')
                .to_string();
            let value_wide: &[u16] = unsafe {
                std::slice::from_raw_parts(data_buf.as_ptr() as *const u16, data_len as usize / 2)
            };
            let value = String::from_utf16_lossy(value_wide)
                .trim_end_matches('\0')
                .to_string();

            if !value.is_empty() {
                let exe_path = extract_exe_path(&value);
                let exe_name = PathBuf::from(&exe_path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| exe_path.clone());
                entries.push(StartupEntry { name, exe_path, exe_name });
            }
        }

        index += 1;
    }

    unsafe {
        let _ = RegCloseKey(hkey);
    }
    Ok(entries)
}

fn read_startup_folder(path: &PathBuf) -> Result<Vec<StartupEntry>, String> {
    let mut entries = Vec::new();
    if !path.exists() {
        return Ok(entries);
    }

    let read_dir = std::fs::read_dir(path).map_err(|e| e.to_string())?;
    for entry in read_dir.flatten() {
        let file_path = entry.path();
        let ext = file_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if ext == "exe" {
            let name = file_path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let exe_path = file_path.to_string_lossy().to_string();
            let exe_name = file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            entries.push(StartupEntry { name, exe_path, exe_name });
        } else if ext == "lnk" {
            let name = file_path
                .file_stem()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let exe_name = format!("{}.exe", name.to_lowercase().replace(' ', ""));
            entries.push(StartupEntry {
                name: name.clone(),
                exe_path: file_path.to_string_lossy().to_string(),
                exe_name,
            });
        }
    }
    Ok(entries)
}

fn startup_folder_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        paths.push(PathBuf::from(appdata).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"));
    }
    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        paths.push(PathBuf::from(programdata).join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"));
    }
    paths
}

fn extract_exe_path(cmd: &str) -> String {
    let cmd = cmd.trim();
    if cmd.starts_with('"') {
        cmd.trim_start_matches('"')
            .split('"')
            .next()
            .unwrap_or(cmd)
            .to_string()
    } else {
        cmd.split_whitespace().next().unwrap_or(cmd).to_string()
    }
}

// ── Process snapshot ─────────────────────────────────────────────────────────

fn running_exe_names() -> Vec<String> {
    let mut names = Vec::new();
    unsafe {
        let snapshot = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            Ok(h) => h,
            Err(_) => return names,
        };

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(entry.szExeFile.len());
                names.push(String::from_utf16_lossy(&entry.szExeFile[..end]));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        let _ = CloseHandle(snapshot);
    }
    names
}

fn classify_status(duration_ms: i64) -> String {
    if duration_ms > 30_000 {
        "slow".to_string()
    } else {
        "ok".to_string()
    }
}

fn calculate_score(total_ms: i64, program_count: usize) -> i64 {
    let time_score = if total_ms <= 60_000 {
        100
    } else if total_ms >= 120_000 {
        50
    } else {
        100 - ((total_ms - 60_000) * 50 / 60_000)
    };
    let count_penalty = if program_count > 10 {
        ((program_count - 10) * 5).min(30) as i64
    } else {
        0
    };
    (time_score - count_penalty).max(10)
}

// ── DB helpers ──────────────────────────────────────────────────────────────

fn open_db() -> Result<Connection, String> {
    let p = db_path();
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Connection::open(&p).map_err(|e| e.to_string())
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "PRAGMA journal_mode=DELETE;
         PRAGMA foreign_keys=ON;
         CREATE TABLE IF NOT EXISTS boot_session (
             id                INTEGER PRIMARY KEY AUTOINCREMENT,
             started_at        TEXT    NOT NULL,
             completed_at      TEXT,
             total_duration_ms INTEGER,
             score             INTEGER
         );
         CREATE TABLE IF NOT EXISTS program_event (
             id          INTEGER PRIMARY KEY AUTOINCREMENT,
             session_id  INTEGER NOT NULL REFERENCES boot_session(id),
             name        TEXT    NOT NULL,
             exe_path    TEXT,
             start_ms    INTEGER,
             end_ms      INTEGER,
             status      TEXT CHECK(status IN ('ok','slow','disabled','failed'))
         );
         CREATE INDEX IF NOT EXISTS idx_program_event_session ON program_event(session_id);",
    )
    .map_err(|e| e.to_string())
}

fn begin_session(conn: &mut Connection) -> Result<i64, String> {
    let now = Utc::now().to_rfc3339();
    conn.execute("INSERT INTO boot_session (started_at) VALUES (?1)", params![now])
        .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

fn complete_session(
    conn: &mut Connection,
    session_id: i64,
    total_ms: i64,
    score: i64,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE boot_session SET completed_at=?1, total_duration_ms=?2, score=?3 WHERE id=?4",
        params![now, total_ms, score, session_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn upsert_program_event(
    conn: &mut Connection,
    session_id: i64,
    name: &str,
    exe_path: Option<&str>,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    status: &str,
) -> Result<(), String> {
    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM program_event WHERE session_id=?1 AND name=?2",
            params![session_id, name],
            |row| row.get(0),
        )
        .ok();

    if let Some(id) = existing {
        conn.execute(
            "UPDATE program_event SET end_ms=?1, status=?2 WHERE id=?3",
            params![end_ms, status, id],
        )
        .map_err(|e| e.to_string())?;
    } else {
        conn.execute(
            "INSERT INTO program_event (session_id, name, exe_path, start_ms, end_ms, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![session_id, name, exe_path, start_ms, end_ms, status],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ── URL launcher ─────────────────────────────────────────────────────────────

fn open_startup_urls() {
    let content = match std::fs::read_to_string(config_path()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let json: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };
    let urls = match json.get("startup_urls").and_then(|v| v.as_array()) {
        Some(arr) => arr.clone(),
        None => return,
    };

    for url_val in urls {
        if let Some(url) = url_val.as_str() {
            let operation = HSTRING::from("open");
            let file = HSTRING::from(url);
            unsafe {
                ShellExecuteW(
                    HWND(std::ptr::null_mut()),
                    PCWSTR(operation.as_ptr()),
                    PCWSTR(file.as_ptr()),
                    PCWSTR::null(),
                    PCWSTR::null(),
                    SW_SHOWNORMAL,
                );
            }
        }
    }
}

