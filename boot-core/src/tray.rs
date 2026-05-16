// tray.rs — Windows 시스템 트레이 아이콘 제어
// Shell_NotifyIcon API를 사용해 트레이 아이콘을 등록/업데이트합니다.

use anyhow::{Context, Result};
use log::{error, info};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use windows::core::{s, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject, FillRect, HBRUSH,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIcon, CreateWindowExW, DefWindowProcW, DestroyIcon, DispatchMessageW, GetMessageW,
    LoadCursorW, PostQuitMessage, RegisterClassExW, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HICON, IDC_ARROW, MSG, WM_APP, WM_DESTROY, WM_LBUTTONDOWN, WM_RBUTTONDOWN,
    WNDCLASSEXW, WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW,
};

use crate::watcher::MonitorState;

const WM_TRAYICON: u32 = WM_APP + 1;
const TRAY_ID: u32 = 1;
const WINDOW_CLASS: &str = "BootReadyTray\0";

// 전역 상태 (WndProc 콜백에서 접근)
static STATE: once_cell::sync::OnceCell<Arc<Mutex<MonitorState>>> =
    once_cell::sync::OnceCell::new();

pub struct TrayApp {
    hwnd: HWND,
    icon_waiting: HICON,
    icon_ready: HICON,
}

impl TrayApp {
    pub fn new(state: Arc<Mutex<MonitorState>>) -> Result<Self> {
        STATE.set(state).ok();

        let hinstance = unsafe { GetModuleHandleW(None)? };

        // 아이콘 생성 (16x16 단색 비트맵)
        let icon_waiting = create_color_icon(hinstance, 0x00888888)?; // 회색 (대기 중)
        let icon_ready = create_color_icon(hinstance, 0x0075E01D)?;   // 초록 (완료)

        // 숨겨진 메시지 전용 창 생성
        let hwnd = create_message_window(hinstance)?;
        info!("tray message window created");

        // 트레이에 대기 아이콘 추가 (아직 완료 전이므로 숨김)
        // 완료 후 show_ready()가 호출될 때 표시
        let app = TrayApp {
            hwnd,
            icon_waiting,
            icon_ready,
        };

        Ok(app)
    }

    pub fn run(self) -> Result<()> {
        let hwnd = self.hwnd;
        let icon_ready = self.icon_ready;

        // 완료 감지 후 아이콘 전환을 위한 백그라운드 스레드
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(500));

                let is_complete = STATE
                    .get()
                    .and_then(|s| s.lock().ok())
                    .map(|s| s.is_complete)
                    .unwrap_or(false);

                if is_complete {
                    // 트레이에 완료 아이콘 추가
                    unsafe {
                        add_tray_icon(hwnd, icon_ready, "BootReady — 부팅 완료");
                    }
                    info!("tray icon shown (ready)");
                    break;
                }
            }
        });

        // Windows 메시지 펌프
        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        // 앱 종료 시 트레이 아이콘 제거
        unsafe {
            remove_tray_icon(hwnd);
            DestroyIcon(self.icon_waiting).ok();
            DestroyIcon(self.icon_ready).ok();
        }

        Ok(())
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32 & 0xFFFF;
            match event {
                // 왼쪽 클릭 또는 더블클릭 → bootready-ui.exe 실행
                WM_LBUTTONDOWN | 0x0203 /* WM_LBUTTONDBLCLK */ => {
                    launch_ui();
                }
                // 오른쪽 클릭 → 컨텍스트 메뉴 (미구현, 추후 추가)
                WM_RBUTTONDOWN => {}
                _ => {}
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            remove_tray_icon(hwnd);
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn launch_ui() {
    let ui_exe = find_ui_exe();
    info!("launching UI: {:?}", ui_exe);

    if let Some(path) = ui_exe {
        std::process::Command::new(path)
            .spawn()
            .unwrap_or_else(|e| {
                error!("failed to launch bootready-ui: {e}");
                // 폴백: 없으면 조용히 무시
                unsafe { std::mem::zeroed() }
            });
    }
}

fn find_ui_exe() -> Option<std::path::PathBuf> {
    // 1. 동일 디렉터리에서 탐색
    if let Ok(current_exe) = std::env::current_exe() {
        let sibling = current_exe
            .parent()?
            .join("bootready-ui.exe");
        if sibling.exists() {
            return Some(sibling);
        }
    }

    // 2. 환경변수 PATH 탐색
    which_bootready_ui()
}

fn which_bootready_ui() -> Option<std::path::PathBuf> {
    std::env::var("PATH").ok().and_then(|path_var| {
        path_var
            .split(';')
            .map(|dir| std::path::PathBuf::from(dir).join("bootready-ui.exe"))
            .find(|p| p.exists())
    })
}

unsafe fn add_tray_icon(hwnd: HWND, icon: HICON, tip: &str) {
    let mut tip_buf = [0u16; 128];
    for (i, c) in tip.encode_utf16().enumerate().take(127) {
        tip_buf[i] = c;
    }

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: icon,
        szTip: tip_buf,
        ..Default::default()
    };

    Shell_NotifyIconW(NIM_ADD, &mut nid);
}

unsafe fn remove_tray_icon(hwnd: HWND) {
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ID,
        ..Default::default()
    };
    Shell_NotifyIconW(NIM_DELETE, &mut nid);
}

/// 단색 16x16 아이콘 생성
fn create_color_icon(hinstance: HINSTANCE, color_bgr: u32) -> Result<HICON> {
    const SIZE: usize = 16;
    let mut and_mask = [0xFFu8; SIZE * SIZE / 8]; // 투명 마스크 (전체 불투명)
    let mut xor_mask = [0u8; SIZE * SIZE * 4];    // BGRA 픽셀 데이터

    for i in 0..SIZE * SIZE {
        let b = (color_bgr & 0xFF) as u8;
        let g = ((color_bgr >> 8) & 0xFF) as u8;
        let r = ((color_bgr >> 16) & 0xFF) as u8;
        xor_mask[i * 4] = b;
        xor_mask[i * 4 + 1] = g;
        xor_mask[i * 4 + 2] = r;
        xor_mask[i * 4 + 3] = 0xFF;
    }

    // AND 마스크: 모두 0 (전체 불투명)
    and_mask.iter_mut().for_each(|b| *b = 0x00);

    unsafe {
        CreateIcon(
            Some(hinstance.into()),
            SIZE as i32,
            SIZE as i32,
            1,
            32,
            and_mask.as_ptr(),
            xor_mask.as_ptr(),
        )
        .context("CreateIcon failed")
    }
}

fn create_message_window(hinstance: HINSTANCE) -> Result<HWND> {
    let class_name: Vec<u16> = WINDOW_CLASS.encode_utf16().collect();

    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance.into(),
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };

    unsafe {
        RegisterClassExW(&wc);

        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW, // 태스크바에 표시 안 함
            PCWSTR(class_name.as_ptr()),
            PCWSTR(std::ptr::null()),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0,
            0,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .context("CreateWindowEx failed")?;

        Ok(hwnd)
    }
}
