//! Narrow Windows boundary for the Rust-first app shell.
//!
//! This module owns raw Win32 shell contracts that must stay independent from
//! any optional WinUI 3 shell fallback.

use dodbogi_core::{
    CheckStatus, DodbogiSettings, Dpi, MonitorGeometry, PhysicalRect, SourceWindow, StartupCheck,
    StartupReport, SupportEnvelope, PARITY_TARGET,
};
use dodbogi_input::{InputEventKind, SourceInputEvent};

const WINDOWS_POINTER_SENSITIVITY: [f64; 20] = [
    0.03125, 0.0625, 0.125, 0.25, 0.375, 0.5, 0.625, 0.75, 0.875, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25,
    2.5, 2.75, 3.0, 3.25, 3.5,
];

fn adjusted_mouse_speed(origin_speed: i32, scale: f64, acceleration_on: bool) -> i32 {
    let origin_speed = origin_speed.clamp(1, 20);
    if !scale.is_finite() || scale <= 0.0 {
        return origin_speed;
    }

    if acceleration_on {
        ((origin_speed as f64 / scale).round() as i32).clamp(1, 20)
    } else {
        let target = WINDOWS_POINTER_SENSITIVITY[(origin_speed - 1) as usize] / scale;
        WINDOWS_POINTER_SENSITIVITY
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                let left_delta = (**left - target).abs();
                let right_delta = (**right - target).abs();
                left_delta
                    .partial_cmp(&right_delta)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(index, _)| index as i32 + 1)
            .unwrap_or(origin_speed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySpec {
    pub id: u32,
    pub name: &'static str,
    pub accelerator: String,
}

pub const DEFAULT_WINDOWED_HOTKEY: &str = "Ctrl+Alt+Q";
pub const DEFAULT_FULLSCREEN_HOTKEY: &str = "Ctrl+Alt+A";

pub fn default_hotkeys() -> Vec<HotkeySpec> {
    vec![
        HotkeySpec {
            id: 1,
            name: "windowed-scale-toggle",
            accelerator: DEFAULT_WINDOWED_HOTKEY.to_string(),
        },
        HotkeySpec {
            id: 2,
            name: "fullscreen-scale-toggle",
            accelerator: DEFAULT_FULLSCREEN_HOTKEY.to_string(),
        },
    ]
}

pub fn hotkeys_from_settings(settings: &DodbogiSettings) -> Vec<HotkeySpec> {
    vec![
        HotkeySpec {
            id: 1,
            name: "windowed-scale-toggle",
            accelerator: settings.hotkeys.windowed_toggle.clone(),
        },
        HotkeySpec {
            id: 2,
            name: "fullscreen-scale-toggle",
            accelerator: settings.hotkeys.fullscreen_toggle.clone(),
        },
    ]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemHotkeyRegistration {
    pub spec: HotkeySpec,
    pub registered: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemHotkeyReport {
    pub registrations: Vec<SystemHotkeyRegistration>,
}

impl SystemHotkeyReport {
    pub fn registered_count(&self) -> usize {
        self.registrations
            .iter()
            .filter(|registration| registration.registered)
            .count()
    }

    pub fn failed_count(&self) -> usize {
        self.registrations.len() - self.registered_count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellMessage {
    Hotkey {
        id: u32,
        name: &'static str,
    },
    TrayMenu {
        item_id: &'static str,
    },
    OverlayInput {
        hwnd: isize,
        kind: InputEventKind,
        screen_x: i32,
        screen_y: i32,
    },
    TrayError(String),
    Quit,
}

#[derive(Debug, Default)]
pub struct HotkeyRegistry {
    registered: Vec<HotkeySpec>,
}

impl HotkeyRegistry {
    pub fn register_defaults(&mut self) {
        self.registered = default_hotkeys();
    }

    pub fn unregister_all(&mut self) {
        self.registered.clear();
    }

    pub fn registered(&self) -> &[HotkeySpec] {
        &self.registered
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayMenuItem {
    pub id: &'static str,
    pub label: &'static str,
    pub enabled: bool,
    pub checked: bool,
}

pub fn default_tray_menu_items() -> Vec<TrayMenuItem> {
    vec![
        TrayMenuItem {
            id: "toggle-windowed",
            label: "Start/stop windowed scaling",
            enabled: true,
            checked: false,
        },
        TrayMenuItem {
            id: "toggle-fullscreen",
            label: "Start/stop fullscreen scaling",
            enabled: true,
            checked: false,
        },
        TrayMenuItem {
            id: "profile-default",
            label: "Profile: Default",
            enabled: true,
            checked: true,
        },
        TrayMenuItem {
            id: "screenshot",
            label: "Take screenshot",
            enabled: true,
            checked: false,
        },
        TrayMenuItem {
            id: "settings",
            label: "Settings",
            enabled: true,
            checked: false,
        },
        TrayMenuItem {
            id: "diagnostics",
            label: "Diagnostics",
            enabled: true,
            checked: false,
        },
        TrayMenuItem {
            id: "exit",
            label: "Exit",
            enabled: true,
            checked: false,
        },
    ]
}

#[derive(Debug, Default)]
pub struct TrayController {
    installed: bool,
    menu_items: Vec<TrayMenuItem>,
}

impl TrayController {
    pub fn install_placeholder(&mut self) {
        self.installed = true;
        self.menu_items = default_tray_menu_items();
    }

    pub fn install_with_menu(&mut self, menu_items: Vec<TrayMenuItem>) {
        self.installed = true;
        self.menu_items = menu_items;
    }

    pub fn remove(&mut self) {
        self.installed = false;
        self.menu_items.clear();
    }

    pub fn is_installed(&self) -> bool {
        self.installed
    }

    pub fn menu_items(&self) -> &[TrayMenuItem] {
        &self.menu_items
    }
}

pub fn collect_startup_report() -> StartupReport {
    let envelope = SupportEnvelope::default();
    let mut checks = Vec::new();

    checks.push(StartupCheck {
        name: "support-envelope",
        status: CheckStatus::Unknown,
        detail: format!(
            "{}; runtime D3D11/WGC/capture/effect probes are available through stage smoke checks",
            envelope.description
        ),
    });

    checks.push(StartupCheck {
        name: "rust-first",
        status: CheckStatus::Passed,
        detail: "Rust-owned app shell loaded".to_string(),
    });

    checks.push(StartupCheck {
        name: "winui-fallback",
        status: CheckStatus::Passed,
        detail: "WinUI 3 is not used; fallback remains decision-gated".to_string(),
    });

    StartupReport {
        target: PARITY_TARGET,
        envelope,
        checks,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Win32Error {
    NoForegroundWindow,
    InvalidWindow,
    RejectedSelfWindow,
    EmptyWindowRect,
    Api(String),
    NotImplemented(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputDeliveryMode {
    DryRun,
    SendInputAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDeliveryReport {
    pub mode: InputDeliveryMode,
    pub target_hwnd: isize,
    pub event_kind: &'static str,
    pub source_point: Option<(i32, i32)>,
    pub delivered: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CursorCaptureReport {
    pub captured: bool,
    pub source_point: Option<(i32, i32)>,
    pub overlay_point: Option<(i32, i32)>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlledInputProbeReport {
    pub target_hwnd: isize,
    pub sent_events: u32,
    pub observed_left_down: u32,
    pub observed_left_up: u32,
    pub delivered: bool,
    pub detail: String,
}

fn input_event_kind_name(kind: InputEventKind) -> &'static str {
    match kind {
        InputEventKind::MouseMove => "mouse_move",
        InputEventKind::MouseButtonDown(_) => "mouse_button_down",
        InputEventKind::MouseButtonUp(_) => "mouse_button_up",
        InputEventKind::DoubleClick(_) => "double_click",
        InputEventKind::Wheel { .. } => "wheel",
        InputEventKind::Drag { .. } => "drag",
        InputEventKind::TextSelection { .. } => "text_selection",
        InputEventKind::ContextMenu => "context_menu",
        InputEventKind::KeyboardFocus => "keyboard_focus",
        InputEventKind::KeyDown { .. } => "key_down",
        InputEventKind::KeyUp { .. } => "key_up",
        InputEventKind::TextInput { .. } => "text_input",
        InputEventKind::Touch { .. } => "touch",
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayStyleContract {
    pub no_activate: bool,
    pub topmost: bool,
    pub tool_window: bool,
    pub input_passthrough: bool,
    pub layered_passthrough: bool,
    pub taskbar_entry: bool,
    pub alt_tab_entry: bool,
}

#[cfg(windows)]
mod imp {
    use super::{
        default_hotkeys, default_tray_menu_items, hotkeys_from_settings, input_event_kind_name,
        ControlledInputProbeReport, CursorCaptureReport, Dpi, HotkeySpec, InputDeliveryMode,
        InputDeliveryReport, MonitorGeometry, OverlayStyleContract, PhysicalRect, ShellMessage,
        SourceInputEvent, SourceWindow, SystemHotkeyRegistration, SystemHotkeyReport, TrayMenuItem,
        Win32Error,
    };
    use dodbogi_core::DodbogiSettings;
    use dodbogi_input::{
        DragPhase, InputEventKind, InputTransform, MouseButton, OverlayPoint, SourcePoint,
        TextSelectionPhase,
    };
    use std::{
        ffi::c_void,
        fs,
        mem::size_of,
        path::{Path, PathBuf},
        ptr::null_mut,
        sync::{
            atomic::{AtomicI32, Ordering},
            Mutex, OnceLock,
        },
        thread,
        time::Duration,
    };
    use windows::{
        core::{BOOL, PCSTR, PCWSTR},
        Graphics::Capture::GraphicsCaptureItem,
        Win32::{
            Foundation::{
                GetLastError, COLORREF, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT,
                TRUE, WPARAM,
            },
            Graphics::{
                Direct3D::{
                    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
                    D3D_FEATURE_LEVEL_11_1,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
                },
                Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
                Dxgi::IDXGIAdapter,
                Gdi::{
                    BeginPaint, ClientToScreen, CreateSolidBrush, DeleteObject, EndPaint,
                    EnumDisplayMonitors, FillRect, GetMonitorInfoW, InvalidateRect, UpdateWindow,
                    HDC, HGDIOBJ, HMONITOR, MONITORINFO, PAINTSTRUCT,
                },
            },
            System::{
                Console::{
                    SetConsoleCtrlHandler, CTRL_BREAK_EVENT, CTRL_CLOSE_EVENT, CTRL_C_EVENT,
                    CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT,
                },
                LibraryLoader::{GetModuleHandleW, GetProcAddress},
                Threading::GetCurrentProcessId,
                WinRT::{
                    Graphics::Capture::IGraphicsCaptureItemInterop, RoInitialize,
                    RO_INIT_MULTITHREADED,
                },
            },
            UI::{
                HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
                Input::KeyboardAndMouse::{
                    RegisterHotKey, SendInput, UnregisterHotKey, HOT_KEY_MODIFIERS, INPUT, INPUT_0,
                    INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
                    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN, MOUSEEVENTF_LEFTDOWN,
                    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
                    MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
                    MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
                },
                Magnification::{MagInitialize, MagShowSystemCursor},
                Shell::{
                    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
                    NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
                },
                WindowsAndMessaging::{
                    AppendMenuW, ClipCursor, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
                    DestroyMenu, DestroyWindow, DispatchMessageW, DrawIconEx, GetClientRect,
                    GetClipCursor, GetCursorInfo, GetCursorPos, GetForegroundWindow,
                    GetGUIThreadInfo, GetIconInfo, GetWindowRect, GetWindowThreadProcessId,
                    IsWindow, IsWindowVisible, LoadCursorW, LoadIconW, PeekMessageW, PostMessageW,
                    RegisterClassW, SetCursorPos, SetForegroundWindow, SetLayeredWindowAttributes,
                    SetWindowLongPtrW, SetWindowPos, ShowCursor, SystemParametersInfoW,
                    TranslateMessage, WindowFromPoint, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW,
                    CURSORINFO, DI_NORMAL, GUITHREADINFO, GUI_INMOVESIZE, GWLP_HWNDPARENT, HICON,
                    HTCLIENT, HTTRANSPARENT, HWND_TOP, HWND_TOPMOST, ICONINFO, IDC_ARROW,
                    IDI_APPLICATION, LWA_ALPHA, LWA_COLORKEY, MF_CHECKED, MF_GRAYED, MF_STRING,
                    MF_UNCHECKED, PM_REMOVE, SPI_GETMOUSE, SPI_GETMOUSESPEED, SPI_SETCURSORS,
                    SPI_SETMOUSESPEED, SWP_HIDEWINDOW, SWP_NOACTIVATE, SWP_NOCOPYBITS, SWP_NOMOVE,
                    SWP_NOSENDCHANGING, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
                    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WM_APP, WM_COMMAND, WM_CONTEXTMENU,
                    WM_DESTROY, WM_HOTKEY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP,
                    WM_MBUTTONDBLCLK, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
                    WM_NCHITTEST, WM_PAINT, WM_QUIT, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN,
                    WM_RBUTTONUP, WM_SETCURSOR, WM_USER, WNDCLASSW, WS_EX_LAYERED,
                    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
                },
            },
        },
    };

    #[derive(Default)]
    struct InputProbeCounters {
        left_down: u32,
        left_up: u32,
    }

    static INPUT_PROBE_COUNTERS: OnceLock<Mutex<InputProbeCounters>> = OnceLock::new();
    static CURSOR_SPEED_RESTORE_AT_EXIT: AtomicI32 = AtomicI32::new(0);
    static CURSOR_SPEED_CONSOLE_HANDLER_INSTALLED: OnceLock<()> = OnceLock::new();

    fn input_probe_counters() -> &'static Mutex<InputProbeCounters> {
        INPUT_PROBE_COUNTERS.get_or_init(|| Mutex::new(InputProbeCounters::default()))
    }

    unsafe extern "system" fn cursor_speed_console_handler(ctrl_type: u32) -> BOOL {
        match ctrl_type {
            CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT
            | CTRL_SHUTDOWN_EVENT => {
                restore_cursor_speed_from_global_guard();
                // Do not swallow Ctrl+C/close; this handler exists only to undo
                // the temporary SPI_SETMOUSESPEED state before normal shutdown.
                BOOL(0)
            }
            _ => BOOL(0),
        }
    }

    fn install_cursor_speed_console_handler() {
        CURSOR_SPEED_CONSOLE_HANDLER_INSTALLED.get_or_init(|| {
            let _ = unsafe { SetConsoleCtrlHandler(Some(cursor_speed_console_handler), true) };
        });
    }

    fn restore_cursor_speed_from_global_guard() {
        let origin_speed = CURSOR_SPEED_RESTORE_AT_EXIT.swap(0, Ordering::SeqCst);
        if (1..=20).contains(&origin_speed) {
            let _ = set_cursor_speed(origin_speed);
        }
    }

    fn remember_cursor_speed_guard(
        origin_speed: i32,
        guard_path: Option<&Path>,
    ) -> Result<(), Win32Error> {
        install_cursor_speed_console_handler();
        let origin_speed = origin_speed.clamp(1, 20);
        let _ = CURSOR_SPEED_RESTORE_AT_EXIT.compare_exchange(
            0,
            origin_speed,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        if let Some(path) = guard_path {
            write_cursor_speed_guard(path, origin_speed)?;
        }
        Ok(())
    }

    fn clear_cursor_speed_guard(origin_speed: i32, guard_path: Option<&Path>) {
        let _ = CURSOR_SPEED_RESTORE_AT_EXIT.compare_exchange(
            origin_speed.clamp(1, 20),
            0,
            Ordering::SeqCst,
            Ordering::SeqCst,
        );
        if let Some(path) = guard_path {
            let _ = fs::remove_file(path);
        }
    }

    fn write_cursor_speed_guard(path: &Path, origin_speed: i32) -> Result<(), Win32Error> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Win32Error::Api(format!(
                    "cursor speed guard parent create failed: {error:?}"
                ))
            })?;
        }
        fs::write(path, format!("{}\n", origin_speed.clamp(1, 20)))
            .map_err(|error| Win32Error::Api(format!("cursor speed guard write failed: {error:?}")))
    }

    fn read_cursor_speed_guard(path: &Path) -> Result<Option<i32>, Win32Error> {
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(path).map_err(|error| {
            Win32Error::Api(format!("cursor speed guard read failed: {error:?}"))
        })?;
        let speed = raw.trim().parse::<i32>().map_err(|error| {
            Win32Error::Api(format!("cursor speed guard parse failed: {error:?}"))
        })?;
        Ok(Some(speed.clamp(1, 20)))
    }

    fn hwnd_from_raw(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    fn hwnd_to_raw(hwnd: HWND) -> isize {
        hwnd.0 as isize
    }

    fn is_null_hwnd(hwnd: HWND) -> bool {
        hwnd.0.is_null()
    }

    fn parse_hotkey_accelerator(accelerator: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
        let mut modifiers = MOD_NOREPEAT;
        let mut vk = None;
        for part in accelerator
            .split('+')
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if part.eq_ignore_ascii_case("ctrl") || part.eq_ignore_ascii_case("control") {
                modifiers |= MOD_CONTROL;
            } else if part.eq_ignore_ascii_case("alt") {
                modifiers |= MOD_ALT;
            } else if part.eq_ignore_ascii_case("shift") {
                modifiers |= MOD_SHIFT;
            } else if part.eq_ignore_ascii_case("win") || part.eq_ignore_ascii_case("windows") {
                modifiers |= MOD_WIN;
            } else {
                vk = virtual_key_from_label(part);
            }
        }
        vk.map(|vk| (modifiers, vk))
    }

    #[cfg(test)]
    pub(crate) fn parse_hotkey_accelerator_for_test(accelerator: &str) -> Option<(u32, u32)> {
        parse_hotkey_accelerator(accelerator).map(|(modifiers, vk)| (modifiers.0, vk))
    }

    fn virtual_key_from_label(label: &str) -> Option<u32> {
        let upper = label.trim().to_ascii_uppercase();
        let mut chars = upper.chars();
        let first = chars.next()?;
        if chars.next().is_none() && (first.is_ascii_uppercase() || first.is_ascii_digit()) {
            return Some(first as u32);
        }
        match upper.as_str() {
            "F1" => Some(0x70),
            "F2" => Some(0x71),
            "F3" => Some(0x72),
            "F4" => Some(0x73),
            "F5" => Some(0x74),
            "F6" => Some(0x75),
            "F7" => Some(0x76),
            "F8" => Some(0x77),
            "F9" => Some(0x78),
            "F10" => Some(0x79),
            "F11" => Some(0x7A),
            "F12" => Some(0x7B),
            "F13" => Some(0x7C),
            "F14" => Some(0x7D),
            "F15" => Some(0x7E),
            "F16" => Some(0x7F),
            "F17" => Some(0x80),
            "F18" => Some(0x81),
            "F19" => Some(0x82),
            "F20" => Some(0x83),
            "F21" => Some(0x84),
            "F22" => Some(0x85),
            "F23" => Some(0x86),
            "F24" => Some(0x87),
            "NUM0" => Some(0x60),
            "NUM1" => Some(0x61),
            "NUM2" => Some(0x62),
            "NUM3" => Some(0x63),
            "NUM4" => Some(0x64),
            "NUM5" => Some(0x65),
            "NUM6" => Some(0x66),
            "NUM7" => Some(0x67),
            "NUM8" => Some(0x68),
            "NUM9" => Some(0x69),
            "BACKSPACE" => Some(0x08),
            "TAB" => Some(0x09),
            "SPACE" => Some(0x20),
            "PAGEUP" | "PGUP" => Some(0x21),
            "PAGEDOWN" | "PGDN" => Some(0x22),
            "END" => Some(0x23),
            "HOME" => Some(0x24),
            "LEFT" => Some(0x25),
            "UP" => Some(0x26),
            "RIGHT" => Some(0x27),
            "DOWN" => Some(0x28),
            "INSERT" | "INS" => Some(0x2D),
            "DELETE" | "DEL" => Some(0x2E),
            _ => None,
        }
    }

    fn hotkey_name(id: u32) -> &'static str {
        match id {
            1 => "windowed-scale-toggle",
            2 => "fullscreen-scale-toggle",
            _ => "unknown",
        }
    }

    fn wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn copy_wide<const N: usize>(target: &mut [u16; N], value: &str) {
        let mut encoded: Vec<u16> = value.encode_utf16().take(N.saturating_sub(1)).collect();
        encoded.push(0);
        for (index, code_unit) in encoded.into_iter().enumerate() {
            target[index] = code_unit;
        }
    }

    pub fn foreground_source_window() -> Result<SourceWindow, Win32Error> {
        let hwnd = unsafe { GetForegroundWindow() };
        if is_null_hwnd(hwnd) {
            return Err(Win32Error::NoForegroundWindow);
        }
        source_window_from_hwnd(hwnd)
    }

    pub fn source_window_from_raw(hwnd: isize) -> Result<SourceWindow, Win32Error> {
        source_window_from_hwnd(hwnd_from_raw(hwnd))
    }

    pub fn is_foreground_move_size_active() -> bool {
        let mut info = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetGUIThreadInfo(0, &mut info) }.is_ok() && info.flags.contains(GUI_INMOVESIZE)
    }

    fn source_foreground_capture_active(target_hwnd: isize) -> bool {
        let target = hwnd_from_raw(target_hwnd);
        let mut info = GUITHREADINFO {
            cbSize: size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetGUIThreadInfo(0, &mut info) }.is_err() || is_null_hwnd(info.hwndCapture) {
            return false;
        }

        info.hwndCapture == target || unsafe { GetForegroundWindow() } == target
    }

    fn raw_window_rect(hwnd: HWND) -> Result<PhysicalRect, Win32Error> {
        let mut rect = RECT::default();
        unsafe { GetWindowRect(hwnd, &mut rect) }
            .map_err(|error| Win32Error::Api(format!("GetWindowRect failed: {error:?}")))?;
        let rect = rect_from_win32(rect);
        if rect.is_empty() {
            return Err(Win32Error::EmptyWindowRect);
        }
        Ok(rect)
    }

    fn dwm_extended_frame_rect(hwnd: HWND) -> Option<PhysicalRect> {
        let mut rect = RECT::default();
        let result = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut rect as *mut RECT).cast(),
                size_of::<RECT>() as u32,
            )
        };
        if result.is_err() {
            return None;
        }
        let rect = rect_from_win32(rect);
        (!rect.is_empty()).then_some(rect)
    }

    fn visible_source_rect(hwnd: HWND) -> Result<PhysicalRect, Win32Error> {
        let raw = raw_window_rect(hwnd)?;
        let Some(extended) = dwm_extended_frame_rect(hwnd) else {
            return Ok(raw);
        };

        // Magpie bases windowed geometry on DWM's visible frame bounds when they
        // are available.  GetWindowRect can include invisible resize borders,
        // which is enough to create scaled black slivers and bad cursor mapping.
        // Guard against unusual custom frames by accepting only a sane rect that
        // intersects the raw Win32 window bounds.
        if extended.intersect(raw).is_some() {
            Ok(extended)
        } else {
            Ok(raw)
        }
    }

    fn source_window_from_hwnd(hwnd: HWND) -> Result<SourceWindow, Win32Error> {
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } || !unsafe { IsWindowVisible(hwnd).as_bool() }
        {
            return Err(Win32Error::InvalidWindow);
        }

        let mut process_id = 0u32;
        unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
        }
        if process_id == unsafe { GetCurrentProcessId() } {
            return Err(Win32Error::RejectedSelfWindow);
        }

        let rect = visible_source_rect(hwnd)?;

        Ok(SourceWindow {
            hwnd: hwnd_to_raw(hwnd),
            rect,
        })
    }

    pub fn move_window_for_probe(
        hwnd: isize,
        dx: i32,
        dy: i32,
    ) -> Result<SourceWindow, Win32Error> {
        let hwnd = hwnd_from_raw(hwnd);
        let source = raw_window_rect(hwnd)?;
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                source.left + dx,
                source.top + dy,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_NOSIZE | SWP_NOZORDER,
            )
        }
        .map_err(|error| Win32Error::Api(format!("SetWindowPos source move failed: {error:?}")))?;
        source_window_from_hwnd(hwnd)
    }

    pub fn resize_window_for_probe(
        hwnd: isize,
        width_delta: i32,
        height_delta: i32,
    ) -> Result<SourceWindow, Win32Error> {
        let hwnd = hwnd_from_raw(hwnd);
        let source = raw_window_rect(hwnd)?;
        let width = (source.width() + width_delta).max(120);
        let height = (source.height() + height_delta).max(120);
        unsafe {
            SetWindowPos(
                hwnd,
                None,
                source.left,
                source.top,
                width,
                height,
                SWP_NOACTIVATE | SWP_NOSENDCHANGING | SWP_NOZORDER,
            )
        }
        .map_err(|error| {
            Win32Error::Api(format!("SetWindowPos source resize failed: {error:?}"))
        })?;
        source_window_from_hwnd(hwnd)
    }

    pub fn probe_d3d11_feature_level() -> Result<String, Win32Error> {
        let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let mut device: Option<ID3D11Device> = None;
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut selected = D3D_FEATURE_LEVEL(0);

        unsafe {
            D3D11CreateDevice(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE(null_mut()),
                D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                Some(&levels),
                D3D11_SDK_VERSION,
                Some(&mut device),
                Some(&mut selected),
                Some(&mut context),
            )
        }
        .map_err(|error| Win32Error::Api(format!("D3D11CreateDevice failed: {error:?}")))?;

        Ok(format!("{:?}", selected))
    }

    pub fn create_wgc_item_for_hwnd(hwnd: isize) -> Result<GraphicsCaptureItem, Win32Error> {
        let _ = unsafe { RoInitialize(RO_INIT_MULTITHREADED) };
        let factory: IGraphicsCaptureItemInterop =
            windows::core::factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>().map_err(
                |error| Win32Error::Api(format!("GraphicsCaptureItem factory failed: {error:?}")),
            )?;

        unsafe { factory.CreateForWindow(hwnd_from_raw(hwnd)) }
            .map_err(|error| Win32Error::Api(format!("CreateForWindow failed: {error:?}")))
    }

    fn rect_from_win32(rect: RECT) -> PhysicalRect {
        PhysicalRect {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }

    pub fn enumerate_monitors() -> Result<Vec<MonitorGeometry>, Win32Error> {
        unsafe extern "system" fn enum_proc(
            hmonitor: HMONITOR,
            _hdc: HDC,
            _rect: *mut RECT,
            lparam: LPARAM,
        ) -> BOOL {
            let monitors = unsafe { &mut *(lparam.0 as *mut Vec<MonitorGeometry>) };
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if unsafe { GetMonitorInfoW(hmonitor, &mut info).as_bool() } {
                let mut dpi_x = 96u32;
                let mut dpi_y = 96u32;
                let _ = unsafe {
                    GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
                };
                monitors.push(MonitorGeometry {
                    id: monitors.len() as u32 + 1,
                    bounds: rect_from_win32(info.rcMonitor),
                    work_area: rect_from_win32(info.rcWork),
                    dpi: Dpi { x: dpi_x, y: dpi_y },
                    primary: (info.dwFlags & 1) != 0,
                });
            }
            TRUE
        }

        let mut monitors = Vec::<MonitorGeometry>::new();
        let ok = unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(enum_proc),
                LPARAM(&mut monitors as *mut _ as isize),
            )
        };
        if !ok.as_bool() {
            let err = unsafe { GetLastError() };
            return Err(Win32Error::Api(format!(
                "EnumDisplayMonitors failed: {err:?}"
            )));
        }
        if monitors.is_empty() {
            return Err(Win32Error::Api("no monitors enumerated".to_string()));
        }
        Ok(monitors)
    }

    pub fn client_rect_from_raw(hwnd: isize) -> Result<PhysicalRect, Win32Error> {
        let hwnd = hwnd_from_raw(hwnd);
        let mut client = RECT::default();
        unsafe { GetClientRect(hwnd, &mut client) }
            .map_err(|error| Win32Error::Api(format!("GetClientRect failed: {error:?}")))?;

        let mut top_left = POINT {
            x: client.left,
            y: client.top,
        };
        let mut bottom_right = POINT {
            x: client.right,
            y: client.bottom,
        };
        if !unsafe { ClientToScreen(hwnd, &mut top_left).as_bool() }
            || !unsafe { ClientToScreen(hwnd, &mut bottom_right).as_bool() }
        {
            return Err(Win32Error::Api("ClientToScreen failed".to_string()));
        }

        Ok(PhysicalRect {
            left: top_left.x,
            top: top_left.y,
            right: bottom_right.x,
            bottom: bottom_right.y,
        })
    }

    #[derive(Debug)]
    pub struct SystemHotkeyGuard {
        registered_ids: Vec<i32>,
        report: SystemHotkeyReport,
    }

    impl SystemHotkeyGuard {
        pub fn register_defaults() -> Self {
            Self::register_specs(default_hotkeys())
        }

        pub fn register_from_settings(settings: &DodbogiSettings) -> Self {
            Self::register_specs(hotkeys_from_settings(settings))
        }

        pub fn replace_from_settings(&mut self, settings: &DodbogiSettings) {
            self.unregister_all();
            *self = Self::register_from_settings(settings);
        }

        fn unregister_all(&mut self) {
            for id in self.registered_ids.drain(..) {
                let _ = unsafe { UnregisterHotKey(None, id) };
            }
        }

        fn register_specs(specs: Vec<HotkeySpec>) -> Self {
            let mut registered_ids = Vec::new();
            let mut registrations = Vec::new();

            for spec in specs {
                let Some((modifiers, vk)) = parse_hotkey_accelerator(&spec.accelerator) else {
                    registrations.push(SystemHotkeyRegistration {
                        spec,
                        registered: false,
                        detail: "no virtual-key mapping for hotkey accelerator".to_string(),
                    });
                    continue;
                };

                match unsafe { RegisterHotKey(None, spec.id as i32, modifiers, vk) } {
                    Ok(()) => {
                        registered_ids.push(spec.id as i32);
                        registrations.push(SystemHotkeyRegistration {
                            spec,
                            registered: true,
                            detail: "RegisterHotKey succeeded".to_string(),
                        });
                    }
                    Err(error) => registrations.push(SystemHotkeyRegistration {
                        spec,
                        registered: false,
                        detail: format!("RegisterHotKey failed: {error:?}"),
                    }),
                }
            }

            Self {
                registered_ids,
                report: SystemHotkeyReport { registrations },
            }
        }

        pub fn report(&self) -> &SystemHotkeyReport {
            &self.report
        }
    }

    impl Drop for SystemHotkeyGuard {
        fn drop(&mut self) {
            self.unregister_all();
        }
    }

    pub struct OverlayWindow {
        hwnd: HWND,
    }

    impl OverlayWindow {
        pub fn create_hidden() -> Result<Self, Win32Error> {
            unsafe extern "system" fn wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                match msg {
                    WM_NCHITTEST => return LRESULT(HTTRANSPARENT as isize),
                    WM_DESTROY => return LRESULT(0),
                    _ => {}
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }

            let instance = unsafe { GetModuleHandleW(None) }
                .map_err(|error| Win32Error::Api(format!("GetModuleHandleW failed: {error:?}")))?;
            let class_name = windows::core::w!("DodbogiScalingOverlay");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
                lpfnWndProc: Some(wnd_proc),
                hInstance: HINSTANCE(instance.0),
                lpszClassName: class_name,
                ..Default::default()
            };
            let atom = unsafe { RegisterClassW(&wc) };
            if atom == 0 {
                let err = unsafe { GetLastError() };
                // ERROR_CLASS_ALREADY_EXISTS is acceptable.
                if err.0 != 1410 {
                    return Err(Win32Error::Api(format!("RegisterClassW failed: {err:?}")));
                }
            }

            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT,
                    class_name,
                    windows::core::w!("Dodbogi Scaling Overlay"),
                    WS_POPUP,
                    0,
                    0,
                    640,
                    480,
                    None,
                    None,
                    Some(HINSTANCE(instance.0)),
                    None,
                )
            }
            .map_err(|error| Win32Error::Api(format!("CreateWindowExW failed: {error:?}")))?;
            unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), 255, LWA_ALPHA) }.map_err(
                |error| {
                    Win32Error::Api(format!(
                        "SetLayeredWindowAttributes overlay alpha failed: {error:?}"
                    ))
                },
            )?;
            Ok(Self { hwnd })
        }

        pub fn hwnd(&self) -> isize {
            hwnd_to_raw(self.hwnd)
        }

        pub fn attach_to_source(&self, source_hwnd: isize) -> Result<(), Win32Error> {
            let source = HWND(source_hwnd as *mut _);
            if is_null_hwnd(source) {
                return Err(Win32Error::InvalidWindow);
            }
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWLP_HWNDPARENT, source_hwnd);
                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_NOSENDCHANGING | SWP_NOSIZE,
                )
            }
            .map_err(|error| {
                Win32Error::Api(format!("SetWindowPos owner order failed: {error:?}"))
            })?;
            Ok(())
        }

        pub fn apply_layout(
            &self,
            rect: PhysicalRect,
            show: bool,
        ) -> Result<OverlayStyleContract, Win32Error> {
            if rect.is_empty() {
                return Err(Win32Error::EmptyWindowRect);
            }
            let flags = SWP_NOACTIVATE
                | SWP_NOCOPYBITS
                | SWP_NOSENDCHANGING
                | SWP_NOZORDER
                | if show { SWP_SHOWWINDOW } else { SWP_HIDEWINDOW };
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    rect.left,
                    rect.top,
                    rect.width(),
                    rect.height(),
                    flags,
                )
            }
            .map_err(|error| Win32Error::Api(format!("SetWindowPos failed: {error:?}")))?;
            Ok(Self::style_contract())
        }

        pub fn style_contract() -> OverlayStyleContract {
            OverlayStyleContract {
                no_activate: true,
                topmost: false,
                tool_window: true,
                input_passthrough: true,
                layered_passthrough: true,
                taskbar_entry: false,
                alt_tab_entry: false,
            }
        }
    }

    impl Drop for OverlayWindow {
        fn drop(&mut self) {
            if !is_null_hwnd(self.hwnd) {
                let _ = unsafe { DestroyWindow(self.hwnd) };
            }
        }
    }

    const CURSOR_OVERLAY_SIZE: i32 = 128;
    const CURSOR_OVERLAY_COLOR_KEY: COLORREF = COLORREF(0x00ff00ff);

    #[derive(Clone, Copy)]
    struct CursorImage {
        icon: HICON,
        hotspot_x: i32,
        hotspot_y: i32,
    }

    fn rect_contains_point(rect: PhysicalRect, x: i32, y: i32) -> bool {
        x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
    }

    fn clamp_point_to_rect(point: (i32, i32), rect: PhysicalRect) -> (i32, i32) {
        if rect.is_empty() {
            return point;
        }

        let max_x = rect.right.saturating_sub(1).max(rect.left);
        let max_y = rect.bottom.saturating_sub(1).max(rect.top);
        (
            point.0.clamp(rect.left, max_x),
            point.1.clamp(rect.top, max_y),
        )
    }

    fn round_overlay_point(point: OverlayPoint) -> (i32, i32) {
        (point.x.round() as i32, point.y.round() as i32)
    }

    unsafe fn paint_cursor_overlay(hwnd: HWND) -> LRESULT {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        if !hdc.is_invalid() {
            let brush = CreateSolidBrush(CURSOR_OVERLAY_COLOR_KEY);
            if !brush.is_invalid() {
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: CURSOR_OVERLAY_SIZE,
                    bottom: CURSOR_OVERLAY_SIZE,
                };
                let _ = FillRect(hdc, &rect, brush);
                let _ = DeleteObject(HGDIOBJ(brush.0));
            }

            if let Some(cursor) = current_cursor_image() {
                let _ = DrawIconEx(hdc, 0, 0, cursor.icon, 0, 0, 0, None, DI_NORMAL);
            }
        }
        let _ = EndPaint(hwnd, &ps);
        LRESULT(0)
    }

    fn current_cursor_image() -> Option<CursorImage> {
        let mut info = CURSORINFO {
            cbSize: size_of::<CURSORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetCursorInfo(&mut info) }.is_ok() && !info.hCursor.is_invalid() {
            let icon = HICON(info.hCursor.0);
            let (hotspot_x, hotspot_y) = cursor_hotspot(icon).unwrap_or((0, 0));
            return Some(CursorImage {
                icon,
                hotspot_x,
                hotspot_y,
            });
        }

        let icon = unsafe { LoadCursorW(None, IDC_ARROW) }
            .ok()
            .filter(|cursor| !cursor.is_invalid())
            .map(|cursor| HICON(cursor.0))?;
        let (hotspot_x, hotspot_y) = cursor_hotspot(icon).unwrap_or((0, 0));
        Some(CursorImage {
            icon,
            hotspot_x,
            hotspot_y,
        })
    }

    fn cursor_hotspot(icon: HICON) -> Option<(i32, i32)> {
        let mut info = ICONINFO::default();
        unsafe { GetIconInfo(icon, &mut info) }.ok()?;
        if !info.hbmColor.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(info.hbmColor.0)) };
        }
        if !info.hbmMask.is_invalid() {
            let _ = unsafe { DeleteObject(HGDIOBJ(info.hbmMask.0)) };
        }
        Some((info.xHotspot as i32, info.yHotspot as i32))
    }

    type ShowSystemCursorFn = unsafe extern "system" fn(BOOL);

    fn show_system_cursor_proc() -> Option<ShowSystemCursorFn> {
        static PROC: OnceLock<Option<ShowSystemCursorFn>> = OnceLock::new();
        *PROC.get_or_init(|| unsafe {
            let Ok(user32) = GetModuleHandleW(windows::core::w!("user32.dll")) else {
                return None;
            };
            let proc = GetProcAddress(user32, PCSTR(c"ShowSystemCursor".as_ptr().cast()));
            proc.map(|raw| std::mem::transmute::<_, ShowSystemCursorFn>(raw))
        })
    }

    fn magnification_cursor_ready() -> bool {
        static READY: OnceLock<bool> = OnceLock::new();
        *READY.get_or_init(|| unsafe { MagInitialize().as_bool() })
    }

    fn set_system_cursor_visible(show: bool) -> String {
        if let Some(show_system_cursor) = show_system_cursor_proc() {
            unsafe { show_system_cursor(BOOL::from(show)) };
            return "system_cursor_visibility=ShowSystemCursor".to_string();
        }

        if magnification_cursor_ready() && unsafe { MagShowSystemCursor(show).as_bool() } {
            return "system_cursor_visibility=MagShowSystemCursor".to_string();
        }

        if show {
            for _ in 0..16 {
                if unsafe { ShowCursor(true) } >= 0 {
                    break;
                }
            }
            "system_cursor_visibility=ShowCursorFallback".to_string()
        } else {
            for _ in 0..16 {
                if unsafe { ShowCursor(false) } < 0 {
                    break;
                }
            }
            "system_cursor_visibility=ShowCursorFallback".to_string()
        }
    }

    struct CursorOverlayWindow {
        hwnd: HWND,
        owner_hwnd: Option<isize>,
    }

    impl CursorOverlayWindow {
        fn create_hidden() -> Result<Self, Win32Error> {
            unsafe extern "system" fn wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                match msg {
                    WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
                    WM_PAINT => unsafe { paint_cursor_overlay(hwnd) },
                    WM_DESTROY => LRESULT(0),
                    _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
                }
            }

            let instance = unsafe { GetModuleHandleW(None) }
                .map_err(|error| Win32Error::Api(format!("GetModuleHandleW failed: {error:?}")))?;
            let class_name = windows::core::w!("DodbogiCursorOverlay");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: HINSTANCE(instance.0),
                lpszClassName: class_name,
                ..Default::default()
            };
            let atom = unsafe { RegisterClassW(&wc) };
            if atom == 0 {
                let err = unsafe { GetLastError() };
                if err.0 != 1410 {
                    return Err(Win32Error::Api(format!("RegisterClassW failed: {err:?}")));
                }
            }

            let hwnd = unsafe {
                CreateWindowExW(
                    WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_LAYERED | WS_EX_TRANSPARENT,
                    class_name,
                    windows::core::w!("Dodbogi Cursor Overlay"),
                    WS_POPUP,
                    0,
                    0,
                    CURSOR_OVERLAY_SIZE,
                    CURSOR_OVERLAY_SIZE,
                    None,
                    None,
                    Some(HINSTANCE(instance.0)),
                    None,
                )
            }
            .map_err(|error| {
                Win32Error::Api(format!("CreateWindowExW cursor overlay failed: {error:?}"))
            })?;

            unsafe { SetLayeredWindowAttributes(hwnd, CURSOR_OVERLAY_COLOR_KEY, 0, LWA_COLORKEY) }
                .map_err(|error| {
                    Win32Error::Api(format!("SetLayeredWindowAttributes failed: {error:?}"))
                })?;

            Ok(Self {
                hwnd,
                owner_hwnd: None,
            })
        }

        fn attach_to_source(&mut self, source_hwnd: isize) -> Result<(), Win32Error> {
            if self.owner_hwnd == Some(source_hwnd) {
                return Ok(());
            }
            let source = HWND(source_hwnd as *mut _);
            if is_null_hwnd(source) {
                return Err(Win32Error::InvalidWindow);
            }
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWLP_HWNDPARENT, source_hwnd);
                SetWindowPos(
                    self.hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE | SWP_NOCOPYBITS | SWP_NOSENDCHANGING | SWP_NOMOVE | SWP_NOSIZE,
                )
            }
            .map_err(|error| {
                Win32Error::Api(format!("SetWindowPos cursor owner order failed: {error:?}"))
            })?;
            self.owner_hwnd = Some(source_hwnd);
            Ok(())
        }

        fn show_at_cursor(&self, x: i32, y: i32) -> Result<(), Win32Error> {
            let cursor = current_cursor_image().unwrap_or(CursorImage {
                icon: HICON(null_mut()),
                hotspot_x: 0,
                hotspot_y: 0,
            });
            let left = x - cursor.hotspot_x;
            let top = y - cursor.hotspot_y;
            unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    left,
                    top,
                    CURSOR_OVERLAY_SIZE,
                    CURSOR_OVERLAY_SIZE,
                    SWP_NOACTIVATE
                        | SWP_NOCOPYBITS
                        | SWP_NOSENDCHANGING
                        | SWP_NOZORDER
                        | SWP_SHOWWINDOW,
                )
            }
            .map_err(|error| Win32Error::Api(format!("SetWindowPos cursor failed: {error:?}")))?;
            let _ = unsafe { InvalidateRect(Some(self.hwnd), None, false) };
            let _ = unsafe { UpdateWindow(self.hwnd) };
            Ok(())
        }

        fn hide(&self) {
            let _ = unsafe {
                SetWindowPos(
                    self.hwnd,
                    None,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOACTIVATE
                        | SWP_NOCOPYBITS
                        | SWP_NOSENDCHANGING
                        | SWP_NOZORDER
                        | SWP_HIDEWINDOW,
                )
            };
        }
    }

    impl Drop for CursorOverlayWindow {
        fn drop(&mut self) {
            self.hide();
            if !is_null_hwnd(self.hwnd) {
                let _ = unsafe { DestroyWindow(self.hwnd) };
            }
        }
    }

    pub struct CursorCaptureController {
        cursor_overlay: CursorOverlayWindow,
        captured: bool,
        system_cursor_hidden: bool,
        last_overlay_point: Option<(i32, i32)>,
        origin_cursor_speed: Option<i32>,
        adjusted_cursor_speed: Option<i32>,
        cursor_speed_guard_path: Option<PathBuf>,
    }

    impl CursorCaptureController {
        pub fn create() -> Result<Self, Win32Error> {
            Self::create_with_speed_guard_path(None)
        }

        pub fn create_with_speed_guard_path(
            cursor_speed_guard_path: Option<PathBuf>,
        ) -> Result<Self, Win32Error> {
            Ok(Self {
                cursor_overlay: CursorOverlayWindow::create_hidden()?,
                captured: false,
                system_cursor_hidden: false,
                last_overlay_point: None,
                origin_cursor_speed: None,
                adjusted_cursor_speed: None,
                cursor_speed_guard_path,
            })
        }

        pub fn update(
            &mut self,
            transform: &InputTransform,
            target_hwnd: isize,
        ) -> Result<Option<CursorCaptureReport>, Win32Error> {
            let mut hardware = POINT::default();
            unsafe { GetCursorPos(&mut hardware) }
                .map_err(|error| Win32Error::Api(format!("GetCursorPos failed: {error:?}")))?;
            self.cursor_overlay.attach_to_source(target_hwnd)?;

            let source_capture_active = source_foreground_capture_active(target_hwnd);
            let move_size_active = is_foreground_move_size_active();

            if move_size_active {
                if self.captured {
                    // While the real source window is in a native move/size loop,
                    // Windows keeps the hardware cursor on the unscaled source
                    // border.  Magpie keeps the hardware cursor hidden and draws
                    // the visible cursor in scaling space instead; showing the
                    // system cursor here makes it appear to teleport back to the
                    // original window edge.
                    let source_point =
                        if rect_contains_point(transform.source, hardware.x, hardware.y) {
                            (hardware.x, hardware.y)
                        } else {
                            clamp_point_to_rect((hardware.x, hardware.y), transform.source)
                        };
                    let overlay_point = clamp_point_to_rect(
                        transform.source_to_overlay_pixel(SourcePoint {
                            x: source_point.0 as f32,
                            y: source_point.1 as f32,
                        }),
                        transform.destination,
                    );
                    self.cursor_overlay
                        .show_at_cursor(overlay_point.0, overlay_point.1)?;
                    let _ = self.hide_system_cursor();
                    // During a native move/resize loop Windows applies hardware cursor deltas to
                    // the source window, while Dodbogi presents the corresponding scaled overlay
                    // edge.  If we restore the system speed here, a 2x windowed scale makes the
                    // visible overlay edge/cursor travel roughly 2x faster than the user's hand.
                    // Keep the same speed compensation used in normal capture; cleanup is covered
                    // by release/stop paths plus the persistent speed guard.
                    let _ = self.adjust_cursor_speed(transform);
                    self.last_overlay_point = Some(overlay_point);
                }
                return Ok(None);
            }

            if self.captured {
                let _ = self.adjust_cursor_speed(transform);
                if rect_contains_point(transform.source, hardware.x, hardware.y) {
                    let source = SourcePoint {
                        x: hardware.x as f32,
                        y: hardware.y as f32,
                    };
                    let overlay_point = transform.source_to_overlay_pixel(source);
                    let _ = self.hide_system_cursor();
                    self.cursor_overlay
                        .show_at_cursor(overlay_point.0, overlay_point.1)?;
                    self.last_overlay_point = Some(overlay_point);
                    return Ok(None);
                }

                if source_capture_active {
                    let clamped_source_point =
                        clamp_point_to_rect((hardware.x, hardware.y), transform.source);
                    let overlay_point = clamp_point_to_rect(
                        transform.source_to_overlay_pixel(SourcePoint {
                            x: clamped_source_point.0 as f32,
                            y: clamped_source_point.1 as f32,
                        }),
                        transform.destination,
                    );
                    let _ = self.hide_system_cursor();
                    self.cursor_overlay
                        .show_at_cursor(overlay_point.0, overlay_point.1)?;
                    self.last_overlay_point = Some(overlay_point);
                    return Ok(None);
                }

                let overlay = transform.source_to_overlay_point(SourcePoint {
                    x: hardware.x as f32,
                    y: hardware.y as f32,
                });
                let overlay_point =
                    clamp_point_to_rect(round_overlay_point(overlay), transform.destination);
                let speed_detail = self.release_to_overlay_position(Some(overlay_point))?;
                return Ok(Some(CursorCaptureReport {
                    captured: false,
                    source_point: Some((hardware.x, hardware.y)),
                    overlay_point: Some(overlay_point),
                    detail: format!(
                        "cursor_capture_released; hardware cursor returned to scaled overlay position; {speed_detail}"
                    ),
                }));
            }

            let overlay = OverlayPoint {
                x: hardware.x as f32,
                y: hardware.y as f32,
            };
            if source_capture_active {
                return Ok(None);
            }
            if rect_contains_point(transform.destination, hardware.x, hardware.y) {
                let Some(source_point) = transform.overlay_to_source_pixel(overlay) else {
                    return Ok(None);
                };
                let overlay_point =
                    clamp_point_to_rect((hardware.x, hardware.y), transform.destination);
                let speed_detail = self.adjust_cursor_speed(transform);
                if let Err(error) = reliable_set_cursor_pos(source_point.0, source_point.1) {
                    let _ = self.restore_cursor_speed();
                    return Err(error);
                }
                post_setcursor_to_source(target_hwnd, source_point.0, source_point.1);
                let cursor_visibility_detail = self.hide_system_cursor();
                if let Err(error) = self
                    .cursor_overlay
                    .show_at_cursor(overlay_point.0, overlay_point.1)
                {
                    self.show_system_cursor();
                    let _ = self.restore_cursor_speed();
                    return Err(error);
                }
                let focus_detail = request_source_focus(target_hwnd);
                self.captured = true;
                self.last_overlay_point = Some(overlay_point);
                return Ok(Some(CursorCaptureReport {
                    captured: true,
                    source_point: Some(source_point),
                    overlay_point: Some(overlay_point),
                    detail: format!(
                        "cursor_capture_entered; real cursor moved to source, overlay cursor is drawn separately; {cursor_visibility_detail}; {speed_detail}; {focus_detail}"
                    ),
                }));
            }

            self.cursor_overlay.hide();
            self.show_system_cursor();
            let _ = self.restore_cursor_speed();
            Ok(None)
        }

        pub fn release(&mut self) {
            let _ = self.release_to_overlay_position(self.last_overlay_point);
        }

        fn release_to_overlay_position(
            &mut self,
            overlay_point: Option<(i32, i32)>,
        ) -> Result<String, Win32Error> {
            self.cursor_overlay.hide();
            self.show_system_cursor();
            let speed_detail = self
                .restore_cursor_speed()
                .unwrap_or_else(|| "cursor_speed_unchanged".to_string());
            self.captured = false;
            self.last_overlay_point = None;
            let cursor_detail = if let Some((x, y)) = overlay_point {
                match reliable_set_cursor_pos(x, y) {
                    Ok(()) => "cursor_position_restored".to_string(),
                    Err(error) => format!("cursor_position_restore_skipped error={error:?}"),
                }
            } else {
                "cursor_position_unchanged".to_string()
            };
            Ok(format!("{speed_detail}; {cursor_detail}"))
        }

        fn hide_system_cursor(&mut self) -> String {
            if self.system_cursor_hidden {
                return "system_cursor_visibility=already_hidden".to_string();
            }
            let detail = set_system_cursor_visible(false);
            self.system_cursor_hidden = true;
            detail
        }

        fn show_system_cursor(&mut self) {
            if !self.system_cursor_hidden {
                return;
            }
            let _ = set_system_cursor_visible(true);
            reload_system_cursors();
            self.system_cursor_hidden = false;
        }

        fn adjust_cursor_speed(&mut self, transform: &InputTransform) -> String {
            let scale = (f64::from(transform.scale_x) + f64::from(transform.scale_y)) / 2.0;
            let origin_speed = match self.origin_cursor_speed {
                Some(speed) => speed,
                None => match read_cursor_speed() {
                    Ok(speed) => speed,
                    Err(error) => {
                        return format!("cursor_speed_adjust_failed read_speed={error:?}");
                    }
                },
            };
            let acceleration_on = read_mouse_acceleration().unwrap_or(true);
            let new_speed = super::adjusted_mouse_speed(origin_speed, scale, acceleration_on);
            if self.adjusted_cursor_speed == Some(new_speed) {
                return format!(
                    "cursor_speed_adjusted origin={} adjusted={} scale={scale:.3} acceleration={}",
                    origin_speed, new_speed, acceleration_on
                );
            }

            let origin_was_untracked = self.origin_cursor_speed.is_none();
            if origin_was_untracked {
                self.origin_cursor_speed = Some(origin_speed);
                if let Err(error) = remember_cursor_speed_guard(
                    origin_speed,
                    self.cursor_speed_guard_path.as_deref(),
                ) {
                    self.origin_cursor_speed = None;
                    return format!(
                        "cursor_speed_adjust_failed origin={} target={} scale={scale:.3} guard_error={error:?}",
                        origin_speed, new_speed
                    );
                }
            }

            match set_cursor_speed(new_speed) {
                Ok(()) => {
                    self.adjusted_cursor_speed = Some(new_speed);
                    format!(
                        "cursor_speed_adjusted origin={} adjusted={} scale={scale:.3} acceleration={}",
                        origin_speed, new_speed, acceleration_on
                    )
                }
                Err(error) => {
                    if origin_was_untracked {
                        self.origin_cursor_speed = None;
                        self.adjusted_cursor_speed = None;
                        clear_cursor_speed_guard(
                            origin_speed,
                            self.cursor_speed_guard_path.as_deref(),
                        );
                    }
                    format!(
                        "cursor_speed_adjust_failed origin={} target={} scale={scale:.3} error={error:?}",
                        origin_speed, new_speed
                    )
                }
            }
        }

        fn restore_cursor_speed(&mut self) -> Option<String> {
            let origin_speed = self.origin_cursor_speed?;
            match set_cursor_speed(origin_speed) {
                Ok(()) => {
                    self.origin_cursor_speed = None;
                    self.adjusted_cursor_speed = None;
                    clear_cursor_speed_guard(origin_speed, self.cursor_speed_guard_path.as_deref());
                    Some(format!("cursor_speed_restored origin={origin_speed}"))
                }
                Err(error) => Some(format!(
                    "cursor_speed_restore_failed origin={origin_speed} error={error:?}"
                )),
            }
        }
    }

    impl Drop for CursorCaptureController {
        fn drop(&mut self) {
            self.release();
        }
    }

    fn reliable_set_cursor_pos(x: i32, y: i32) -> Result<(), Win32Error> {
        let mut previous_clip = RECT::default();
        let previous_clip_available = unsafe { GetClipCursor(&mut previous_clip) }.is_ok();
        let transition_clip = RECT {
            left: x,
            top: y,
            right: x + 1,
            bottom: y + 1,
        };
        if let Err(clip_error) = unsafe { ClipCursor(Some(&transition_clip)) } {
            return unsafe { SetCursorPos(x, y) }.map_err(|set_error| {
                Win32Error::Api(format!(
                    "ClipCursor transition failed: {clip_error:?}; SetCursorPos fallback failed: {set_error:?}"
                ))
            });
        }
        let set_result = unsafe { SetCursorPos(x, y) };
        thread::sleep(Duration::from_millis(8));
        let restore_result = if previous_clip_available {
            unsafe { ClipCursor(Some(&previous_clip)) }
        } else {
            unsafe { ClipCursor(None) }
        };
        if let Err(error) = restore_result {
            return Err(Win32Error::Api(format!(
                "ClipCursor restore failed: {error:?}"
            )));
        }
        set_result.map_err(|error| Win32Error::Api(format!("SetCursorPos failed: {error:?}")))
    }

    fn post_setcursor_to_source(target_hwnd: isize, x: i32, y: i32) {
        let hwnd_src = hwnd_from_raw(target_hwnd);
        if is_null_hwnd(hwnd_src) || !unsafe { IsWindow(Some(hwnd_src)).as_bool() } {
            return;
        }

        let point = POINT { x, y };
        let hwnd_at_point = unsafe { WindowFromPoint(point) };
        let hwnd_cursor = if is_null_hwnd(hwnd_at_point) {
            hwnd_src
        } else {
            hwnd_at_point
        };
        let lparam = LPARAM(((WM_MOUSEMOVE << 16) | (HTCLIENT & 0xffff)) as isize);
        let _ = unsafe {
            PostMessageW(
                Some(hwnd_cursor),
                WM_SETCURSOR,
                WPARAM(hwnd_to_raw(hwnd_src) as usize),
                lparam,
            )
        };
    }

    fn reload_system_cursors() {
        let _ = unsafe {
            SystemParametersInfoW(
                SPI_SETCURSORS,
                0,
                None,
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
    }

    pub fn cursor_position_for_probe() -> Result<(i32, i32), Win32Error> {
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) }
            .map_err(|error| Win32Error::Api(format!("GetCursorPos failed: {error:?}")))?;
        Ok((point.x, point.y))
    }

    pub fn move_cursor_for_probe(x: i32, y: i32) -> Result<(), Win32Error> {
        reliable_set_cursor_pos(x, y)
    }

    pub fn cursor_speed_for_probe() -> Result<i32, Win32Error> {
        read_cursor_speed()
    }

    pub fn set_cursor_speed_for_probe(speed: i32) -> Result<(), Win32Error> {
        set_cursor_speed(speed)
    }

    pub fn recover_cursor_speed_guard(path: &Path) -> Result<Option<i32>, Win32Error> {
        let Some(origin_speed) = read_cursor_speed_guard(path)? else {
            return Ok(None);
        };
        set_cursor_speed(origin_speed)?;
        CURSOR_SPEED_RESTORE_AT_EXIT.store(0, Ordering::SeqCst);
        let _ = fs::remove_file(path);
        Ok(Some(origin_speed))
    }

    fn read_cursor_speed() -> Result<i32, Win32Error> {
        let mut speed = 0i32;
        unsafe {
            SystemParametersInfoW(
                SPI_GETMOUSESPEED,
                0,
                Some((&mut speed as *mut i32).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .map_err(|error| Win32Error::Api(format!("SPI_GETMOUSESPEED failed: {error:?}")))?;
        Ok(speed.clamp(1, 20))
    }

    fn read_mouse_acceleration() -> Result<bool, Win32Error> {
        let mut values = [0i32; 3];
        unsafe {
            SystemParametersInfoW(
                SPI_GETMOUSE,
                0,
                Some(values.as_mut_ptr().cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .map_err(|error| Win32Error::Api(format!("SPI_GETMOUSE failed: {error:?}")))?;
        Ok(values[2] != 0)
    }

    fn set_cursor_speed(speed: i32) -> Result<(), Win32Error> {
        let speed = speed.clamp(1, 20);
        unsafe {
            SystemParametersInfoW(
                SPI_SETMOUSESPEED,
                0,
                Some(speed as isize as *mut c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .map_err(|error| Win32Error::Api(format!("SPI_SETMOUSESPEED failed: {error:?}")))
    }

    fn request_source_focus(target_hwnd: isize) -> String {
        if target_hwnd == 0 {
            return "source_focus_skipped invalid_hwnd".to_string();
        }
        let hwnd = hwnd_from_raw(target_hwnd);
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            return "source_focus_skipped invalid_hwnd".to_string();
        }
        let focused = unsafe { SetForegroundWindow(hwnd).as_bool() };
        if focused {
            "source_focus_requested=true".to_string()
        } else {
            "source_focus_requested=false".to_string()
        }
    }

    const TRAY_ICON_ID: u32 = 1;
    const TRAY_CALLBACK_MESSAGE: u32 = WM_APP + 1;
    const NIN_SELECT: u32 = WM_USER;
    const NIN_KEYSELECT: u32 = WM_USER + 1;

    #[derive(Debug)]
    pub struct ShellTrayIcon {
        hwnd: HWND,
        menu_items: Vec<TrayMenuItem>,
        installed: bool,
    }

    fn append_tray_menu_items(
        menu: windows::Win32::UI::WindowsAndMessaging::HMENU,
        menu_items: &[TrayMenuItem],
    ) -> Result<usize, Win32Error> {
        let mut appended = 0usize;
        for (index, item) in menu_items.iter().enumerate() {
            let mut flags = MF_STRING;
            flags |= if item.enabled {
                MF_UNCHECKED
            } else {
                MF_GRAYED
            };
            if item.checked {
                flags |= MF_CHECKED;
            }
            let label = wide_null(item.label);
            unsafe { AppendMenuW(menu, flags, 1000 + index, PCWSTR(label.as_ptr())) }
                .map_err(|error| Win32Error::Api(format!("AppendMenuW failed: {error:?}")))?;
            appended += 1;
        }
        Ok(appended)
    }

    impl ShellTrayIcon {
        pub fn install(menu_items: Vec<TrayMenuItem>) -> Result<Self, Win32Error> {
            unsafe extern "system" fn wnd_proc(
                hwnd: HWND,
                msg: u32,
                wparam: WPARAM,
                lparam: LPARAM,
            ) -> LRESULT {
                if msg == WM_DESTROY {
                    return LRESULT(0);
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }

            let instance = unsafe { GetModuleHandleW(None) }
                .map_err(|error| Win32Error::Api(format!("GetModuleHandleW failed: {error:?}")))?;
            let class_name = windows::core::w!("DodbogiShellMessageWindow");
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wnd_proc),
                hInstance: HINSTANCE(instance.0),
                lpszClassName: class_name,
                ..Default::default()
            };
            let atom = unsafe { RegisterClassW(&wc) };
            if atom == 0 {
                let err = unsafe { GetLastError() };
                if err.0 != 1410 {
                    return Err(Win32Error::Api(format!("RegisterClassW failed: {err:?}")));
                }
            }

            let hwnd = unsafe {
                CreateWindowExW(
                    Default::default(),
                    class_name,
                    windows::core::w!("Dodbogi Shell"),
                    WS_POPUP,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(HINSTANCE(instance.0)),
                    None,
                )
            }
            .map_err(|error| {
                Win32Error::Api(format!("CreateWindowExW shell window failed: {error:?}"))
            })?;

            let icon = unsafe { LoadIconW(None, IDI_APPLICATION) }
                .map_err(|error| Win32Error::Api(format!("LoadIconW failed: {error:?}")))?;
            let mut data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: hwnd,
                uID: TRAY_ICON_ID,
                uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
                uCallbackMessage: TRAY_CALLBACK_MESSAGE,
                hIcon: icon,
                ..Default::default()
            };
            copy_wide(&mut data.szTip, "Dodbogi window upscaler");

            if !unsafe { Shell_NotifyIconW(NIM_ADD, &data).as_bool() } {
                let err = unsafe { GetLastError() };
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(Win32Error::Api(format!(
                    "Shell_NotifyIconW(NIM_ADD) failed: {err:?}"
                )));
            }

            data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
            if !unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data).as_bool() } {
                let err = unsafe { GetLastError() };
                let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
                let _ = unsafe { DestroyWindow(hwnd) };
                return Err(Win32Error::Api(format!(
                    "Shell_NotifyIconW(NIM_SETVERSION) failed: {err:?}"
                )));
            }

            Ok(Self {
                hwnd,
                menu_items,
                installed: true,
            })
        }

        pub fn install_default() -> Result<Self, Win32Error> {
            Self::install(default_tray_menu_items())
        }

        pub fn is_installed(&self) -> bool {
            self.installed
        }

        pub fn menu_items(&self) -> &[TrayMenuItem] {
            &self.menu_items
        }

        pub fn build_menu_probe(&self) -> Result<usize, Win32Error> {
            let menu = unsafe { CreatePopupMenu() }
                .map_err(|error| Win32Error::Api(format!("CreatePopupMenu failed: {error:?}")))?;
            let appended = append_tray_menu_items(menu, &self.menu_items);

            unsafe { DestroyMenu(menu) }
                .map_err(|error| Win32Error::Api(format!("DestroyMenu failed: {error:?}")))?;
            appended
        }

        pub fn show_context_menu_at_cursor(&self) -> Result<Option<&'static str>, Win32Error> {
            let menu = unsafe { CreatePopupMenu() }
                .map_err(|error| Win32Error::Api(format!("CreatePopupMenu failed: {error:?}")))?;
            append_tray_menu_items(menu, &self.menu_items)?;

            let mut point = POINT::default();
            unsafe { GetCursorPos(&mut point) }
                .map_err(|error| Win32Error::Api(format!("GetCursorPos failed: {error:?}")))?;
            let _ = unsafe { SetForegroundWindow(self.hwnd) };
            let command = unsafe {
                windows::Win32::UI::WindowsAndMessaging::TrackPopupMenu(
                    menu,
                    windows::Win32::UI::WindowsAndMessaging::TPM_RETURNCMD
                        | windows::Win32::UI::WindowsAndMessaging::TPM_RIGHTBUTTON,
                    point.x,
                    point.y,
                    None,
                    self.hwnd,
                    None,
                )
            };
            unsafe { DestroyMenu(menu) }
                .map_err(|error| Win32Error::Api(format!("DestroyMenu failed: {error:?}")))?;

            if command.0 == 0 {
                return Ok(None);
            }
            let index = command.0.saturating_sub(1000) as usize;
            Ok(self.menu_items.get(index).map(|item| item.id))
        }

        pub fn poll_message(&self) -> Option<ShellMessage> {
            let msg = next_message()?;
            let shell_message = self.message_from_win32(&msg);
            dispatch_message(&msg);
            shell_message
        }

        pub fn drain_messages(&self, limit: usize) -> Vec<ShellMessage> {
            let mut messages = Vec::new();
            for _ in 0..limit {
                let Some(msg) = next_message() else {
                    break;
                };
                if let Some(shell_message) = self.message_from_win32(&msg) {
                    messages.push(shell_message);
                }
                dispatch_message(&msg);
            }
            messages
        }

        fn message_from_win32(
            &self,
            msg: &windows::Win32::UI::WindowsAndMessaging::MSG,
        ) -> Option<ShellMessage> {
            match msg.message {
                WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK
                | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_RBUTTONDBLCLK | WM_MBUTTONDOWN
                | WM_MBUTTONUP | WM_MBUTTONDBLCLK | WM_MOUSEWHEEL => overlay_input_from_win32(msg),
                WM_HOTKEY => Some(ShellMessage::Hotkey {
                    id: msg.wParam.0 as u32,
                    name: hotkey_name(msg.wParam.0 as u32),
                }),
                WM_COMMAND => self.item_id_for_command(msg.wParam.0),
                TRAY_CALLBACK_MESSAGE => {
                    if msg.hwnd != self.hwnd {
                        return None;
                    }
                    let mouse_message = msg.lParam.0 as u32;
                    if mouse_message == WM_LBUTTONUP
                        || mouse_message == WM_LBUTTONDBLCLK
                        || mouse_message == NIN_SELECT
                        || mouse_message == NIN_KEYSELECT
                    {
                        return Some(ShellMessage::TrayMenu {
                            item_id: "settings",
                        });
                    }
                    if mouse_message == WM_RBUTTONUP || mouse_message == WM_CONTEXTMENU {
                        return match self.show_context_menu_at_cursor() {
                            Ok(Some(item_id)) => Some(ShellMessage::TrayMenu { item_id }),
                            Ok(None) => None,
                            Err(error) => Some(ShellMessage::TrayError(format!("{error:?}"))),
                        };
                    }
                    None
                }
                WM_QUIT => Some(ShellMessage::Quit),
                _ => None,
            }
        }

        fn item_id_for_command(&self, command: usize) -> Option<ShellMessage> {
            let index = command.saturating_sub(1000);
            self.menu_items
                .get(index)
                .map(|item| ShellMessage::TrayMenu { item_id: item.id })
        }
    }

    impl Drop for ShellTrayIcon {
        fn drop(&mut self) {
            if self.installed {
                let data = NOTIFYICONDATAW {
                    cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                    hWnd: self.hwnd,
                    uID: TRAY_ICON_ID,
                    ..Default::default()
                };
                let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &data) };
                self.installed = false;
            }
            if !is_null_hwnd(self.hwnd) {
                let _ = unsafe { DestroyWindow(self.hwnd) };
            }
        }
    }

    fn next_message() -> Option<windows::Win32::UI::WindowsAndMessaging::MSG> {
        let mut msg = windows::Win32::UI::WindowsAndMessaging::MSG::default();
        if !unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() } {
            return None;
        }
        Some(msg)
    }

    fn shell_message_from_defaults(
        msg: &windows::Win32::UI::WindowsAndMessaging::MSG,
    ) -> Option<ShellMessage> {
        match msg.message {
            WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_LBUTTONDBLCLK | WM_RBUTTONDOWN
            | WM_RBUTTONUP | WM_RBUTTONDBLCLK | WM_MBUTTONDOWN | WM_MBUTTONUP
            | WM_MBUTTONDBLCLK | WM_MOUSEWHEEL => overlay_input_from_win32(msg),
            WM_HOTKEY => Some(ShellMessage::Hotkey {
                id: msg.wParam.0 as u32,
                name: hotkey_name(msg.wParam.0 as u32),
            }),
            WM_COMMAND => {
                let command = msg.wParam.0;
                let index = command.saturating_sub(1000);
                default_tray_menu_items()
                    .get(index)
                    .map(|item| ShellMessage::TrayMenu { item_id: item.id })
            }
            WM_QUIT => Some(ShellMessage::Quit),
            _ => None,
        }
    }

    fn dispatch_message(msg: &windows::Win32::UI::WindowsAndMessaging::MSG) {
        unsafe {
            let _ = TranslateMessage(msg);
            DispatchMessageW(msg);
        }
    }

    fn signed_word(value: usize, shift: u32) -> i32 {
        (((value >> shift) & 0xffff) as u16 as i16) as i32
    }

    fn lparam_point(lparam: LPARAM) -> POINT {
        POINT {
            x: signed_word(lparam.0 as usize, 0),
            y: signed_word(lparam.0 as usize, 16),
        }
    }

    fn wparam_wheel_delta(wparam: WPARAM) -> i32 {
        signed_word(wparam.0, 16)
    }

    fn client_to_screen_point(hwnd: HWND, mut point: POINT) -> Option<POINT> {
        if unsafe { ClientToScreen(hwnd, &mut point).as_bool() } {
            Some(point)
        } else {
            None
        }
    }

    fn overlay_input_from_win32(
        msg: &windows::Win32::UI::WindowsAndMessaging::MSG,
    ) -> Option<ShellMessage> {
        if is_null_hwnd(msg.hwnd) {
            return None;
        }
        let kind = match msg.message {
            WM_MOUSEMOVE => InputEventKind::MouseMove,
            WM_LBUTTONDOWN => InputEventKind::MouseButtonDown(MouseButton::Left),
            WM_LBUTTONUP => InputEventKind::MouseButtonUp(MouseButton::Left),
            WM_LBUTTONDBLCLK => InputEventKind::DoubleClick(MouseButton::Left),
            WM_RBUTTONDOWN => InputEventKind::MouseButtonDown(MouseButton::Right),
            WM_RBUTTONUP => InputEventKind::MouseButtonUp(MouseButton::Right),
            WM_RBUTTONDBLCLK => InputEventKind::DoubleClick(MouseButton::Right),
            WM_MBUTTONDOWN => InputEventKind::MouseButtonDown(MouseButton::Middle),
            WM_MBUTTONUP => InputEventKind::MouseButtonUp(MouseButton::Middle),
            WM_MBUTTONDBLCLK => InputEventKind::DoubleClick(MouseButton::Middle),
            WM_MOUSEWHEEL => InputEventKind::Wheel {
                delta: wparam_wheel_delta(msg.wParam),
            },
            _ => return None,
        };
        let point = if msg.message == WM_MOUSEWHEEL {
            lparam_point(msg.lParam)
        } else {
            client_to_screen_point(msg.hwnd, lparam_point(msg.lParam))?
        };
        Some(ShellMessage::OverlayInput {
            hwnd: hwnd_to_raw(msg.hwnd),
            kind,
            screen_x: point.x,
            screen_y: point.y,
        })
    }

    pub fn poll_shell_message() -> Option<ShellMessage> {
        let msg = next_message()?;
        let shell_message = shell_message_from_defaults(&msg);
        dispatch_message(&msg);
        shell_message
    }

    pub fn drain_shell_messages(limit: usize) -> Vec<ShellMessage> {
        let mut messages = Vec::new();
        for _ in 0..limit {
            let Some(msg) = next_message() else {
                break;
            };
            if let Some(shell_message) = shell_message_from_defaults(&msg) {
                messages.push(shell_message);
            }
            dispatch_message(&msg);
        }
        messages
    }

    pub fn pump_one_message_if_available() {
        let _ = poll_shell_message();
    }

    pub fn run_controlled_input_probe() -> Result<ControlledInputProbeReport, Win32Error> {
        unsafe extern "system" fn wnd_proc(
            hwnd: HWND,
            msg: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            match msg {
                WM_LBUTTONDOWN => {
                    if let Ok(mut counters) = input_probe_counters().lock() {
                        counters.left_down += 1;
                    }
                    LRESULT(0)
                }
                WM_LBUTTONUP => {
                    if let Ok(mut counters) = input_probe_counters().lock() {
                        counters.left_up += 1;
                    }
                    LRESULT(0)
                }
                WM_DESTROY => LRESULT(0),
                _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }

        {
            let mut counters = input_probe_counters()
                .lock()
                .map_err(|_| Win32Error::Api("input probe counter lock poisoned".to_string()))?;
            *counters = InputProbeCounters::default();
        }

        let instance = unsafe { GetModuleHandleW(None) }
            .map_err(|error| Win32Error::Api(format!("GetModuleHandleW failed: {error:?}")))?;
        let class_name = windows::core::w!("DodbogiInputProbeWindow");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: HINSTANCE(instance.0),
            lpszClassName: class_name,
            ..Default::default()
        };
        let atom = unsafe { RegisterClassW(&wc) };
        if atom == 0 {
            let err = unsafe { GetLastError() };
            if err.0 != 1410 {
                return Err(Win32Error::Api(format!("RegisterClassW failed: {err:?}")));
            }
        }

        let probe_left = 420;
        let probe_top = 220;
        let probe_width = 160;
        let probe_height = 96;

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                class_name,
                windows::core::w!("Dodbogi Input Probe"),
                WS_POPUP,
                probe_left,
                probe_top,
                probe_width,
                probe_height,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )
        }
        .map_err(|error| {
            Win32Error::Api(format!("CreateWindowExW input probe failed: {error:?}"))
        })?;

        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                probe_left,
                probe_top,
                probe_width,
                probe_height,
                SWP_SHOWWINDOW | SWP_NOSENDCHANGING,
            )
        }
        .map_err(|error| Win32Error::Api(format!("SetWindowPos input probe failed: {error:?}")))?;
        let _ = unsafe { SetForegroundWindow(hwnd) };
        for _ in 0..8 {
            if let Some(msg) = next_message() {
                dispatch_message(&msg);
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }

        let center = SourcePoint {
            x: (probe_left + probe_width / 2) as f32,
            y: (probe_top + probe_height / 2) as f32,
        };
        let down = deliver_input_to_source(
            hwnd_to_raw(hwnd),
            SourceInputEvent {
                kind: InputEventKind::MouseButtonDown(MouseButton::Left),
                point: Some(center),
            },
            InputDeliveryMode::SendInputAllowed,
        )?;
        let up = deliver_input_to_source(
            hwnd_to_raw(hwnd),
            SourceInputEvent {
                kind: InputEventKind::MouseButtonUp(MouseButton::Left),
                point: Some(center),
            },
            InputDeliveryMode::SendInputAllowed,
        )?;

        for _ in 0..32 {
            if let Some(msg) = next_message() {
                dispatch_message(&msg);
            } else {
                thread::sleep(Duration::from_millis(5));
            }
        }

        let (observed_left_down, observed_left_up) = {
            let counters = input_probe_counters()
                .lock()
                .map_err(|_| Win32Error::Api("input probe counter lock poisoned".to_string()))?;
            (counters.left_down, counters.left_up)
        };

        let _ = unsafe { DestroyWindow(hwnd) };

        let delivered =
            down.delivered && up.delivered && observed_left_down > 0 && observed_left_up > 0;
        Ok(ControlledInputProbeReport {
            target_hwnd: hwnd_to_raw(hwnd),
            sent_events: (down.delivered as u32) + (up.delivered as u32),
            observed_left_down,
            observed_left_up,
            delivered,
            detail: if delivered {
                "controlled SendInput probe delivered and target HWND observed mouse down/up"
                    .to_string()
            } else {
                format!(
                    "SendInput reports down={} up={} but observed down={} up={}",
                    down.delivered, up.delivered, observed_left_down, observed_left_up
                )
            },
        })
    }

    fn button_flags(
        button: MouseButton,
    ) -> (
        windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
        windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
    ) {
        match button {
            MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
            MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
        }
    }

    fn mouse_input(
        flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
        mouse_data: u32,
    ) -> INPUT {
        INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: mouse_data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn key_input(
        virtual_key: u16,
        flags: windows::Win32::UI::Input::KeyboardAndMouse::KEYBD_EVENT_FLAGS,
    ) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(virtual_key),
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn unicode_input(code_unit: u16, key_up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: code_unit,
                    dwFlags: if key_up {
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                    } else {
                        KEYEVENTF_UNICODE
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    fn input_sequence(kind: InputEventKind) -> Vec<INPUT> {
        match kind {
            InputEventKind::MouseMove => vec![mouse_input(MOUSEEVENTF_MOVE, 0)],
            InputEventKind::MouseButtonDown(button) => vec![mouse_input(button_flags(button).0, 0)],
            InputEventKind::MouseButtonUp(button) => vec![mouse_input(button_flags(button).1, 0)],
            InputEventKind::DoubleClick(button) => {
                let (down, up) = button_flags(button);
                vec![
                    mouse_input(down, 0),
                    mouse_input(up, 0),
                    mouse_input(down, 0),
                    mouse_input(up, 0),
                ]
            }
            InputEventKind::Wheel { delta } => vec![mouse_input(MOUSEEVENTF_WHEEL, delta as u32)],
            InputEventKind::Drag { button, phase } => {
                let (down, up) = button_flags(button);
                match phase {
                    DragPhase::Start => vec![mouse_input(down, 0)],
                    DragPhase::Move => vec![mouse_input(MOUSEEVENTF_MOVE, 0)],
                    DragPhase::End => vec![mouse_input(up, 0)],
                }
            }
            InputEventKind::TextSelection { phase } => match phase {
                TextSelectionPhase::Start => vec![mouse_input(MOUSEEVENTF_LEFTDOWN, 0)],
                TextSelectionPhase::Update => vec![mouse_input(MOUSEEVENTF_MOVE, 0)],
                TextSelectionPhase::End => vec![mouse_input(MOUSEEVENTF_LEFTUP, 0)],
            },
            InputEventKind::ContextMenu => vec![
                mouse_input(MOUSEEVENTF_RIGHTDOWN, 0),
                mouse_input(MOUSEEVENTF_RIGHTUP, 0),
            ],
            InputEventKind::KeyboardFocus => Vec::new(),
            InputEventKind::KeyDown { virtual_key } => {
                vec![key_input(virtual_key, Default::default())]
            }
            InputEventKind::KeyUp { virtual_key } => vec![key_input(virtual_key, KEYEVENTF_KEYUP)],
            InputEventKind::TextInput { ch } => {
                let mut units = [0u16; 2];
                ch.encode_utf16(&mut units)
                    .iter()
                    .flat_map(|unit| [unicode_input(*unit, false), unicode_input(*unit, true)])
                    .collect()
            }
            InputEventKind::Touch { .. } => Vec::new(),
        }
    }

    pub fn deliver_input_to_source(
        target_hwnd: isize,
        event: SourceInputEvent,
        mode: InputDeliveryMode,
    ) -> Result<InputDeliveryReport, Win32Error> {
        let source_point = event
            .point
            .map(|point| (point.x.round() as i32, point.y.round() as i32));
        let event_kind = input_event_kind_name(event.kind);

        if target_hwnd == 0 {
            return Err(Win32Error::InvalidWindow);
        }

        if mode == InputDeliveryMode::DryRun {
            return Ok(InputDeliveryReport {
                mode,
                target_hwnd,
                event_kind,
                source_point,
                delivered: false,
                detail: "dry-run; SendInput not called".to_string(),
            });
        }

        let hwnd = hwnd_from_raw(target_hwnd);
        if !unsafe { IsWindow(Some(hwnd)).as_bool() } {
            return Err(Win32Error::InvalidWindow);
        }

        if matches!(event.kind, InputEventKind::MouseMove) {
            return Ok(InputDeliveryReport {
                mode,
                target_hwnd,
                event_kind,
                source_point,
                delivered: false,
                detail: "standalone mouse move is intentionally not delivered; hover must not warp the hardware cursor".to_string(),
            });
        }

        if let Some((x, y)) = source_point {
            reliable_set_cursor_pos(x, y)?;
        }

        if matches!(event.kind, InputEventKind::KeyboardFocus) {
            let focused = unsafe { SetForegroundWindow(hwnd).as_bool() };
            return Ok(InputDeliveryReport {
                mode,
                target_hwnd,
                event_kind,
                source_point,
                delivered: focused,
                detail: if focused {
                    "source HWND foreground requested for keyboard forwarding".to_string()
                } else {
                    "SetForegroundWindow was denied by Windows focus policy".to_string()
                },
            });
        }

        let inputs = input_sequence(event.kind);
        if inputs.is_empty() {
            return Ok(InputDeliveryReport {
                mode,
                target_hwnd,
                event_kind,
                source_point,
                delivered: false,
                detail: "event kind has no safe SendInput representation in this wrapper"
                    .to_string(),
            });
        }

        let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if sent != inputs.len() as u32 {
            return Err(Win32Error::Api(format!(
                "SendInput sent {sent} of {} events",
                inputs.len()
            )));
        }

        Ok(InputDeliveryReport {
            mode,
            target_hwnd,
            event_kind,
            source_point,
            delivered: true,
            detail: format!("SendInput delivered {} event(s)", inputs.len()),
        })
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{
        input_event_kind_name, ControlledInputProbeReport, CursorCaptureReport, InputDeliveryMode,
        InputDeliveryReport, MonitorGeometry, OverlayStyleContract, PhysicalRect, ShellMessage,
        SourceInputEvent, SourceWindow, SystemHotkeyReport, TrayMenuItem, Win32Error,
    };
    use dodbogi_input::InputTransform;
    use std::path::{Path, PathBuf};

    pub fn foreground_source_window() -> Result<SourceWindow, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn source_window_from_raw(_hwnd: isize) -> Result<SourceWindow, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn is_foreground_move_size_active() -> bool {
        false
    }

    pub fn move_window_for_probe(
        _hwnd: isize,
        _dx: i32,
        _dy: i32,
    ) -> Result<SourceWindow, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn resize_window_for_probe(
        _hwnd: isize,
        _width_delta: i32,
        _height_delta: i32,
    ) -> Result<SourceWindow, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn probe_d3d11_feature_level() -> Result<String, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn create_wgc_item_for_hwnd(_hwnd: isize) -> Result<(), Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn enumerate_monitors() -> Result<Vec<MonitorGeometry>, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn client_rect_from_raw(_hwnd: isize) -> Result<PhysicalRect, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    #[derive(Debug)]
    pub struct SystemHotkeyGuard {
        report: SystemHotkeyReport,
    }

    impl SystemHotkeyGuard {
        pub fn register_defaults() -> Self {
            Self {
                report: SystemHotkeyReport {
                    registrations: Vec::new(),
                },
            }
        }

        pub fn report(&self) -> &SystemHotkeyReport {
            &self.report
        }
    }

    pub struct OverlayWindow;

    impl OverlayWindow {
        pub fn create_hidden() -> Result<Self, Win32Error> {
            Err(Win32Error::NotImplemented("Windows-only"))
        }

        pub fn hwnd(&self) -> isize {
            0
        }

        pub fn attach_to_source(&self, _source_hwnd: isize) -> Result<(), Win32Error> {
            Err(Win32Error::NotImplemented("Windows-only"))
        }

        pub fn apply_layout(
            &self,
            _rect: PhysicalRect,
            _show: bool,
        ) -> Result<OverlayStyleContract, Win32Error> {
            Err(Win32Error::NotImplemented("Windows-only"))
        }

        pub fn style_contract() -> OverlayStyleContract {
            OverlayStyleContract {
                no_activate: true,
                topmost: false,
                tool_window: true,
                input_passthrough: true,
                layered_passthrough: true,
                taskbar_entry: false,
                alt_tab_entry: false,
            }
        }
    }

    pub struct CursorCaptureController;

    impl CursorCaptureController {
        pub fn create() -> Result<Self, Win32Error> {
            Err(Win32Error::NotImplemented("Windows-only"))
        }

        pub fn create_with_speed_guard_path(
            _cursor_speed_guard_path: Option<PathBuf>,
        ) -> Result<Self, Win32Error> {
            Err(Win32Error::NotImplemented("Windows-only"))
        }

        pub fn update(
            &mut self,
            _transform: &InputTransform,
            _target_hwnd: isize,
        ) -> Result<Option<CursorCaptureReport>, Win32Error> {
            Err(Win32Error::NotImplemented("Windows-only"))
        }

        pub fn release(&mut self) {}
    }

    #[derive(Debug)]
    pub struct ShellTrayIcon {
        menu_items: Vec<TrayMenuItem>,
    }

    impl ShellTrayIcon {
        pub fn install(menu_items: Vec<TrayMenuItem>) -> Result<Self, Win32Error> {
            Ok(Self { menu_items })
        }

        pub fn install_default() -> Result<Self, Win32Error> {
            Ok(Self {
                menu_items: super::default_tray_menu_items(),
            })
        }

        pub fn is_installed(&self) -> bool {
            false
        }

        pub fn menu_items(&self) -> &[TrayMenuItem] {
            &self.menu_items
        }

        pub fn build_menu_probe(&self) -> Result<usize, Win32Error> {
            Ok(self.menu_items.len())
        }

        pub fn show_context_menu_at_cursor(&self) -> Result<Option<&'static str>, Win32Error> {
            Ok(None)
        }

        pub fn poll_message(&self) -> Option<ShellMessage> {
            None
        }

        pub fn drain_messages(&self, _limit: usize) -> Vec<ShellMessage> {
            Vec::new()
        }
    }

    pub fn poll_shell_message() -> Option<ShellMessage> {
        None
    }

    pub fn drain_shell_messages(_limit: usize) -> Vec<ShellMessage> {
        Vec::new()
    }

    pub fn pump_one_message_if_available() {
        let _ = poll_shell_message();
    }

    pub fn run_controlled_input_probe() -> Result<ControlledInputProbeReport, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn cursor_position_for_probe() -> Result<(i32, i32), Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn move_cursor_for_probe(_x: i32, _y: i32) -> Result<(), Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn cursor_speed_for_probe() -> Result<i32, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn set_cursor_speed_for_probe(_speed: i32) -> Result<(), Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn recover_cursor_speed_guard(_path: &Path) -> Result<Option<i32>, Win32Error> {
        Err(Win32Error::NotImplemented("Windows-only"))
    }

    pub fn deliver_input_to_source(
        target_hwnd: isize,
        event: SourceInputEvent,
        mode: InputDeliveryMode,
    ) -> Result<InputDeliveryReport, Win32Error> {
        if mode == InputDeliveryMode::DryRun {
            Ok(InputDeliveryReport {
                mode,
                target_hwnd,
                event_kind: input_event_kind_name(event.kind),
                source_point: event
                    .point
                    .map(|point| (point.x.round() as i32, point.y.round() as i32)),
                delivered: false,
                detail: "dry-run; Windows-only SendInput not called".to_string(),
            })
        } else {
            Err(Win32Error::NotImplemented("Windows-only"))
        }
    }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;
    use dodbogi_input::MouseButton;

    #[test]
    fn hotkey_registry_registers_and_unregisters_defaults() {
        let mut registry = HotkeyRegistry::default();
        registry.register_defaults();
        assert_eq!(registry.registered().len(), 2);
        assert_eq!(registry.registered()[0].accelerator, "Ctrl+Alt+Q");
        assert_eq!(registry.registered()[1].accelerator, "Ctrl+Alt+A");
        registry.unregister_all();
        assert!(registry.registered().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn dynamic_hotkey_parser_accepts_user_selected_modifier_sets() {
        let (mods, vk) =
            imp::parse_hotkey_accelerator_for_test("Shift+F9").expect("Shift+F9 should parse");
        assert_eq!(vk, 0x78);
        assert_eq!(mods & 0x0004, 0x0004);
        assert_eq!(mods & 0x0002, 0, "Ctrl must not be forced");
        assert_eq!(mods & 0x0001, 0, "Alt must not be forced");

        let (mods, vk) =
            imp::parse_hotkey_accelerator_for_test("Win+Space").expect("Win+Space should parse");
        assert_eq!(vk, 0x20);
        assert_eq!(mods & 0x0008, 0x0008);
    }

    #[test]
    fn system_hotkey_report_counts_successes_and_failures() {
        let defaults = default_hotkeys();
        let report = SystemHotkeyReport {
            registrations: vec![
                SystemHotkeyRegistration {
                    spec: defaults[0].clone(),
                    registered: true,
                    detail: "ok".to_string(),
                },
                SystemHotkeyRegistration {
                    spec: defaults[1].clone(),
                    registered: false,
                    detail: "conflict".to_string(),
                },
            ],
        };

        assert_eq!(report.registered_count(), 1);
        assert_eq!(report.failed_count(), 1);
    }

    #[test]
    fn tray_placeholder_exposes_expected_menu_contract() {
        let mut tray = TrayController::default();
        tray.install_placeholder();
        assert!(tray.is_installed());
        assert!(tray.menu_items().iter().any(|item| item.id == "settings"));
        assert!(tray
            .menu_items()
            .iter()
            .any(|item| item.id == "diagnostics"));
        assert!(tray.menu_items().iter().any(|item| item.id == "exit"));
        tray.remove();
        assert!(!tray.is_installed());
        assert!(tray.menu_items().is_empty());
    }

    #[test]
    fn overlay_style_contract_is_noactivate_not_topmost_and_input_passthrough() {
        let style = OverlayWindow::style_contract();
        assert!(style.no_activate);
        assert!(!style.topmost);
        assert!(style.tool_window);
        assert!(style.input_passthrough);
        assert!(style.layered_passthrough);
        assert!(!style.taskbar_entry);
        assert!(!style.alt_tab_entry);
    }

    #[test]
    fn cursor_speed_adjustment_matches_zoom_scale_with_acceleration() {
        assert_eq!(adjusted_mouse_speed(10, 2.0, true), 5);
        assert_eq!(adjusted_mouse_speed(1, 4.0, true), 1);
        assert_eq!(adjusted_mouse_speed(20, 0.5, true), 20);
    }

    #[test]
    fn cursor_speed_adjustment_uses_windows_sensitivity_curve_without_acceleration() {
        assert_eq!(adjusted_mouse_speed(10, 2.0, false), 6);
        assert_eq!(adjusted_mouse_speed(10, 1.0, false), 10);
        assert_eq!(adjusted_mouse_speed(20, 2.0, false), 13);
    }

    #[test]
    fn dry_run_input_delivery_does_not_send_input() {
        let report = deliver_input_to_source(
            1,
            SourceInputEvent {
                kind: InputEventKind::MouseButtonDown(MouseButton::Left),
                point: Some(dodbogi_input::SourcePoint { x: 10.0, y: 20.0 }),
            },
            InputDeliveryMode::DryRun,
        )
        .expect("dry-run should not require a live HWND on non-Windows");
        assert!(!report.delivered);
        assert_eq!(report.source_point, Some((10, 20)));
        assert_eq!(report.event_kind, "mouse_button_down");
    }

    #[cfg(windows)]
    #[test]
    fn live_mouse_move_delivery_is_suppressed_to_avoid_cursor_warp() {
        let window = OverlayWindow::create_hidden().expect("hidden HWND should be creatable");
        let report = deliver_input_to_source(
            window.hwnd(),
            SourceInputEvent {
                kind: InputEventKind::MouseMove,
                point: Some(dodbogi_input::SourcePoint { x: 10.0, y: 20.0 }),
            },
            InputDeliveryMode::SendInputAllowed,
        )
        .expect("mouse move suppression should not require SendInput");

        assert!(!report.delivered);
        assert_eq!(report.event_kind, "mouse_move");
        assert_eq!(report.source_point, Some((10, 20)));
        assert!(report.detail.contains("hover must not warp"));
    }
}
