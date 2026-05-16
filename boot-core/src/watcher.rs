// watcher.rs — 시작프로그램 목록 수집 및 프로세스 활성화 감지

use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_NO_MORE_ITEMS, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumValueW, RegOpenKeyExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
    KEY_READ, REG_SZ,
};
use windows::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
};

use crate::db::{Database, ProgramEvent};

/// 시작프로그램 하나를 나타내는 구조체
#[derive(Debug, Clone)]
pub struct StartupEntry {
    pub name: String,
    pub exe_path: String,
    /// exe 파일명만 추출 (프로세스 목록 대조용)
    pub exe_name: String,
}

/// 프로세스 감시 상태 (IPC 및 트레이에서 읽음)
#[derive(Debug, Default, Clone)]
pub struct MonitorState {
    pub is_complete: bool,
    pub total_programs: usize,
    pub active_programs: usize,
    pub boot_start_ms: u64,
    pub completion_ms: Option<u64>,
    pub score: Option<i64>,
}

impl MonitorState {
    pub fn new() -> Self {
        Self {
            boot_start_ms: epoch_ms(),
            ..Default::default()
        }
    }
}

pub struct StartupWatcher {
    session_id: i64,
    db: Arc<Mutex<Database>>,
    state: Arc<Mutex<MonitorState>>,
    boot_start: Instant,
    entries: Vec<StartupEntry>,
    /// name → (start_ms, end_ms, status)
    tracked: HashMap<String, (i64, Option<i64>, String)>,
}

impl StartupWatcher {
    pub fn new(
        session_id: i64,
        db: Arc<Mutex<Database>>,
        state: Arc<Mutex<MonitorState>>,
    ) -> Self {
        Self {
            session_id,
            db,
            state,
            boot_start: Instant::now(),
            entries: Vec::new(),
            tracked: HashMap::new(),
        }
    }

    pub fn run(&mut self) -> Result<()> {
        // 시작프로그램 목록 수집
        self.entries = collect_startup_entries()?;
        info!("found {} startup entries", self.entries.len());

        {
            let mut state = self.state.lock().unwrap();
            state.total_programs = self.entries.len();
        }

        // DB에 초기 레코드 삽입 (status=disabled 포함)
        for entry in &self.entries {
            let start_ms = self.boot_start.elapsed().as_millis() as i64;
            self.tracked
                .insert(entry.name.clone(), (start_ms, None, "ok".to_string()));

            if let Ok(mut db) = self.db.lock() {
                let _ = db.upsert_program_event(
                    self.session_id,
                    &entry.name,
                    Some(&entry.exe_path),
                    Some(start_ms),
                    None,
                    "ok",
                );
            }
        }

        // 완료 감지: 마지막 프로세스 활성화 후 IDLE_SECS 동안 신규 없으면 완료
        const POLL_MS: u64 = 500;
        const IDLE_SECS: u64 = 5;
        const MAX_WAIT_SECS: u64 = 300; // 5분 타임아웃

        let mut last_new_process = Instant::now();
        let mut completed_names: Vec<String> = Vec::new();

        loop {
            std::thread::sleep(Duration::from_millis(POLL_MS));

            let elapsed = self.boot_start.elapsed();

            if elapsed.as_secs() > MAX_WAIT_SECS {
                warn!("startup watcher timeout after {}s", MAX_WAIT_SECS);
                self.finalize(elapsed.as_millis() as i64)?;
                break;
            }

            // 현재 실행 중인 프로세스 스냅샷
            let running = match running_exe_names() {
                Ok(r) => r,
                Err(e) => {
                    warn!("process snapshot failed: {e}");
                    continue;
                }
            };

            let mut any_new = false;
            for entry in &self.entries {
                if completed_names.contains(&entry.name) {
                    continue;
                }

                let exe_lower = entry.exe_name.to_lowercase();
                if running.iter().any(|r| r.to_lowercase() == exe_lower) {
                    let end_ms = elapsed.as_millis() as i64;
                    let start_ms = self
                        .tracked
                        .get(&entry.name)
                        .map(|(s, _, _)| *s)
                        .unwrap_or(0);

                    let status = classify_status(end_ms - start_ms);

                    debug!("program active: {} ({}ms, {})", entry.name, end_ms - start_ms, status);

                    self.tracked
                        .insert(entry.name.clone(), (start_ms, Some(end_ms), status.clone()));

                    if let Ok(mut db) = self.db.lock() {
                        let _ = db.upsert_program_event(
                            self.session_id,
                            &entry.name,
                            Some(&entry.exe_path),
                            Some(start_ms),
                            Some(end_ms),
                            &status,
                        );
                    }

                    completed_names.push(entry.name.clone());
                    any_new = true;

                    let mut state = self.state.lock().unwrap();
                    state.active_programs = completed_names.len();
                }
            }

            if any_new {
                last_new_process = Instant::now();
            }

            // 모든 프로그램이 감지됐거나 충분한 idle 시간이 지나면 완료
            let all_detected = completed_names.len() >= self.entries.len();
            let idle_expired = last_new_process.elapsed().as_secs() >= IDLE_SECS;

            // 최소 부팅 시작 후 3초는 대기
            if elapsed.as_secs() >= 3 && (all_detected || idle_expired) {
                info!(
                    "startup complete: {}/{} programs detected in {}ms",
                    completed_names.len(),
                    self.entries.len(),
                    elapsed.as_millis()
                );
                self.finalize(elapsed.as_millis() as i64)?;
                break;
            }
        }

        Ok(())
    }

    fn finalize(&mut self, total_ms: i64) -> Result<()> {
        let score = calculate_score(total_ms, self.entries.len());

        if let Ok(mut db) = self.db.lock() {
            db.complete_session(self.session_id, total_ms, score)?;
        }

        {
            let mut state = self.state.lock().unwrap();
            state.is_complete = true;
            state.completion_ms = Some(total_ms as u64);
            state.score = Some(score);
        }

        info!("session finalized: total={}ms score={}", total_ms, score);
        Ok(())
    }
}

/// Windows 레지스트리 + 시작 폴더에서 시작프로그램 목록 수집
pub fn collect_startup_entries() -> Result<Vec<StartupEntry>> {
    let mut entries: Vec<StartupEntry> = Vec::new();

    // 레지스트리 경로 목록
    let reg_paths: &[(HKEY, &str)] = &[
        (
            HKEY_CURRENT_USER,
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        ),
        (
            HKEY_CURRENT_USER,
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
        ),
        (
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run",
        ),
        (
            HKEY_LOCAL_MACHINE,
            "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\RunOnce",
        ),
    ];

    for (hive, path) in reg_paths {
        match read_registry_run_key(*hive, path) {
            Ok(mut found) => entries.append(&mut found),
            Err(e) => warn!("registry read failed ({path}): {e}"),
        }
    }

    // 시작 폴더
    for folder_path in startup_folder_paths() {
        match read_startup_folder(&folder_path) {
            Ok(mut found) => entries.append(&mut found),
            Err(e) => warn!("startup folder read failed ({:?}): {e}", folder_path),
        }
    }

    // 중복 제거 (exe_path 기준)
    let mut seen = std::collections::HashSet::new();
    entries.retain(|e| seen.insert(e.exe_path.to_lowercase()));

    Ok(entries)
}

fn read_registry_run_key(hive: HKEY, subkey: &str) -> Result<Vec<StartupEntry>> {
    let mut entries = Vec::new();

    let subkey_wide: Vec<u16> = subkey.encode_utf16().chain([0]).collect();
    let mut hkey = HKEY::default();

    unsafe {
        let result = RegOpenKeyExW(
            hive,
            PCWSTR(subkey_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );
        if result.is_err() {
            return Ok(entries); // 키 없으면 빈 목록
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

        // REG_SZ 또는 REG_EXPAND_SZ만 처리
        if reg_type == REG_SZ.0 || reg_type == 2 {
            let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
            let value_wide: &[u16] = unsafe {
                std::slice::from_raw_parts(
                    data_buf.as_ptr() as *const u16,
                    data_len as usize / 2,
                )
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

                entries.push(StartupEntry {
                    name: name.trim_end_matches('\0').to_string(),
                    exe_path,
                    exe_name,
                });
            }
        }

        index += 1;
    }

    unsafe { RegCloseKey(hkey) };
    Ok(entries)
}

fn read_startup_folder(path: &PathBuf) -> Result<Vec<StartupEntry>> {
    let mut entries = Vec::new();

    if !path.exists() {
        return Ok(entries);
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let file_path = entry.path();
        let ext = file_path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        // .lnk, .exe 파일만 처리
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
            // .lnk 단축키: 파일명에서 이름 추출 (exe 경로 해석은 생략, 이름만 사용)
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

/// 현재 실행 중인 프로세스 exe 이름 목록 수집
fn running_exe_names() -> Result<Vec<String>> {
    let mut names = Vec::new();

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
            .context("CreateToolhelp32Snapshot failed")?;

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let name = String::from_utf16_lossy(
                    &entry.szExeFile[..entry
                        .szExeFile
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(entry.szExeFile.len())],
                );
                names.push(name);

                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }

        CloseHandle(snapshot)?;
    }

    Ok(names)
}

fn startup_folder_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Ok(appdata) = std::env::var("APPDATA") {
        paths.push(
            PathBuf::from(appdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"),
        );
    }

    if let Ok(programdata) = std::env::var("PROGRAMDATA") {
        paths.push(
            PathBuf::from(programdata)
                .join("Microsoft\\Windows\\Start Menu\\Programs\\Startup"),
        );
    }

    paths
}

/// "C:\path\to\app.exe" --args → "C:\path\to\app.exe"
fn extract_exe_path(cmd: &str) -> String {
    let cmd = cmd.trim();
    if cmd.starts_with('"') {
        // 따옴표로 묶인 경로
        cmd.trim_start_matches('"')
            .split('"')
            .next()
            .unwrap_or(cmd)
            .to_string()
    } else {
        // 공백 전까지
        cmd.split_whitespace()
            .next()
            .unwrap_or(cmd)
            .to_string()
    }
}

/// 프로세스 시작 시간 기준으로 상태 분류
fn classify_status(duration_ms: i64) -> String {
    if duration_ms > 30_000 {
        "slow".to_string()
    } else {
        "ok".to_string()
    }
}

/// 부팅 점수 계산 (0~100)
/// - 총 시간: 60초 이하 → 100점, 120초 이상 → 50점
/// - 프로그램 수 패널티: 10개 이상 -5점씩
pub fn calculate_score(total_ms: i64, program_count: usize) -> i64 {
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

fn epoch_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
