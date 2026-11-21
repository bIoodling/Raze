#![windows_subsystem = "windows"]
use std::{
    ffi::c_void,
    fs,
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
    thread,
    time::Instant,
};
use windows_sys::Win32::UI::HiDpi::*;
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{
        LibraryLoader::GetModuleHandleW,
        Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
        Threading::*,
    },
    UI::{
        Input::{
            GetRawInputData, KeyboardAndMouse::*, RegisterRawInputDevices, RAWINPUT,
            RAWINPUTDEVICE, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
        Shell::*,
        WindowsAndMessaging::*,
    },
};
mod model;
use model::*;
include!("engine.rs");
include!("input.rs");

const WM_REFRESH: u32 = WM_APP + 10;
const WM_TRAY: u32 = WM_APP + 1;
const INJECTED_TAG: usize = 0x4849_4B41;
const WIDTH: i32 = 376;
const ROWS: [i32; 6] = [76, 132, 188, 244, 300, 356];
const FIELD_H: i32 = 36;
const CONTROL_X: i32 = 154;
const CONTROL_R: i32 = 352;
const GAP: i32 = 10;
const BG: COLORREF = 0x100908;
const PURPLE: COLORREF = 0xF126AD;
const WHITE: COLORREF = 0xF4EFF8;
const MUTED: COLORREF = 0xB6A9BB;
const OUTLINE: COLORREF = 0xA866B8;
const EM_SETSEL: u32 = 0x00B1;
const ID_EDIT: usize = 100;
const EN_KILLFOCUS: usize = 0x0200;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Compact,
    Settings,
    Confirm,
    Activate,
}
fn screen_size(s: Screen) -> (i32, i32) {
    match s {
        Screen::Compact => (308, 228),
        Screen::Settings => (WIDTH, 434),
        Screen::Confirm => (292, 180),
        Screen::Activate => (292, 220),
    }
}
fn ease(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
struct Transition {
    start: Instant,
    from: (i32, i32),
    to: (i32, i32),
}
struct UiState {
    screen: Screen,
    previous: Screen,
    capture: CaptureTarget,
    macro_down: bool,
    ui_down: bool,
    focus: usize,
    feedback: String,
    animation: Option<Transition>,
    scale: f64,
    suppress_key: Option<u16>,
}
struct Shared {
    config: Mutex<Config>,
    active: AtomicBool,
    desired_active: AtomicBool,
    exiting: AtomicBool,
    stop_event: isize,
    wake_event: isize,
}
static SHARED: OnceLock<Arc<Shared>> = OnceLock::new();
static UI: OnceLock<Mutex<UiState>> = OnceLock::new();
static ACTIVATED: AtomicBool = AtomicBool::new(false);
static mut HWND_MAIN: HWND = null_mut();
static mut EDIT: HWND = null_mut();
static mut EDIT_PROC: WNDPROC = None;
static mut EDIT_BRUSH: HBRUSH = null_mut();
static mut FONT_NORMAL: HFONT = null_mut();
static mut FONT_SMALL: HFONT = null_mut();
static mut FONT_TITLE: HFONT = null_mut();
static mut FONT_STATUS: HFONT = null_mut();
static mut FONT_EDIT: HFONT = null_mut();
static mut ICON: HICON = null_mut();

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}
fn rect(l: i32, t: i32, r: i32, b: i32) -> RECT {
    RECT {
        left: l,
        top: t,
        right: r,
        bottom: b,
    }
}
fn point_in(r: RECT, x: i32, y: i32) -> bool {
    x >= r.left && x < r.right && y >= r.top && y < r.bottom
}
fn lparam_xy(lp: LPARAM) -> (i32, i32) {
    (
        ((lp as i16) as i32),
        (((lp >> 16) as i16) as i32),
    )
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
fn data_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("RazeMacro")
        .join("Development")
}
fn save_config(c: Config) -> std::io::Result<()> {
    fs::create_dir_all(data_dir())?;
    fs::write(data_dir().join("config.ini"), encode_config(c))
}
fn snapshot() -> Config {
    *SHARED.get().unwrap().config.lock().unwrap()
}
fn mutate_config(f: impl FnOnce(&mut Config)) {
    set_active(false);
    let mut c = SHARED.get().unwrap().config.lock().unwrap();
    f(&mut c);
    let copy = *c;
    drop(c);
    if save_config(copy).is_err() {
        feedback("Could not save settings");
    }
    unsafe {
        SetEvent(SHARED.get().unwrap().wake_event as HANDLE);
        InvalidateRect(HWND_MAIN, null(), 0);
    }
}
fn feedback(s: &str) {
    UI.get().unwrap().lock().unwrap().feedback = s.into();
    unsafe {
        InvalidateRect(HWND_MAIN, null(), 0);
    }
}
fn license_is_valid() -> bool {
    ACTIVATED.load(Ordering::Acquire)
}
fn modifiers_down() -> bool {
    unsafe {
        [VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|&k| GetAsyncKeyState(k as i32) < 0)
    }
}
fn set_active(value: bool) {
    let shared = SHARED.get().unwrap();
    let allowed = value && license_is_valid() && can_activate(snapshot()) && !modifiers_down();
    shared.desired_active.store(allowed, Ordering::Release);
    shared.active.store(allowed, Ordering::Release);
    unsafe {
        SetEvent(shared.wake_event as HANDLE);
        if !HWND_MAIN.is_null() {
            PostMessageW(HWND_MAIN, WM_REFRESH, 0, 0);
        }
    }
}
fn inject_binding(b: KeyBinding) {
    // Never combine generated input with held Windows/modifier shortcuts.
    if modifiers_down() {
        set_active(false);
        return;
    }
    inject_binding_unchecked(b);
}
unsafe fn process_binding_event(binding: KeyBinding, down: bool, up: bool) {
    let (capture, screen) = {
        let ui = UI.get().unwrap().lock().unwrap();
        (ui.capture, ui.screen)
    };
    if modifiers_down() {
        set_active(false);
        if capture != CaptureTarget::None && down {
            feedback("Use one key without modifiers");
        }
        if up {
            let mut ui = UI.get().unwrap().lock().unwrap();
            ui.macro_down = false;
            ui.ui_down = false;
        }
        return;
    }
    if capture != CaptureTarget::None {
        if GetForegroundWindow() != HWND_MAIN {
            return;
        }
        if down {
            if binding.kind == BindKind::Keyboard {
                UI.get().unwrap().lock().unwrap().suppress_key = Some(binding.vk);
            }
            if binding.kind == BindKind::Keyboard && binding.vk == VK_ESCAPE {
                UI.get().unwrap().lock().unwrap().capture = CaptureTarget::None;
                feedback("");
                return;
            }
            if binding.kind == BindKind::Keyboard && matches!(binding.vk, 8 | 46) {
                capture_binding(KeyBinding::none());
            } else {
                capture_binding(binding);
            }
        }
        return;
    }
    let c = snapshot();
    // UI typing/navigation must never start the engine.
    let editing = GetForegroundWindow() == HWND_MAIN && screen != Screen::Compact;
    if !c.ui_key.is_none() && binding.same_key(c.ui_key) && !editing {
        let mut ui = UI.get().unwrap().lock().unwrap();
        if down && !ui.ui_down {
            ui.ui_down = true;
            drop(ui);
            set_active(false);
            if IsWindowVisible(HWND_MAIN) != 0 {
                ShowWindow(HWND_MAIN, SW_HIDE);
            } else {
                ShowWindow(HWND_MAIN, SW_SHOW);
                SetForegroundWindow(HWND_MAIN);
            }
        } else if up {
            ui.ui_down = false;
        }
    }
    if !c.macro_key.is_none() && binding.same_key(c.macro_key) {
        let mut ui = UI.get().unwrap().lock().unwrap();
        if up {
            ui.macro_down = false;
            drop(ui);
            if c.behavior == Behavior::Hold {
                set_active(false);
            }
        } else if down && !ui.macro_down && !editing {
            ui.macro_down = true;
            drop(ui);
            let next = c.behavior == Behavior::Hold
                || !SHARED.get().unwrap().active.load(Ordering::Acquire);
            if can_activate(c) {
                set_active(next);
            } else {
                feedback("Assign the required bindings in settings");
            }
        }
    }
    // Any reserved keyboard key also stops input (e.g. Esc or the secure attention chord).
    if down && binding.kind == BindKind::Keyboard && !safe_binding(binding) {
        set_active(false);
    }
}
unsafe fn capture_binding(b: KeyBinding) {
    let target = UI.get().unwrap().lock().unwrap().capture;
    let mut c = snapshot();
    match assign(&mut c, target, b) {
        Ok(()) => {
            mutate_config(|cfg| *cfg = c);
            let mut ui = UI.get().unwrap().lock().unwrap();
            ui.capture = CaptureTarget::None;
            ui.feedback = if b.is_none() {
                "Binding cleared".into()
            } else {
                String::new()
            };
        }
        Err(message) => feedback(message),
    }
    InvalidateRect(HWND_MAIN, null(), 0);
}
fn key_name(b: KeyBinding) -> String {
    match b.kind {
        BindKind::None => "Click to bind".into(),
        BindKind::Mouse => format!("Mouse{}", b.mouse_button),
        BindKind::Keyboard => match b.vk {
            0x30..=0x39 | 0x41..=0x5A => char::from_u32(b.vk as u32).unwrap().to_string(),
            0x70..=0x7A => format!("F{}", b.vk - 0x70 + 1),
            0x60..=0x69 => format!("Num {}", b.vk - 0x60),
            _ => format!("Num 0x{:X}", b.vk),
        },
    }
}
unsafe fn create_font(size: i32, weight: i32) -> HFONT {
    // Windows-provided Consolas: use the installed font; do not redistribute it.
    CreateFontW(
        -size,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        CLEARTYPE_QUALITY as u32,
        DEFAULT_PITCH as u32,
        wide("Consolas").as_ptr(),
    )
}
fn scale() -> f64 {
    UI.get().unwrap().lock().unwrap().scale
}
fn px(v: i32) -> i32 {
    (v as f64 * scale()).round() as i32
}
unsafe fn init_fonts() {
    FONT_NORMAL = create_font(17, 400);
    FONT_SMALL = create_font(13, 400);
    FONT_TITLE = create_font(20, 600);
    FONT_STATUS = create_font(46, 700);
    EDIT_BRUSH = CreateSolidBrush(rgb(17, 13, 23));
    FONT_EDIT = create_font(px(17), 400);
}
fn field(row: usize) -> RECT {
    rect(CONTROL_X, ROWS[row], CONTROL_R, ROWS[row] + FIELD_H)
}
fn pair(row: usize, second: bool) -> RECT {
    let half = (CONTROL_R - CONTROL_X - GAP) / 2;
    let x = if second {
        CONTROL_X + half + GAP
    } else {
        CONTROL_X
    };
    rect(x, ROWS[row], x + half, ROWS[row] + FIELD_H)
}
unsafe fn line(hdc: HDC, x: i32, y: i32, r: i32, color: COLORREF) {
    let pen = CreatePen(PS_SOLID, 1, color);
    let old = SelectObject(hdc, pen as HGDIOBJ);
    MoveToEx(hdc, x, y, null_mut());
    LineTo(hdc, r, y);
    SelectObject(hdc, old);
    DeleteObject(pen as HGDIOBJ);
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
            y: y + b * size / 65,
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
unsafe fn button(hdc: HDC, r: RECT, label: &str, selected: bool, focus: bool, small: bool) {
    fill_round(hdc, r, 9, if selected { PURPLE } else { rgb(17, 13, 23) });
    stroke_round(hdc, r, 9, if focus { WHITE } else { OUTLINE });
    text(
        hdc,
        label,
        r,
        if small { MUTED } else { WHITE },
        if small { FONT_SMALL } else { FONT_NORMAL },
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
}
unsafe fn header(hdc: HDC, screen: Screen, w: i32) {
    let x = if screen == Screen::Settings {
        26
    } else {
        logo(hdc, 24, 16, 30);
        74
    };
    text(
        hdc,
        "R A Z E",
        rect(x, 10, w - 78, 44),
        WHITE,
        FONT_TITLE,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE,
    );
    text(
        hdc,
        "—",
        rect(w - 80, 10, w - 46, 44),
        WHITE,
        FONT_NORMAL,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    text(
        hdc,
        "×",
        rect(w - 44, 10, w - 12, 44),
        WHITE,
        FONT_TITLE,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE,
    );
    line(hdc, 14, 54, w - 14, OUTLINE);
}
unsafe fn draw_content(hdc: HDC, screen: Screen, animated: bool, preview: bool) {
    let (w, h) = screen_size(screen);
    fill_round(hdc, rect(0, 0, w, h), 16, BG);
    stroke_round(hdc, rect(0, 0, w - 1, h - 1), 16, PURPLE);
    header(hdc, screen, w);
    if animated {
        return;
    }
    let c = snapshot();
    let active = SHARED.get().unwrap().active.load(Ordering::Acquire);
    let (capture, focus, message) = {
        let ui = UI.get().unwrap().lock().unwrap();
        (ui.capture, ui.focus, ui.feedback.clone())
    };
    match screen {
        Screen::Settings => {
            for (i, label) in [
                "Mode",
                "Rate",
                "Spam Key",
                "Hold Key",
                "Toggle UI",
                "Behaviour",
            ]
            .iter()
            .enumerate()
            {
                text(
                    hdc,
                    label,
                    rect(28, ROWS[i], 150, ROWS[i] + FIELD_H),
                    WHITE,
                    FONT_NORMAL,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );
            }
            button(
                hdc,
                pair(0, false),
                "CPS",
                c.mode == Mode::Cps,
                focus == 0,
                false,
            );
            button(
                hdc,
                pair(0, true),
                "KPS",
                c.mode == Mode::Kps,
                focus == 1,
                false,
            );
            fill_round(hdc, field(1), 9, rgb(17, 13, 23));
            stroke_round(hdc, field(1), 9, if focus == 2 { WHITE } else { OUTLINE });
            if preview {
                text(
                    hdc,
                    &c.rate.to_string(),
                    rect(166, 132, 340, 168),
                    WHITE,
                    FONT_NORMAL,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE,
                );
            }
            for (i, b, t) in [
                (2, c.spam, CaptureTarget::Spam),
                (3, c.macro_key, CaptureTarget::Macro),
                (4, c.ui_key, CaptureTarget::Ui),
            ] {
                let label = if capture == t {
                    "Press one key".into()
                } else {
                    key_name(b)
                };
                button(
                    hdc,
                    field(i),
                    &label,
                    false,
                    focus == i + 1 || capture == t,
                    b.is_none() && capture != t,
                );
            }
            button(
                hdc,
                pair(5, false),
                "Toggle",
                c.behavior == Behavior::Toggle,
                focus == 6,
                false,
            );
            button(
                hdc,
                pair(5, true),
                "Hold",
                c.behavior == Behavior::Hold,
                focus == 7,
                false,
            );
            text(
                hdc,
                if message.is_empty() {
                    "Esc: back  ·  Del: clear selected binding"
                } else {
                    &message
                },
                rect(20, 402, w - 20, 425),
                MUTED,
                FONT_SMALL,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
        Screen::Compact => {
            text(
                hdc,
                if active { "ON" } else { "OFF" },
                rect(30, 64, w - 25, 122),
                PURPLE,
                FONT_STATUS,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
            let rate = format!(
                "{} {}",
                if active { c.rate } else { 0 },
                if c.mode == Mode::Cps { "CPS" } else { "KPS" }
            );
            text(
                hdc,
                &rate,
                rect(34, 123, w - 24, 154),
                MUTED,
                FONT_TITLE,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
            line(hdc, 36, 168, w - 24, OUTLINE);
            text(
                hdc,
                if active { "ACTIVE" } else { "IDLE" },
                rect(34, 176, 230, 214),
                MUTED,
                FONT_TITLE,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE,
            );
            text(
                hdc,
                "···",
                rect(w - 60, 176, w - 18, 214),
                WHITE,
                FONT_TITLE,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
        Screen::Confirm => {
            text(
                hdc,
                "CLOSE RAZE?",
                rect(20, 64, w - 20, 106),
                WHITE,
                FONT_TITLE,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            button(
                hdc,
                rect(24, 120, 140, 158),
                "CANCEL",
                false,
                focus == 0,
                false,
            );
            button(
                hdc,
                rect(152, 120, w - 24, 158),
                "CLOSE",
                true,
                focus == 1,
                false,
            );
        }
        Screen::Activate => {
            text(
                hdc,
                "Enter License Key",
                rect(18, 66, w - 18, 94),
                WHITE,
                FONT_NORMAL,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
            stroke_round(hdc, rect(20, 100, w - 20, 134), 9, OUTLINE);
            if preview {
                text(
                    hdc,
                    "XXXX-XXXX-XXXX-XXXX",
                    rect(20, 100, w - 20, 134),
                    MUTED,
                    FONT_SMALL,
                    DT_CENTER | DT_VCENTER | DT_SINGLELINE,
                );
            }
            button(
                hdc,
                rect(20, 146, w - 20, 180),
                "Activate",
                true,
                focus == 1,
                false,
            );
            text(
                hdc,
                if message.is_empty() {
                    "DEVELOPMENT BUILD · OFFLINE ACTIVATION"
                } else {
                    &message
                },
                rect(12, 188, w - 12, 212),
                MUTED,
                FONT_SMALL,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE,
            );
        }
    }
}
unsafe fn paint_ui(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = zeroed();
    let dc = BeginPaint(hwnd, &mut ps);
    if dc.is_null() {
        return;
    }
    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    let mem = CreateCompatibleDC(dc);
    let bmp = CreateCompatibleBitmap(dc, client.right.max(1), client.bottom.max(1));
    let old = SelectObject(mem, bmp as HGDIOBJ);
    let brush = CreateSolidBrush(BG);
    FillRect(mem, &client, brush);
    DeleteObject(brush as HGDIOBJ);
    let (screen, animated) = {
        let ui = UI.get().unwrap().lock().unwrap();
        (ui.screen, ui.animation.is_some())
    };
    SetMapMode(mem, MM_ANISOTROPIC);
    SetWindowExtEx(mem, 1000, 1000, null_mut());
    let factor = (scale() * 1000.0).round() as i32;
    SetViewportExtEx(mem, factor, factor, null_mut());
    if animated {
        let w = (client.right as f64 / scale()).round() as i32;
        let h = (client.bottom as f64 / scale()).round() as i32;
        fill_round(mem, rect(0, 0, w, h), 16, BG);
        stroke_round(mem, rect(0, 0, w - 1, h - 1), 16, PURPLE);
        header(mem, screen, w);
    } else {
        draw_content(mem, screen, false, false);
    }
    SetMapMode(mem, MM_TEXT);
    BitBlt(dc, 0, 0, client.right, client.bottom, mem, 0, 0, SRCCOPY);
    SelectObject(mem, old);
    DeleteObject(bmp as HGDIOBJ);
    DeleteDC(mem);
    EndPaint(hwnd, &ps);
}
unsafe fn edit_text() -> String {
    let n = GetWindowTextLengthW(EDIT);
    let mut b = vec![0u16; n.max(0) as usize + 1];
    let n = GetWindowTextW(EDIT, b.as_mut_ptr(), b.len() as i32);
    String::from_utf16_lossy(&b[..n.max(0) as usize])
}
unsafe fn commit_rate() {
    if EDIT.is_null() {
        return;
    }
    match parse_rate(&edit_text()) {
        Ok(v) => {
            mutate_config(|c| c.rate = v);
            feedback("");
        }
        Err(s) => {
            feedback(s);
            SetWindowTextW(EDIT, wide(&snapshot().rate.to_string()).as_ptr());
        }
    }
}
unsafe extern "system" fn edit_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if msg == WM_KEYDOWN {
        let screen = UI.get().unwrap().lock().unwrap().screen;
        if wp == 0x41 && GetAsyncKeyState(VK_CONTROL as i32) < 0 {
            SendMessageW(hwnd, EM_SETSEL, 0, -1);
            return 0;
        }
        if wp == VK_ESCAPE as usize {
            if screen == Screen::Settings {
                SetWindowTextW(EDIT, wide(&snapshot().rate.to_string()).as_ptr());
                SetFocus(HWND_MAIN);
                transition(Screen::Compact);
            }
            return 0;
        }
        if wp == VK_TAB as usize {
            focus_next(GetAsyncKeyState(VK_SHIFT as i32) < 0);
            return 0;
        }
        if wp == VK_RETURN as usize {
            if screen == Screen::Settings {
                commit_rate();
                SetFocus(HWND_MAIN);
            } else {
                activate();
            }
            return 0;
        }
    }
    CallWindowProcW(EDIT_PROC, hwnd, msg, wp, lp)
}
unsafe fn destroy_edit() {
    if !EDIT.is_null() {
        let h = EDIT;
        EDIT = null_mut();
        DestroyWindow(h);
    }
}
unsafe fn create_edit() {
    destroy_edit();
    let screen = UI.get().unwrap().lock().unwrap().screen;
    let (r, value) = match screen {
        Screen::Settings => (rect(164, 140, 342, 162), snapshot().rate.to_string()),
        Screen::Activate => (rect(30, 107, 262, 129), String::new()),
        _ => return,
    };
    EDIT = CreateWindowExW(
        0,
        wide("EDIT").as_ptr(),
        wide(&value).as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
        px(r.left),
        px(r.top),
        px(r.right - r.left),
        px(r.bottom - r.top),
        HWND_MAIN,
        ID_EDIT as HMENU,
        GetModuleHandleW(null()),
        null(),
    );
    SendMessageW(EDIT, WM_SETFONT, FONT_EDIT as usize, 1);
    SendMessageW(
        EDIT,
        0x00C5,
        if screen == Screen::Settings { 12 } else { 64 },
        0,
    );
    EDIT_PROC = Some(std::mem::transmute::<
        isize,
        unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    >(SetWindowLongPtrW(
        EDIT,
        GWLP_WNDPROC,
        edit_proc as *const () as isize,
    )));
    if screen == Screen::Activate {
        SetFocus(EDIT);
    }
}
unsafe fn transition(target: Screen) {
    set_active(false);
    let current = UI.get().unwrap().lock().unwrap().screen;
    if current == Screen::Settings {
        commit_rate();
    }
    destroy_edit();
    SetFocus(HWND_MAIN);
    let mut r: RECT = zeroed();
    GetWindowRect(HWND_MAIN, &mut r);
    let to = screen_size(target);
    let to = (px(to.0), px(to.1));
    {
        let mut ui = UI.get().unwrap().lock().unwrap();
        if target == Screen::Confirm {
            ui.previous = current;
        }
        ui.screen = target;
        ui.capture = CaptureTarget::None;
        ui.focus = 0;
        ui.feedback.clear();
        ui.animation = Some(Transition {
            start: Instant::now(),
            from: (r.right - r.left, r.bottom - r.top),
            to,
        });
    }
    SetTimer(HWND_MAIN, 1, 6, None);
    InvalidateRect(HWND_MAIN, null(), 0);
}
unsafe fn tick_animation() {
    let dimensions = {
        let ui = UI.get().unwrap().lock().unwrap();
        ui.animation.as_ref().map(|a| {
            let t = (a.start.elapsed().as_secs_f64() / 0.18).clamp(0.0, 1.0);
            let e = ease(t);
            (
                ((a.from.0 as f64 + (a.to.0 - a.from.0) as f64 * e).round() as i32),
                ((a.from.1 as f64 + (a.to.1 - a.from.1) as f64 * e).round() as i32),
                t >= 1.0,
            )
        })
    };
    if let Some((w, h, done)) = dimensions {
        SetWindowPos(
            HWND_MAIN,
            null_mut(),
            0,
            0,
            w,
            h,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        if done {
            KillTimer(HWND_MAIN, 1);
            UI.get().unwrap().lock().unwrap().animation = None;
            create_edit();
        }
        InvalidateRect(HWND_MAIN, null(), 0);
    }
}
unsafe fn activate() {
    if dev_key_valid(&edit_text()) {
        let result = fs::create_dir_all(data_dir())
            .and_then(|_| fs::write(data_dir().join("development.activation"), "RAZE-DEV-2026"));
        if result.is_err() {
            feedback("Unable to save development activation");
            return;
        }
        ACTIVATED.store(true, Ordering::Release);
        transition(Screen::Compact);
    } else {
        feedback("Invalid development key");
    }
}
unsafe fn begin_capture(target: CaptureTarget) {
    set_active(false);
    commit_rate();
    SetFocus(HWND_MAIN);
    UI.get().unwrap().lock().unwrap().capture = target;
    feedback("Esc: cancel  ·  Delete / Backspace: clear");
}
unsafe fn focus_next(reverse: bool) {
    let (screen, index) = {
        let ui = UI.get().unwrap().lock().unwrap();
        (ui.screen, ui.focus)
    };
    let count = if screen == Screen::Settings { 8 } else { 2 };
    let next = if reverse {
        (index + count - 1) % count
    } else {
        (index + 1) % count
    };
    UI.get().unwrap().lock().unwrap().focus = next;
    if (screen == Screen::Settings && next == 2) || (screen == Screen::Activate && next == 0) {
        SetFocus(EDIT);
        SendMessageW(EDIT, EM_SETSEL, 0, -1);
    } else {
        SetFocus(HWND_MAIN);
    }
    InvalidateRect(HWND_MAIN, null(), 0);
}
unsafe fn invoke_focused() {
    let (screen, i) = {
        let ui = UI.get().unwrap().lock().unwrap();
        (ui.screen, ui.focus)
    };
    match screen {
        Screen::Settings => match i {
            0 => mutate_config(|c| c.mode = Mode::Cps),
            1 => mutate_config(|c| c.mode = Mode::Kps),
            2 => {
                SetFocus(EDIT);
            }
            3 => begin_capture(CaptureTarget::Spam),
            4 => begin_capture(CaptureTarget::Macro),
            5 => begin_capture(CaptureTarget::Ui),
            6 => mutate_config(|c| c.behavior = Behavior::Toggle),
            _ => mutate_config(|c| c.behavior = Behavior::Hold),
        },
        Screen::Compact => transition(Screen::Settings),
        Screen::Confirm => {
            if i == 0 {
                let previous = UI.get().unwrap().lock().unwrap().previous;
                transition(previous);
            } else {
                DestroyWindow(HWND_MAIN);
            }
        }
        Screen::Activate => activate(),
    }
}
unsafe fn click(x: i32, y: i32) {
    let (screen, anim) = {
        let ui = UI.get().unwrap().lock().unwrap();
        (ui.screen, ui.animation.is_some())
    };
    let (w, _) = screen_size(screen);
    if point_in(rect(w - 44, 10, w - 12, 44), x, y) {
        if screen == Screen::Confirm {
            DestroyWindow(HWND_MAIN);
        } else {
            transition(Screen::Confirm);
        }
        return;
    }
    if point_in(rect(w - 80, 10, w - 46, 44), x, y) {
        ShowWindow(HWND_MAIN, SW_MINIMIZE);
        return;
    }
    if anim {
        return;
    }
    match screen {
        Screen::Settings => {
            for (i, r) in [
                (0, pair(0, false)),
                (1, pair(0, true)),
                (2, field(1)),
                (3, field(2)),
                (4, field(3)),
                (5, field(4)),
                (6, pair(5, false)),
                (7, pair(5, true)),
            ] {
                if point_in(r, x, y) {
                    UI.get().unwrap().lock().unwrap().focus = i;
                    invoke_focused();
                    break;
                }
            }
        }
        Screen::Compact => {
            if y >= 168 {
                transition(Screen::Settings);
            }
        }
        Screen::Confirm => {
            if point_in(rect(24, 120, 140, 158), x, y) {
                let p = UI.get().unwrap().lock().unwrap().previous;
                transition(p);
            } else if point_in(rect(152, 120, w - 24, 158), x, y) {
                DestroyWindow(HWND_MAIN);
            }
        }
        Screen::Activate => {
            if point_in(rect(20, 146, w - 20, 180), x, y) {
                activate();
            }
        }
    }
    InvalidateRect(HWND_MAIN, null(), 0);
}
unsafe fn make_icon() -> HICON {
    // Small native vector-backed icon; no generic Windows application icon.
    let dc = GetDC(null_mut());
    let mem = CreateCompatibleDC(dc);
    let bmp = CreateCompatibleBitmap(dc, 32, 32);
    let old = SelectObject(mem, bmp as HGDIOBJ);
    let b = CreateSolidBrush(BG);
    FillRect(mem, &rect(0, 0, 32, 32), b);
    DeleteObject(b as HGDIOBJ);
    logo(mem, 2, 4, 28);
    SelectObject(mem, old);
    let mask = CreateBitmap(32, 32, 1, 1, [0u8; 128].as_ptr() as *const c_void);
    let ii = ICONINFO {
        fIcon: 1,
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: mask,
        hbmColor: bmp,
    };
    let icon = CreateIconIndirect(&ii);
    DeleteObject(mask as HGDIOBJ);
    DeleteObject(bmp as HGDIOBJ);
    DeleteDC(mem);
    ReleaseDC(null_mut(), dc);
    icon
}
unsafe fn tray(add: bool) {
    let mut n: NOTIFYICONDATAW = zeroed();
    n.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    n.hWnd = HWND_MAIN;
    n.uID = 1;
    n.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    n.uCallbackMessage = WM_TRAY;
    n.hIcon = ICON;
    for (a, b) in n.szTip.iter_mut().zip(wide("Raze Macro · Development")) {
        *a = b;
    }
    Shell_NotifyIconW(if add { NIM_ADD } else { NIM_DELETE }, &n);
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_ui(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_SIZE => {
            let mut r: RECT = zeroed();
            GetClientRect(hwnd, &mut r);
            let region = CreateRoundRectRgn(0, 0, r.right + 1, r.bottom + 1, px(16), px(16));
            SetWindowRgn(hwnd, region, 0);
            0
        }
        WM_NCHITTEST => {
            let (x, y) = lparam_xy(lp);
            let mut pt = POINT { x, y };
            ScreenToClient(hwnd, &mut pt);
            let (s, anim) = {
                let ui = UI.get().unwrap().lock().unwrap();
                (ui.screen, ui.animation.is_some())
            };
            let (w, _) = screen_size(s);
            if !anim && pt.y >= 0 && pt.y < px(52) && pt.x < px(w - 82) {
                HTCAPTION as isize
            } else {
                HTCLIENT as isize
            }
        }
        WM_LBUTTONDOWN => {
            let (x, y) = lparam_xy(lp);
            click((x as f64 / scale()) as i32, (y as f64 / scale()) as i32);
            0
        }
        WM_INPUT => {
            handle_raw_input(lp);
            DefWindowProcW(hwnd, msg, wp, lp)
        }
        WM_TIMER => {
            if wp == 1 {
                tick_animation();
            }
            0
        }
        WM_COMMAND => {
            if wp & 0xFFFF == ID_EDIT
                && (wp >> 16) & 0xFFFF == EN_KILLFOCUS
                && UI.get().unwrap().lock().unwrap().screen == Screen::Settings
            {
                commit_rate();
            }
            0
        }
        WM_CTLCOLOREDIT => {
            SetTextColor(wp as HDC, WHITE);
            SetBkColor(wp as HDC, rgb(17, 13, 23));
            EDIT_BRUSH as isize
        }
        WM_KEYDOWN => {
            let key = wp as u16;
            {
                let mut ui = UI.get().unwrap().lock().unwrap();
                if ui.suppress_key.take() == Some(key) {
                    return 0;
                }
            }
            let (screen, capture) = {
                let ui = UI.get().unwrap().lock().unwrap();
                (ui.screen, ui.capture)
            };
            if capture != CaptureTarget::None {
                return 0;
            }
            match key {
                VK_TAB => focus_next(GetAsyncKeyState(VK_SHIFT as i32) < 0),
                VK_RETURN | VK_SPACE => invoke_focused(),
                VK_DELETE | VK_BACK => {
                    if screen == Screen::Settings {
                        let i = UI.get().unwrap().lock().unwrap().focus;
                        let target = match i {
                            3 => CaptureTarget::Spam,
                            4 => CaptureTarget::Macro,
                            5 => CaptureTarget::Ui,
                            _ => CaptureTarget::None,
                        };
                        if target != CaptureTarget::None {
                            UI.get().unwrap().lock().unwrap().capture = target;
                            capture_binding(KeyBinding::none());
                        }
                    }
                }
                VK_ESCAPE => match screen {
                    Screen::Settings => transition(Screen::Compact),
                    Screen::Confirm => {
                        let p = UI.get().unwrap().lock().unwrap().previous;
                        transition(p);
                    }
                    _ => transition(Screen::Confirm),
                },
                _ => {}
            }
            0
        }
        WM_ACTIVATEAPP => {
            if wp == 0 {
                set_active(false);
                let mut ui = UI.get().unwrap().lock().unwrap();
                ui.capture = CaptureTarget::None;
            }
            0
        }
        WM_ENTERSIZEMOVE => {
            set_active(false);
            0
        }
        WM_DPICHANGED => {
            UI.get().unwrap().lock().unwrap().scale = (wp & 0xFFFF) as f64 / 96.0;
            let r = &*(lp as *const RECT);
            let s = UI.get().unwrap().lock().unwrap().screen;
            let (w, h) = screen_size(s);
            SetWindowPos(
                hwnd,
                null_mut(),
                r.left,
                r.top,
                px(w),
                px(h),
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            DeleteObject(FONT_EDIT as HGDIOBJ);
            FONT_EDIT = create_font(px(17), 400);
            create_edit();
            InvalidateRect(hwnd, null(), 0);
            0
        }
        WM_POWERBROADCAST => {
            set_active(false);
            1
        }
        WM_QUERYENDSESSION => 1,
        WM_ENDSESSION => {
            if wp != 0 {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_CLOSE => {
            transition(Screen::Confirm);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, 1);
            destroy_edit();
            tray(false);
            let shared = SHARED.get().unwrap();
            shared.exiting.store(true, Ordering::Release);
            shared.active.store(false, Ordering::Release);
            SetEvent(shared.stop_event as HANDLE);
            SetEvent(shared.wake_event as HANDLE);
            PostQuitMessage(0);
            0
        }
        WM_TRAY => {
            if matches!(lp as u32, WM_LBUTTONUP | WM_RBUTTONUP) {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
            0
        }
        WM_REFRESH => {
            InvalidateRect(hwnd, null(), 0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}
fn main() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let stop = CreateEventW(null(), 1, 0, null());
        let wake = CreateEventW(null(), 0, 0, null());
        if stop.is_null() || wake.is_null() {
            return;
        }
        let config = fs::read_to_string(data_dir().join("config.ini"))
            .map(|s| decode_config(&s))
            .unwrap_or_default();
        let activated = fs::read_to_string(data_dir().join("development.activation"))
            .map(|s| dev_key_valid(&s))
            .unwrap_or(false);
        ACTIVATED.store(activated, Ordering::Release);
        let shared = Arc::new(Shared {
            config: Mutex::new(config),
            active: AtomicBool::new(false),
            desired_active: AtomicBool::new(false),
            exiting: AtomicBool::new(false),
            stop_event: stop as isize,
            wake_event: wake as isize,
        });
        SHARED.set(shared.clone()).ok();
        let initial = if activated {
            Screen::Compact
        } else {
            Screen::Activate
        };
        UI.set(Mutex::new(UiState {
            screen: initial,
            previous: initial,
            capture: CaptureTarget::None,
            macro_down: false,
            ui_down: false,
            focus: 0,
            feedback: String::new(),
            animation: None,
            suppress_key: None,
            scale: GetDpiForSystem() as f64 / 96.0,
        }))
        .ok();
        init_fonts();
        ICON = make_icon();
        if std::env::args().any(|s| s == "--render-previews") {
            render_previews();
            CloseHandle(stop);
            CloseHandle(wake);
            return;
        }
        let instance = GetModuleHandleW(null());
        let class = wide("RazeMacroDevelopment");
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hIcon: ICON,
            lpszClassName: class.as_ptr(),
            ..zeroed()
        };
        if RegisterClassW(&wc) == 0 {
            return;
        }
        let (w, h) = screen_size(initial);
        HWND_MAIN = CreateWindowExW(
            WS_EX_APPWINDOW,
            class.as_ptr(),
            wide("Raze Macro").as_ptr(),
            WS_POPUP | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            px(w),
            px(h),
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        if HWND_MAIN.is_null() {
            return;
        }
        // Native non-client dragging: Windows owns the drag loop; no cursor polling.
        register_raw_input(HWND_MAIN);
        tray(true);
        create_edit();
        let worker = thread::spawn(move || macro_worker(shared));
        ShowWindow(HWND_MAIN, SW_SHOW);
        UpdateWindow(HWND_MAIN);
        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = worker.join();
        CloseHandle(stop);
        CloseHandle(wake);
        for font in [FONT_NORMAL, FONT_SMALL, FONT_TITLE, FONT_STATUS, FONT_EDIT] {
            DeleteObject(font as HGDIOBJ);
        }
        DeleteObject(EDIT_BRUSH as HGDIOBJ);
        DestroyIcon(ICON);
        UnregisterClassW(class.as_ptr(), instance);
    }
}
include!("preview.rs");

#[cfg(test)]
mod ui_tests {
    use super::*;
    #[test]
    fn spacious_settings_geometry() {
        assert_eq!(WIDTH, 376);
        for i in 0..6 {
            let r = field(i);
            assert!(r.left >= 150 && r.right <= WIDTH - 24);
            assert_eq!(r.bottom - r.top, 36);
            if i > 0 {
                assert!(r.top - field(i - 1).bottom >= 20);
            }
        }
        assert_eq!(pair(0, true).left - pair(0, false).right, 10);
        assert_eq!(
            pair(0, true).right - pair(0, true).left,
            pair(0, false).right - pair(0, false).left
        );
        assert!(field(5).bottom < screen_size(Screen::Settings).1 - 30);
    }
    #[test]
    fn frame_time_animation_endpoints() {
        assert_eq!(ease(-1.0), 0.0);
        assert_eq!(ease(2.0), 1.0);
        let mut prev = 0.0;
        for i in 0..100 {
            let e = ease(i as f64 / 99.0);
            assert!(e >= prev);
            prev = e;
        }
    }
}
