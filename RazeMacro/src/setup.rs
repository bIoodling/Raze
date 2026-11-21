#![windows_subsystem = "windows"]
use std::{
    ffi::c_void,
    fs,
    mem::{size_of, zeroed},
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Child, Command},
    ptr::{null, null_mut},
    time::Instant,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, WindowsAndMessaging::*},
};
const PAYLOAD: &[u8] = include_bytes!(env!("RAZE_INNER_SETUP"));
const BG: COLORREF = 0x100908;
const PURPLE: COLORREF = 0xF126AD;
const WHITE: COLORREF = 0xF4EFF8;
const MUTED: COLORREF = 0xB6A9BB;
static mut FONT_NORMAL: HFONT = null_mut();
static mut FONT_TITLE: HFONT = null_mut();
struct Setup {
    stage: u8,
    desktop: bool,
    launch: bool,
    child: Option<Child>,
    temp: Option<PathBuf>,
    started: Instant,
    error: String,
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}
unsafe fn fill_round(hdc: HDC, r: RECT, radius: i32, color: COLORREF) {
    let brush = CreateSolidBrush(color);
    let old_brush = SelectObject(hdc, brush as HGDIOBJ);
    let old_pen = SelectObject(hdc, GetStockObject(NULL_PEN) as HGDIOBJ);
    RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, old_pen);
    SelectObject(hdc, old_brush);
    DeleteObject(brush as HGDIOBJ);
}
unsafe fn stroke_round(hdc: HDC, r: RECT, radius: i32, color: COLORREF) {
    let pen = CreatePen(PS_SOLID, 1, color);
    let old_pen = SelectObject(hdc, pen as HGDIOBJ);
    let old_brush = SelectObject(hdc, GetStockObject(HOLLOW_BRUSH) as HGDIOBJ);
    RoundRect(hdc, r.left, r.top, r.right, r.bottom, radius, radius);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    DeleteObject(pen as HGDIOBJ);
}
unsafe fn text(hdc: HDC, s: &str, r: RECT, color: COLORREF, font: HFONT, flags: u32) {
    let w = wide(s);
    let old = SelectObject(hdc, font as HGDIOBJ);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);
    let mut rr = r;
    DrawTextW(hdc, w.as_ptr(), -1, &mut rr, flags);
    SelectObject(hdc, old);
}
unsafe fn logo(hdc: HDC, x: i32, y: i32, size: i32) {
    // Vector reconstruction of the supplied angular R mark, kept sharp at every DPI.
    let coords = [
        (8, 0),
        (84, 0),
        (90, 8),
        (74, 39),
        (60, 39),
        (76, 65),
        (53, 65),
        (37, 39),
        (24, 39),
        (11, 65),
        (0, 65),
        (14, 30),
        (21, 26),
        (59, 26),
        (65, 14),
        (16, 14),
    ];
    let points: Vec<POINT> = coords
        .into_iter()
        .map(|(a, b)| POINT {
            x: x + a * size / 90,
            y: y + b * size / 90,
        })
        .collect();
    let brush = CreateSolidBrush(PURPLE);
    let old = SelectObject(hdc, brush as HGDIOBJ);
    let pen = SelectObject(hdc, GetStockObject(NULL_PEN) as HGDIOBJ);
    Polygon(hdc, points.as_ptr(), points.len() as i32);
    SelectObject(hdc, pen);
    SelectObject(hdc, old);
    DeleteObject(brush as HGDIOBJ);
}
fn stage_payload() -> std::io::Result<PathBuf> {
    let root = std::env::temp_dir().join(format!(
        "RazeSetup-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root)?;
    let path = root.join("Raze-Core-Setup.exe");
    fs::write(&path, PAYLOAD)?;
    Ok(path)
}
fn cleanup(path: &std::path::Path) {
    let _ = fs::remove_file(path);
    if let Some(parent) = path.parent() {
        let _ = fs::remove_dir(parent);
    }
}
unsafe fn state(hwnd: HWND) -> &'static mut Setup {
    &mut *(GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Setup)
}
unsafe fn start(hwnd: HWND) {
    let s = state(hwnd);
    if s.stage == 1 {
        return;
    }
    let result = stage_payload().and_then(|path| {
        let child = Command::new(&path)
            .args([
                "/VERYSILENT",
                "/SUPPRESSMSGBOXES",
                "/NORESTART",
                if s.desktop {
                    "/TASKS=desktopicon"
                } else {
                    "/TASKS="
                },
            ])
            .creation_flags(0x08000000)
            .spawn();
        match child {
            Ok(child) => Ok((path, child)),
            Err(e) => {
                cleanup(&path);
                Err(e)
            }
        }
    });
    match result {
        Ok((path, child)) => {
            s.temp = Some(path);
            s.child = Some(child);
            s.stage = 1;
            s.started = Instant::now();
            s.error.clear();
        }
        Err(_) => {
            s.error = "Unable to start installation".into();
            s.stage = 3;
        }
    }
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn finish(hwnd: HWND) {
    let s = state(hwnd);
    if s.launch {
        if let Some(base) = std::env::var_os("LOCALAPPDATA") {
            let exe = PathBuf::from(base)
                .join("Programs")
                .join("Raze Macro Development")
                .join("Raze-Macro.exe");
            if Command::new(exe)
                .creation_flags(0x08000000)
                .spawn()
                .is_err()
            {
                s.error = "Launch failed. Use Start Menu.".into();
                InvalidateRect(hwnd, null(), 0);
                return;
            }
        }
    }
    DestroyWindow(hwnd);
}
unsafe fn poll(hwnd: HWND) {
    let s = state(hwnd);
    if s.stage != 1 {
        return;
    }
    let result = s.child.as_mut().unwrap().try_wait();
    match result {
        Ok(Some(code)) => {
            s.stage = if code.success() { 2 } else { 3 };
            if !code.success() {
                s.error = "Installation failed. Try again.".into();
            }
            s.child = None;
            if let Some(p) = s.temp.take() {
                cleanup(&p);
            }
        }
        Err(_) => {
            s.error = "Unable to read install status".into();
        }
        Ok(None) => {}
    }
    InvalidateRect(hwnd, null(), 0);
}
unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    let mut r: RECT = zeroed();
    GetClientRect(hwnd, &mut r);
    let mem = CreateCompatibleDC(dc);
    let bmp = CreateCompatibleBitmap(dc, r.right.max(1), r.bottom.max(1));
    let old = SelectObject(mem, bmp as HGDIOBJ);
    let dpi = GetDpiForWindow(hwnd) as i32;
    SetMapMode(mem, MM_ANISOTROPIC);
    SetWindowExtEx(mem, 96, 96, null_mut());
    SetViewportExtEx(mem, dpi, dpi, null_mut());
    draw_setup(mem, state(hwnd));
    SetMapMode(mem, MM_TEXT);
    BitBlt(dc, 0, 0, r.right, r.bottom, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old);
    DeleteObject(bmp as HGDIOBJ);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = &*(lp as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, cs.lpCreateParams as isize);
            SetTimer(hwnd, 1, 16, None);
            0
        }
        WM_PAINT => {
            paint(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_TIMER => {
            poll(hwnd);
            0
        }
        WM_SIZE => {
            let mut r: RECT = zeroed();
            GetClientRect(hwnd, &mut r);
            let radius = (14 * GetDpiForWindow(hwnd) / 96) as i32;
            SetWindowRgn(
                hwnd,
                CreateRoundRectRgn(0, 0, r.right + 1, r.bottom + 1, radius, radius),
                0,
            );
            0
        }
        WM_DPICHANGED => {
            let r = &*(lp as *const RECT);
            let dpi = (wp & 0xFFFF) as i32;
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                260 * dpi / 96,
                218 * dpi / 96,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            0
        }
        WM_NCHITTEST => {
            let mut p = POINT {
                x: lp as i16 as i32,
                y: ((lp >> 16) as i16) as i32,
            };
            ScreenToClient(hwnd, &mut p);
            let dpi = GetDpiForWindow(hwnd) as i32;
            if p.y < 44 * dpi / 96 && p.x < 190 * dpi / 96 {
                HTCAPTION as isize
            } else {
                HTCLIENT as isize
            }
        }
        WM_LBUTTONDOWN => {
            let dpi = GetDpiForWindow(hwnd) as i32;
            let x = (lp as i16 as i32) * 96 / dpi;
            let y = (((lp >> 16) as i16) as i32) * 96 / dpi;
            if y < 44 && x >= 228 {
                SendMessageW(hwnd, WM_CLOSE, 0, 0);
            } else if y < 44 && x >= 196 {
                ShowWindow(hwnd, SW_MINIMIZE);
            } else if (110..138).contains(&y) {
                let s = state(hwnd);
                if s.stage == 2 {
                    s.launch = !s.launch;
                } else if s.stage != 1 {
                    s.desktop = !s.desktop;
                }
                InvalidateRect(hwnd, null(), 0);
            } else if (156..194).contains(&y) && (24..236).contains(&x) {
                if state(hwnd).stage == 2 {
                    finish(hwnd);
                } else {
                    start(hwnd);
                }
            }
            0
        }
        WM_KEYDOWN => {
            match wp as u16 {
                0x0D => {
                    if state(hwnd).stage == 2 {
                        finish(hwnd);
                    } else {
                        start(hwnd);
                    }
                }
                0x20 => {
                    let s = state(hwnd);
                    if s.stage == 2 {
                        s.launch = !s.launch;
                    } else if s.stage != 1 {
                        s.desktop = !s.desktop;
                    }
                    InvalidateRect(hwnd, null(), 0);
                }
                0x1B => {
                    SendMessageW(hwnd, WM_CLOSE, 0, 0);
                }
                _ => {}
            }
            0
        }
        WM_CLOSE => {
            if state(hwnd).stage != 1 {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
fn main() {
    // CI and enterprise silent switches pass through to the embedded installation engine.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|s| matches!(s.to_ascii_uppercase().as_str(), "/VERYSILENT" | "/SILENT"))
    {
        let code = match stage_payload() {
            Ok(path) => {
                let status = Command::new(&path)
                    .args(&args)
                    .creation_flags(0x08000000)
                    .status();
                cleanup(&path);
                status.ok().and_then(|s| s.code()).unwrap_or(1)
            }
            Err(_) => 1,
        };
        std::process::exit(code);
    }
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        FONT_NORMAL = CreateFontW(
            -14,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Consolas").as_ptr(),
        );
        FONT_TITLE = CreateFontW(
            -18,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            CLEARTYPE_QUALITY as u32,
            0,
            wide("Consolas").as_ptr(),
        );
        if args.iter().any(|a| a == "--render-previews") {
            render_setup_previews();
            return;
        }
        let class = wide("RazeMacroSetup");
        let instance = GetModuleHandleW(null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            hIcon: LoadIconW(instance, 1usize as *const u16),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let mut setup = Box::new(Setup {
            stage: 0,
            desktop: false,
            launch: true,
            child: None,
            temp: None,
            started: Instant::now(),
            error: String::new(),
        });
        let dpi = GetDpiForSystem() as i32;
        let hwnd = CreateWindowExW(
            WS_EX_APPWINDOW,
            class.as_ptr(),
            wide("Raze Macro Setup").as_ptr(),
            WS_POPUP,
            (GetSystemMetrics(SM_CXSCREEN) - 260 * dpi / 96) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - 218 * dpi / 96) / 2,
            260 * dpi / 96,
            218 * dpi / 96,
            null_mut(),
            null_mut(),
            instance,
            (&mut *setup) as *mut Setup as *const c_void,
        );
        if hwnd.is_null() {
            return;
        }
        ShowWindow(hwnd, SW_SHOW);
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        DeleteObject(FONT_NORMAL as HGDIOBJ);
        DeleteObject(FONT_TITLE as HGDIOBJ);
    }
}

unsafe fn draw_setup(mem: HDC, s: &Setup) {
    fill_round(mem, rect(0, 0, 260, 218), 14, BG);
    stroke_round(mem, rect(0, 0, 259, 217), 14, PURPLE);
    logo(mem, 18, 14, 27);
    text(
        mem,
        "R A Z E",
        rect(60, 8, 188, 43),
        WHITE,
        FONT_TITLE,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    text(
        mem,
        "—",
        rect(196, 8, 222, 43),
        WHITE,
        FONT_NORMAL,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    text(
        mem,
        "×",
        rect(228, 8, 252, 43),
        WHITE,
        FONT_TITLE,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    let label = match s.stage {
        0 => "v1.0.0-dev",
        1 => "Installing...",
        2 => "Installation Complete",
        _ => &s.error,
    };
    text(
        mem,
        label,
        rect(14, 60, 246, 104),
        WHITE,
        if s.stage == 0 {
            FONT_TITLE
        } else {
            FONT_NORMAL
        },
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    if s.stage == 1 {
        fill_round(mem, rect(22, 126, 238, 138), 10, 0x281A25);
        let t = s.started.elapsed().as_secs_f64();
        let x = 22.0 + ((t * 0.9).sin() * 0.5 + 0.5) * 144.0;
        fill_round(mem, rect(x as i32, 126, x as i32 + 72, 138), 10, PURPLE);
    } else {
        let checked = if s.stage == 2 { s.launch } else { s.desktop };
        stroke_round(mem, rect(25, 114, 40, 129), 4, PURPLE);
        if checked {
            text(
                mem,
                "✓",
                rect(24, 110, 43, 134),
                PURPLE,
                FONT_NORMAL,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
        text(
            mem,
            if s.stage == 2 {
                "Launch after install"
            } else {
                "Desktop shortcut"
            },
            rect(48, 110, 242, 134),
            MUTED,
            FONT_NORMAL,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE,
        );
        fill_round(mem, rect(24, 156, 236, 194), 9, PURPLE);
        text(
            mem,
            if s.stage == 2 {
                if s.launch {
                    "Launch"
                } else {
                    "Finish"
                }
            } else if s.stage == 3 {
                "Retry"
            } else {
                "Install"
            },
            rect(24, 156, 236, 194),
            WHITE,
            FONT_TITLE,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );
    }
}

unsafe fn render_setup_previews() {
    fs::create_dir_all("previews").unwrap();
    for (name, stage) in [
        ("installer-start", 0),
        ("installer-progress", 1),
        ("installer-complete", 2),
    ] {
        let mut info: BITMAPINFO = zeroed();
        info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = 260;
        info.bmiHeader.biHeight = -218;
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let dc = CreateCompatibleDC(null_mut());
        let mut pixels: *mut c_void = null_mut();
        let bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
        assert!(!bmp.is_null());
        let old = SelectObject(dc, bmp as HGDIOBJ);
        let s = Setup {
            stage,
            desktop: false,
            launch: true,
            child: None,
            temp: None,
            started: Instant::now(),
            error: String::new(),
        };
        draw_setup(dc, &s);
        GdiFlush();
        let len = 260 * 218 * 4usize;
        let mut bytes = Vec::with_capacity(54 + len);
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&((54 + len) as u32).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 4]);
        bytes.extend_from_slice(&54u32.to_le_bytes());
        bytes.extend_from_slice(&40u32.to_le_bytes());
        bytes.extend_from_slice(&260i32.to_le_bytes());
        bytes.extend_from_slice(&(-218i32).to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&32u16.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 24]);
        bytes.extend_from_slice(std::slice::from_raw_parts(pixels as *const u8, len));
        fs::write(format!("previews/{name}-96dpi.bmp"), bytes).unwrap();
        SelectObject(dc, old);
        DeleteObject(bmp as HGDIOBJ);
        DeleteDC(dc);
    }
}
