// ipc.rs — Named Pipe 서버
// \\.\pipe\bootready 로 Electron UI와 통신합니다.
// 요청: JSON 단일 행 ({"cmd":"status"} 등)
// 응답: JSON 단일 행

use anyhow::{Context, Result};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    FILE_FLAG_OVERLAPPED, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE,
    PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::db::{BootSession, Database, ProgramEvent};
use crate::watcher::MonitorState;

const PIPE_NAME: &str = r"\\.\pipe\bootready";
const PIPE_BUFFER_SIZE: u32 = 65536;

#[derive(Debug, Deserialize)]
struct Request {
    cmd: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum Response {
    Status(StatusResponse),
    Session(SessionResponse),
    Error { message: String },
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    is_complete: bool,
    total_programs: usize,
    active_programs: usize,
    total_ms: Option<u64>,
    score: Option<i64>,
}

#[derive(Debug, Serialize)]
struct SessionResponse {
    session: BootSession,
    events: Vec<ProgramEvent>,
}

pub fn serve(db: Arc<Mutex<Database>>, state: Arc<Mutex<MonitorState>>) -> Result<()> {
    info!("IPC server starting on {}", PIPE_NAME);

    let pipe_name_wide: Vec<u16> = PIPE_NAME.encode_utf16().chain([0]).collect();

    loop {
        // Named Pipe 인스턴스 생성
        let pipe = unsafe {
            CreateNamedPipeW(
                PCWSTR(pipe_name_wide.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                PIPE_BUFFER_SIZE,
                PIPE_BUFFER_SIZE,
                0,
                None,
            )
        };

        if pipe == INVALID_HANDLE_VALUE {
            error!("CreateNamedPipe failed");
            std::thread::sleep(std::time::Duration::from_secs(1));
            continue;
        }

        // 클라이언트 연결 대기 (블로킹)
        debug!("waiting for pipe client...");
        let connected = unsafe { ConnectNamedPipe(pipe, None) };

        if connected.is_err() {
            unsafe { CloseHandle(pipe).ok() };
            continue;
        }

        info!("pipe client connected");

        // 클라이언트별 스레드에서 처리
        let db_clone = Arc::clone(&db);
        let state_clone = Arc::clone(&state);

        std::thread::spawn(move || {
            if let Err(e) = handle_client(pipe, db_clone, state_clone) {
                error!("pipe client error: {e}");
            }
            unsafe {
                DisconnectNamedPipe(pipe).ok();
                CloseHandle(pipe).ok();
            }
        });
    }
}

fn handle_client(
    pipe: windows::Win32::Foundation::HANDLE,
    db: Arc<Mutex<Database>>,
    state: Arc<Mutex<MonitorState>>,
) -> Result<()> {
    // HANDLE → File-like 인터페이스 (unsafe raw 포인터 활용)
    use std::os::windows::io::FromRawHandle;
    let raw = pipe.0 as *mut std::ffi::c_void;

    // 읽기용 복제 (소유권 관리 주의: HANDLE은 CloseHandle로 외부 정리)
    let reader_file = unsafe { std::fs::File::from_raw_handle(raw as _) };
    let mut writer_file = unsafe { std::fs::File::from_raw_handle(raw as _) };
    let reader = BufReader::new(reader_file);

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        debug!("pipe recv: {line}");

        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => handle_request(&req.cmd, &db, &state),
            Err(e) => Response::Error {
                message: format!("invalid request: {e}"),
            },
        };

        let mut json = serde_json::to_string(&response).unwrap_or_else(|_| "{}".into());
        json.push('\n');

        if writer_file.write_all(json.as_bytes()).is_err() {
            break;
        }
    }

    // from_raw_handle로 생성된 File이 drop될 때 HANDLE을 닫으려 시도하므로
    // 소유권 누수 방지를 위해 into_raw_handle로 회수
    use std::os::windows::io::IntoRawHandle;
    let _ = std::mem::ManuallyDrop::new(writer_file);

    Ok(())
}

fn handle_request(
    cmd: &str,
    db: &Arc<Mutex<Database>>,
    state: &Arc<Mutex<MonitorState>>,
) -> Response {
    match cmd {
        "status" => {
            let s = state.lock().unwrap();
            Response::Status(StatusResponse {
                is_complete: s.is_complete,
                total_programs: s.total_programs,
                active_programs: s.active_programs,
                total_ms: s.completion_ms,
                score: s.score,
            })
        }
        "latest_session" => {
            let db = db.lock().unwrap();
            match db.get_latest_session() {
                Ok(Some(session)) => {
                    let events = db
                        .get_program_events(session.id)
                        .unwrap_or_default();
                    Response::Session(SessionResponse { session, events })
                }
                Ok(None) => Response::Error {
                    message: "no session found".into(),
                },
                Err(e) => Response::Error {
                    message: format!("db error: {e}"),
                },
            }
        }
        _ => Response::Error {
            message: format!("unknown command: {cmd}"),
        },
    }
}
