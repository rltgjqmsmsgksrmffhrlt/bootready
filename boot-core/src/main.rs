// boot-core: BootReady 백그라운드 모니터
// 시작 프로그램을 감시하고, 완료 시 트레이 아이콘을 표시합니다.

#![windows_subsystem = "windows"]

mod db;
mod ipc;
mod tray;
mod watcher;

use anyhow::Result;
use log::{error, info};
use std::sync::{Arc, Mutex};
use std::thread;

use crate::db::Database;
use crate::tray::TrayApp;
use crate::watcher::{MonitorState, StartupWatcher};

fn main() -> Result<()> {
    // 로거 초기화 (릴리스 빌드에서는 파일 로그)
    #[cfg(debug_assertions)]
    env_logger::init();

    #[cfg(not(debug_assertions))]
    init_file_logger();

    info!("boot-core starting");

    // DB 초기화
    let db_path = db::get_db_path()?;
    let db = Arc::new(Mutex::new(Database::open(&db_path)?));
    info!("database opened at {:?}", db_path);

    // 부팅 세션 시작
    let session_id = {
        let mut database = db.lock().unwrap();
        database.begin_session()?
    };
    info!("boot session started: id={}", session_id);

    // 감시 상태 공유
    let state = Arc::new(Mutex::new(MonitorState::new()));

    // 시작프로그램 감시 스레드
    let db_watcher = Arc::clone(&db);
    let state_watcher = Arc::clone(&state);
    thread::spawn(move || {
        let mut watcher = StartupWatcher::new(session_id, db_watcher, state_watcher);
        if let Err(e) = watcher.run() {
            error!("watcher error: {e}");
        }
    });

    // IPC Named Pipe 서버 스레드
    let db_ipc = Arc::clone(&db);
    let state_ipc = Arc::clone(&state);
    thread::spawn(move || {
        if let Err(e) = ipc::serve(db_ipc, state_ipc) {
            error!("ipc server error: {e}");
        }
    });

    // 트레이 앱 실행 (Windows 메시지 펌프 — 메인 스레드에서 실행)
    let tray = TrayApp::new(Arc::clone(&state))?;
    tray.run()?;

    info!("boot-core shutting down");
    Ok(())
}

#[cfg(not(debug_assertions))]
fn init_file_logger() {
    use std::fs::OpenOptions;
    let log_dir = dirs_or_fallback();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_file = log_dir.join("boot-core.log");

    // 간단한 파일 로거 (env_logger + 파일 출력)
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .expect("cannot open log file");

    env_logger::Builder::new()
        .target(env_logger::Target::Pipe(Box::new(file)))
        .filter_level(log::LevelFilter::Info)
        .init();
}

fn dirs_or_fallback() -> std::path::PathBuf {
    std::env::var("APPDATA")
        .map(|p| std::path::PathBuf::from(p).join("BootReady"))
        .unwrap_or_else(|_| std::path::PathBuf::from("C:\\ProgramData\\BootReady"))
}
