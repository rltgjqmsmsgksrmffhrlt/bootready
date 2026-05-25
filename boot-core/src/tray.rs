// tray.rs — Windows 시스템 트레이 아이콘 제어 (Finalist B: Ring + Dot)
//
// 부팅 전체 흐름 (idle = Sweep · 정적):
//   1) boot-core 시작 직후      → 회색 트랙만 (0% 진행)
//   2) 시작프로그램이 하나씩 완료 → 회색 트랙 + 12시 시작 결정형 호가 시계방향으로 자람
//   3) 모두 완료                 → 초록 닫힌 링 + 중앙 점 (그리고 그 상태로 유지)
//
// 회전·펄스 같은 산만한 모션 없음. 렌더는 진행률·상태가 바뀐 프레임에서만 일어남.

use anyhow::{Context, Result};
use log::{error, info};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject,
    BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, CreateWindowExW, DefWindowProcW, DestroyIcon, DispatchMessageW,
    GetMessageW, LoadCursorW, PostMessageW, PostQuitMessage, RegisterClassExW, TranslateMessage,
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, HICON, ICONINFO, IDC_ARROW, MSG, WM_APP,
    WM_DESTROY, WM_LBUTTONDOWN, WM_RBUTTONDOWN, WNDCLASSEXW, WS_EX_TOOLWINDOW,
    WS_OVERLAPPEDWINDOW,
};

use crate::watcher::MonitorState;

const WM_TRAYICON: u32 = WM_APP + 1;
const TRAY_ID: u32 = 1;
const WINDOW_CLASS: &str = "BootReadyTray\0";

// 폴링 틱. watcher.rs가 500ms마다 상태를 갱신하므로 그에 맞춰서.
const POLL_TICK_MS: u64 = 250;

// 색상 (RGB)
const COLOR_ACCENT: (u8, u8, u8) = (0x1d, 0x9e, 0x75); // #1d9e75 (앱 액센트)
const COLOR_ARC_IDLE: (u8, u8, u8) = (0xb0, 0xb0, 0xb0); // 회색 진행 호
const COLOR_TRACK: (u8, u8, u8) = (0x70, 0x70, 0x70); // 회색 트랙
const TRACK_ALPHA: f32 = 0.38; // 트랙 흐림 정도

// 링 지오메트리 (16×16 캔버스)
const RING_R: f32 = 6.6;
const RING_STROKE: f32 = 1.8;
const DOT_R: f32 = 1.7;

// 전역 상태 (WndProc 콜백에서 접근)
static STATE: once_cell::sync::OnceCell<Arc<Mutex<MonitorState>>> =
    once_cell::sync::OnceCell::new();

pub struct TrayApp {
    hwnd: HWND,
}

impl TrayApp {
    pub fn new(state: Arc<Mutex<MonitorState>>) -> Result<Self> {
        STATE.set(state).ok();

        let hinstance = unsafe { GetModuleHandleW(None)? };
        let hwnd = create_message_window(hinstance.into())?;
        info!("tray message window created");

        Ok(TrayApp { hwnd })
    }

    pub fn run(self) -> Result<()> {
        let hwnd_raw = self.hwnd.0 as usize;

        // 아이콘 업데이트 스레드 — 진행률·상태에 따라 100ms마다 다시 그림
        thread::spawn(move || {
            let hwnd = HWND(hwnd_raw as *mut _);
            run_icon_updater(hwnd);
        });

        // Windows 메시지 펌프
        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            remove_tray_icon(self.hwnd);
        }

        Ok(())
    }
}

/// 트레이 아이콘 업데이트 루프 (별도 스레드)
fn run_icon_updater(hwnd: HWND) {
    let start = Instant::now();
    let mut tray_added = false;
    let mut current_icon: Option<HICON> = None;

    // 마지막으로 렌더한 상태 — 같은 상태면 재생성 생략
    let mut last_progress = -1.0f32;
    let mut last_ready = false;

    // 시작프로그램이 0개이면 일정 시간 후 즉시 ready 상태로 전환
    const NO_PROGRAMS_FALLBACK_SECS: u64 = 3;

    loop {
        thread::sleep(Duration::from_millis(POLL_TICK_MS));

        let (active, total, is_complete) = match STATE.get().and_then(|s| s.lock().ok()) {
            Some(s) => (s.active_programs, s.total_programs, s.is_complete),
            None => continue,
        };

        let elapsed = start.elapsed();
        let total_resolved = total > 0
            || elapsed.as_secs() >= NO_PROGRAMS_FALLBACK_SECS;

        let progress = if total == 0 {
            0.0
        } else {
            (active as f32 / total as f32).clamp(0.0, 1.0)
        };

        // ready = 명시적 완료 OR 시작프로그램이 0개이고 fallback 시간이 지남
        let ready = is_complete || (total == 0 && total_resolved);

        // 재렌더 여부 판단 — 상태/진행률 변화가 있을 때만
        let needs_redraw = !tray_added
            || ready != last_ready
            || (progress - last_progress).abs() > 0.001;

        if !needs_redraw {
            continue;
        }

        let icon = match create_ring_icon(progress, ready) {
            Ok(i) => i,
            Err(e) => {
                error!("failed to create ring icon: {e}");
                continue;
            }
        };

        let tip = build_tip(active, total, is_complete);

        unsafe {
            if !tray_added {
                add_tray_icon(hwnd, icon, &tip);
                tray_added = true;
                info!("tray icon added (progress={:.0}%)", progress * 100.0);
            } else {
                modify_tray_icon(hwnd, icon, &tip);
            }
        }

        // 이전 아이콘 폐기
        if let Some(old) = current_icon.take() {
            unsafe {
                DestroyIcon(old).ok();
            }
        }
        current_icon = Some(icon);

        last_progress = progress;
        last_ready = ready;

        if ready {
            info!("tray icon settled to ready state — signaling UI then exiting");
            // 녹색 링을 3초간 표시 후 bootready-ui에 팝업 신호를 보내고 트레이 제거
            thread::sleep(Duration::from_secs(3));
            write_show_signal();
            unsafe {
                PostMessageW(hwnd, WM_DESTROY, WPARAM(0), LPARAM(0)).ok();
            }
            return;
        }
    }
}

fn build_tip(active: usize, total: usize, is_complete: bool) -> String {
    if is_complete {
        "BootReady — 부팅 완료".to_string()
    } else if total == 0 {
        "BootReady — 시작프로그램 없음".to_string()
    } else {
        format!("BootReady — {}/{} 프로그램 시작됨", active, total)
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
                WM_LBUTTONDOWN | 0x0203 /* WM_LBUTTONDBLCLK */ => {
                    launch_ui();
                }
                WM_RBUTTONDOWN => {
                    // 컨텍스트 메뉴는 추후 구현
                }
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
    // 신호 파일 방식: Electron이 감지하면 창을 표시
    write_show_signal();

    // exe가 있으면 새 프로세스로 실행 (설치본)
    if let Some(path) = find_ui_exe() {
        info!("launching UI exe: {:?}", path);
        let _ = std::process::Command::new(path)
            .spawn()
            .map_err(|e| error!("failed to launch bootready-ui: {e}"));
    }
}

fn write_show_signal() {
    let appdata = match std::env::var("APPDATA") {
        Ok(v) => std::path::PathBuf::from(v),
        Err(_) => return,
    };
    let dir = appdata.join("BootReady");
    let _ = std::fs::create_dir_all(&dir);
    let signal = dir.join("show.signal");
    if let Err(e) = std::fs::write(&signal, b"show") {
        error!("failed to write show signal: {e}");
    } else {
        info!("show signal written");
    }
}

fn find_ui_exe() -> Option<std::path::PathBuf> {
    // Tauri currentUser 설치 경로: %LOCALAPPDATA%\Programs\BootReady\BootReady.exe
    if let Ok(localappdata) = std::env::var("LOCALAPPDATA") {
        let path = std::path::PathBuf::from(localappdata)
            .join("Programs")
            .join("BootReady")
            .join("BootReady.exe");
        if path.exists() {
            return Some(path);
        }
    }
    // boot-core.exe와 같은 폴더 (개발/이전 구조 fallback)
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            for name in &["BootReady.exe", "bootready-ui.exe"] {
                let sibling = parent.join(name);
                if sibling.exists() {
                    return Some(sibling);
                }
            }
        }
    }
    None
}

unsafe fn add_tray_icon(hwnd: HWND, icon: HICON, tip: &str) {
    let tip_buf = encode_tip(tip);
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

unsafe fn modify_tray_icon(hwnd: HWND, icon: HICON, tip: &str) {
    let tip_buf = encode_tip(tip);
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
    Shell_NotifyIconW(NIM_MODIFY, &mut nid);
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

fn encode_tip(tip: &str) -> [u16; 128] {
    let mut buf = [0u16; 128];
    for (i, c) in tip.encode_utf16().enumerate().take(127) {
        buf[i] = c;
    }
    buf
}

// ──────────────────────────────────────────────────────────────────────────
// 아이콘 렌더링
// ──────────────────────────────────────────────────────────────────────────

/// Ring + Dot 아이콘 생성.
/// - `progress`: 0..1, 회색 진행 호의 길이 (12시 시작, 시계방향)
/// - `ready`:    true이면 초록 닫힌 링 + 중앙 점
fn create_ring_icon(progress: f32, ready: bool) -> Result<HICON> {
    const SIZE: i32 = 16;
    const SZ: usize = SIZE as usize;

    let (arc_color, has_dot, has_track) = if ready {
        (COLOR_ACCENT, true, false)
    } else {
        (COLOR_ARC_IDLE, false, true)
    };

    // 완료 시 닫힌 원(100%), 진행 중엔 실제 진행률
    let effective_progress = if ready { 1.0 } else { progress };

    let inner_r = RING_R - RING_STROKE / 2.0;
    let outer_r = RING_R + RING_STROKE / 2.0;
    let center = (SZ as f32 - 1.0) / 2.0;
    // 한 픽셀이 차지하는 각도(0..1 비율). 호의 시작/끝을 부드럽게 깎는 데 사용.
    let angular_pixel = 1.0 / (2.0 * std::f32::consts::PI * RING_R);

    let mut pixels = vec![0u32; SZ * SZ];

    for y in 0..SZ {
        for x in 0..SZ {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            // 중앙 점 (ready 상태 한정)
            if has_dot {
                let dot_a = (DOT_R + 0.5 - dist).clamp(0.0, 1.0);
                if dot_a > 0.0 {
                    pixels[y * SZ + x] = pack_bgra(arc_color, dot_a);
                    continue;
                }
            }

            // 링 밴드 반경 안티앨리어싱: 안쪽/바깥쪽 둘 다 1px 깃털
            let band_a = (outer_r + 0.5 - dist)
                .min(dist - inner_r + 0.5)
                .clamp(0.0, 1.0);
            if band_a <= 0.0 {
                continue;
            }

            // 12시 기준, 시계 방향 각도 (0..1)
            let raw = dx.atan2(-dy); // [-π, π], 12시=0, 3시=π/2, 6시=π, 9시=-π/2
            let mut t = raw / (2.0 * std::f32::consts::PI);
            if t < 0.0 {
                t += 1.0;
            }

            // 호 안에 있는지 — 끝 모서리만 1픽셀 폭으로 스무딩 (12시 시작은 자연 경계)
            let arc_factor = smoothstep(0.0, angular_pixel, effective_progress - t);
            let arc_a = band_a * arc_factor;

            // 트랙 (회색, 진행 중에만)
            let track_a = if has_track { band_a * TRACK_ALPHA } else { 0.0 };

            // 호를 트랙 위에 합성 (over operator, pre-multiplied)
            let one_minus_arc = 1.0 - arc_a;
            let final_a = arc_a + track_a * one_minus_arc;
            if final_a <= 0.0 {
                continue;
            }

            // pre-multiplied 색상
            let r = arc_color.0 as f32 * arc_a + COLOR_TRACK.0 as f32 * track_a * one_minus_arc;
            let g = arc_color.1 as f32 * arc_a + COLOR_TRACK.1 as f32 * track_a * one_minus_arc;
            let b = arc_color.2 as f32 * arc_a + COLOR_TRACK.2 as f32 * track_a * one_minus_arc;

            let pa = (final_a * 255.0).min(255.0) as u32;
            let pr = r.min(255.0) as u32;
            let pg = g.min(255.0) as u32;
            let pb = b.min(255.0) as u32;
            // 메모리: B G R A (little-endian u32 = 0xAARRGGBB)
            pixels[y * SZ + x] = pb | (pg << 8) | (pr << 16) | (pa << 24);
        }
    }

    unsafe { build_hicon_from_pixels(&pixels, SIZE) }
}

#[inline]
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    if x <= edge0 {
        return 0.0;
    }
    if x >= edge1 {
        return 1.0;
    }
    let t = (x - edge0) / (edge1 - edge0);
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn pack_bgra(rgb: (u8, u8, u8), alpha: f32) -> u32 {
    let a = (alpha * 255.0).min(255.0) as u32;
    let r = (rgb.0 as f32 * alpha).min(255.0) as u32;
    let g = (rgb.1 as f32 * alpha).min(255.0) as u32;
    let b = (rgb.2 as f32 * alpha).min(255.0) as u32;
    b | (g << 8) | (r << 16) | (a << 24)
}

/// 32bpp BGRA 픽셀 버퍼로부터 HICON 생성
unsafe fn build_hicon_from_pixels(pixels: &[u32], size: i32) -> Result<HICON> {
    let sz = size as usize;

    let dc = CreateCompatibleDC(None);

    let bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size,
            biHeight: -size, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: 0, // BI_RGB
            ..Default::default()
        },
        ..Default::default()
    };

    let mut bits_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
    let color_bmp = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits_ptr, None, 0)
        .context("CreateDIBSection failed")?;

    std::slice::from_raw_parts_mut(bits_ptr as *mut u32, sz * sz).copy_from_slice(pixels);

    DeleteDC(dc);

    // 1-bit 마스크: 전부 0 → color bitmap 알파 채널을 그대로 사용
    let mask_bytes = vec![0u8; sz * 4]; // DWORD 정렬 (16px → 4바이트/행)
    let mask_bmp = CreateBitmap(size, size, 1, 1, Some(mask_bytes.as_ptr() as *const _));
    if mask_bmp.is_invalid() {
        DeleteObject(color_bmp);
        return Err(anyhow::anyhow!("CreateBitmap mask failed"));
    }

    let icon = CreateIconIndirect(&ICONINFO {
        fIcon: true.into(),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask_bmp,
        hbmColor: color_bmp,
    })
    .context("CreateIconIndirect failed")?;

    DeleteObject(color_bmp);
    DeleteObject(mask_bmp);

    Ok(icon)
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
            Some((&hinstance).into()),
            None,
        )
        .context("CreateWindowEx failed")?;

        Ok(hwnd)
    }
}
