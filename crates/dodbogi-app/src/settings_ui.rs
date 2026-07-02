use dodbogi_core::{
    AppProfile, DodbogiSettings, RuntimePaths, load_settings_from_path, save_settings_to_path,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicIsize, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::{
            COLORREF, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::Gdi::{
            AddFontMemResourceEx, BeginPaint, CLIP_DEFAULT_PRECIS, ClientToScreen,
            CreateCompatibleDC, CreateFontW, CreatePen, CreateSolidBrush, DEFAULT_CHARSET,
            DEFAULT_GUI_FONT, DEFAULT_PITCH, DEFAULT_QUALITY, DT_CENTER, DT_END_ELLIPSIS, DT_LEFT,
            DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, EndPaint,
            FillRect, GetStockObject, HBRUSH, HDC, HGDIOBJ, HOLLOW_BRUSH, InvalidateRect,
            OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE,
            RDW_UPDATENOW, Rectangle, RedrawWindow, RoundRect, SRCCOPY, SelectObject, SetBkMode,
            SetTextColor, StretchBlt, TRANSPARENT, TextOutW, UpdateWindow, WHITE_BRUSH,
        },
        System::{Com::CoTaskMemFree, LibraryLoader::GetModuleHandleW},
        UI::{
            Input::KeyboardAndMouse::{
                EnableWindow, GetAsyncKeyState, GetFocus, GetKeyState, SetFocus, TME_LEAVE,
                TRACKMOUSEEVENT, TrackMouseEvent, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
            },
            Shell::{
                BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS, BROWSEINFOW, SHBrowseForFolderW,
                SHGetPathFromIDListW,
            },
            WindowsAndMessaging::{
                BN_CLICKED, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CallWindowProcW,
                CreateWindowExW, DefWindowProcW, EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL,
                ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, GWLP_WNDPROC, GetClientRect,
                GetCursorPos, GetDlgCtrlID, GetDlgItem, GetForegroundWindow, GetParent,
                GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU, HWND_TOP, IDC_ARROW,
                IDYES, IMAGE_BITMAP, IMAGE_ICON, IsWindowVisible, KillTimer, LB_ADDSTRING,
                LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBN_DBLCLK, LBN_SELCHANGE, LBS_NOTIFY,
                LR_LOADFROMFILE, LoadCursorW, LoadImageW, MB_ICONERROR, MB_ICONQUESTION, MB_OK,
                MB_YESNO, MINMAXINFO, MessageBoxW, RegisterClassW, SET_WINDOW_POS_FLAGS,
                STM_SETIMAGE, SW_HIDE, SW_RESTORE, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
                SWP_NOZORDER, SWP_SHOWWINDOW, SendMessageW, SetForegroundWindow, SetTimer,
                SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC, WM_DESTROY,
                WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP,
                WM_MOUSEMOVE, WM_NCCREATE, WM_NCLBUTTONDOWN, WM_PAINT, WM_PARENTNOTIFY, WM_SETFONT,
                WM_SETICON, WM_SIZE, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WNDPROC, WS_BORDER,
                WS_CHILD, WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
                WS_VISIBLE, WS_VSCROLL,
            },
        },
    },
    core::{PCWSTR, PWSTR, w},
};

const MIN_TRACK_WIDTH: i32 = 720;
const MIN_TRACK_HEIGHT: i32 = 760;
const ID_LIVE_APPLY_TIMER: usize = 2001;

const HOTKEY_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/hotkey.bmp");
const SCALE_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/scale.bmp");
const SAVE_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/save.bmp");
const WINDOW_ZOOM_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/window-zoom.bmp");
const POINTER_ZOOM_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/pointer-zoom.bmp");
const SETTINGS_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/settings.bmp");
const TRAY_ICON_BMP: &[u8] = include_bytes!("../assets/icons/32/minimize-to-tray.bmp");
const APP_ICON_ICO: &[u8] = include_bytes!("../assets/icons/app.ico");
const UI_FONT_TTF: &[u8] = include_bytes!("../assets/fonts/NeoDunggeunmoPro-Regular.ttf");
const UI_FONT_FACE: &str = "NeoDunggeunmo Pro";
const ROW_ICON_SIZE: i32 = 32;
const SS_BITMAP_STYLE: i32 = 0x000E;
const SS_NOTIFY_STYLE: i32 = 0x0100;
const SS_LEFTNOWORDWRAP_STYLE: i32 = 0x000C;
const SS_WHITERECT_STYLE: i32 = 0x0006;
const BS_OWNERDRAW_STYLE: i32 = 0x000B;
const STN_CLICKED_NOTIFY: u32 = 0;
const LBS_OWNERDRAWFIXED_STYLE: i32 = 0x0010;
const LBS_HASSTRINGS_STYLE: i32 = 0x0040;
const LBS_NOINTEGRALHEIGHT_STYLE: i32 = 0x0100;
const WM_DRAWITEM_MSG: u32 = 0x002B;
const WM_MEASUREITEM_MSG: u32 = 0x002C;
const WM_SETREDRAW_MSG: u32 = 0x000B;
const WM_MOUSELEAVE_MSG: u32 = 0x02A3;
const LB_GETTEXT_MSG: u32 = 0x0189;
const LB_SETTOPINDEX_MSG: u32 = 0x0197;
const LB_GETITEMRECT_MSG: u32 = 0x0198;
const ODS_SELECTED_FLAG: u32 = 0x0001;
const ODS_DISABLED_FLAG: u32 = 0x0004;
const UI_STROKE_WIDTH: i32 = 2;
const UI_RADIUS: i32 = 8;
const INPUT_RADIUS: i32 = 8;
const PROFILE_DELETE_W: i32 = 18;
const PROFILE_DELETE_GAP: i32 = 2;
const PROFILE_DELETE_RIGHT_PAD: i32 = 3;
const ICON_SMALL_WPARAM: usize = 0;
const ICON_BIG_WPARAM: usize = 1;

const ID_PROFILE_LIST: i32 = 1001;
const ID_ADD_PROFILE: i32 = 1002;
const ID_NAME_EDIT: i32 = 1003;
const ID_HOTKEY_CHANGE: i32 = 1004;
const ID_SCALE_EDIT: i32 = 1005;
const ID_SCALE_UP: i32 = 1006;
const ID_SCALE_DOWN: i32 = 1007;
const ID_SETTINGS_BUTTON: i32 = 1008;
const ID_TRAY_BUTTON: i32 = 1009;
const ID_HOTKEY_MOD_PRIMARY: i32 = 1010;
const ID_HOTKEY_MOD_SECONDARY: i32 = 1011;
const ID_HOTKEY_KEY: i32 = 1012;
const ID_SCALE_PERCENT: i32 = 1013;
const ID_PROFILE_TITLE: i32 = 1014;
const ID_HOTKEY_ICON: i32 = 1015;
const ID_SCALE_ICON: i32 = 1016;
const ID_HOTKEY_LABEL: i32 = 1017;
const ID_SCALE_LABEL: i32 = 1018;
const ID_DELETE_PROFILE: i32 = 1019;
const ID_POINTER_LABEL: i32 = 1020;
const ID_POINTER_HOTKEY_VALUE: i32 = 1021;
const ID_POINTER_HOTKEY_CHANGE: i32 = 1022;
const ID_POINTER_RANGE_LABEL: i32 = 1023;
const ID_POINTER_WIDTH_EDIT: i32 = 1024;
const ID_POINTER_X_LABEL: i32 = 1025;
const ID_POINTER_HEIGHT_EDIT: i32 = 1026;
const ID_POINTER_SCALE_LABEL: i32 = 1027;
const ID_POINTER_SCALE_EDIT: i32 = 1028;
const ID_POINTER_PERCENT: i32 = 1029;
const ID_SCREENSHOT_TITLE: i32 = 1030;
const ID_WINDOW_SCREENSHOT_LABEL: i32 = 1031;
const ID_WINDOW_SCREENSHOT_HOTKEY_VALUE: i32 = 1032;
const ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE: i32 = 1033;
const ID_WINDOW_SCREENSHOT_PATH_EDIT: i32 = 1034;
const ID_POINTER_SCREENSHOT_LABEL: i32 = 1035;
const ID_POINTER_SCREENSHOT_HOTKEY_VALUE: i32 = 1036;
const ID_POINTER_SCREENSHOT_HOTKEY_CHANGE: i32 = 1037;
const ID_POINTER_SCREENSHOT_PATH_EDIT: i32 = 1038;
const ID_WINDOW_SCREENSHOT_BROWSE: i32 = 1039;
const ID_POINTER_SCREENSHOT_BROWSE: i32 = 1040;
const ID_POINTER_RANGE_HELP: i32 = 1041;
const ID_SCREENSHOT_ICON: i32 = 1042;
const ID_WINDOW_SCREENSHOT_PATH_LABEL: i32 = 1043;
const ID_POINTER_SCREENSHOT_PATH_LABEL: i32 = 1044;
const ID_POINTER_COLOR_LABEL: i32 = 1045;
const ID_POINTER_COLOR_HOTKEY_VALUE: i32 = 1046;
const ID_POINTER_COLOR_HOTKEY_CHANGE: i32 = 1047;
const ID_POINTER_COLOR_COPY_LABEL: i32 = 1048;
const ID_POINTER_COLOR_COPY_HOTKEY_VALUE: i32 = 1049;
const ID_POINTER_COLOR_COPY_HOTKEY_CHANGE: i32 = 1050;
const ID_POINTER_CURSOR_LABEL: i32 = 1051;
const ID_POINTER_CURSOR_HOTKEY_VALUE: i32 = 1052;
const ID_POINTER_CURSOR_HOTKEY_CHANGE: i32 = 1053;
const ID_POINTER_COLOR_TOGGLE_LABEL: i32 = 1054;
const ID_POINTER_COLOR_TOGGLE: i32 = 1055;
const ID_POINTER_CURSOR_TOGGLE_LABEL: i32 = 1056;
const ID_POINTER_CURSOR_TOGGLE: i32 = 1057;
const ID_POINTER_SCALE_UP: i32 = 1058;
const ID_POINTER_SCALE_DOWN: i32 = 1059;

const ID_SETTINGS_PANEL_BG: i32 = 1098;
const ID_SETTINGS_PANEL_TITLE: i32 = 1099;
const ID_SETTINGS_CLOSE: i32 = 1100;
const ID_LANGUAGE_COMBO: i32 = 1101;
const ID_RESET_BUTTON: i32 = 1102;
const ID_LOG_BUTTON: i32 = 1103;
const ID_SETTINGS_LANGUAGE_LABEL: i32 = 1104;
const ID_LANGUAGE_MENU: i32 = 1105;
const ID_HOTKEY_PANEL_BG: i32 = 1198;
const ID_HOTKEY_PANEL_TITLE: i32 = 1199;
const ID_HOTKEY_APPLY: i32 = 1202;
const ID_HOTKEY_CANCEL: i32 = 1203;
const ID_HOTKEY_HELP: i32 = 1204;
const ID_HOTKEY_CURRENT_LABEL: i32 = 1205;
const ID_HOTKEY_CURRENT_VALUE: i32 = 1206;
const ID_HOTKEY_NEW_LABEL: i32 = 1207;
const ID_HOTKEY_NEW_VALUE: i32 = 1208;
const ID_LOG_EDIT: i32 = 1301;
const EM_SETSEL_MSG: u32 = 0x00B1;
const EM_REPLACESEL_MSG: u32 = 0x00C2;
const PROFILE_ROW_HEIGHT: i32 = 28;

#[derive(Clone, Copy)]
enum UiString {
    WindowTitle,
    Profiles,
    Change,
    Settings,
    Language,
    ResetDefaults,
    LogOutput,
    Close,
    Apply,
    Cancel,
    HotkeyChange,
    HotkeyHelp,
    CurrentHotkey,
    NewHotkey,
    ResetQuestion,
    NewProfile,
    WindowScaling,
    ShortcutSettings,
    ScreenshotStorage,
    WindowScalePercent,
    PointerScalePercent,
    PointerRangeHelp,
    Browse,
    PointerScreenshotPath,
    PointerMagnifier,
    Range,
    WindowScreenshot,
    PointerScreenshot,
    WindowZoom,
    PointerZoom,
    PointerColorCode,
    PointerColorCodeCopy,
    PointerCursor,
    ColorCodeToggle,
    CursorToggle,
    ToggleOn,
    ToggleOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HotkeyEditTarget {
    WindowScale,
    PointerMagnifier,
    WindowScreenshot,
    PointerScreenshot,
    PointerColorCode,
    PointerColorCodeCopy,
    PointerCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsUiEvent {
    HotkeysChanged,
    ProfileChanged,
    GlobalSettingsChanged,
    LogOutputRequested,
    WindowHiddenToTray,
    WindowCloseRequested,
}

#[derive(Debug)]
pub struct SettingsUiWindow {
    hwnd: isize,
}

impl SettingsUiWindow {
    pub fn show(paths: RuntimePaths) -> Result<Self, String> {
        let mut slot = state_slot()
            .lock()
            .map_err(|_| "settings UI lock poisoned".to_string())?;
        if let Some(state) = slot.as_mut() {
            unsafe {
                let hwnd = hwnd_from_raw(state.hwnd);
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetWindowPos(
                    hwnd,
                    Some(HWND_TOP),
                    0,
                    0,
                    0,
                    0,
                    SET_WINDOW_POS_FLAGS(SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_SHOWWINDOW.0),
                );
                let _ = SetForegroundWindow(hwnd);
                let _ = SetFocus(Some(hwnd));
                let _ = UpdateWindow(hwnd);
            }
            return Ok(Self { hwnd: state.hwnd });
        }

        let icon_dir = ensure_icon_files(&paths)?;
        let mut settings = load_settings_from_path(&paths.settings_file)
            .map_err(|error| format!("settings load failed: {error}"))?;
        if normalize_loaded_settings(&mut settings) {
            save_settings_to_path(&settings, &paths.settings_file)
                .map_err(|error| format!("settings migration save failed: {error}"))?;
        }
        let selected_index = selected_index_for_settings(&settings);
        let ui_language = settings.ui.language.clone();
        *slot = Some(SettingsUiState::new(
            paths,
            icon_dir,
            settings,
            selected_index,
        ));
        drop(slot);

        register_window_class()?;
        let instance = unsafe { GetModuleHandleW(None) }
            .map_err(|error| format!("GetModuleHandleW failed: {error:?}"))?;
        let title = wide_null(ui_text(&ui_language, UiString::WindowTitle));
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("DodbogiSettingsWindow"),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                720,
                760,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )
        }
        .map_err(|error| format!("CreateWindowExW settings window failed: {error:?}"))?;

        let icon_dir = {
            let mut slot = state_slot()
                .lock()
                .map_err(|_| "settings UI lock poisoned".to_string())?;
            let state = slot
                .as_mut()
                .ok_or_else(|| "settings UI state missing after window create".to_string())?;
            state.hwnd = raw_from_hwnd(hwnd);
            state.icon_dir.clone()
        };
        let app_icon_path = icon_dir.join("app.ico");
        apply_window_icon(hwnd, &app_icon_path);
        create_controls(hwnd, &icon_dir)?;
        unsafe {
            let _ = SetTimer(Some(hwnd), ID_LIVE_APPLY_TIMER, 60, None);
        }
        layout_controls(hwnd);
        apply_default_font(hwnd);
        if let Some(state) = state_slot()
            .lock()
            .map_err(|_| "settings UI lock poisoned".to_string())?
            .as_mut()
        {
            refresh_all_controls(state);
        }

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        Ok(Self {
            hwnd: raw_from_hwnd(hwnd),
        })
    }

    pub fn hwnd(&self) -> isize {
        self.hwnd
    }

    pub fn is_hotkey_capture_foreground(&self) -> bool {
        is_hotkey_capture_foreground_for(self.hwnd)
    }
}

#[derive(Debug)]
pub struct LogOutputWindow {
    hwnd: isize,
}

impl LogOutputWindow {
    pub fn show(log_file: &Path) -> Result<Self, String> {
        let mut slot = log_slot()
            .lock()
            .map_err(|_| "log output UI lock poisoned".to_string())?;
        if let Some(state) = slot.as_ref() {
            let hwnd = hwnd_from_raw(state.hwnd);
            unsafe {
                let _ = ShowWindow(hwnd, SW_RESTORE);
                let _ = ShowWindow(hwnd, SW_SHOW);
                let _ = SetForegroundWindow(hwnd);
            }
            return Ok(Self { hwnd: state.hwnd });
        }

        register_log_window_class()?;
        let instance = unsafe { GetModuleHandleW(None) }
            .map_err(|error| format!("GetModuleHandleW failed: {error:?}"))?;
        let title = wide_null("Dodbogi Log");
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("DodbogiLogWindow"),
                PCWSTR(title.as_ptr()),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                760,
                420,
                None,
                None,
                Some(HINSTANCE(instance.0)),
                None,
            )
        }
        .map_err(|error| format!("CreateWindowExW log window failed: {error:?}"))?;

        let edit = create_log_edit(hwnd)?;
        let _ = send(edit, WM_SETFONT, sketch_font_object().0 as usize, 1);
        set_text(edit, &recent_log_text(log_file));
        *slot = Some(LogOutputState {
            hwnd: raw_from_hwnd(hwnd),
            edit_hwnd: raw_from_hwnd(edit),
        });
        drop(slot);
        layout_log_window(hwnd);
        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOW);
        }
        Ok(Self {
            hwnd: raw_from_hwnd(hwnd),
        })
    }

    pub fn hide(&self) {
        unsafe {
            let _ = ShowWindow(hwnd_from_raw(self.hwnd), SW_HIDE);
        }
    }

    pub fn append_line(&self, line: &str) {
        let Ok(slot) = log_slot().lock() else {
            return;
        };
        let Some(state) = slot.as_ref() else {
            return;
        };
        if state.hwnd != self.hwnd {
            return;
        }
        append_log_text(hwnd_from_raw(state.edit_hwnd), line);
    }
}

struct LogOutputState {
    hwnd: isize,
    edit_hwnd: isize,
}

pub fn drain_settings_ui_events() -> Vec<SettingsUiEvent> {
    event_slot()
        .lock()
        .map(|mut events| events.drain(..).collect())
        .unwrap_or_default()
}

struct SettingsUiState {
    hwnd: isize,
    paths: RuntimePaths,
    icon_dir: PathBuf,
    settings: DodbogiSettings,
    selected_index: usize,
    loading: bool,
    settings_panel_visible: bool,
    hotkey_panel_visible: bool,
    language_menu_visible: bool,
    pending_hotkey: Option<String>,
    pending_hotkey_target: HotkeyEditTarget,
    pending_profile_name: Option<String>,
    hovered_profile_index: Option<usize>,
    pressed_delete_profile_index: Option<usize>,
    rename_enter_down: bool,
    rename_escape_down: bool,
}

impl SettingsUiState {
    fn new(
        paths: RuntimePaths,
        icon_dir: PathBuf,
        settings: DodbogiSettings,
        selected_index: usize,
    ) -> Self {
        Self {
            hwnd: 0,
            paths,
            icon_dir,
            settings,
            selected_index,
            loading: false,
            settings_panel_visible: false,
            hotkey_panel_visible: false,
            language_menu_visible: false,
            pending_hotkey: None,
            pending_hotkey_target: HotkeyEditTarget::WindowScale,
            pending_profile_name: None,
            hovered_profile_index: None,
            pressed_delete_profile_index: None,
            rename_enter_down: false,
            rename_escape_down: false,
        }
    }
}

static SETTINGS_UI_STATE: OnceLock<Mutex<Option<SettingsUiState>>> = OnceLock::new();
static SETTINGS_UI_EVENTS: OnceLock<Mutex<Vec<SettingsUiEvent>>> = OnceLock::new();
static SETTINGS_PANEL_PAINT_VISIBLE: AtomicBool = AtomicBool::new(false);
static HOTKEY_PANEL_PAINT_VISIBLE: AtomicBool = AtomicBool::new(false);
static PROFILE_LIST_ORIGINAL_PROC: AtomicIsize = AtomicIsize::new(0);
static LOG_OUTPUT_STATE: OnceLock<Mutex<Option<LogOutputState>>> = OnceLock::new();
static BUTTON_ICON_PATHS: OnceLock<Mutex<Vec<(i32, PathBuf)>>> = OnceLock::new();

fn state_slot() -> &'static Mutex<Option<SettingsUiState>> {
    SETTINGS_UI_STATE.get_or_init(|| Mutex::new(None))
}

fn event_slot() -> &'static Mutex<Vec<SettingsUiEvent>> {
    SETTINGS_UI_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn log_slot() -> &'static Mutex<Option<LogOutputState>> {
    LOG_OUTPUT_STATE.get_or_init(|| Mutex::new(None))
}

fn is_hotkey_capture_foreground_for(hwnd_raw: isize) -> bool {
    let Ok(slot) = state_slot().lock() else {
        return false;
    };
    let Some(state) = slot.as_ref() else {
        return false;
    };
    state.hotkey_panel_visible && unsafe { GetForegroundWindow() == hwnd_from_raw(hwnd_raw) }
}

fn button_icon_paths() -> &'static Mutex<Vec<(i32, PathBuf)>> {
    BUTTON_ICON_PATHS.get_or_init(|| Mutex::new(Vec::new()))
}

fn remember_button_icon_path(id: i32, path: &Path) {
    if let Ok(mut paths) = button_icon_paths().lock() {
        paths.retain(|(stored_id, _)| *stored_id != id);
        paths.push((id, path.to_path_buf()));
    }
}

fn button_icon_path(id: i32) -> Option<PathBuf> {
    button_icon_paths().lock().ok().and_then(|paths| {
        paths
            .iter()
            .find(|(stored_id, _)| *stored_id == id)
            .map(|(_, path)| path.clone())
    })
}

fn ui_text(lang: &str, key: UiString) -> &'static str {
    let english = lang.eq_ignore_ascii_case("en");
    match (english, key) {
        (true, UiString::WindowTitle) => "Dodbogi",
        (false, UiString::WindowTitle) => "Dodbogi",
        (true, UiString::Profiles) => "Profiles",
        (false, UiString::Profiles) => "\u{d504}\u{b85c}\u{d30c}\u{c77c}",
        (true, UiString::Change) => "Change",
        (false, UiString::Change) => "\u{bcc0}\u{acbd}",
        (true, UiString::Settings) => "Settings",
        (false, UiString::Settings) => "\u{c124}\u{c815}",
        (true, UiString::Language) => "Language",
        (false, UiString::Language) => "\u{c5b8}\u{c5b4}",
        (true, UiString::ResetDefaults) => "Reset to defaults",
        (false, UiString::ResetDefaults) => {
            "\u{ae30}\u{bcf8}\u{ac12}\u{c73c}\u{b85c} \u{cd08}\u{ae30}\u{d654}"
        }
        (true, UiString::LogOutput) => "Log output",
        (false, UiString::LogOutput) => "\u{b85c}\u{adf8} \u{cd9c}\u{b825}",
        (true, UiString::Close) => "Close",
        (false, UiString::Close) => "\u{b2eb}\u{ae30}",
        (true, UiString::Apply) => "Apply",
        (false, UiString::Apply) => "\u{c801}\u{c6a9}",
        (true, UiString::Cancel) => "Cancel",
        (false, UiString::Cancel) => "\u{cde8}\u{c18c}",
        (true, UiString::HotkeyChange) => "Change hotkey",
        (false, UiString::HotkeyChange) => "\u{b2e8}\u{cd95}\u{d0a4} \u{bcc0}\u{acbd}",
        (true, UiString::HotkeyHelp) => "Press the shortcut you want to use.",
        (false, UiString::HotkeyHelp) => {
            "\u{c0ac}\u{c6a9}\u{d560} \u{b2e8}\u{cd95}\u{d0a4}\u{b97c} \u{b204}\u{b974}\u{c138}\u{c694}."
        }
        (true, UiString::CurrentHotkey) => "Current",
        (false, UiString::CurrentHotkey) => "\u{d604}\u{c7ac}",
        (true, UiString::NewHotkey) => "New",
        (false, UiString::NewHotkey) => "\u{c0c8} \u{b2e8}\u{cd95}\u{d0a4}",
        (true, UiString::ResetQuestion) => "Reset settings to defaults?",
        (false, UiString::ResetQuestion) => {
            "\u{c124}\u{c815}\u{c744} \u{ae30}\u{bcf8}\u{ac12}\u{c73c}\u{b85c} \u{cd08}\u{ae30}\u{d654}\u{d560}\u{ae4c}\u{c694}?"
        }
        (true, UiString::NewProfile) => "New profile",
        (false, UiString::NewProfile) => "\u{c0c8} \u{d504}\u{b85c}\u{d30c}\u{c77c}",
        (true, UiString::WindowScaling) => "Window scaling",
        (false, UiString::WindowScaling) => "\u{cc3d} \u{d655}\u{b300}",
        (true, UiString::ShortcutSettings) => "Shortcut settings",
        (false, UiString::ShortcutSettings) => "\u{b2e8}\u{cd95}\u{d0a4} \u{c124}\u{c815}",
        (true, UiString::ScreenshotStorage) => "Screenshot storage",
        (false, UiString::ScreenshotStorage) => "\u{c2a4}\u{d06c}\u{b9b0}\u{c0f7} \u{c800}\u{c7a5}",
        (true, UiString::WindowScalePercent) => "Window zoom scale",
        (false, UiString::WindowScalePercent) => "\u{cc3d} \u{d655}\u{b300} \u{bc30}\u{c728}",
        (true, UiString::PointerScalePercent) => "Pointer zoom scale",
        (false, UiString::PointerScalePercent) => {
            "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{bc30}\u{c728}"
        }
        (true, UiString::PointerRangeHelp) => "Source pixel range to magnify",
        (false, UiString::PointerRangeHelp) => {
            "\u{d655}\u{b300}\u{d560} \u{c6d0}\u{bcf8} \u{d53d}\u{c140} \u{bc94}\u{c704}"
        }
        (true, UiString::Browse) => "Browse",
        (false, UiString::Browse) => "\u{cc3e}\u{c544}\u{bcf4}\u{ae30}",
        (true, UiString::PointerScreenshotPath) => "Screenshot path",
        (false, UiString::PointerScreenshotPath) => {
            "\u{c2a4}\u{d06c}\u{b9b0}\u{c0f7} \u{c800}\u{c7a5} \u{acbd}\u{b85c}"
        }
        (true, UiString::PointerMagnifier) => "Pointer zoom",
        (false, UiString::PointerMagnifier) => "\u{bd80}\u{bd84} \u{d655}\u{b300}",
        (true, UiString::Range) => "Pointer zoom area",
        (false, UiString::Range) => {
            "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{c601}\u{c5ed} \u{d06c}\u{ae30}"
        }
        (true, UiString::WindowScreenshot) => "Window zoom screenshot",
        (false, UiString::WindowScreenshot) => {
            "\u{cc3d} \u{d655}\u{b300} \u{c2a4}\u{d06c}\u{b9b0}\u{c0f7}"
        }
        (true, UiString::PointerScreenshot) => "Pointer zoom screenshot",
        (false, UiString::PointerScreenshot) => {
            "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{c2a4}\u{d06c}\u{b9b0}\u{c0f7}"
        }
        (true, UiString::WindowZoom) => "Window zoom",
        (false, UiString::WindowZoom) => "\u{cc3d} \u{d655}\u{b300}",
        (true, UiString::PointerZoom) => "Pointer zoom",
        (false, UiString::PointerZoom) => "\u{bd80}\u{bd84} \u{d655}\u{b300}",
        (true, UiString::PointerColorCode) => "Pointer color code",
        (false, UiString::PointerColorCode) => {
            "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{c0c9}\u{c0c1} \u{cf54}\u{b4dc} \u{bcf4}\u{ae30}"
        }
        (true, UiString::PointerColorCodeCopy) => "Copy pointer color code",
        (false, UiString::PointerColorCodeCopy) => {
            "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{c0c9}\u{c0c1} \u{cf54}\u{b4dc} \u{bcf5}\u{c0ac}"
        }
        (true, UiString::PointerCursor) => "Pointer cursor",
        (false, UiString::PointerCursor) => {
            "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{b9c8}\u{c6b0}\u{c2a4} \u{cee4}\u{c11c} \u{bcf4}\u{ae30}"
        }
        (true, UiString::ColorCodeToggle) => "Show color code",
        (false, UiString::ColorCodeToggle) => "\u{c0c9}\u{c0c1} \u{cf54}\u{b4dc} \u{bcf4}\u{ae30}",
        (true, UiString::CursorToggle) => "Show mouse cursor",
        (false, UiString::CursorToggle) => {
            "\u{b9c8}\u{c6b0}\u{c2a4} \u{cee4}\u{c11c} \u{bcf4}\u{ae30}"
        }
        (true, UiString::ToggleOn) => "ON",
        (false, UiString::ToggleOn) => "\u{cf1c}\u{c9d0}",
        (true, UiString::ToggleOff) => "OFF",
        (false, UiString::ToggleOff) => "\u{aebc}\u{c9d0}",
    }
}

pub fn ensure_app_icon_file(paths: &RuntimePaths) -> Result<PathBuf, String> {
    let icon_dir = paths.cache_dir.join("ui-icons");
    fs::create_dir_all(&icon_dir).map_err(|error| format!("icon cache create failed: {error}"))?;
    let path = icon_dir.join("app.ico");
    fs::write(&path, APP_ICON_ICO).map_err(|error| {
        format!(
            "app icon cache write failed for {}: {error}",
            path.display()
        )
    })?;
    Ok(path)
}

fn ensure_icon_files(paths: &RuntimePaths) -> Result<PathBuf, String> {
    let icon_dir = paths.cache_dir.join("ui-icons");
    fs::create_dir_all(&icon_dir).map_err(|error| format!("icon cache create failed: {error}"))?;
    for (name, bytes) in [
        ("hotkey.bmp", HOTKEY_ICON_BMP),
        ("scale.bmp", SCALE_ICON_BMP),
        ("save.bmp", SAVE_ICON_BMP),
        ("window-zoom.bmp", WINDOW_ZOOM_ICON_BMP),
        ("pointer-zoom.bmp", POINTER_ZOOM_ICON_BMP),
        ("settings.bmp", SETTINGS_ICON_BMP),
        ("minimize-to-tray.bmp", TRAY_ICON_BMP),
    ] {
        let path = icon_dir.join(name);
        fs::write(&path, bytes)
            .map_err(|error| format!("icon cache write failed for {}: {error}", path.display()))?;
    }
    ensure_app_icon_file(paths)?;
    Ok(icon_dir)
}

fn push_event(event: SettingsUiEvent) {
    if let Ok(mut events) = event_slot().lock() {
        events.push(event);
    }
}

fn register_window_class() -> Result<(), String> {
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCCREATE => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            WM_CREATE => LRESULT(0),
            WM_GETMINMAXINFO => {
                apply_min_track_size(lparam);
                LRESULT(0)
            }
            WM_SIZE => {
                layout_controls(hwnd);
                unsafe {
                    let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE | RDW_ALLCHILDREN);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                erase_background(hwnd, HDC(wparam.0 as *mut _));
                LRESULT(1)
            }
            WM_PAINT => {
                paint_settings_window(hwnd);
                LRESULT(0)
            }
            WM_CTLCOLORSTATIC => unsafe {
                let hdc = HDC(wparam.0 as *mut _);
                let _ = SetBkMode(hdc, TRANSPARENT);
                let child_id = GetDlgCtrlID(HWND(lparam.0 as *mut _));
                let brush = if child_id == ID_SETTINGS_PANEL_BG || child_id == ID_HOTKEY_PANEL_BG {
                    GetStockObject(WHITE_BRUSH)
                } else {
                    // Transparent STATIC controls do not erase their previous glyphs reliably on
                    // SetWindowTextW.  Returning a real white brush keeps the hotkey "current/new"
                    // values from drawing new text over stale text.
                    GetStockObject(WHITE_BRUSH)
                };
                LRESULT(brush.0 as isize)
            },
            WM_DRAWITEM_MSG => {
                draw_owner_draw_item(lparam);
                LRESULT(1)
            }
            WM_MEASUREITEM_MSG => {
                measure_owner_draw_item(lparam);
                LRESULT(1)
            }
            WM_LBUTTONDOWN | WM_NCLBUTTONDOWN => {
                commit_profile_name_edit_for_external_click(hwnd);
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_PARENTNOTIFY => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            WM_COMMAND => {
                handle_command(hwnd, wparam);
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == ID_LIVE_APPLY_TIMER {
                    let _ = poll_hotkey_capture(hwnd);
                    poll_rename_edit_keys(hwnd);
                    apply_live_edits_from_controls(hwnd);
                    return LRESULT(0);
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if handle_keydown(hwnd, wparam.0 as u32) {
                    return LRESULT(0);
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            WM_CLOSE => {
                push_event(SettingsUiEvent::WindowCloseRequested);
                LRESULT(0)
            }
            WM_DESTROY => {
                unsafe {
                    let _ = KillTimer(Some(hwnd), ID_LIVE_APPLY_TIMER);
                }
                SETTINGS_PANEL_PAINT_VISIBLE.store(false, Ordering::Relaxed);
                HOTKEY_PANEL_PAINT_VISIBLE.store(false, Ordering::Relaxed);
                if let Ok(mut slot) = state_slot().lock() {
                    *slot = None;
                }
                LRESULT(0)
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    let instance = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("GetModuleHandleW failed: {error:?}"))?;
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.ok();
    let wc = WNDCLASSW {
        style: CS_DBLCLKS,
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(instance.0),
        hCursor: cursor.unwrap_or_default(),
        hbrBackground: HBRUSH(unsafe { GetStockObject(WHITE_BRUSH) }.0),
        lpszClassName: w!("DodbogiSettingsWindow"),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassW(&wc) };
    if atom == 0 {
        let err = unsafe { GetLastError() };
        if err.0 != 1410 {
            return Err(format!("RegisterClassW settings window failed: {err:?}"));
        }
    }
    Ok(())
}

fn register_log_window_class() -> Result<(), String> {
    unsafe extern "system" fn wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_CREATE => LRESULT(0),
            WM_SIZE => {
                layout_log_window(hwnd);
                unsafe {
                    let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE | RDW_ERASE);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => {
                erase_background(hwnd, HDC(wparam.0 as *mut _));
                LRESULT(1)
            }
            WM_CLOSE => {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                if let Ok(mut slot) = log_slot().lock() {
                    *slot = None;
                }
                unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
    }

    let instance = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("GetModuleHandleW failed: {error:?}"))?;
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.ok();
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(instance.0),
        hCursor: cursor.unwrap_or_default(),
        hbrBackground: HBRUSH(unsafe { GetStockObject(WHITE_BRUSH) }.0),
        lpszClassName: w!("DodbogiLogWindow"),
        ..Default::default()
    };
    let atom = unsafe { RegisterClassW(&wc) };
    if atom == 0 {
        let err = unsafe { GetLastError() };
        if err.0 != 1410 {
            return Err(format!("RegisterClassW log window failed: {err:?}"));
        }
    }
    Ok(())
}

fn create_log_edit(hwnd: HWND) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("EDIT"),
        "",
        style(
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER | WS_VSCROLL,
            &[ES_MULTILINE, ES_AUTOVSCROLL, ES_READONLY],
        ),
        12,
        12,
        720,
        360,
        ID_LOG_EDIT,
    )
}

fn layout_log_window(hwnd: HWND) {
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let margin = 12;
    move_child(
        hwnd,
        ID_LOG_EDIT,
        margin,
        margin,
        (client.right - client.left - margin * 2).max(120),
        (client.bottom - client.top - margin * 2).max(80),
    );
}

fn recent_log_text(path: &Path) -> String {
    let Ok(raw) = fs::read_to_string(path) else {
        return String::new();
    };
    let mut lines = raw.lines().rev().take(300).collect::<Vec<_>>();
    lines.reverse();
    let mut text = lines.join("\r\n");
    if !text.is_empty() {
        text.push_str("\r\n");
    }
    text
}

fn append_log_text(edit: HWND, line: &str) {
    if edit.0.is_null() {
        return;
    }
    let mut text = line.replace("\r\n", "\n").replace('\r', "\n");
    text.push_str("\r\n");
    let wide = wide_null(&text);
    let _ = send(edit, EM_SETSEL_MSG, usize::MAX, -1);
    let _ = send(edit, EM_REPLACESEL_MSG, 0, wide.as_ptr() as isize);
}

fn apply_min_track_size(lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let info = lparam.0 as *mut MINMAXINFO;
    unsafe {
        (*info).ptMinTrackSize = POINT {
            x: MIN_TRACK_WIDTH,
            y: MIN_TRACK_HEIGHT,
        };
    }
}

fn erase_background(hwnd: HWND, hdc: HDC) {
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    fill_rect_color(hdc, &client, rgb(255, 254, 249));
}

#[derive(Clone, Copy)]
struct UiLayout {
    margin: i32,
    sidebar_x: i32,
    sidebar_y: i32,
    sidebar_w: i32,
    sidebar_h: i32,
    content_panel: RECT,
    settings_panel: RECT,
    hotkey_panel: RECT,
    window_group: RECT,
    pointer_row: RECT,
    screenshot_row: RECT,
}

fn layout_sidebar_bottom(sidebar_y: i32, sidebar_h: i32) -> i32 {
    sidebar_y + sidebar_h
}

fn current_layout(hwnd: HWND) -> UiLayout {
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let client_w = (client.right - client.left).max(1);
    let client_h = (client.bottom - client.top).max(1);
    let margin = 24.min((client_w / 24).max(12));
    let sidebar_w = (client_w / 5).clamp(164, 196).min((client_w / 3).max(150));
    let sidebar_x = margin;
    let sidebar_y = 84;
    let profile_button_stack_h = 18;
    let sidebar_h = (client_h - sidebar_y - profile_button_stack_h - margin * 2).max(180);
    let content_x = sidebar_x + sidebar_w + margin;
    let content_y = sidebar_y;
    let content_w = (client_w - content_x - margin).max(330);
    let content_h = sidebar_h.max(300);
    let mut content_panel = RECT {
        left: content_x,
        top: content_y,
        right: content_x + content_w,
        bottom: content_y + content_h,
    };
    let group_pad = 24.min((content_w / 18).max(20));
    let group_left = content_x + group_pad;
    let group_right = content_panel.right - group_pad;
    content_panel.bottom = (layout_sidebar_bottom(sidebar_y, sidebar_h)).min(client_h - margin);
    let row_gap = 16;
    let group_top = content_y + 24;
    let window_group_h = 220;
    let pointer_group_h = 104;
    let screenshot_group_h = 190;
    let window_group = RECT {
        left: group_left,
        top: group_top,
        right: group_right,
        bottom: group_top + window_group_h,
    };
    let pointer_top = window_group.bottom + row_gap;
    let pointer_row = RECT {
        left: group_left,
        top: pointer_top,
        right: group_right,
        bottom: pointer_top + pointer_group_h,
    };
    let screenshot_top = pointer_row.bottom + row_gap;
    let screenshot_max_bottom = content_panel.bottom - 10;
    let screenshot_row = RECT {
        left: group_left,
        top: screenshot_top,
        right: group_right,
        bottom: (screenshot_top + screenshot_group_h).min(screenshot_max_bottom),
    };
    let modal_w = (content_w - 48).max(340).clamp(340, 440);
    let settings_w = modal_w;
    let settings_h = 302;
    let modal_top = (group_top - 18).clamp(content_y + 26, content_panel.bottom - settings_h - 18);
    let settings_left = content_x + ((content_w - settings_w) / 2).max(24);
    let settings_panel = RECT {
        left: settings_left,
        top: modal_top,
        right: settings_left + settings_w,
        bottom: modal_top + settings_h,
    };
    let hotkey_w = (content_w - 56).max(340).clamp(340, 420);
    let hotkey_h = 226;
    let hotkey_left = content_x + ((content_w - hotkey_w) / 2).max(24);
    let hotkey_panel = RECT {
        left: hotkey_left,
        top: modal_top,
        right: hotkey_left + hotkey_w,
        bottom: modal_top + hotkey_h,
    };
    UiLayout {
        margin,
        sidebar_x,
        sidebar_y,
        sidebar_w,
        sidebar_h,
        content_panel,
        settings_panel,
        hotkey_panel,
        window_group,
        pointer_row,
        screenshot_row,
    }
}

fn layout_controls(hwnd: HWND) {
    let layout = current_layout(hwnd);
    let toolbar_y = 16;
    let toolbar_w = 38;
    let toolbar_h = 36;
    let toolbar_gap = 6;
    let tray_x = layout.content_panel.right - toolbar_w;
    let settings_x = tray_x - toolbar_gap - toolbar_w;
    move_child(
        hwnd,
        ID_PROFILE_TITLE,
        layout.sidebar_x,
        layout.margin + 34,
        140,
        24,
    );
    let (profile_count, selected_index, modal_active) = sidebar_layout_state(hwnd);
    let list_rect = profile_list_rect(&layout, profile_count);
    move_child(
        hwnd,
        ID_PROFILE_LIST,
        list_rect.left,
        list_rect.top,
        list_rect.right - list_rect.left,
        list_rect.bottom - list_rect.top,
    );
    layout_profile_buttons(hwnd, &layout, profile_count, selected_index, modal_active);
    move_child(
        hwnd,
        ID_SETTINGS_BUTTON,
        settings_x,
        toolbar_y,
        toolbar_w,
        toolbar_h,
    );
    move_child(
        hwnd,
        ID_TRAY_BUTTON,
        tray_x,
        toolbar_y,
        toolbar_w,
        toolbar_h,
    );

    let shortcut = layout.window_group;
    let shortcut_icon_x = shortcut.left + 22;
    let shortcut_title_y = shortcut.top + 16;
    let shortcut_label_x = shortcut.left + 64;
    let shortcut_value_x = shortcut.left + 270;
    let shortcut_label_w = (shortcut_value_x - shortcut_label_x - 8).max(120);
    let shortcut_value_w = (shortcut.right - shortcut_value_x - 24).clamp(118, 170);
    let shortcut_row_h = 20;
    let shortcut_row_gap = 1;
    // Title icon/text occupies the top band; keep only a compact gap below the
    // divider so the shortcut rows do not look vertically detached.
    let mut shortcut_y = shortcut.top + 64;

    move_child(
        hwnd,
        ID_HOTKEY_ICON,
        shortcut_icon_x,
        shortcut_title_y,
        32,
        32,
    );
    move_child(
        hwnd,
        ID_HOTKEY_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_HOTKEY_MOD_PRIMARY,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_HOTKEY_MOD_SECONDARY,
        shortcut_value_x,
        shortcut_y,
        1,
        1,
    );
    move_child(hwnd, ID_HOTKEY_KEY, shortcut_value_x, shortcut_y, 1, 1);
    move_child(
        hwnd,
        ID_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    shortcut_y += shortcut_row_h + shortcut_row_gap;
    move_child(
        hwnd,
        ID_POINTER_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_HOTKEY_VALUE,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    shortcut_y += shortcut_row_h + shortcut_row_gap;
    move_child(
        hwnd,
        ID_WINDOW_SCREENSHOT_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_WINDOW_SCREENSHOT_HOTKEY_VALUE,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    shortcut_y += shortcut_row_h + shortcut_row_gap;
    move_child(
        hwnd,
        ID_POINTER_SCREENSHOT_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_SCREENSHOT_HOTKEY_VALUE,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_SCREENSHOT_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    shortcut_y += shortcut_row_h + shortcut_row_gap;
    move_child(
        hwnd,
        ID_POINTER_COLOR_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_COLOR_HOTKEY_VALUE,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_COLOR_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    shortcut_y += shortcut_row_h + shortcut_row_gap;
    move_child(
        hwnd,
        ID_POINTER_COLOR_COPY_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_COLOR_COPY_HOTKEY_VALUE,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_COLOR_COPY_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    shortcut_y += shortcut_row_h + shortcut_row_gap;
    move_child(
        hwnd,
        ID_POINTER_CURSOR_LABEL,
        shortcut_label_x,
        shortcut_y,
        shortcut_label_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_CURSOR_HOTKEY_VALUE,
        shortcut_value_x,
        shortcut_y,
        shortcut_value_w,
        shortcut_row_h,
    );
    move_child(
        hwnd,
        ID_POINTER_CURSOR_HOTKEY_CHANGE,
        shortcut.right - 92,
        shortcut_y - 4,
        1,
        1,
    );

    let window_zoom = layout.pointer_row;
    let window_label_x = window_zoom.left + 64;
    let window_value_x = window_zoom.left + 270;
    let window_label_w = (window_value_x - window_label_x - 10).max(120);
    let window_row1_y = window_zoom.top + 64;
    let zoom_edit_w = 48;
    let edit_h = 24;
    let scale_button_w = 26;
    let scale_button_half_h = 13;
    let scale_buttons_x = window_value_x + zoom_edit_w + 38;
    move_child(
        hwnd,
        ID_SCALE_ICON,
        window_zoom.left + 20,
        window_zoom.top + 17,
        32,
        32,
    );
    move_child(
        hwnd,
        ID_SCALE_LABEL,
        window_label_x,
        window_row1_y,
        window_label_w,
        24,
    );
    move_edit_field(
        hwnd,
        ID_SCALE_EDIT,
        window_value_x,
        window_row1_y - 3,
        zoom_edit_w,
        edit_h,
    );
    move_child(
        hwnd,
        ID_SCALE_PERCENT,
        window_value_x + zoom_edit_w + 8,
        window_row1_y,
        24,
        24,
    );
    move_child(
        hwnd,
        ID_SCALE_UP,
        scale_buttons_x,
        window_row1_y - 3,
        scale_button_w,
        scale_button_half_h,
    );
    move_child(
        hwnd,
        ID_SCALE_DOWN,
        scale_buttons_x,
        window_row1_y - 3 + scale_button_half_h,
        scale_button_w,
        scale_button_half_h,
    );
    show_child(hwnd, ID_SCALE_UP, true);
    show_child(hwnd, ID_SCALE_DOWN, true);
    // Screenshot path is a global setting, shown only in the settings panel.
    move_child(hwnd, ID_WINDOW_SCREENSHOT_PATH_LABEL, 1, 1, 1, 1);
    move_child(hwnd, ID_WINDOW_SCREENSHOT_PATH_EDIT, 1, 1, 1, 1);
    move_child(hwnd, ID_WINDOW_SCREENSHOT_BROWSE, 1, 1, 1, 1);
    if !is_settings_panel_visible_for(hwnd) {
        show_child(hwnd, ID_WINDOW_SCREENSHOT_PATH_LABEL, false);
        show_child(hwnd, ID_WINDOW_SCREENSHOT_PATH_EDIT, false);
        show_child(hwnd, ID_WINDOW_SCREENSHOT_BROWSE, false);
    }

    let pointer = layout.screenshot_row;
    let pointer_label_x = pointer.left + 64;
    let pointer_value_x = pointer.left + 270;
    let pointer_label_w = (pointer_value_x - pointer_label_x - 10).max(120);
    let pointer_row1_y = pointer.top + 64;
    let pointer_row2_y = pointer_row1_y + 28;
    let pointer_row3_y = pointer_row2_y + 28;
    let pointer_row4_y = pointer_row3_y + 28;
    let pointer_scale_buttons_x = pointer_value_x + zoom_edit_w + 38;
    move_child(
        hwnd,
        ID_SCREENSHOT_ICON,
        pointer.left + 20,
        pointer.top + 17,
        32,
        32,
    );
    move_child(
        hwnd,
        ID_SCREENSHOT_TITLE,
        pointer.left + 60,
        pointer.top + 18,
        1,
        1,
    );
    move_child(
        hwnd,
        ID_POINTER_SCALE_LABEL,
        pointer_label_x,
        pointer_row1_y,
        pointer_label_w,
        24,
    );
    move_edit_field(
        hwnd,
        ID_POINTER_SCALE_EDIT,
        pointer_value_x,
        pointer_row1_y - 3,
        zoom_edit_w,
        edit_h,
    );
    move_child(
        hwnd,
        ID_POINTER_PERCENT,
        pointer_value_x + zoom_edit_w + 8,
        pointer_row1_y,
        24,
        24,
    );
    move_child(
        hwnd,
        ID_POINTER_SCALE_UP,
        pointer_scale_buttons_x,
        pointer_row1_y - 3,
        scale_button_w,
        scale_button_half_h,
    );
    move_child(
        hwnd,
        ID_POINTER_SCALE_DOWN,
        pointer_scale_buttons_x,
        pointer_row1_y - 3 + scale_button_half_h,
        scale_button_w,
        scale_button_half_h,
    );
    show_child(hwnd, ID_POINTER_SCALE_UP, true);
    show_child(hwnd, ID_POINTER_SCALE_DOWN, true);
    move_child(
        hwnd,
        ID_POINTER_RANGE_LABEL,
        pointer_label_x,
        pointer_row2_y,
        pointer_label_w,
        24,
    );
    move_edit_field(
        hwnd,
        ID_POINTER_WIDTH_EDIT,
        pointer_value_x,
        pointer_row2_y - 3,
        48,
        edit_h,
    );
    move_child(
        hwnd,
        ID_POINTER_X_LABEL,
        pointer_value_x + 56,
        pointer_row2_y,
        18,
        24,
    );
    move_edit_field(
        hwnd,
        ID_POINTER_HEIGHT_EDIT,
        pointer_value_x + 78,
        pointer_row2_y - 3,
        48,
        edit_h,
    );
    move_child(
        hwnd,
        ID_POINTER_RANGE_HELP,
        pointer_value_x + 154,
        pointer_row2_y,
        1,
        1,
    );
    move_child(
        hwnd,
        ID_POINTER_COLOR_TOGGLE_LABEL,
        pointer_label_x,
        pointer_row3_y,
        pointer_label_w,
        24,
    );
    move_child(
        hwnd,
        ID_POINTER_COLOR_TOGGLE,
        pointer_value_x - 2,
        pointer_row3_y - 3,
        56,
        edit_h,
    );
    move_child(
        hwnd,
        ID_POINTER_CURSOR_TOGGLE_LABEL,
        pointer_label_x,
        pointer_row4_y,
        pointer_label_w,
        24,
    );
    move_child(
        hwnd,
        ID_POINTER_CURSOR_TOGGLE,
        pointer_value_x - 2,
        pointer_row4_y - 3,
        56,
        edit_h,
    );
    move_child(hwnd, ID_POINTER_SCREENSHOT_PATH_LABEL, 1, 1, 1, 1);
    move_child(hwnd, ID_POINTER_SCREENSHOT_PATH_EDIT, 1, 1, 1, 1);
    move_child(hwnd, ID_POINTER_SCREENSHOT_BROWSE, 1, 1, 1, 1);
    show_child(hwnd, ID_POINTER_SCREENSHOT_PATH_LABEL, false);
    show_child(hwnd, ID_POINTER_SCREENSHOT_PATH_EDIT, false);
    show_child(hwnd, ID_POINTER_SCREENSHOT_BROWSE, false);

    hide_legacy_action_buttons(hwnd);

    let sp = layout.settings_panel;
    move_child(
        hwnd,
        ID_SETTINGS_PANEL_BG,
        sp.left + UI_STROKE_WIDTH,
        sp.top + UI_STROKE_WIDTH,
        sp.right - sp.left - UI_STROKE_WIDTH * 2,
        sp.bottom - sp.top - UI_STROKE_WIDTH * 2,
    );
    move_child(
        hwnd,
        ID_SETTINGS_PANEL_TITLE,
        sp.left + 26,
        sp.top + 22,
        sp.right - sp.left - 52,
        24,
    );
    let settings_label_x = sp.left + 28;
    let settings_value_x = sp.left + 126;
    move_child(
        hwnd,
        ID_SETTINGS_LANGUAGE_LABEL,
        settings_label_x,
        sp.top + 66,
        86,
        24,
    );
    move_child(
        hwnd,
        ID_LANGUAGE_COMBO,
        settings_value_x,
        sp.top + 58,
        196,
        32,
    );
    move_child(
        hwnd,
        ID_LANGUAGE_MENU,
        settings_value_x,
        sp.top + 92,
        196,
        58,
    );
    move_child(
        hwnd,
        ID_WINDOW_SCREENSHOT_PATH_LABEL,
        settings_label_x,
        sp.top + 112,
        86,
        24,
    );
    let path_row_y = sp.top + 112;
    move_child(
        hwnd,
        ID_WINDOW_SCREENSHOT_PATH_EDIT,
        settings_value_x,
        path_row_y,
        (sp.right - settings_value_x - 28).max(220),
        24,
    );
    move_child(hwnd, ID_WINDOW_SCREENSHOT_BROWSE, 1, 1, 1, 1);
    move_child(hwnd, ID_RESET_BUTTON, sp.left + 28, sp.top + 166, 186, 34);
    move_child(hwnd, ID_LOG_BUTTON, sp.left + 28, sp.top + 210, 112, 30);
    move_child(
        hwnd,
        ID_SETTINGS_CLOSE,
        sp.right - 106,
        sp.bottom - 46,
        72,
        32,
    );

    let hp = layout.hotkey_panel;
    move_child(
        hwnd,
        ID_HOTKEY_PANEL_BG,
        hp.left + UI_STROKE_WIDTH,
        hp.top + UI_STROKE_WIDTH,
        hp.right - hp.left - UI_STROKE_WIDTH * 2,
        hp.bottom - hp.top - UI_STROKE_WIDTH * 2,
    );
    move_child(
        hwnd,
        ID_HOTKEY_PANEL_TITLE,
        hp.left + 26,
        hp.top + 22,
        hp.right - hp.left - 52,
        24,
    );
    move_child(hwnd, ID_HOTKEY_HELP, hp.left + 28, hp.top + 54, 360, 24);
    move_child(
        hwnd,
        ID_HOTKEY_CURRENT_LABEL,
        hp.left + 28,
        hp.top + 88,
        92,
        24,
    );
    move_child(
        hwnd,
        ID_HOTKEY_CURRENT_VALUE,
        hp.left + 132,
        hp.top + 88,
        hp.right - hp.left - 164,
        24,
    );
    move_child(
        hwnd,
        ID_HOTKEY_NEW_LABEL,
        hp.left + 28,
        hp.top + 122,
        92,
        24,
    );
    move_child(
        hwnd,
        ID_HOTKEY_NEW_VALUE,
        hp.left + 132,
        hp.top + 122,
        hp.right - hp.left - 164,
        24,
    );
    move_child(
        hwnd,
        ID_HOTKEY_APPLY,
        hp.right - 200,
        hp.bottom - 48,
        72,
        32,
    );
    move_child(
        hwnd,
        ID_HOTKEY_CANCEL,
        hp.right - 116,
        hp.bottom - 48,
        72,
        32,
    );

    invalidate(hwnd);
}

fn move_child(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) {
    let child = get(parent, id);
    if child.0.is_null() {
        return;
    }
    let flags = SET_WINDOW_POS_FLAGS(SWP_NOZORDER.0 | SWP_NOACTIVATE.0);
    let _ = unsafe { SetWindowPos(child, None, x, y, w, h, flags) };
}

fn move_edit_field(parent: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) {
    let pad_x = 8;
    // Win32 single-line EDIT controls paint text from their own font metrics
    // instead of vertically centering in the outer sketch frame.  Keeping the
    // inner EDIT close to the font height and using symmetrical top/bottom
    // padding makes the numeric text sit visually in the middle of the frame.
    let pad_top = 4;
    let pad_bottom = 4;
    move_child(
        parent,
        id,
        x + pad_x,
        y + pad_top,
        (w - pad_x * 2).max(12),
        (h - pad_top - pad_bottom).max(12),
    );
}

fn sidebar_layout_state(hwnd: HWND) -> (usize, usize, bool) {
    let Ok(slot) = state_slot().try_lock() else {
        return (1, 0, false);
    };
    let Some(state) = slot.as_ref() else {
        return (1, 0, false);
    };
    if state.hwnd != raw_from_hwnd(hwnd) {
        return (1, 0, false);
    }
    (
        profiles(&state.settings).len(),
        state.selected_index,
        state.settings_panel_visible || state.hotkey_panel_visible,
    )
}

fn layout_profile_buttons_for_state(state: &SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let layout = current_layout(hwnd);
    let profile_count = profiles(&state.settings).len();
    let list_rect = profile_list_rect(&layout, profile_count);
    move_child(
        hwnd,
        ID_PROFILE_LIST,
        list_rect.left,
        list_rect.top,
        list_rect.right - list_rect.left,
        list_rect.bottom - list_rect.top,
    );
    layout_profile_buttons(
        hwnd,
        &layout,
        profile_count,
        state.selected_index,
        state.settings_panel_visible || state.hotkey_panel_visible,
    );
}

fn layout_profile_buttons(
    hwnd: HWND,
    layout: &UiLayout,
    profile_count: usize,
    _selected_index: usize,
    modal_active: bool,
) {
    let list_rect = profile_list_rect(layout, profile_count);
    let add_y = (list_rect.bottom + 2)
        .min(layout.sidebar_y + layout.sidebar_h - 38)
        .max(layout.sidebar_y + 8);
    move_child(
        hwnd,
        ID_ADD_PROFILE,
        layout.sidebar_x + 8,
        add_y,
        layout.sidebar_w - 16,
        28,
    );
    show_child(hwnd, ID_ADD_PROFILE, true);
    show_child(hwnd, ID_DELETE_PROFILE, false);
    set_child_enabled(hwnd, ID_ADD_PROFILE, !modal_active);
    set_child_enabled(hwnd, ID_DELETE_PROFILE, false);
    raise_child(hwnd, ID_ADD_PROFILE);
    invalidate_sidebar(hwnd, layout);
}

fn profile_list_rect(layout: &UiLayout, profile_count: usize) -> RECT {
    let rows_h = (profile_count.max(1) as i32 * PROFILE_ROW_HEIGHT)
        .min((layout.sidebar_h - 48).max(PROFILE_ROW_HEIGHT + 4));
    RECT {
        left: layout.sidebar_x + 8,
        top: layout.sidebar_y + 6,
        right: layout.sidebar_x + layout.sidebar_w - 8,
        bottom: layout.sidebar_y + 6 + rows_h,
    }
}

fn profile_item_rect_in_parent(hwnd: HWND, profile_index: usize) -> Option<RECT> {
    let list = get(hwnd, ID_PROFILE_LIST);
    if list.0.is_null() {
        return None;
    }
    let mut item_rect = RECT::default();
    if send(
        list,
        LB_GETITEMRECT_MSG,
        profile_index,
        &mut item_rect as *mut RECT as isize,
    ) < 0
    {
        return None;
    }
    let mut parent_origin = POINT { x: 0, y: 0 };
    let mut list_origin = POINT { x: 0, y: 0 };
    let parent_ok = unsafe { ClientToScreen(hwnd, &mut parent_origin).as_bool() };
    let list_ok = unsafe { ClientToScreen(list, &mut list_origin).as_bool() };
    if !parent_ok || !list_ok {
        return None;
    }
    let dx = list_origin.x - parent_origin.x;
    let dy = list_origin.y - parent_origin.y;
    Some(RECT {
        left: dx + item_rect.left,
        top: dy + item_rect.top,
        right: dx + item_rect.right,
        bottom: dy + item_rect.bottom,
    })
}

fn child_frame_rect(parent: HWND, id: i32, pad_x: i32, pad_y: i32) -> Option<RECT> {
    let child = get(parent, id);
    if child.0.is_null() {
        return None;
    }
    let mut child_rect = RECT::default();
    if unsafe { GetWindowRect(child, &mut child_rect) }.is_err() {
        return None;
    }
    let mut origin = POINT { x: 0, y: 0 };
    if !unsafe { ClientToScreen(parent, &mut origin).as_bool() } {
        return None;
    }
    Some(RECT {
        left: child_rect.left - origin.x - pad_x,
        top: child_rect.top - origin.y - pad_y,
        right: child_rect.right - origin.x + pad_x,
        bottom: child_rect.bottom - origin.y + pad_y,
    })
}

fn fallback_profile_item_rect(
    layout: &UiLayout,
    profile_count: usize,
    profile_index: usize,
) -> RECT {
    let list_rect = profile_list_rect(layout, profile_count);
    let top = list_rect.top + (profile_index as i32 * PROFILE_ROW_HEIGHT);
    RECT {
        left: list_rect.left,
        top,
        right: list_rect.right,
        bottom: top + PROFILE_ROW_HEIGHT,
    }
}

fn profile_delete_icon_rect(row_frame: RECT) -> RECT {
    let right = row_frame.right - PROFILE_DELETE_RIGHT_PAD;
    let left = right - PROFILE_DELETE_W;
    RECT {
        left,
        top: row_frame.top + 2,
        right,
        bottom: row_frame.bottom - 2,
    }
}

fn profile_delete_hit_rect(row_frame: RECT) -> RECT {
    RECT {
        left: row_frame.right
            - (PROFILE_DELETE_W + PROFILE_DELETE_GAP + PROFILE_DELETE_RIGHT_PAD + 8),
        top: row_frame.top,
        right: row_frame.right,
        bottom: row_frame.bottom,
    }
}

fn invalidate_sidebar(hwnd: HWND, layout: &UiLayout) {
    let rect = RECT {
        left: layout.sidebar_x - 6,
        top: layout.margin + 28,
        right: layout.sidebar_x + layout.sidebar_w + 8,
        bottom: layout.sidebar_y + layout.sidebar_h + 8,
    };
    unsafe {
        let _ = InvalidateRect(Some(hwnd), Some(&rect), false);
    }
}

fn current_ui_language(hwnd: HWND) -> String {
    let Ok(slot) = state_slot().try_lock() else {
        return "ko".to_string();
    };
    let Some(state) = slot.as_ref() else {
        return "ko".to_string();
    };
    if state.hwnd != raw_from_hwnd(hwnd) {
        return "ko".to_string();
    }
    state.settings.ui.language.clone()
}

fn is_settings_panel_visible_for(hwnd: HWND) -> bool {
    let Ok(slot) = state_slot().try_lock() else {
        return false;
    };
    let Some(state) = slot.as_ref() else {
        return false;
    };
    state.hwnd == raw_from_hwnd(hwnd) && state.settings_panel_visible
}

fn draw_group_title(hdc: HDC, rect: &RECT, title: &str) {
    unsafe {
        let mut text = wide_null(title);
        let text_len = text.len().saturating_sub(1);
        let mut title_rect = RECT {
            left: rect.left + 64,
            top: rect.top + 17,
            right: rect.right - 24,
            bottom: rect.top + 49,
        };
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, rgb(0, 0, 0));
        let old_font = SelectObject(hdc, sketch_heading_font_object());
        let _ = DrawTextW(
            hdc,
            &mut text[..text_len],
            &mut title_rect as *mut RECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        let _ = SelectObject(hdc, old_font);
    }
}

fn paint_settings_window(hwnd: HWND) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let _ = SetBkMode(hdc, TRANSPARENT);
        let white_brush = HBRUSH(GetStockObject(WHITE_BRUSH).0);
        let card_brush = CreateSolidBrush(rgb(255, 255, 255));
        let layout = current_layout(hwnd);
        let settings_panel_visible = SETTINGS_PANEL_PAINT_VISIBLE.load(Ordering::Relaxed);
        let hotkey_panel_visible = HOTKEY_PANEL_PAINT_VISIBLE.load(Ordering::Relaxed);
        let lang = current_ui_language(hwnd);
        let _ = FillRect(hdc, &layout.content_panel, white_brush);
        sketch_round_rect(hdc, &layout.content_panel, UI_RADIUS, UI_STROKE_WIDTH);
        let list_frame = RECT {
            left: layout.sidebar_x,
            top: layout.sidebar_y,
            right: layout.sidebar_x + layout.sidebar_w,
            bottom: layout.sidebar_y + layout.sidebar_h,
        };
        let _ = FillRect(hdc, &list_frame, white_brush);
        sketch_round_rect(hdc, &list_frame, UI_RADIUS, UI_STROKE_WIDTH);
        draw_group_title(
            hdc,
            &layout.window_group,
            ui_text(&lang, UiString::ShortcutSettings),
        );
        draw_section_separator(hdc, &layout.window_group);
        draw_group_title(
            hdc,
            &layout.pointer_row,
            ui_text(&lang, UiString::WindowZoom),
        );
        draw_section_separator(hdc, &layout.pointer_row);
        draw_group_title(
            hdc,
            &layout.screenshot_row,
            ui_text(&lang, UiString::PointerZoom),
        );
        draw_section_separator(hdc, &layout.screenshot_row);
        let modal_cover_for_paint = if settings_panel_visible {
            let mut rect = layout.settings_panel;
            rect.right += 8;
            rect.bottom += 8;
            Some(rect)
        } else if hotkey_panel_visible {
            let mut rect = layout.hotkey_panel;
            rect.right += 8;
            rect.bottom += 8;
            Some(rect)
        } else {
            None
        };
        for id in [
            ID_SCALE_EDIT,
            ID_POINTER_SCALE_EDIT,
            ID_POINTER_WIDTH_EDIT,
            ID_POINTER_HEIGHT_EDIT,
        ] {
            if let Some(frame) = child_frame_rect(hwnd, id, 8, 4) {
                if modal_cover_for_paint
                    .as_ref()
                    .map(|modal| rects_intersect(&frame, modal))
                    .unwrap_or(false)
                {
                    continue;
                }
                draw_input_frame(hdc, &frame);
            }
        }
        if settings_panel_visible {
            let shadow = offset_rect(layout.settings_panel, 4, 4);
            fill_rect_color(hdc, &shadow, rgb(221, 219, 211));
            let _ = FillRect(hdc, &layout.settings_panel, white_brush);
            sketch_round_rect(hdc, &layout.settings_panel, UI_RADIUS, UI_STROKE_WIDTH);
        }
        if hotkey_panel_visible {
            let shadow = offset_rect(layout.hotkey_panel, 4, 4);
            fill_rect_color(hdc, &shadow, rgb(221, 219, 211));
            let _ = FillRect(hdc, &layout.hotkey_panel, white_brush);
            sketch_round_rect(hdc, &layout.hotkey_panel, UI_RADIUS, UI_STROKE_WIDTH);
        }
        let _ = DeleteObject(card_brush.into());
        let _ = EndPaint(hwnd, &ps);
    }
}

fn draw_section_separator(hdc: HDC, rect: &RECT) {
    let line = RECT {
        left: rect.left + 22,
        top: rect.top + 54,
        right: rect.right - 22,
        bottom: rect.top + 56,
    };
    fill_rect_color(hdc, &line, rgb(18, 31, 39));
}

fn offset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right + dx,
        bottom: rect.bottom + dy,
    }
}

fn sketch_round_rect(hdc: HDC, rect: &RECT, radius: i32, width: i32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, width.max(1), rgb(18, 31, 39));
        let old_pen = SelectObject(hdc, HGDIOBJ(pen.0));
        let old_brush = SelectObject(hdc, GetStockObject(HOLLOW_BRUSH));
        if radius <= 0 {
            let _ = Rectangle(hdc, rect.left, rect.top, rect.right, rect.bottom);
        } else {
            let _ = RoundRect(
                hdc,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                radius,
                radius,
            );
        }
        let _ = SelectObject(hdc, old_brush);
        let _ = SelectObject(hdc, old_pen);
        let _ = DeleteObject(pen.into());
    }
}

fn draw_input_frame(hdc: HDC, rect: &RECT) {
    unsafe {
        let bg = CreateSolidBrush(rgb(255, 255, 255));
        let _ = FillRect(hdc, rect, bg);
        let _ = DeleteObject(bg.into());
    }
    sketch_round_rect(hdc, rect, INPUT_RADIUS, UI_STROKE_WIDTH);
}

fn create_controls(hwnd: HWND, icon_dir: &Path) -> Result<(), String> {
    create_static(hwnd, "?袁⑥쨮???뵬", 24, 32, 120, 22, ID_PROFILE_TITLE)?;
    create_bitmap_button(
        hwnd,
        ID_SETTINGS_BUTTON,
        815,
        8,
        44,
        30,
        &icon_dir.join("settings.bmp"),
        true,
    )?;
    create_bitmap_button(
        hwnd,
        ID_TRAY_BUTTON,
        865,
        8,
        44,
        30,
        &icon_dir.join("minimize-to-tray.bmp"),
        true,
    )?;

    let profile_list = create_listbox(hwnd, ID_PROFILE_LIST, 13, 76, 150, 290)?;
    subclass_profile_listbox(profile_list)?;
    create_button(hwnd, "+", ID_ADD_PROFILE, 13, 389, 150, 30)?;
    create_button(hwnd, "-", ID_DELETE_PROFILE, 13, 389, 72, 30)?;
    show_child(hwnd, ID_DELETE_PROFILE, false);
    create_edit(hwnd, ID_NAME_EDIT, 13, 76, 150, 24)?;
    show_child(hwnd, ID_NAME_EDIT, false);

    create_bitmap_static(hwnd, ID_HOTKEY_ICON, 272, 63, &icon_dir.join("hotkey.bmp"))?;
    create_static(hwnd, "???", 376, 66, 70, 24, ID_HOTKEY_LABEL)?;
    create_static(hwnd, "Ctrl", 478, 66, 72, 24, ID_HOTKEY_MOD_PRIMARY)?;
    create_static(hwnd, "Alt", 603, 66, 72, 24, ID_HOTKEY_MOD_SECONDARY)?;
    create_static(hwnd, "Q", 765, 66, 72, 24, ID_HOTKEY_KEY)?;
    create_button(hwnd, "\u{bcc0}\u{acbd}", ID_HOTKEY_CHANGE, 867, 60, 80, 30)?;

    create_bitmap_static(
        hwnd,
        ID_SCALE_ICON,
        272,
        105,
        &icon_dir.join("window-zoom.bmp"),
    )?;
    create_static(
        hwnd,
        "\u{cc3d} \u{d655}\u{b300}",
        376,
        108,
        70,
        24,
        ID_SCALE_LABEL,
    )?;
    create_edit(hwnd, ID_SCALE_EDIT, 575, 102, 84, 28)?;
    create_static(hwnd, "%", 765, 108, 40, 24, ID_SCALE_PERCENT)?;
    create_button(hwnd, "?", ID_SCALE_UP, 867, 98, 36, 26)?;
    create_button(hwnd, "?", ID_SCALE_DOWN, 911, 98, 36, 26)?;

    create_static(hwnd, "?? ??", 376, 150, 86, 24, ID_POINTER_LABEL)?;
    create_static(
        hwnd,
        "Ctrl + Alt + E",
        478,
        150,
        130,
        24,
        ID_POINTER_HOTKEY_VALUE,
    )?;
    create_button(hwnd, "??", ID_POINTER_HOTKEY_CHANGE, 867, 144, 80, 30)?;
    create_static(hwnd, "??", 290, 184, 48, 22, ID_POINTER_RANGE_LABEL)?;
    create_edit(hwnd, ID_POINTER_WIDTH_EDIT, 344, 180, 52, 28)?;
    create_static(hwnd, "x", 402, 184, 16, 22, ID_POINTER_X_LABEL)?;
    create_edit(hwnd, ID_POINTER_HEIGHT_EDIT, 420, 180, 52, 28)?;
    create_static(hwnd, "??", 490, 184, 48, 22, ID_POINTER_SCALE_LABEL)?;
    create_edit(hwnd, ID_POINTER_SCALE_EDIT, 540, 180, 54, 28)?;
    create_static(hwnd, "%", 602, 184, 22, 22, ID_POINTER_PERCENT)?;
    create_button(hwnd, "?", ID_POINTER_SCALE_UP, 650, 180, 36, 26)?;
    create_button(hwnd, "?", ID_POINTER_SCALE_DOWN, 694, 180, 36, 26)?;
    create_static(
        hwnd,
        "\u{d655}\u{b300}\u{d560} \u{c6d0}\u{bcf8} \u{d53d}\u{c140} \u{bc94}\u{c704}",
        620,
        184,
        170,
        22,
        ID_POINTER_RANGE_HELP,
    )?;

    create_bitmap_static(
        hwnd,
        ID_SCREENSHOT_ICON,
        290,
        230,
        &icon_dir.join("pointer-zoom.bmp"),
    )?;
    create_static(hwnd, "????", 290, 230, 140, 24, ID_SCREENSHOT_TITLE)?;
    create_static(hwnd, "? ??", 290, 260, 86, 24, ID_WINDOW_SCREENSHOT_LABEL)?;
    create_static(
        hwnd,
        "\u{cc3d} \u{d655}\u{b300} \u{c2a4}\u{d06c}\u{b9b0}\u{c0f7} \u{c800}\u{c7a5} \u{acbd}\u{b85c}",
        290,
        260,
        200,
        24,
        ID_WINDOW_SCREENSHOT_PATH_LABEL,
    )?;
    create_static(
        hwnd,
        "Shift + Alt + Q",
        380,
        260,
        104,
        24,
        ID_WINDOW_SCREENSHOT_HOTKEY_VALUE,
    )?;
    create_button(
        hwnd,
        "??",
        ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE,
        492,
        256,
        76,
        30,
    )?;
    create_static(hwnd, "", 580, 257, 220, 24, ID_WINDOW_SCREENSHOT_PATH_EDIT)?;
    create_button(
        hwnd,
        "筌≪뼚釉섋퉪?용┛",
        ID_WINDOW_SCREENSHOT_BROWSE,
        805,
        256,
        84,
        30,
    )?;
    create_static(hwnd, "?? ??", 290, 292, 86, 24, ID_POINTER_SCREENSHOT_LABEL)?;
    create_static(
        hwnd,
        "\u{bd80}\u{bd84} \u{d655}\u{b300} \u{c2a4}\u{d06c}\u{b9b0}\u{c0f7} \u{c800}\u{c7a5} \u{acbd}\u{b85c}",
        290,
        292,
        200,
        24,
        ID_POINTER_SCREENSHOT_PATH_LABEL,
    )?;
    create_static(
        hwnd,
        "Shift + Alt + E",
        380,
        292,
        104,
        24,
        ID_POINTER_SCREENSHOT_HOTKEY_VALUE,
    )?;
    create_button(
        hwnd,
        "??",
        ID_POINTER_SCREENSHOT_HOTKEY_CHANGE,
        492,
        288,
        76,
        30,
    )?;
    create_edit(hwnd, ID_POINTER_SCREENSHOT_PATH_EDIT, 580, 289, 220, 28)?;
    create_button(
        hwnd,
        "筌≪뼚釉섋퉪?용┛",
        ID_POINTER_SCREENSHOT_BROWSE,
        805,
        288,
        84,
        30,
    )?;

    create_static(hwnd, "?? ?? ??", 290, 326, 180, 24, ID_POINTER_COLOR_LABEL)?;
    create_static(
        hwnd,
        "Ctrl + Alt + C",
        480,
        326,
        130,
        24,
        ID_POINTER_COLOR_HOTKEY_VALUE,
    )?;
    create_button(hwnd, "??", ID_POINTER_COLOR_HOTKEY_CHANGE, 650, 320, 80, 30)?;
    create_static(
        hwnd,
        "?? ?? ??",
        290,
        360,
        180,
        24,
        ID_POINTER_COLOR_COPY_LABEL,
    )?;
    create_static(
        hwnd,
        "Shift + Alt + C",
        480,
        360,
        130,
        24,
        ID_POINTER_COLOR_COPY_HOTKEY_VALUE,
    )?;
    create_button(
        hwnd,
        "??",
        ID_POINTER_COLOR_COPY_HOTKEY_CHANGE,
        650,
        354,
        80,
        30,
    )?;
    create_static(
        hwnd,
        "??? ?? ??",
        290,
        394,
        180,
        24,
        ID_POINTER_CURSOR_LABEL,
    )?;
    create_static(
        hwnd,
        "Ctrl + Alt + R",
        480,
        394,
        130,
        24,
        ID_POINTER_CURSOR_HOTKEY_VALUE,
    )?;
    create_button(
        hwnd,
        "??",
        ID_POINTER_CURSOR_HOTKEY_CHANGE,
        650,
        388,
        80,
        30,
    )?;
    create_static(
        hwnd,
        "?? ?? ??",
        290,
        430,
        180,
        24,
        ID_POINTER_COLOR_TOGGLE_LABEL,
    )?;
    create_button(hwnd, "??", ID_POINTER_COLOR_TOGGLE, 480, 424, 74, 30)?;
    create_static(
        hwnd,
        "??? ?? ??",
        290,
        464,
        180,
        24,
        ID_POINTER_CURSOR_TOGGLE_LABEL,
    )?;
    create_button(hwnd, "??", ID_POINTER_CURSOR_TOGGLE, 480, 458, 74, 30)?;

    create_global_settings_panel(hwnd)?;
    create_hotkey_panel(hwnd)?;
    Ok(())
}

fn apply_default_font(hwnd: HWND) {
    let font = sketch_font_object();
    for id in control_ids() {
        let child = get(hwnd, *id);
        if child.0.is_null() {
            continue;
        }
        let _ = send(child, WM_SETFONT, font.0 as usize, 1);
    }
    let heading = sketch_heading_font_object();
    for id in [
        ID_PROFILE_TITLE,
        ID_SETTINGS_PANEL_TITLE,
        ID_HOTKEY_PANEL_TITLE,
    ] {
        let child = get(hwnd, id);
        if !child.0.is_null() {
            let _ = send(child, WM_SETFONT, heading.0 as usize, 1);
        }
    }
    let path_font = path_font_object();
    for id in [
        ID_WINDOW_SCREENSHOT_PATH_LABEL,
        ID_WINDOW_SCREENSHOT_PATH_EDIT,
    ] {
        let child = get(hwnd, id);
        if !child.0.is_null() {
            let _ = send(child, WM_SETFONT, path_font.0 as usize, 1);
        }
    }
}

fn ensure_ui_font_registered() -> bool {
    static FONT_RESOURCE_HANDLE: OnceLock<isize> = OnceLock::new();
    *FONT_RESOURCE_HANDLE.get_or_init(|| {
        let mut font_count = 0u32;
        let handle = unsafe {
            AddFontMemResourceEx(
                UI_FONT_TTF.as_ptr().cast(),
                UI_FONT_TTF.len() as u32,
                None,
                &mut font_count as *mut u32 as *const u32,
            )
        };
        if handle.0.is_null() || font_count == 0 {
            0
        } else {
            handle.0 as isize
        }
    }) != 0
}

fn sketch_font_object() -> HGDIOBJ {
    static FONT_HANDLE: OnceLock<isize> = OnceLock::new();
    let raw = *FONT_HANDLE.get_or_init(|| {
        let face = wide_null(if ensure_ui_font_registered() {
            UI_FONT_FACE
        } else {
            "GulimChe"
        });
        unsafe {
            CreateFontW(
                -15,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                DEFAULT_QUALITY,
                DEFAULT_PITCH.0 as u32,
                PCWSTR(face.as_ptr()),
            )
            .0 as isize
        }
    });
    if raw == 0 {
        unsafe { GetStockObject(DEFAULT_GUI_FONT) }
    } else {
        HGDIOBJ(raw as *mut _)
    }
}

fn sketch_heading_font_object() -> HGDIOBJ {
    static FONT_HANDLE: OnceLock<isize> = OnceLock::new();
    let raw = *FONT_HANDLE.get_or_init(|| {
        let face = wide_null(if ensure_ui_font_registered() {
            UI_FONT_FACE
        } else {
            "GulimChe"
        });
        unsafe {
            CreateFontW(
                -18,
                0,
                0,
                0,
                700,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                DEFAULT_QUALITY,
                DEFAULT_PITCH.0 as u32,
                PCWSTR(face.as_ptr()),
            )
            .0 as isize
        }
    });
    if raw == 0 {
        unsafe { GetStockObject(DEFAULT_GUI_FONT) }
    } else {
        HGDIOBJ(raw as *mut _)
    }
}

fn path_font_object() -> HGDIOBJ {
    static FONT_HANDLE: OnceLock<isize> = OnceLock::new();
    let raw = *FONT_HANDLE.get_or_init(|| {
        let face = wide_null(if ensure_ui_font_registered() {
            UI_FONT_FACE
        } else {
            "GulimChe"
        });
        unsafe {
            CreateFontW(
                -15,
                0,
                0,
                0,
                400,
                0,
                0,
                0,
                DEFAULT_CHARSET,
                OUT_DEFAULT_PRECIS,
                CLIP_DEFAULT_PRECIS,
                DEFAULT_QUALITY,
                DEFAULT_PITCH.0 as u32,
                PCWSTR(face.as_ptr()),
            )
            .0 as isize
        }
    });
    if raw == 0 {
        unsafe { GetStockObject(DEFAULT_GUI_FONT) }
    } else {
        HGDIOBJ(raw as *mut _)
    }
}

fn create_global_settings_panel(hwnd: HWND) -> Result<(), String> {
    create_panel_background(hwnd, ID_SETTINGS_PANEL_BG, 610, 90, 380, 278)?;
    create_static(
        hwnd,
        "\u{c124}\u{c815}",
        636,
        112,
        220,
        24,
        ID_SETTINGS_PANEL_TITLE,
    )?;
    create_static(
        hwnd,
        "\u{c5b8}\u{c5b4}",
        628,
        104,
        70,
        24,
        ID_SETTINGS_LANGUAGE_LABEL,
    )?;
    create_button(
        hwnd,
        "\u{d55c}\u{ad6d}\u{c5b4}",
        ID_LANGUAGE_COMBO,
        700,
        100,
        190,
        32,
    )?;
    create_plain_listbox(hwnd, ID_LANGUAGE_MENU, 700, 132, 190, 58)?;
    create_button(
        hwnd,
        "\u{ae30}\u{bcf8}\u{ac12}\u{c73c}\u{b85c} \u{cd08}\u{ae30}\u{d654}",
        ID_RESET_BUTTON,
        628,
        162,
        160,
        30,
    )?;
    create_button(
        hwnd,
        "\u{b85c}\u{adf8} \u{cd9c}\u{b825}",
        ID_LOG_BUTTON,
        628,
        206,
        112,
        30,
    )?;
    create_button(
        hwnd,
        "\u{b2eb}\u{ae30}",
        ID_SETTINGS_CLOSE,
        808,
        256,
        70,
        28,
    )?;
    for id in settings_panel_ids() {
        show_child(hwnd, *id, false);
    }
    show_child(hwnd, ID_LANGUAGE_MENU, false);
    Ok(())
}

fn create_hotkey_panel(hwnd: HWND) -> Result<(), String> {
    create_panel_background(hwnd, ID_HOTKEY_PANEL_BG, 330, 290, 420, 218)?;
    create_static(hwnd, "??? ??", 356, 312, 220, 24, ID_HOTKEY_PANEL_TITLE)?;
    create_static(hwnd, "??? ???? ????.", 368, 314, 300, 24, ID_HOTKEY_HELP)?;
    create_static(
        hwnd,
        "\u{d604}\u{c7ac}",
        368,
        344,
        90,
        24,
        ID_HOTKEY_CURRENT_LABEL,
    )?;
    create_static(
        hwnd,
        "Ctrl + Alt + Q",
        470,
        344,
        180,
        24,
        ID_HOTKEY_CURRENT_VALUE,
    )?;
    create_static(hwnd, "? ???", 368, 374, 90, 24, ID_HOTKEY_NEW_LABEL)?;
    create_static(
        hwnd,
        "Ctrl + Alt + Q",
        470,
        374,
        180,
        24,
        ID_HOTKEY_NEW_VALUE,
    )?;
    create_button(hwnd, "\u{c801}\u{c6a9}", ID_HOTKEY_APPLY, 470, 312, 70, 30)?;
    create_button(hwnd, "\u{cde8}\u{c18c}", ID_HOTKEY_CANCEL, 552, 312, 70, 30)?;
    for id in hotkey_panel_ids() {
        show_child(hwnd, *id, false);
    }
    Ok(())
}

fn create_panel_background(
    hwnd: HWND,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("STATIC"),
        "",
        style(WS_CHILD | WS_CLIPSIBLINGS, &[SS_WHITERECT_STYLE]),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_static(
    hwnd: HWND,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    id: i32,
) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("STATIC"),
        text,
        style(
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            &[SS_NOTIFY_STYLE, SS_LEFTNOWORDWRAP_STYLE],
        ),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_button(
    hwnd: HWND,
    text: &str,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("BUTTON"),
        text,
        style(WS_CHILD | WS_VISIBLE | WS_TABSTOP, &[BS_OWNERDRAW_STYLE]),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_bitmap_button(
    hwnd: HWND,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    path: &Path,
    clickable: bool,
) -> Result<HWND, String> {
    let base = if clickable {
        WS_CHILD | WS_VISIBLE | WS_TABSTOP
    } else {
        WS_CHILD | WS_VISIBLE
    };
    let button = create_child(
        hwnd,
        w!("BUTTON"),
        "",
        style(base, &[BS_OWNERDRAW_STYLE]),
        x,
        y,
        w,
        h,
        id,
    )?;
    remember_button_icon_path(id, path);
    if !clickable {
        unsafe {
            let _ = EnableWindow(button, false);
        }
    }
    Ok(button)
}

fn create_bitmap_static(hwnd: HWND, id: i32, x: i32, y: i32, path: &Path) -> Result<HWND, String> {
    let control = create_child(
        hwnd,
        w!("STATIC"),
        "",
        style(WS_CHILD | WS_VISIBLE, &[SS_BITMAP_STYLE]),
        x,
        y,
        ROW_ICON_SIZE,
        ROW_ICON_SIZE,
        id,
    )?;
    let path_wide = wide_null(&path.to_string_lossy());
    let handle = load_bitmap_image(&path_wide, path)?;
    let _ = send(
        control,
        STM_SETIMAGE,
        IMAGE_BITMAP.0 as usize,
        handle.0 as isize,
    );
    Ok(control)
}

fn load_bitmap_image(
    path_wide: &[u16],
    path: &Path,
) -> Result<windows::Win32::Foundation::HANDLE, String> {
    unsafe {
        LoadImageW(
            None,
            PCWSTR(path_wide.as_ptr()),
            IMAGE_BITMAP,
            ROW_ICON_SIZE,
            ROW_ICON_SIZE,
            LR_LOADFROMFILE,
        )
    }
    .map_err(|error| format!("LoadImageW icon failed for {}: {error:?}", path.display()))
}

fn apply_window_icon(hwnd: HWND, path: &Path) {
    let small = load_icon_image(path, 16);
    let big = load_icon_image(path, 32);
    unsafe {
        if let Ok(icon) = small {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_SMALL_WPARAM)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
        if let Ok(icon) = big {
            let _ = SendMessageW(
                hwnd,
                WM_SETICON,
                Some(WPARAM(ICON_BIG_WPARAM)),
                Some(LPARAM(icon.0 as isize)),
            );
        }
    }
}

fn load_icon_image(path: &Path, size: i32) -> Result<windows::Win32::Foundation::HANDLE, String> {
    let path_wide = wide_null(&path.to_string_lossy());
    unsafe {
        LoadImageW(
            None,
            PCWSTR(path_wide.as_ptr()),
            IMAGE_ICON,
            size,
            size,
            LR_LOADFROMFILE,
        )
    }
    .map_err(|error| {
        format!(
            "LoadImageW app icon failed for {}: {error:?}",
            path.display()
        )
    })
}

fn create_edit(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("EDIT"),
        "",
        style(WS_CHILD | WS_VISIBLE | WS_TABSTOP, &[ES_AUTOHSCROLL]),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_listbox(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("LISTBOX"),
        "",
        style(
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            &[
                LBS_NOTIFY,
                LBS_OWNERDRAWFIXED_STYLE,
                LBS_HASSTRINGS_STYLE,
                LBS_NOINTEGRALHEIGHT_STYLE,
            ],
        ),
        x,
        y,
        w,
        h,
        id,
    )
}

fn subclass_profile_listbox(list: HWND) -> Result<(), String> {
    let previous = unsafe {
        SetWindowLongPtrW(
            list,
            GWLP_WNDPROC,
            profile_listbox_proc as *const () as usize as isize,
        )
    };
    if previous == 0 {
        let error = unsafe { GetLastError() };
        if error.0 != 0 {
            return Err(format!("profile list subclass failed: {error:?}"));
        }
    }
    PROFILE_LIST_ORIGINAL_PROC.store(previous, Ordering::Relaxed);
    Ok(())
}

unsafe extern "system" fn profile_listbox_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            let x = loword(lparam.0 as usize) as i16 as i32;
            let y = hiword(lparam.0 as usize) as i16 as i32;
            track_profile_list_mouse_leave(hwnd);
            update_profile_list_hover(hwnd, Some((x, y)));
        }
        WM_MOUSELEAVE_MSG => {
            update_profile_list_hover(hwnd, None);
            clear_profile_list_delete_press(hwnd);
        }
        WM_LBUTTONDOWN => {
            let x = loword(lparam.0 as usize) as i16 as i32;
            let y = hiword(lparam.0 as usize) as i16 as i32;
            if begin_profile_list_delete_click(hwnd, x, y) {
                return LRESULT(0);
            }
        }
        WM_LBUTTONUP => {
            let x = loword(lparam.0 as usize) as i16 as i32;
            let y = hiword(lparam.0 as usize) as i16 as i32;
            if finish_profile_list_delete_click(hwnd, x, y) {
                return LRESULT(0);
            }
        }
        _ => {}
    }
    let previous = PROFILE_LIST_ORIGINAL_PROC.load(Ordering::Relaxed);
    if previous != 0 {
        let proc: WNDPROC = unsafe { std::mem::transmute(previous) };
        unsafe { CallWindowProcW(proc, hwnd, msg, wparam, lparam) }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

fn clear_profile_list_delete_press(list: HWND) {
    let Ok(parent) = (unsafe { GetParent(list) }) else {
        return;
    };
    if parent.0.is_null() {
        return;
    }
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.hwnd == raw_from_hwnd(parent) {
        state.pressed_delete_profile_index = None;
    }
}

fn track_profile_list_mouse_leave(list: HWND) {
    let mut event = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: TME_LEAVE,
        hwndTrack: list,
        dwHoverTime: 0,
    };
    unsafe {
        let _ = TrackMouseEvent(&mut event);
    }
}

fn update_profile_list_hover(list: HWND, point: Option<(i32, i32)>) {
    let Ok(parent) = (unsafe { GetParent(list) }) else {
        return;
    };
    if parent.0.is_null() {
        return;
    }
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.hwnd != raw_from_hwnd(parent) {
        return;
    }
    let profile_count = profiles(&state.settings).len();
    let next = if state.settings_panel_visible || state.hotkey_panel_visible {
        None
    } else if let Some((x, y)) = point {
        profile_index_at_list_point(list, profile_count, x, y)
            .map(|(index, _)| index)
            .filter(|index| *index > 0)
    } else {
        None
    };
    if state.hovered_profile_index != next {
        state.hovered_profile_index = next;
        unsafe {
            let _ = InvalidateRect(Some(list), None, false);
        }
    }
}

fn hovered_profile_index_for_profile_list(list: HWND) -> Option<usize> {
    let Ok(parent) = (unsafe { GetParent(list) }) else {
        return None;
    };
    if parent.0.is_null() {
        return None;
    }
    let Ok(slot) = state_slot().try_lock() else {
        return None;
    };
    let Some(state) = slot.as_ref() else {
        return None;
    };
    if state.hwnd == raw_from_hwnd(parent) {
        state.hovered_profile_index
    } else {
        None
    }
}

fn profile_index_at_list_point(
    list: HWND,
    profile_count: usize,
    x: i32,
    y: i32,
) -> Option<(usize, RECT)> {
    for index in 0..profile_count {
        let mut row = RECT::default();
        if send(
            list,
            LB_GETITEMRECT_MSG,
            index,
            &mut row as *mut RECT as isize,
        ) >= 0
            && rect_contains_point(row, x, y)
        {
            return Some((index, row));
        }
    }
    None
}

fn profile_delete_index_at_point(
    state: &SettingsUiState,
    list: HWND,
    x: i32,
    y: i32,
) -> Option<usize> {
    if state.loading || state.settings_panel_visible || state.hotkey_panel_visible {
        return None;
    }
    let profile_count = profiles(&state.settings).len();
    let (delete_index, row) = profile_index_at_list_point(list, profile_count, x, y)?;
    if delete_index == 0 {
        return None;
    }
    let delete_rect = profile_delete_hit_rect(inset_rect(row, 2, 2));
    rect_contains_point(delete_rect, x, y).then_some(delete_index)
}

fn begin_profile_list_delete_click(list: HWND, x: i32, y: i32) -> bool {
    let Ok(parent) = (unsafe { GetParent(list) }) else {
        return false;
    };
    if parent.0.is_null() {
        return false;
    }
    let Ok(mut slot) = state_slot().try_lock() else {
        return false;
    };
    let Some(state) = slot.as_mut() else {
        return false;
    };
    if state.hwnd != raw_from_hwnd(parent) {
        return false;
    }
    let Some(delete_index) = profile_delete_index_at_point(state, list, x, y) else {
        state.pressed_delete_profile_index = None;
        return false;
    };
    state.pressed_delete_profile_index = Some(delete_index);
    true
}

fn finish_profile_list_delete_click(list: HWND, x: i32, y: i32) -> bool {
    let Ok(parent) = (unsafe { GetParent(list) }) else {
        return false;
    };
    if parent.0.is_null() {
        return false;
    }
    let Ok(mut slot) = state_slot().try_lock() else {
        return false;
    };
    let Some(state) = slot.as_mut() else {
        return false;
    };
    if state.hwnd != raw_from_hwnd(parent) {
        return false;
    }
    let pressed_index = state.pressed_delete_profile_index.take();
    let Some(delete_index) = profile_delete_index_at_point(state, list, x, y) else {
        return pressed_index.is_some();
    };
    if pressed_index.is_some_and(|index| index != delete_index) {
        return true;
    }
    commit_profile_name_edit(state, true);
    state.pending_profile_name = None;
    state.loading = true;
    show_child(parent, ID_NAME_EDIT, false);
    state.loading = false;
    if delete_profile_at(state, delete_index) {
        let _ = save_settings(state);
        push_event(SettingsUiEvent::HotkeysChanged);
        push_event(SettingsUiEvent::ProfileChanged);
        refresh_all_controls(state);
        layout_profile_buttons_for_state(state);
    }
    true
}

fn create_plain_listbox(
    hwnd: HWND,
    id: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("LISTBOX"),
        "",
        style(
            WS_CHILD | WS_VISIBLE | WS_TABSTOP,
            &[LBS_NOTIFY, LBS_NOINTEGRALHEIGHT_STYLE],
        ),
        x,
        y,
        w,
        h,
        id,
    )
}

fn create_child(
    hwnd: HWND,
    class: PCWSTR,
    text: &str,
    style: windows::Win32::UI::WindowsAndMessaging::WINDOW_STYLE,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    id: i32,
) -> Result<HWND, String> {
    let instance = unsafe { GetModuleHandleW(None) }
        .map_err(|error| format!("GetModuleHandleW failed: {error:?}"))?;
    let text = wide_null(text);
    let menu = if id == 0 {
        None
    } else {
        Some(HMENU(id as isize as *mut _))
    };
    let style = WINDOW_STYLE(style.0 | WS_CLIPSIBLINGS.0);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            PCWSTR(text.as_ptr()),
            style,
            x,
            y,
            width,
            height,
            Some(hwnd),
            menu,
            Some(HINSTANCE(instance.0)),
            None,
        )
    }
    .map_err(|error| format!("CreateWindowExW child failed: {error:?}"))
}

fn apply_live_edits_from_controls(hwnd: HWND) {
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.hwnd != raw_from_hwnd(hwnd) || state.loading {
        return;
    }
    sanitize_scale_edit(state, false);
    sanitize_pointer_numeric_edit(state, ID_POINTER_WIDTH_EDIT, false);
    sanitize_pointer_numeric_edit(state, ID_POINTER_HEIGHT_EDIT, false);
    sanitize_pointer_numeric_edit(state, ID_POINTER_SCALE_EDIT, false);
    if state.settings_panel_visible {
        apply_screenshot_path_edit(state, ID_WINDOW_SCREENSHOT_PATH_EDIT);
    }
}

fn style(base: WINDOW_STYLE, extra: &[i32]) -> WINDOW_STYLE {
    let mut value = base.0;
    for item in extra {
        value |= *item as u32;
    }
    WINDOW_STYLE(value)
}

#[repr(C)]
struct OwnerDrawItem {
    ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    item_action: u32,
    item_state: u32,
    hwnd_item: HWND,
    hdc: HDC,
    rc_item: RECT,
    item_data: usize,
}

#[repr(C)]
struct OwnerMeasureItem {
    ctl_type: u32,
    ctl_id: u32,
    item_id: u32,
    item_width: u32,
    item_height: u32,
    item_data: usize,
}

fn measure_owner_draw_item(lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let item = unsafe { &mut *(lparam.0 as *mut OwnerMeasureItem) };
    if item.ctl_id as i32 == ID_PROFILE_LIST {
        item.item_height = PROFILE_ROW_HEIGHT as u32;
    }
}

fn draw_owner_draw_item(lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let item = unsafe { &*(lparam.0 as *const OwnerDrawItem) };
    if item.ctl_id as i32 == ID_PROFILE_LIST {
        draw_owner_profile_item(item);
    } else {
        draw_owner_button(item);
    }
}

fn draw_owner_profile_item(item: &OwnerDrawItem) {
    if item.item_id == u32::MAX {
        return;
    }
    let selected = (item.item_state & ODS_SELECTED_FLAG) != 0;
    let rect = inset_rect(item.rc_item, 2, 2);
    let editing = selected && rename_edit_visible_for_profile_list(item.hwnd_item);
    fill_rect_color(
        item.hdc,
        &item.rc_item,
        if selected {
            rgb(244, 246, 240)
        } else {
            rgb(255, 255, 255)
        },
    );
    if selected {
        sketch_round_rect(item.hdc, &rect, UI_RADIUS, UI_STROKE_WIDTH);
    }
    let hovered = hovered_profile_index_for_profile_list(item.hwnd_item)
        .map(|index| index == item.item_id as usize)
        .unwrap_or(false);
    let show_delete = item.item_id > 0 && hovered;
    if !editing {
        let text = listbox_item_text(item.hwnd_item, item.item_id);
        let reserved_action_width = if show_delete {
            PROFILE_DELETE_W + PROFILE_DELETE_GAP
        } else {
            0
        };
        let text_rect = RECT {
            left: rect.left + 8,
            top: rect.top + 2,
            right: rect.right - 8 - reserved_action_width,
            bottom: rect.bottom - 2,
        };
        draw_text_ellipsis(
            item.hdc,
            &text,
            &text_rect,
            if selected {
                rgb(0, 0, 0)
            } else {
                rgb(35, 35, 35)
            },
        );
    }
    if show_delete {
        let delete_rect = profile_delete_icon_rect(rect);
        let _ = draw_profile_action_icon(item.hdc, &delete_rect, ID_DELETE_PROFILE, false);
    }
}

fn rename_edit_visible_for_profile_list(list_hwnd: HWND) -> bool {
    unsafe {
        let Ok(parent) = GetParent(list_hwnd) else {
            return false;
        };
        !parent.0.is_null() && IsWindowVisible(get(parent, ID_NAME_EDIT)).as_bool()
    }
}

fn draw_owner_button(item: &OwnerDrawItem) {
    let id = item.ctl_id as i32;
    let disabled = (item.item_state & ODS_DISABLED_FLAG) != 0;
    if is_scale_arrow_button(id) {
        draw_scale_spinner_half(item.hdc, &item.rc_item, id, disabled);
        return;
    }
    if id == ID_DELETE_PROFILE {
        fill_rect_color(item.hdc, &item.rc_item, rgb(244, 246, 240));
        let rect = inset_rect(item.rc_item, 3, 3);
        let _ = draw_profile_action_icon(item.hdc, &rect, id, disabled);
        return;
    }
    let selected = (item.item_state & ODS_SELECTED_FLAG) != 0;
    let mut rect = inset_rect(item.rc_item, 2, 2);
    if selected {
        rect.left += 1;
        rect.top += 1;
        rect.right += 1;
        rect.bottom += 1;
    }
    fill_rect_color(
        item.hdc,
        &item.rc_item,
        if disabled {
            rgb(250, 250, 246)
        } else {
            rgb(255, 255, 255)
        },
    );
    fill_rect_color(
        item.hdc,
        &rect,
        if disabled {
            rgb(244, 244, 240)
        } else if selected {
            rgb(235, 241, 232)
        } else {
            rgb(255, 255, 255)
        },
    );
    sketch_round_rect(item.hdc, &rect, UI_RADIUS, UI_STROKE_WIDTH);
    if id == ID_LANGUAGE_COMBO {
        draw_owner_combo(item.hdc, &rect, &get_text(item.hwnd_item), disabled);
        return;
    }
    if draw_toolbar_icon(item.hdc, &rect, id, disabled) {
        return;
    }
    if draw_profile_action_icon(item.hdc, &rect, id, disabled) {
        return;
    }
    let label = owner_button_label(id, &get_text(item.hwnd_item));
    draw_text_center(
        item.hdc,
        &label,
        &rect,
        if disabled {
            rgb(160, 160, 160)
        } else {
            rgb(0, 0, 0)
        },
    );
}

fn is_scale_arrow_button(id: i32) -> bool {
    id == ID_SCALE_UP
        || id == ID_SCALE_DOWN
        || id == ID_POINTER_SCALE_UP
        || id == ID_POINTER_SCALE_DOWN
}

fn draw_profile_action_icon(hdc: HDC, rect: &RECT, id: i32, disabled: bool) -> bool {
    if id != ID_ADD_PROFILE && id != ID_DELETE_PROFILE {
        return false;
    }
    let color = if disabled {
        rgb(150, 150, 150)
    } else {
        rgb(14, 25, 32)
    };
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let cx = rect.left + width / 2;
    let cy = rect.top + height / 2;
    let len = (width.min(height) / 2).clamp(7, 12);
    let stroke = 3;
    let horizontal = RECT {
        left: cx - len / 2,
        top: cy - stroke / 2,
        right: cx + len / 2 + 1,
        bottom: cy + stroke / 2 + 1,
    };
    fill_rect_color(hdc, &horizontal, color);
    if id == ID_ADD_PROFILE {
        let vertical = RECT {
            left: cx - stroke / 2,
            top: cy - len / 2,
            right: cx + stroke / 2 + 1,
            bottom: cy + len / 2 + 1,
        };
        fill_rect_color(hdc, &vertical, color);
    }
    true
}

fn draw_scale_spinner_half(hdc: HDC, rect: &RECT, id: i32, disabled: bool) {
    let bg = if disabled {
        rgb(244, 244, 240)
    } else {
        rgb(255, 255, 255)
    };
    fill_rect_color(hdc, rect, bg);
    let line = if disabled {
        rgb(150, 150, 150)
    } else {
        rgb(18, 31, 39)
    };
    let up = id == ID_SCALE_UP || id == ID_POINTER_SCALE_UP;
    let w = rect.right - rect.left;
    let h = rect.bottom - rect.top;
    let t = UI_STROKE_WIDTH.max(1);
    let r = 4;
    let left_top = if up { rect.top + r } else { rect.top };
    let right_top = left_top;
    let left_bottom = if up { rect.bottom } else { rect.bottom - r };
    let right_bottom = left_bottom;
    fill_rect_color(
        hdc,
        &RECT {
            left: rect.left,
            top: left_top,
            right: rect.left + t,
            bottom: left_bottom,
        },
        line,
    );
    fill_rect_color(
        hdc,
        &RECT {
            left: rect.right - t,
            top: right_top,
            right: rect.right,
            bottom: right_bottom,
        },
        line,
    );
    if up {
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.left + r,
                top: rect.top,
                right: rect.right - r,
                bottom: rect.top + t,
            },
            line,
        );
        // Pixel-rounded upper corners.
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.left + 2,
                top: rect.top + 2,
                right: rect.left + 4,
                bottom: rect.top + 4,
            },
            line,
        );
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.right - 4,
                top: rect.top + 2,
                right: rect.right - 2,
                bottom: rect.top + 4,
            },
            line,
        );
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.left + 3,
                top: rect.bottom - t,
                right: rect.right - 3,
                bottom: rect.bottom,
            },
            line,
        );
    } else {
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.left + r,
                top: rect.bottom - t,
                right: rect.right - r,
                bottom: rect.bottom,
            },
            line,
        );
        // Pixel-rounded lower corners.
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.left + 2,
                top: rect.bottom - 4,
                right: rect.left + 4,
                bottom: rect.bottom - 2,
            },
            line,
        );
        fill_rect_color(
            hdc,
            &RECT {
                left: rect.right - 4,
                top: rect.bottom - 4,
                right: rect.right - 2,
                bottom: rect.bottom - 2,
            },
            line,
        );
    }

    let color = if disabled {
        rgb(150, 150, 150)
    } else {
        rgb(14, 25, 32)
    };
    let cx = rect.left + w / 2;
    let arrow_top = rect.top + ((h - 5) / 2).max(2);
    for row in 0..4 {
        let half = if up { row + 1 } else { 4 - row };
        let y = arrow_top + row;
        let block = RECT {
            left: cx - half,
            top: y,
            right: cx + half + 1,
            bottom: y + 1,
        };
        fill_rect_color(hdc, &block, color);
    }
}

fn draw_owner_combo(hdc: HDC, rect: &RECT, label: &str, disabled: bool) {
    let text_color = if disabled {
        rgb(160, 160, 160)
    } else {
        rgb(0, 0, 0)
    };
    draw_text_left(hdc, label, rect.left + 12, rect.top + 7, text_color);
    let arrow_left = rect.right - 34;
    let divider = RECT {
        left: arrow_left,
        top: rect.top + 3,
        right: arrow_left + UI_STROKE_WIDTH,
        bottom: rect.bottom - 3,
    };
    fill_rect_color(hdc, &divider, rgb(18, 31, 39));
    let cx = arrow_left + 17;
    let cy = rect.top + ((rect.bottom - rect.top) / 2) - 2;
    let color = if disabled {
        rgb(150, 150, 150)
    } else {
        rgb(18, 31, 39)
    };
    for row in 0..5 {
        let half = 4 - row;
        let block = RECT {
            left: cx - half,
            top: cy + row,
            right: cx + half + 1,
            bottom: cy + row + 1,
        };
        fill_rect_color(hdc, &block, color);
    }
}

fn owner_button_label(id: i32, text: &str) -> String {
    match id {
        ID_SCALE_UP => "?".to_string(),
        ID_SCALE_DOWN => "?".to_string(),
        ID_POINTER_SCALE_UP => "?".to_string(),
        ID_POINTER_SCALE_DOWN => "?".to_string(),
        _ => text.to_string(),
    }
}

fn draw_toolbar_icon(hdc: HDC, rect: &RECT, id: i32, disabled: bool) -> bool {
    if id != ID_SETTINGS_BUTTON && id != ID_TRAY_BUTTON {
        return false;
    }
    if let Some(path) = button_icon_path(id) {
        if draw_bitmap_icon_from_file(hdc, rect, &path, 24).is_ok() {
            return true;
        }
    }
    let pattern = if id == ID_SETTINGS_BUTTON {
        pixel_icon_settings()
    } else {
        pixel_icon_tray()
    };
    draw_pixel_pattern(hdc, rect, pattern, disabled);
    true
}

fn draw_bitmap_icon_from_file(
    hdc: HDC,
    rect: &RECT,
    path: &Path,
    target_size: i32,
) -> Result<(), String> {
    let path_wide = wide_null(&path.to_string_lossy());
    let bitmap = load_bitmap_image(&path_wide, path)?;
    unsafe {
        let memory_dc = CreateCompatibleDC(Some(hdc));
        if memory_dc.0.is_null() {
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            return Err("CreateCompatibleDC failed".to_string());
        }
        let old_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let icon_size = target_size
            .min(rect.right - rect.left)
            .min(rect.bottom - rect.top);
        let left = rect.left + ((rect.right - rect.left - icon_size) / 2);
        let top = rect.top + ((rect.bottom - rect.top - icon_size) / 2);
        let _ = StretchBlt(
            hdc,
            left,
            top,
            icon_size,
            icon_size,
            Some(memory_dc),
            0,
            0,
            ROW_ICON_SIZE,
            ROW_ICON_SIZE,
            SRCCOPY,
        );
        let _ = SelectObject(memory_dc, old_bitmap);
        let _ = DeleteDC(memory_dc);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
    }
    Ok(())
}

fn draw_pixel_pattern(hdc: HDC, rect: &RECT, pattern: &[&str; 24], disabled: bool) {
    let cell = ((rect.right - rect.left).min(rect.bottom - rect.top) / 24).max(1);
    let icon_w = cell * 24;
    let left = rect.left + ((rect.right - rect.left - icon_w) / 2);
    let top = rect.top + ((rect.bottom - rect.top - icon_w) / 2);
    for (row, line) in pattern.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if let Some(color) = pixel_icon_color(ch, disabled) {
                let block = RECT {
                    left: left + col as i32 * cell,
                    top: top + row as i32 * cell,
                    right: left + (col as i32 + 1) * cell,
                    bottom: top + (row as i32 + 1) * cell,
                };
                fill_rect_color(hdc, &block, color);
            }
        }
    }
}

fn pixel_icon_color(ch: char, disabled: bool) -> Option<COLORREF> {
    if disabled {
        return match ch {
            'B' | 'D' => Some(rgb(145, 145, 145)),
            'M' => Some(rgb(174, 174, 174)),
            'L' | 'G' => Some(rgb(224, 224, 224)),
            'W' => Some(rgb(255, 255, 255)),
            _ => None,
        };
    }
    match ch {
        'B' => Some(rgb(14, 25, 32)),
        'D' => Some(rgb(47, 63, 72)),
        'M' => Some(rgb(122, 145, 153)),
        'L' => Some(rgb(224, 236, 238)),
        'G' => Some(rgb(196, 203, 201)),
        'W' => Some(rgb(255, 255, 255)),
        _ => None,
    }
}

fn pixel_icon_settings() -> &'static [&'static str; 24] {
    &[
        "........................",
        "........................",
        "........................",
        "...........BB...........",
        "..........BBB...........",
        ".....BBB..BBB...BBBB....",
        ".....BBBBBBBBBBBBBBB....",
        ".....BBBBBBBBBBBBBB.....",
        "......BBBB....BBBB......",
        "......BBB......BBB......",
        "....BBBB........BBBB....",
        "...BBBBB........BBBBB...",
        "...BBBBB........BBBBB...",
        "......BB........BB......",
        "......BBB......BBB......",
        "......BBBB....BBBB......",
        ".....BBBBBBBBBBBBBB.....",
        ".....BBBBBBBBBBBBBBB....",
        ".....BBB..BBB...BBBB....",
        ".....BB...BBB....BB.....",
        "...........BB...........",
        "........................",
        "........................",
        "........................",
    ]
}

fn pixel_icon_tray() -> &'static [&'static str; 24] {
    &[
        "........................",
        "........................",
        "........................",
        "........................",
        "...BBBBBBBBBBBBBBBBBB...",
        "...B............BB.BB...",
        "...B................B...",
        "...B................B...",
        "...B................B...",
        "...B.......BB.......B...",
        "...B.......BB.......B...",
        "...B.......BB.......B...",
        "...B.......BB.......B...",
        "...B.....B.BBBB.....B...",
        "...B.....BBBBBB.....B...",
        "...B......BBBB......B...",
        "...B......BBB.......B...",
        "........................",
        ".BBBBBBBBBBBBBBBBBBBBBB.",
        ".B....................B.",
        ".BBBBBBBBBBBBBBBBBBBBBB.",
        "........................",
        "........................",
        "........................",
    ]
}

fn listbox_item_text(hwnd: HWND, item_id: u32) -> String {
    let mut buffer = [0u16; 256];
    let result = send(
        hwnd,
        LB_GETTEXT_MSG,
        item_id as usize,
        buffer.as_mut_ptr() as isize,
    );
    if result < 0 {
        return String::new();
    }
    let len = buffer.iter().position(|value| *value == 0).unwrap_or(0);
    String::from_utf16_lossy(&buffer[..len])
}

fn fill_rect_color(hdc: HDC, rect: &RECT, color: COLORREF) {
    unsafe {
        let brush = CreateSolidBrush(color);
        let _ = FillRect(hdc, rect, brush);
        let _ = DeleteObject(brush.into());
    }
}

fn draw_text_left(hdc: HDC, text: &str, x: i32, y: i32, color: COLORREF) {
    let text = wide_null(text);
    unsafe {
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, color);
        let old_font = SelectObject(hdc, sketch_font_object());
        let _ = TextOutW(hdc, x, y, &text[..text.len() - 1]);
        let _ = SelectObject(hdc, old_font);
    }
}

fn draw_text_ellipsis(hdc: HDC, text: &str, rect: &RECT, color: COLORREF) {
    let mut text = wide_null(text);
    let text_len = text.len().saturating_sub(1);
    let mut rect = *rect;
    unsafe {
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, color);
        let old_font = SelectObject(hdc, sketch_font_object());
        let _ = DrawTextW(
            hdc,
            &mut text[..text_len],
            &mut rect as *mut RECT,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        let _ = SelectObject(hdc, old_font);
    }
}

fn draw_text_center(hdc: HDC, text: &str, rect: &RECT, color: COLORREF) {
    let mut text = wide_null(text);
    let text_len = text.len().saturating_sub(1);
    let mut rect = *rect;
    unsafe {
        let _ = SetBkMode(hdc, TRANSPARENT);
        let _ = SetTextColor(hdc, color);
        let old_font = SelectObject(hdc, sketch_font_object());
        let _ = DrawTextW(
            hdc,
            &mut text[..text_len],
            &mut rect as *mut RECT,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        let _ = SelectObject(hdc, old_font);
    }
}

fn inset_rect(rect: RECT, dx: i32, dy: i32) -> RECT {
    RECT {
        left: rect.left + dx,
        top: rect.top + dy,
        right: rect.right - dx,
        bottom: rect.bottom - dy,
    }
}

fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    COLORREF(red as u32 | ((green as u32) << 8) | ((blue as u32) << 16))
}

fn should_open_folder_picker_from_command(id: i32, code: u32) -> bool {
    matches!(
        (id, code),
        (ID_WINDOW_SCREENSHOT_PATH_EDIT, STN_CLICKED_NOTIFY)
    )
}

fn handle_command(hwnd: HWND, wparam: WPARAM) {
    let id = loword(wparam.0) as i32;
    let code = hiword(wparam.0) as u32;
    if should_open_folder_picker_from_command(id, code) {
        browse_screenshot_folder(hwnd, id);
        return;
    }
    if code == BN_CLICKED
        && (id == ID_WINDOW_SCREENSHOT_BROWSE || id == ID_POINTER_SCREENSHOT_BROWSE)
    {
        browse_screenshot_folder(hwnd, id);
        return;
    }
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.loading {
        return;
    }

    if id != ID_NAME_EDIT && id != ID_PROFILE_LIST {
        commit_profile_name_edit(state, true);
    }

    match id {
        ID_PROFILE_LIST if code == LBN_SELCHANGE => {
            let selected = send(get(hwnd, ID_PROFILE_LIST), LB_GETCURSEL, 0, 0);
            commit_profile_name_edit(state, true);
            if selected >= 0 {
                state.selected_index = selected as usize;
                if let Some(profile) = profile_at(&state.settings, state.selected_index) {
                    let profile_id = profile.id.clone();
                    let hotkey = profile.windowed_hotkey.clone();
                    state.settings.profiles.active_profile_id = profile_id;
                    state.settings.hotkeys.windowed_toggle = hotkey;
                    let _ = save_settings(state);
                    push_event(SettingsUiEvent::HotkeysChanged);
                    push_event(SettingsUiEvent::ProfileChanged);
                }
                refresh_profile_list(state);
                refresh_profile_controls(state);
                layout_profile_buttons_for_state(state);
            }
        }
        ID_PROFILE_LIST if code == LBN_DBLCLK => {
            let selected = send(get(hwnd, ID_PROFILE_LIST), LB_GETCURSEL, 0, 0);
            commit_profile_name_edit(state, true);
            if selected >= 0 {
                state.selected_index = selected as usize;
                if let Some((profile_id, hotkey)) =
                    profile_at(&state.settings, state.selected_index)
                        .map(|profile| (profile.id.clone(), profile.windowed_hotkey.clone()))
                {
                    state.settings.profiles.active_profile_id = profile_id;
                    state.settings.hotkeys.windowed_toggle = hotkey;
                }
            }
            show_profile_rename_edit(state);
        }
        ID_ADD_PROFILE if code == BN_CLICKED => {
            add_profile(state);
            let _ = save_settings(state);
            push_event(SettingsUiEvent::HotkeysChanged);
            push_event(SettingsUiEvent::ProfileChanged);
            refresh_all_controls(state);
            layout_profile_buttons_for_state(state);
        }
        ID_DELETE_PROFILE if code == BN_CLICKED => {
            if delete_selected_profile(state) {
                let _ = save_settings(state);
                push_event(SettingsUiEvent::HotkeysChanged);
                push_event(SettingsUiEvent::ProfileChanged);
                refresh_all_controls(state);
                layout_profile_buttons_for_state(state);
            }
        }
        ID_NAME_EDIT if code == EN_KILLFOCUS => {
            commit_profile_name_edit(state, true);
        }
        ID_NAME_EDIT if code == EN_CHANGE => {
            if rename_edit_visible(state) {
                state.pending_profile_name = Some(get_text(get(hwnd, ID_NAME_EDIT)));
            }
        }
        ID_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::WindowScale)
        }
        ID_HOTKEY_MOD_PRIMARY if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::WindowScale)
        }
        ID_POINTER_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerMagnifier)
        }
        ID_POINTER_HOTKEY_VALUE if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerMagnifier)
        }
        ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::WindowScreenshot)
        }
        ID_WINDOW_SCREENSHOT_HOTKEY_VALUE if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::WindowScreenshot)
        }
        ID_POINTER_SCREENSHOT_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerScreenshot)
        }
        ID_POINTER_SCREENSHOT_HOTKEY_VALUE if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerScreenshot)
        }
        ID_POINTER_COLOR_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerColorCode)
        }
        ID_POINTER_COLOR_HOTKEY_VALUE if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerColorCode)
        }
        ID_POINTER_COLOR_COPY_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerColorCodeCopy)
        }
        ID_POINTER_COLOR_COPY_HOTKEY_VALUE if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerColorCodeCopy)
        }
        ID_POINTER_CURSOR_HOTKEY_CHANGE if code == BN_CLICKED => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerCursor)
        }
        ID_POINTER_CURSOR_HOTKEY_VALUE if code == STN_CLICKED_NOTIFY => {
            show_hotkey_panel_for(state, HotkeyEditTarget::PointerCursor)
        }
        ID_SCALE_EDIT if code == EN_CHANGE => sanitize_scale_edit(state, false),
        ID_SCALE_EDIT if code == EN_KILLFOCUS => sanitize_scale_edit(state, true),
        ID_POINTER_WIDTH_EDIT if code == EN_CHANGE => {
            sanitize_pointer_numeric_edit(state, id, false)
        }
        ID_POINTER_WIDTH_EDIT if code == EN_KILLFOCUS => {
            sanitize_pointer_numeric_edit(state, id, true)
        }
        ID_POINTER_HEIGHT_EDIT if code == EN_CHANGE => {
            sanitize_pointer_numeric_edit(state, id, false)
        }
        ID_POINTER_HEIGHT_EDIT if code == EN_KILLFOCUS => {
            sanitize_pointer_numeric_edit(state, id, true)
        }
        ID_POINTER_SCALE_EDIT if code == EN_CHANGE => {
            sanitize_pointer_numeric_edit(state, id, false)
        }
        ID_POINTER_SCALE_EDIT if code == EN_KILLFOCUS => {
            sanitize_pointer_numeric_edit(state, id, true)
        }
        ID_WINDOW_SCREENSHOT_PATH_EDIT if code == EN_CHANGE => {
            apply_screenshot_path_edit(state, id)
        }
        ID_SCALE_UP if code == BN_CLICKED => adjust_scale(state, 10),
        ID_SCALE_DOWN if code == BN_CLICKED => adjust_scale(state, -10),
        ID_POINTER_SCALE_UP if code == BN_CLICKED => adjust_pointer_scale(state, 10),
        ID_POINTER_SCALE_DOWN if code == BN_CLICKED => adjust_pointer_scale(state, -10),
        ID_POINTER_COLOR_TOGGLE if code == BN_CLICKED => {
            if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
                profile.pointer_color_code_enabled = !profile.pointer_color_code_enabled;
                let _ = save_settings(state);
                push_event(SettingsUiEvent::ProfileChanged);
                refresh_profile_controls(state);
            }
        }
        ID_POINTER_CURSOR_TOGGLE if code == BN_CLICKED => {
            if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
                profile.draw_cursor = !profile.draw_cursor;
                let _ = save_settings(state);
                push_event(SettingsUiEvent::ProfileChanged);
                refresh_profile_controls(state);
            }
        }
        ID_SETTINGS_BUTTON if code == BN_CLICKED => toggle_settings_panel(state),
        ID_TRAY_BUTTON if code == BN_CLICKED => hide_to_tray(hwnd),
        ID_SETTINGS_CLOSE if code == BN_CLICKED => show_settings_panel(state, false),
        ID_LANGUAGE_COMBO if code == BN_CLICKED => toggle_language_menu(state),
        ID_LANGUAGE_MENU if code == LBN_SELCHANGE || code == LBN_DBLCLK => {
            apply_language_menu(state)
        }
        ID_RESET_BUTTON if code == BN_CLICKED => reset_settings(hwnd, state),
        ID_LOG_BUTTON if code == BN_CLICKED => push_event(SettingsUiEvent::LogOutputRequested),
        ID_HOTKEY_APPLY if code == BN_CLICKED => apply_pending_hotkey(state),
        ID_HOTKEY_CANCEL if code == BN_CLICKED => show_hotkey_panel(state, false),
        _ => {}
    }
}

fn handle_keydown(hwnd: HWND, vk: u32) -> bool {
    let Ok(mut slot) = state_slot().try_lock() else {
        return false;
    };
    let Some(state) = slot.as_mut() else {
        return false;
    };
    if !state.hotkey_panel_visible {
        if rename_edit_visible(state) {
            match vk {
                0x0D => {
                    commit_profile_name_edit(state, true);
                    return true;
                }
                0x1B => {
                    state.pending_profile_name = None;
                    show_child(hwnd, ID_NAME_EDIT, false);
                    set_child_enabled(hwnd, ID_PROFILE_LIST, true);
                    layout_profile_buttons_for_state(state);
                    return true;
                }
                _ => {}
            }
        }
        return false;
    }
    match vk {
        0x1B => {
            show_hotkey_panel(state, false);
            true
        }
        0x0D => {
            apply_pending_hotkey(state);
            true
        }
        vk if is_modifier_key(vk) => true,
        _ => {
            if let Some(hotkey) = format_hotkey_from_vk(vk) {
                state.pending_hotkey = Some(hotkey.clone());
                refresh_hotkey_panel_texts(state);
                invalidate(hwnd);
                true
            } else {
                false
            }
        }
    }
}

fn poll_rename_edit_keys(hwnd: HWND) {
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.hwnd != raw_from_hwnd(hwnd) || state.loading || !rename_edit_visible(state) {
        state.rename_enter_down = false;
        state.rename_escape_down = false;
        return;
    }
    let edit = get(hwnd, ID_NAME_EDIT);
    if unsafe { GetFocus() } != edit {
        return;
    }
    let enter_down = key_down(0x0D);
    if enter_down && !state.rename_enter_down {
        commit_profile_name_edit(state, true);
    }
    state.rename_enter_down = enter_down;

    let escape_down = key_down(0x1B);
    if escape_down && !state.rename_escape_down {
        state.pending_profile_name = None;
        state.loading = true;
        show_child(hwnd, ID_NAME_EDIT, false);
        set_child_enabled(hwnd, ID_PROFILE_LIST, true);
        state.loading = false;
        layout_profile_buttons_for_state(state);
    }
    state.rename_escape_down = escape_down;
}

fn refresh_all_controls(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    set_redraw(hwnd, false);
    refresh_localized_texts(state);
    refresh_profile_list(state);
    refresh_profile_controls(state);
    refresh_global_controls(state);
    show_settings_panel(state, state.settings_panel_visible);
    show_hotkey_panel(state, state.hotkey_panel_visible);
    layout_profile_buttons_for_state(state);
    set_redraw(hwnd, true);
    unsafe {
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
        );
    }
}

fn refresh_localized_texts(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let lang = state.settings.ui.language.as_str();
    let title = wide_null(ui_text(lang, UiString::WindowTitle));
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(title.as_ptr()));
    }
    set_text(
        get(hwnd, ID_PROFILE_TITLE),
        ui_text(lang, UiString::Profiles),
    );
    set_text(get(hwnd, ID_ADD_PROFILE), "+");
    set_text(get(hwnd, ID_DELETE_PROFILE), "-");
    set_text(
        get(hwnd, ID_HOTKEY_LABEL),
        ui_text(lang, UiString::WindowScaling),
    );
    set_text(get(hwnd, ID_HOTKEY_CHANGE), ui_text(lang, UiString::Change));
    set_text(
        get(hwnd, ID_SCALE_LABEL),
        ui_text(lang, UiString::WindowScalePercent),
    );
    set_text(
        get(hwnd, ID_POINTER_LABEL),
        ui_text(lang, UiString::PointerMagnifier),
    );
    set_text(
        get(hwnd, ID_POINTER_HOTKEY_CHANGE),
        ui_text(lang, UiString::Change),
    );
    set_text(
        get(hwnd, ID_POINTER_RANGE_LABEL),
        ui_text(lang, UiString::Range),
    );
    set_text(
        get(hwnd, ID_POINTER_SCALE_LABEL),
        ui_text(lang, UiString::PointerScalePercent),
    );
    set_text(
        get(hwnd, ID_POINTER_RANGE_HELP),
        ui_text(lang, UiString::PointerRangeHelp),
    );
    set_text(
        get(hwnd, ID_SCREENSHOT_TITLE),
        ui_text(lang, UiString::ScreenshotStorage),
    );
    set_text(
        get(hwnd, ID_WINDOW_SCREENSHOT_LABEL),
        ui_text(lang, UiString::WindowScreenshot),
    );
    let screenshot_path_label = if lang == "en" {
        "Save path"
    } else {
        "\u{c800}\u{c7a5} \u{acbd}\u{b85c}"
    };
    set_text(
        get(hwnd, ID_WINDOW_SCREENSHOT_PATH_LABEL),
        screenshot_path_label,
    );
    set_text(
        get(hwnd, ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE),
        ui_text(lang, UiString::Change),
    );
    set_text(
        get(hwnd, ID_POINTER_SCREENSHOT_LABEL),
        ui_text(lang, UiString::PointerScreenshot),
    );
    set_text(
        get(hwnd, ID_POINTER_SCREENSHOT_PATH_LABEL),
        ui_text(lang, UiString::PointerScreenshotPath),
    );
    set_text(
        get(hwnd, ID_POINTER_SCREENSHOT_HOTKEY_CHANGE),
        ui_text(lang, UiString::Change),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_LABEL),
        ui_text(lang, UiString::PointerColorCode),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_HOTKEY_CHANGE),
        ui_text(lang, UiString::Change),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_COPY_LABEL),
        ui_text(lang, UiString::PointerColorCodeCopy),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_COPY_HOTKEY_CHANGE),
        ui_text(lang, UiString::Change),
    );
    set_text(
        get(hwnd, ID_POINTER_CURSOR_LABEL),
        ui_text(lang, UiString::PointerCursor),
    );
    set_text(
        get(hwnd, ID_POINTER_CURSOR_HOTKEY_CHANGE),
        ui_text(lang, UiString::Change),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_TOGGLE_LABEL),
        ui_text(lang, UiString::ColorCodeToggle),
    );
    set_text(
        get(hwnd, ID_POINTER_CURSOR_TOGGLE_LABEL),
        ui_text(lang, UiString::CursorToggle),
    );
    set_text(
        get(hwnd, ID_WINDOW_SCREENSHOT_BROWSE),
        ui_text(lang, UiString::Browse),
    );
    set_text(
        get(hwnd, ID_POINTER_SCREENSHOT_BROWSE),
        ui_text(lang, UiString::Browse),
    );
    set_text(
        get(hwnd, ID_SETTINGS_PANEL_TITLE),
        ui_text(lang, UiString::Settings),
    );
    set_text(
        get(hwnd, ID_SETTINGS_LANGUAGE_LABEL),
        ui_text(lang, UiString::Language),
    );
    set_text(
        get(hwnd, ID_RESET_BUTTON),
        ui_text(lang, UiString::ResetDefaults),
    );
    set_text(get(hwnd, ID_LOG_BUTTON), ui_text(lang, UiString::LogOutput));
    set_text(get(hwnd, ID_SETTINGS_CLOSE), ui_text(lang, UiString::Close));
    set_text(get(hwnd, ID_HOTKEY_APPLY), ui_text(lang, UiString::Apply));
    set_text(get(hwnd, ID_HOTKEY_CANCEL), ui_text(lang, UiString::Cancel));
    set_text(
        get(hwnd, ID_HOTKEY_PANEL_TITLE),
        ui_text(lang, UiString::HotkeyChange),
    );
    set_text(
        get(hwnd, ID_HOTKEY_HELP),
        ui_text(lang, UiString::HotkeyHelp),
    );
    set_text(
        get(hwnd, ID_HOTKEY_CURRENT_LABEL),
        ui_text(lang, UiString::CurrentHotkey),
    );
    set_text(
        get(hwnd, ID_HOTKEY_NEW_LABEL),
        ui_text(lang, UiString::NewHotkey),
    );
    refresh_hotkey_panel_texts(state);
    invalidate(hwnd);
}

fn refresh_hotkey_panel_texts(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let current = current_hotkey_for_target(state);
    let pending = state
        .pending_hotkey
        .clone()
        .unwrap_or_else(|| current.clone());
    set_text(
        get(hwnd, ID_HOTKEY_CURRENT_VALUE),
        &format_hotkey_display(&current),
    );
    set_text(
        get(hwnd, ID_HOTKEY_NEW_VALUE),
        &format_hotkey_display(&pending),
    );
}

fn refresh_profile_list(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let list = get(hwnd, ID_PROFILE_LIST);
    state.loading = true;
    set_redraw(list, false);
    let _ = send(list, LB_RESETCONTENT, 0, 0);
    for profile in profiles(&state.settings) {
        let name = wide_null(&profile.display_name);
        let _ = send(list, LB_ADDSTRING, 0, name.as_ptr() as isize);
    }
    let _ = send(list, LB_SETCURSEL, state.selected_index, 0);
    let layout = current_layout(hwnd);
    let profile_count = profiles(&state.settings).len();
    if state
        .hovered_profile_index
        .is_some_and(|index| index >= profile_count)
    {
        state.hovered_profile_index = None;
    }
    let list_rect = profile_list_rect(&layout, profile_count);
    let visible_rows = ((list_rect.bottom - list_rect.top) / PROFILE_ROW_HEIGHT).max(1) as usize;
    let top_index = state
        .selected_index
        .saturating_add(1)
        .saturating_sub(visible_rows);
    let _ = send(list, LB_SETTOPINDEX_MSG, top_index, 0);
    set_redraw(list, true);
    unsafe {
        let _ = RedrawWindow(Some(list), None, None, RDW_INVALIDATE | RDW_UPDATENOW);
    }
    state.loading = false;
}

fn refresh_profile_controls(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let Some(profile) = profile_at(&state.settings, state.selected_index).cloned() else {
        return;
    };
    state.loading = true;
    set_text(get(hwnd, ID_NAME_EDIT), &profile.display_name);
    set_text(
        get(hwnd, ID_HOTKEY_MOD_PRIMARY),
        &format_hotkey_display(&profile.windowed_hotkey),
    );
    set_text(get(hwnd, ID_HOTKEY_MOD_SECONDARY), "");
    set_text(get(hwnd, ID_HOTKEY_KEY), "");
    set_text(
        get(hwnd, ID_SCALE_EDIT),
        &profile.windowed_scale_percent.to_string(),
    );
    set_text(
        get(hwnd, ID_POINTER_HOTKEY_VALUE),
        &format_hotkey_display(&state.settings.hotkeys.pointer_magnifier_toggle),
    );
    set_text(
        get(hwnd, ID_POINTER_WIDTH_EDIT),
        &profile.pointer_magnifier_width.to_string(),
    );
    set_text(
        get(hwnd, ID_POINTER_HEIGHT_EDIT),
        &profile.pointer_magnifier_height.to_string(),
    );
    set_text(
        get(hwnd, ID_POINTER_SCALE_EDIT),
        &profile.pointer_magnifier_scale_percent.to_string(),
    );
    set_text(
        get(hwnd, ID_WINDOW_SCREENSHOT_HOTKEY_VALUE),
        &format_hotkey_display(&state.settings.hotkeys.screenshot),
    );
    set_text(
        get(hwnd, ID_POINTER_SCREENSHOT_HOTKEY_VALUE),
        &format_hotkey_display(&state.settings.hotkeys.pointer_screenshot),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_HOTKEY_VALUE),
        &format_hotkey_display(&state.settings.hotkeys.pointer_color_code_toggle),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_COPY_HOTKEY_VALUE),
        &format_hotkey_display(&state.settings.hotkeys.pointer_color_code_copy),
    );
    set_text(
        get(hwnd, ID_POINTER_CURSOR_HOTKEY_VALUE),
        &format_hotkey_display(&state.settings.hotkeys.pointer_cursor_toggle),
    );
    set_text(
        get(hwnd, ID_POINTER_COLOR_TOGGLE),
        ui_text(
            state.settings.ui.language.as_str(),
            if profile.pointer_color_code_enabled {
                UiString::ToggleOn
            } else {
                UiString::ToggleOff
            },
        ),
    );
    set_text(
        get(hwnd, ID_POINTER_CURSOR_TOGGLE),
        ui_text(
            state.settings.ui.language.as_str(),
            if profile.draw_cursor {
                UiString::ToggleOn
            } else {
                UiString::ToggleOff
            },
        ),
    );
    let screenshot_dir = if state.settings.screenshots.window_dir.trim().is_empty() {
        state.settings.screenshots.pointer_dir.as_str()
    } else {
        state.settings.screenshots.window_dir.as_str()
    };
    set_text(
        get(hwnd, ID_WINDOW_SCREENSHOT_PATH_EDIT),
        &screenshot_path_display_text(screenshot_dir),
    );
    show_child(hwnd, ID_DELETE_PROFILE, false);
    set_child_enabled(hwnd, ID_DELETE_PROFILE, false);
    state.loading = false;
}

fn show_profile_rename_edit(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let Some(profile) = profile_at(&state.settings, state.selected_index).cloned() else {
        return;
    };
    let layout = current_layout(hwnd);
    let profile_count = profiles(&state.settings).len();
    let row_rect = profile_item_rect_in_parent(hwnd, state.selected_index).unwrap_or_else(|| {
        fallback_profile_item_rect(&layout, profile_count, state.selected_index)
    });
    let row_frame = inset_rect(row_rect, 2, 2);
    let delete_reserve = if state.selected_index > 0 {
        PROFILE_DELETE_W + PROFILE_DELETE_GAP + PROFILE_DELETE_RIGHT_PAD
    } else {
        0
    };
    let edit_left = row_frame.left + 8;
    let edit_top = row_frame.top + 2;
    let edit_right = row_frame.right - delete_reserve;
    let edit_w = (edit_right - edit_left).max(64);
    let edit_h = (row_frame.bottom - row_frame.top - 4).max(18);
    let edit = get(hwnd, ID_NAME_EDIT);
    state.loading = true;
    set_child_enabled(hwnd, ID_PROFILE_LIST, true);
    show_child(hwnd, ID_DELETE_PROFILE, false);
    unsafe {
        let _ = SetWindowPos(
            edit,
            Some(HWND_TOP),
            edit_left,
            edit_top,
            edit_w,
            edit_h,
            SET_WINDOW_POS_FLAGS(SWP_NOACTIVATE.0),
        );
    }
    unsafe {
        let _ = RedrawWindow(
            Some(get(hwnd, ID_PROFILE_LIST)),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
        );
    }
    show_child(hwnd, ID_NAME_EDIT, true);
    raise_child(hwnd, ID_NAME_EDIT);
    state.loading = false;
    invalidate_sidebar(hwnd, &layout);
    unsafe {
        let _ = SetFocus(Some(edit));
    }
    set_text(edit, &profile.display_name);
    unsafe {
        let _ = RedrawWindow(
            Some(edit),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
        );
        let _ = UpdateWindow(edit);
    }
    state.pending_profile_name = Some(profile.display_name);
    let caret = get_text(edit).encode_utf16().count() as isize;
    let _ = send(edit, EM_SETSEL_MSG, caret as usize, caret);
}

fn rename_edit_visible(state: &SettingsUiState) -> bool {
    unsafe { IsWindowVisible(get(hwnd_from_raw(state.hwnd), ID_NAME_EDIT)).as_bool() }
}

fn commit_profile_name_edit_for_external_click(hwnd: HWND) {
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if !rename_edit_visible(state) || cursor_over_child(hwnd, ID_NAME_EDIT) {
        return;
    }
    commit_profile_name_edit(state, true);
}

fn cursor_over_child(parent: HWND, id: i32) -> bool {
    let child = get(parent, id);
    if child.0.is_null() {
        return false;
    }
    let mut point = POINT::default();
    let mut rect = RECT::default();
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return false;
    }
    if unsafe { GetWindowRect(child, &mut rect) }.is_err() {
        return false;
    }
    rect_contains_point(rect, point.x, point.y)
}

fn rect_contains_point(rect: RECT, x: i32, y: i32) -> bool {
    x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom
}

fn commit_profile_name_edit(state: &mut SettingsUiState, hide: bool) {
    if !rename_edit_visible(state) {
        return;
    }
    let hwnd = hwnd_from_raw(state.hwnd);
    let edit = get(hwnd, ID_NAME_EDIT);
    let name = state
        .pending_profile_name
        .clone()
        .unwrap_or_else(|| get_text(edit))
        .trim()
        .to_string();
    if !name.is_empty() {
        if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
            if profile.display_name != name {
                profile.display_name = name;
                let _ = save_settings(state);
                refresh_profile_list(state);
            }
        }
    }
    if hide {
        state.pending_profile_name = None;
        state.loading = true;
        show_child(hwnd, ID_NAME_EDIT, false);
        set_child_enabled(
            hwnd,
            ID_PROFILE_LIST,
            !(state.settings_panel_visible || state.hotkey_panel_visible),
        );
        state.loading = false;
        layout_profile_buttons_for_state(state);
    }
}

fn refresh_global_controls(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let button = get(hwnd, ID_LANGUAGE_COMBO);
    let menu = get(hwnd, ID_LANGUAGE_MENU);
    state.loading = true;
    let korean = wide_null("\u{d55c}\u{ad6d}\u{c5b4}");
    let english = wide_null("English");
    let _ = send(menu, LB_RESETCONTENT, 0, 0);
    let _ = send(menu, LB_ADDSTRING, 0, korean.as_ptr() as isize);
    let _ = send(menu, LB_ADDSTRING, 0, english.as_ptr() as isize);
    let selected = if state.settings.ui.language.eq_ignore_ascii_case("en") {
        1
    } else {
        0
    };
    let _ = send(menu, LB_SETCURSEL, selected, 0);
    set_text(
        button,
        if selected == 1 {
            "English"
        } else {
            "\u{d55c}\u{ad6d}\u{c5b4}"
        },
    );
    show_child(
        hwnd,
        ID_LANGUAGE_MENU,
        state.settings_panel_visible && state.language_menu_visible,
    );
    state.loading = false;
}

fn show_settings_panel(state: &mut SettingsUiState, visible: bool) {
    if !visible && state.settings_panel_visible {
        apply_screenshot_path_edit(state, ID_WINDOW_SCREENSHOT_PATH_EDIT);
    }
    state.settings_panel_visible = visible;
    SETTINGS_PANEL_PAINT_VISIBLE.store(visible, Ordering::Relaxed);
    let hwnd = hwnd_from_raw(state.hwnd);
    if visible {
        state.hotkey_panel_visible = false;
        HOTKEY_PANEL_PAINT_VISIBLE.store(false, Ordering::Relaxed);
        state.pending_hotkey = None;
        for id in hotkey_panel_ids() {
            show_child(hwnd, *id, false);
        }
    } else {
        state.language_menu_visible = false;
    }
    for id in settings_panel_ids() {
        show_child(hwnd, *id, visible);
    }
    show_child(
        hwnd,
        ID_LANGUAGE_MENU,
        visible && state.language_menu_visible,
    );
    if visible {
        raise_panel_children(hwnd, settings_panel_ids());
        if state.language_menu_visible {
            raise_child(hwnd, ID_LANGUAGE_MENU);
        }
    }
    update_modal_base_enabled(state);
    refresh_localized_texts(state);
    invalidate(hwnd);
}

fn toggle_settings_panel(state: &mut SettingsUiState) {
    let visible = !state.settings_panel_visible;
    show_settings_panel(state, visible);
}

fn toggle_language_menu(state: &mut SettingsUiState) {
    if !state.settings_panel_visible {
        return;
    }
    state.language_menu_visible = !state.language_menu_visible;
    let hwnd = hwnd_from_raw(state.hwnd);
    show_child(hwnd, ID_LANGUAGE_MENU, state.language_menu_visible);
    if state.language_menu_visible {
        raise_child(hwnd, ID_LANGUAGE_MENU);
    }
    refresh_global_controls(state);
    invalidate(hwnd);
}

fn apply_language_menu(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let selection = send(get(hwnd, ID_LANGUAGE_MENU), LB_GETCURSEL, 0, 0);
    if selection < 0 {
        return;
    }
    state.settings.ui.language = if selection == 1 { "en" } else { "ko" }.to_string();
    state.language_menu_visible = false;
    let _ = save_settings(state);
    refresh_localized_texts(state);
    refresh_global_controls(state);
    show_child(hwnd, ID_LANGUAGE_MENU, false);
    push_event(SettingsUiEvent::GlobalSettingsChanged);
}

fn current_hotkey_for_target(state: &SettingsUiState) -> String {
    match state.pending_hotkey_target {
        HotkeyEditTarget::WindowScale => profile_at(&state.settings, state.selected_index)
            .map(|profile| profile.windowed_hotkey.clone())
            .unwrap_or_else(|| state.settings.hotkeys.windowed_toggle.clone()),
        HotkeyEditTarget::PointerMagnifier => {
            state.settings.hotkeys.pointer_magnifier_toggle.clone()
        }
        HotkeyEditTarget::WindowScreenshot => state.settings.hotkeys.screenshot.clone(),
        HotkeyEditTarget::PointerScreenshot => state.settings.hotkeys.pointer_screenshot.clone(),
        HotkeyEditTarget::PointerColorCode => {
            state.settings.hotkeys.pointer_color_code_toggle.clone()
        }
        HotkeyEditTarget::PointerColorCodeCopy => {
            state.settings.hotkeys.pointer_color_code_copy.clone()
        }
        HotkeyEditTarget::PointerCursor => state.settings.hotkeys.pointer_cursor_toggle.clone(),
    }
}

fn show_hotkey_panel_for(state: &mut SettingsUiState, target: HotkeyEditTarget) {
    state.pending_hotkey_target = target;
    show_hotkey_panel(state, true);
}

fn show_hotkey_panel(state: &mut SettingsUiState, visible: bool) {
    state.hotkey_panel_visible = visible;
    HOTKEY_PANEL_PAINT_VISIBLE.store(visible, Ordering::Relaxed);
    let hwnd = hwnd_from_raw(state.hwnd);
    if visible {
        if state.settings_panel_visible {
            apply_screenshot_path_edit(state, ID_WINDOW_SCREENSHOT_PATH_EDIT);
        }
        state.settings_panel_visible = false;
        SETTINGS_PANEL_PAINT_VISIBLE.store(false, Ordering::Relaxed);
        state.language_menu_visible = false;
        for id in settings_panel_ids() {
            show_child(hwnd, *id, false);
        }
        show_child(hwnd, ID_LANGUAGE_MENU, false);
        state.pending_hotkey = Some(current_hotkey_for_target(state));
    } else {
        state.pending_hotkey = None;
    }
    for id in hotkey_panel_ids() {
        show_child(hwnd, *id, visible);
    }
    if visible {
        raise_panel_children(hwnd, hotkey_panel_ids());
    }
    update_modal_base_enabled(state);
    refresh_localized_texts(state);
    refresh_hotkey_panel_texts(state);
    invalidate(hwnd);
    if visible {
        unsafe {
            let _ = SetForegroundWindow(hwnd);
            let _ = SetFocus(Some(hwnd));
        }
    }
}

fn apply_pending_hotkey(state: &mut SettingsUiState) {
    let Some(hotkey) = state.pending_hotkey.clone() else {
        return;
    };
    match state.pending_hotkey_target {
        HotkeyEditTarget::WindowScale => {
            if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
                profile.windowed_hotkey = hotkey.clone();
            }
            state.settings.hotkeys.windowed_toggle = hotkey;
        }
        HotkeyEditTarget::PointerMagnifier => {
            state.settings.hotkeys.pointer_magnifier_toggle = hotkey;
        }
        HotkeyEditTarget::WindowScreenshot => {
            state.settings.hotkeys.screenshot = hotkey;
        }
        HotkeyEditTarget::PointerScreenshot => {
            state.settings.hotkeys.pointer_screenshot = hotkey;
        }
        HotkeyEditTarget::PointerColorCode => {
            state.settings.hotkeys.pointer_color_code_toggle = hotkey;
        }
        HotkeyEditTarget::PointerColorCodeCopy => {
            state.settings.hotkeys.pointer_color_code_copy = hotkey;
        }
        HotkeyEditTarget::PointerCursor => {
            state.settings.hotkeys.pointer_cursor_toggle = hotkey;
        }
    }
    let _ = save_settings(state);
    show_hotkey_panel(state, false);
    refresh_profile_controls(state);
    push_event(SettingsUiEvent::HotkeysChanged);
}

fn sanitize_scale_edit(state: &mut SettingsUiState, commit: bool) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let edit = get(hwnd, ID_SCALE_EDIT);
    let raw = get_text(edit);
    let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits != raw {
        replace_scale_edit_text(state, &digits);
        return;
    }
    if digits.is_empty() {
        if commit {
            restore_scale_edit_text(state);
        }
        return;
    }
    let Ok(mut value) = digits.parse::<u32>() else {
        restore_scale_edit_text(state);
        return;
    };
    if value > 1000 {
        value = 1000;
        replace_scale_edit_text(state, &value.to_string());
    }
    if value < 50 {
        if commit {
            value = 50;
            replace_scale_edit_text(state, &value.to_string());
        } else {
            return;
        }
    }
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        if profile.windowed_scale_percent != value {
            profile.windowed_scale_percent = value;
            let _ = save_settings(state);
            push_event(SettingsUiEvent::ProfileChanged);
        }
    }
}

fn replace_scale_edit_text(state: &mut SettingsUiState, text: &str) {
    let edit = get(hwnd_from_raw(state.hwnd), ID_SCALE_EDIT);
    state.loading = true;
    set_text(edit, text);
    let len = text.encode_utf16().count();
    let _ = send(edit, EM_SETSEL_MSG, len, len as isize);
    state.loading = false;
}

fn restore_scale_edit_text(state: &mut SettingsUiState) {
    let text = profile_at(&state.settings, state.selected_index)
        .map(|profile| profile.windowed_scale_percent.to_string())
        .unwrap_or_else(|| "200".to_string());
    replace_scale_edit_text(state, &text);
}

fn sanitize_pointer_numeric_edit(state: &mut SettingsUiState, id: i32, commit: bool) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let edit = get(hwnd, id);
    let raw = get_text(edit);
    let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits != raw {
        replace_pointer_numeric_edit_text(state, id, &digits);
        return;
    }
    if digits.is_empty() {
        if commit {
            restore_pointer_numeric_edit_text(state, id);
        }
        return;
    }
    let Some((min, max)) = pointer_numeric_bounds(id) else {
        return;
    };
    let Ok(mut value) = digits.parse::<u32>() else {
        restore_pointer_numeric_edit_text(state, id);
        return;
    };
    if value > max {
        value = max;
        replace_pointer_numeric_edit_text(state, id, &value.to_string());
    }
    if value < min {
        if commit {
            value = min;
            replace_pointer_numeric_edit_text(state, id, &value.to_string());
        } else {
            return;
        }
    }
    let mut changed = false;
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        let slot = match id {
            ID_POINTER_WIDTH_EDIT => &mut profile.pointer_magnifier_width,
            ID_POINTER_HEIGHT_EDIT => &mut profile.pointer_magnifier_height,
            ID_POINTER_SCALE_EDIT => &mut profile.pointer_magnifier_scale_percent,
            _ => return,
        };
        if *slot != value {
            *slot = value;
            changed = true;
        }
    }
    if changed {
        let _ = save_settings(state);
        push_event(SettingsUiEvent::ProfileChanged);
    }
}

fn pointer_numeric_bounds(id: i32) -> Option<(u32, u32)> {
    match id {
        ID_POINTER_WIDTH_EDIT => Some((1, 1200)),
        ID_POINTER_HEIGHT_EDIT => Some((1, 900)),
        ID_POINTER_SCALE_EDIT => Some((50, 1000)),
        _ => None,
    }
}

fn replace_pointer_numeric_edit_text(state: &mut SettingsUiState, id: i32, text: &str) {
    let edit = get(hwnd_from_raw(state.hwnd), id);
    state.loading = true;
    set_text(edit, text);
    let len = text.encode_utf16().count();
    let _ = send(edit, EM_SETSEL_MSG, len, len as isize);
    state.loading = false;
}

fn restore_pointer_numeric_edit_text(state: &mut SettingsUiState, id: i32) {
    let text = profile_at(&state.settings, state.selected_index)
        .map(|profile| match id {
            ID_POINTER_WIDTH_EDIT => profile.pointer_magnifier_width,
            ID_POINTER_HEIGHT_EDIT => profile.pointer_magnifier_height,
            ID_POINTER_SCALE_EDIT => profile.pointer_magnifier_scale_percent,
            _ => 0,
        })
        .filter(|value| *value > 0)
        .unwrap_or_else(|| match id {
            ID_POINTER_WIDTH_EDIT => 100,
            ID_POINTER_HEIGHT_EDIT => 100,
            ID_POINTER_SCALE_EDIT => 200,
            _ => 0,
        })
        .to_string();
    replace_pointer_numeric_edit_text(state, id, &text);
}

fn apply_screenshot_path_edit(state: &mut SettingsUiState, id: i32) {
    if state.loading || id != ID_WINDOW_SCREENSHOT_PATH_EDIT {
        return;
    }
    let hwnd = hwnd_from_raw(state.hwnd);
    let text = get_text(get(hwnd, id));
    let normalized = if text.trim() == program_root_dir_text().trim() {
        String::new()
    } else {
        text
    };
    if state.settings.screenshots.window_dir != normalized
        || state.settings.screenshots.pointer_dir != normalized
    {
        state.settings.screenshots.window_dir = normalized.clone();
        state.settings.screenshots.pointer_dir = normalized;
        let _ = save_settings(state);
        push_event(SettingsUiEvent::GlobalSettingsChanged);
    }
}

fn browse_screenshot_folder(hwnd: HWND, trigger_id: i32) {
    let Some(folder) = choose_folder(hwnd) else {
        unsafe {
            let _ = SetFocus(Some(hwnd));
        }
        return;
    };
    let edit_id = match trigger_id {
        ID_WINDOW_SCREENSHOT_BROWSE | ID_WINDOW_SCREENSHOT_PATH_EDIT => {
            ID_WINDOW_SCREENSHOT_PATH_EDIT
        }
        _ => return,
    };
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.hwnd != raw_from_hwnd(hwnd) {
        return;
    }
    state.loading = true;
    set_text(get(hwnd, edit_id), &folder);
    state.loading = false;
    if edit_id == ID_WINDOW_SCREENSHOT_PATH_EDIT {
        state.settings.screenshots.window_dir = folder.clone();
        state.settings.screenshots.pointer_dir = folder;
    }
    let _ = save_settings(state);
    push_event(SettingsUiEvent::GlobalSettingsChanged);
    unsafe {
        let _ = SetFocus(Some(hwnd));
    }
}

fn choose_folder(owner: HWND) -> Option<String> {
    let title = wide_null(
        "\u{c2a4}\u{d06c}\u{b9b0}\u{c0f7} \u{c800}\u{c7a5} \u{d3f4}\u{b354} \u{c120}\u{d0dd}",
    );
    let mut display_name = [0u16; 260];
    let mut info = BROWSEINFOW {
        hwndOwner: owner,
        pszDisplayName: PWSTR(display_name.as_mut_ptr()),
        lpszTitle: PCWSTR(title.as_ptr()),
        ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
        ..Default::default()
    };
    let pidl = unsafe { SHBrowseForFolderW(&mut info) };
    if pidl.is_null() {
        return None;
    }
    let mut path = [0u16; 260];
    let ok = unsafe { SHGetPathFromIDListW(pidl, &mut path).as_bool() };
    unsafe {
        CoTaskMemFree(Some(pidl as *const _));
    }
    if !ok {
        return None;
    }
    let len = path.iter().position(|ch| *ch == 0).unwrap_or(path.len());
    if len == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&path[..len]))
}

fn screenshot_path_display_text(raw: &str) -> String {
    if raw.trim().is_empty() {
        program_root_dir_text()
    } else {
        raw.to_string()
    }
}

fn program_root_dir_text() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .display()
        .to_string()
}

fn adjust_scale(state: &mut SettingsUiState, delta: i32) {
    let value = profile_at(&state.settings, state.selected_index)
        .map(|profile| profile.windowed_scale_percent as i32)
        .unwrap_or(200);
    let next = (value + delta).clamp(50, 1000) as u32;
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        profile.windowed_scale_percent = next;
    }
    let _ = save_settings(state);
    refresh_profile_controls(state);
    push_event(SettingsUiEvent::ProfileChanged);
}

fn adjust_pointer_scale(state: &mut SettingsUiState, delta: i32) {
    let value = profile_at(&state.settings, state.selected_index)
        .map(|profile| profile.pointer_magnifier_scale_percent as i32)
        .unwrap_or(200);
    let next = (value + delta).clamp(50, 1000) as u32;
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        profile.pointer_magnifier_scale_percent = next;
    }
    let _ = save_settings(state);
    refresh_profile_controls(state);
    push_event(SettingsUiEvent::ProfileChanged);
}

fn reset_settings(hwnd: HWND, state: &mut SettingsUiState) {
    let text = wide_null(ui_text(
        &state.settings.ui.language,
        UiString::ResetQuestion,
    ));
    let title = wide_null("Dodbogi");
    let result = unsafe {
        MessageBoxW(
            Some(hwnd),
            PCWSTR(text.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_YESNO | MB_ICONQUESTION,
        )
    };
    if result != IDYES {
        return;
    }
    state.settings = DodbogiSettings::default();
    state.selected_index = 0;
    state.hovered_profile_index = None;
    let _ = save_settings(state);
    refresh_all_controls(state);
    push_event(SettingsUiEvent::HotkeysChanged);
    push_event(SettingsUiEvent::ProfileChanged);
    push_event(SettingsUiEvent::GlobalSettingsChanged);
}

fn add_profile(state: &mut SettingsUiState) {
    let mut next = 1usize;
    loop {
        let id = format!("profile-{next}");
        if profiles(&state.settings)
            .iter()
            .all(|profile| profile.id != id)
        {
            let mut profile = AppProfile::default_profile();
            profile.id = id.clone();
            profile.display_name = format!(
                "{} {next}",
                ui_text(&state.settings.ui.language, UiString::NewProfile)
            );
            profile.match_rule = dodbogi_core::ProfileMatchRule::empty();
            state.settings.profiles.per_app_profiles.push(profile);
            state.settings.profiles.active_profile_id = id;
            state.selected_index = state.settings.profiles.per_app_profiles.len();
            state.hovered_profile_index = None;
            if let Some(profile) = profile_at(&state.settings, state.selected_index) {
                state.settings.hotkeys.windowed_toggle = profile.windowed_hotkey.clone();
            }
            break;
        }
        next += 1;
    }
}

fn delete_selected_profile(state: &mut SettingsUiState) -> bool {
    delete_profile_at(state, state.selected_index)
}

fn delete_profile_at(state: &mut SettingsUiState, profile_index: usize) -> bool {
    if profile_index == 0 {
        return false;
    }
    let Some(removed_profile) = profile_at(&state.settings, profile_index).cloned() else {
        state.selected_index = selected_index_for_settings(&state.settings);
        state.hovered_profile_index = None;
        return false;
    };
    let removed_id = removed_profile.id;
    let removed_was_active = state.settings.profiles.active_profile_id == removed_id;
    let remove_index = profile_index - 1;
    if remove_index >= state.settings.profiles.per_app_profiles.len() {
        state.selected_index = selected_index_for_settings(&state.settings);
        state.hovered_profile_index = None;
        return false;
    }

    state
        .settings
        .profiles
        .per_app_profiles
        .remove(remove_index);
    let profile_count = profiles(&state.settings).len();
    state.hovered_profile_index = None;

    if profile_count == 0 {
        state.selected_index = 0;
    } else if profile_index < state.selected_index {
        state.selected_index = state.selected_index.saturating_sub(1);
    } else if profile_index == state.selected_index {
        state.selected_index = state.selected_index.min(profile_count - 1);
    } else {
        state.selected_index = state.selected_index.min(profile_count - 1);
    }

    if removed_was_active {
        activate_profile_at_index(state, state.selected_index);
    } else {
        let active_id = state.settings.profiles.active_profile_id.clone();
        if let Some(profile) = profile_by_id(&state.settings, &active_id) {
            state.settings.hotkeys.windowed_toggle = profile.windowed_hotkey.clone();
        } else {
            activate_profile_at_index(state, state.selected_index);
        }
    }
    true
}

fn activate_profile_at_index(state: &mut SettingsUiState, index: usize) {
    if let Some(profile) = profile_at(&state.settings, index).cloned() {
        state.settings.profiles.active_profile_id = profile.id;
        state.settings.hotkeys.windowed_toggle = profile.windowed_hotkey;
    } else {
        state.settings.profiles.active_profile_id =
            state.settings.profiles.default_profile.id.clone();
        state.settings.hotkeys.windowed_toggle = state
            .settings
            .profiles
            .default_profile
            .windowed_hotkey
            .clone();
        state.selected_index = 0;
    }
}

fn save_settings(state: &SettingsUiState) -> Result<(), String> {
    save_settings_to_path(&state.settings, &state.paths.settings_file)
        .map_err(|error| format!("settings save failed: {error}"))
}

fn normalize_loaded_settings(settings: &mut DodbogiSettings) -> bool {
    let mut changed = false;
    if settings.profiles.default_profile.display_name.trim() == "Default profile" {
        settings.profiles.default_profile.display_name = "疫꿸퀡???袁⑥쨮???뵬".to_string();
        changed = true;
    }
    if settings
        .profiles
        .default_profile
        .windowed_hotkey
        .trim()
        .is_empty()
    {
        settings.profiles.default_profile.windowed_hotkey = "Ctrl+Alt+Q".to_string();
        changed = true;
    }
    if settings.hotkeys.windowed_toggle.trim().is_empty()
        || settings.hotkeys.windowed_toggle.trim() == "Ctrl+Alt+M"
    {
        settings.hotkeys.windowed_toggle =
            settings.profiles.active_profile().windowed_hotkey.clone();
        changed = true;
    }
    if !(50..=1000).contains(&settings.profiles.default_profile.windowed_scale_percent) {
        settings.profiles.default_profile.windowed_scale_percent = 200;
        changed = true;
    }
    let screenshot_hotkey = settings.hotkeys.screenshot.trim();
    if screenshot_hotkey.is_empty() || screenshot_hotkey.eq_ignore_ascii_case("Ctrl+Alt+P") {
        settings.hotkeys.screenshot = "Shift+Alt+Q".to_string();
        changed = true;
    }
    if settings.hotkeys.pointer_magnifier_toggle.trim().is_empty() {
        settings.hotkeys.pointer_magnifier_toggle = "Ctrl+Alt+E".to_string();
        changed = true;
    }
    let pointer_screenshot_hotkey = settings.hotkeys.pointer_screenshot.trim();
    if pointer_screenshot_hotkey.is_empty()
        || pointer_screenshot_hotkey.eq_ignore_ascii_case("Ctrl+Alt+Shift+P")
    {
        settings.hotkeys.pointer_screenshot = "Shift+Alt+E".to_string();
        changed = true;
    }
    if settings.hotkeys.pointer_color_code_toggle.trim().is_empty() {
        settings.hotkeys.pointer_color_code_toggle = "Ctrl+Alt+C".to_string();
        changed = true;
    }
    if settings.hotkeys.pointer_color_code_copy.trim().is_empty() {
        settings.hotkeys.pointer_color_code_copy = "Shift+Alt+C".to_string();
        changed = true;
    }
    if settings.hotkeys.pointer_cursor_toggle.trim().is_empty() {
        settings.hotkeys.pointer_cursor_toggle = "Ctrl+Alt+R".to_string();
        changed = true;
    }
    changed |= normalize_profile_pointer_settings(&mut settings.profiles.default_profile);
    for profile in &mut settings.profiles.per_app_profiles {
        changed |= normalize_profile_pointer_settings(profile);
    }
    changed
}

fn normalize_profile_pointer_settings(profile: &mut AppProfile) -> bool {
    let mut changed = false;
    if profile.pointer_magnifier_width == 320 && profile.pointer_magnifier_height == 180 {
        profile.pointer_magnifier_width = 100;
        profile.pointer_magnifier_height = 100;
        changed = true;
    }
    if !(1..=1200).contains(&profile.pointer_magnifier_width) {
        profile.pointer_magnifier_width = 100;
        changed = true;
    }
    if !(1..=900).contains(&profile.pointer_magnifier_height) {
        profile.pointer_magnifier_height = 100;
        changed = true;
    }
    if !(50..=1000).contains(&profile.pointer_magnifier_scale_percent) {
        profile.pointer_magnifier_scale_percent = 200;
        changed = true;
    }
    changed
}

fn selected_index_for_settings(settings: &DodbogiSettings) -> usize {
    profiles(settings)
        .iter()
        .position(|profile| profile.id == settings.profiles.active_profile_id)
        .unwrap_or(0)
}

fn profiles(settings: &DodbogiSettings) -> Vec<&AppProfile> {
    std::iter::once(&settings.profiles.default_profile)
        .chain(settings.profiles.per_app_profiles.iter())
        .collect()
}

fn profile_at(settings: &DodbogiSettings, index: usize) -> Option<&AppProfile> {
    if index == 0 {
        Some(&settings.profiles.default_profile)
    } else {
        settings.profiles.per_app_profiles.get(index - 1)
    }
}

fn profile_by_id<'a>(settings: &'a DodbogiSettings, id: &str) -> Option<&'a AppProfile> {
    profiles(settings)
        .into_iter()
        .find(|profile| profile.id == id)
}

fn selected_profile_mut(settings: &mut DodbogiSettings, index: usize) -> Option<&mut AppProfile> {
    if index == 0 {
        Some(&mut settings.profiles.default_profile)
    } else {
        settings.profiles.per_app_profiles.get_mut(index - 1)
    }
}

fn format_hotkey_from_vk(vk: u32) -> Option<String> {
    if is_modifier_key(vk) {
        return None;
    }
    let key = key_label_from_vk(vk)?;
    let mut parts: Vec<String> = Vec::new();
    if modifier_down(VK_CONTROL.0 as i32) {
        parts.push("Ctrl".to_string());
    }
    if modifier_down(VK_MENU.0 as i32) {
        parts.push("Alt".to_string());
    }
    if modifier_down(VK_SHIFT.0 as i32) {
        parts.push("Shift".to_string());
    }
    if modifier_down(VK_LWIN.0 as i32) || modifier_down(VK_RWIN.0 as i32) {
        parts.push("Win".to_string());
    }
    parts.push(key);
    Some(parts.join("+"))
}

fn format_hotkey_display(hotkey: &str) -> String {
    hotkey.replace('+', " + ")
}

fn key_down(vk: i32) -> bool {
    unsafe { (GetKeyState(vk) as u16 & 0x8000) != 0 }
}

fn async_key_down(vk: i32) -> bool {
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

fn modifier_down(vk: i32) -> bool {
    key_down(vk) || async_key_down(vk)
}

fn poll_hotkey_capture(hwnd: HWND) -> bool {
    let Ok(mut slot) = state_slot().try_lock() else {
        return false;
    };
    let Some(state) = slot.as_mut() else {
        return false;
    };
    if !state.hotkey_panel_visible {
        return false;
    }
    if !any_modifier_down() {
        return false;
    }
    let mut captured = false;
    for vk in hotkey_capture_candidates() {
        if async_key_down(*vk as i32) {
            if let Some(hotkey) = format_hotkey_from_vk(*vk) {
                if state.pending_hotkey.as_deref() != Some(hotkey.as_str()) {
                    state.pending_hotkey = Some(hotkey);
                    refresh_hotkey_panel_texts(state);
                    invalidate(hwnd);
                }
                captured = true;
            }
            break;
        }
    }
    captured
}

fn any_modifier_down() -> bool {
    modifier_down(VK_CONTROL.0 as i32)
        || modifier_down(VK_MENU.0 as i32)
        || modifier_down(VK_SHIFT.0 as i32)
        || modifier_down(VK_LWIN.0 as i32)
        || modifier_down(VK_RWIN.0 as i32)
}

fn hotkey_capture_candidates() -> &'static [u32] {
    &[
        0x08, 0x09, 0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E, 0x30, 0x31,
        0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47,
        0x48, 0x49, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E, 0x4F, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56,
        0x57, 0x58, 0x59, 0x5A, 0x60, 0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x70,
        0x71, 0x72, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x7B, 0x7C, 0x7D, 0x7E, 0x7F,
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    ]
}

fn is_modifier_key(vk: u32) -> bool {
    matches!(vk, 0x10 | 0x11 | 0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

fn key_label_from_vk(vk: u32) -> Option<String> {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).map(|ch| ch.to_string()),
        0x60..=0x69 => Some(format!("Num{}", vk - 0x60)),
        0x70..=0x87 => Some(format!("F{}", vk - 0x6F)),
        0x08 => Some("Backspace".to_string()),
        0x09 => Some("Tab".to_string()),
        0x20 => Some("Space".to_string()),
        0x21 => Some("PageUp".to_string()),
        0x22 => Some("PageDown".to_string()),
        0x23 => Some("End".to_string()),
        0x24 => Some("Home".to_string()),
        0x25 => Some("Left".to_string()),
        0x26 => Some("Up".to_string()),
        0x27 => Some("Right".to_string()),
        0x28 => Some("Down".to_string()),
        0x2D => Some("Insert".to_string()),
        0x2E => Some("Delete".to_string()),
        _ => None,
    }
}

fn control_ids() -> &'static [i32] {
    &[
        ID_PROFILE_TITLE,
        ID_PROFILE_LIST,
        ID_ADD_PROFILE,
        ID_DELETE_PROFILE,
        ID_NAME_EDIT,
        ID_SETTINGS_BUTTON,
        ID_TRAY_BUTTON,
        ID_HOTKEY_ICON,
        ID_HOTKEY_LABEL,
        ID_HOTKEY_MOD_PRIMARY,
        ID_HOTKEY_MOD_SECONDARY,
        ID_HOTKEY_KEY,
        ID_HOTKEY_CHANGE,
        ID_SCALE_ICON,
        ID_SCALE_LABEL,
        ID_SCALE_EDIT,
        ID_SCALE_PERCENT,
        ID_SCALE_UP,
        ID_SCALE_DOWN,
        ID_POINTER_LABEL,
        ID_POINTER_HOTKEY_VALUE,
        ID_POINTER_HOTKEY_CHANGE,
        ID_POINTER_RANGE_LABEL,
        ID_POINTER_WIDTH_EDIT,
        ID_POINTER_X_LABEL,
        ID_POINTER_HEIGHT_EDIT,
        ID_POINTER_SCALE_LABEL,
        ID_POINTER_SCALE_EDIT,
        ID_POINTER_PERCENT,
        ID_POINTER_SCALE_UP,
        ID_POINTER_SCALE_DOWN,
        ID_POINTER_RANGE_HELP,
        ID_SCREENSHOT_ICON,
        ID_SCREENSHOT_TITLE,
        ID_WINDOW_SCREENSHOT_LABEL,
        ID_WINDOW_SCREENSHOT_HOTKEY_VALUE,
        ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE,
        ID_POINTER_SCREENSHOT_LABEL,
        ID_POINTER_SCREENSHOT_HOTKEY_VALUE,
        ID_POINTER_SCREENSHOT_HOTKEY_CHANGE,
        ID_POINTER_COLOR_LABEL,
        ID_POINTER_COLOR_HOTKEY_VALUE,
        ID_POINTER_COLOR_HOTKEY_CHANGE,
        ID_POINTER_COLOR_COPY_LABEL,
        ID_POINTER_COLOR_COPY_HOTKEY_VALUE,
        ID_POINTER_COLOR_COPY_HOTKEY_CHANGE,
        ID_POINTER_CURSOR_LABEL,
        ID_POINTER_CURSOR_HOTKEY_VALUE,
        ID_POINTER_CURSOR_HOTKEY_CHANGE,
        ID_POINTER_COLOR_TOGGLE_LABEL,
        ID_POINTER_COLOR_TOGGLE,
        ID_POINTER_CURSOR_TOGGLE_LABEL,
        ID_POINTER_CURSOR_TOGGLE,
        ID_SETTINGS_PANEL_BG,
        ID_SETTINGS_PANEL_TITLE,
        ID_LANGUAGE_COMBO,
        ID_LANGUAGE_MENU,
        ID_RESET_BUTTON,
        ID_LOG_BUTTON,
        ID_SETTINGS_LANGUAGE_LABEL,
        ID_SETTINGS_CLOSE,
        ID_HOTKEY_PANEL_BG,
        ID_HOTKEY_PANEL_TITLE,
        ID_HOTKEY_APPLY,
        ID_HOTKEY_CANCEL,
        ID_HOTKEY_HELP,
        ID_HOTKEY_CURRENT_LABEL,
        ID_HOTKEY_CURRENT_VALUE,
        ID_HOTKEY_NEW_LABEL,
        ID_HOTKEY_NEW_VALUE,
    ]
}

fn settings_panel_ids() -> &'static [i32] {
    &[
        ID_SETTINGS_PANEL_BG,
        ID_SETTINGS_PANEL_TITLE,
        ID_SETTINGS_LANGUAGE_LABEL,
        ID_LANGUAGE_COMBO,
        ID_WINDOW_SCREENSHOT_PATH_LABEL,
        ID_WINDOW_SCREENSHOT_PATH_EDIT,
        ID_RESET_BUTTON,
        ID_LOG_BUTTON,
        ID_SETTINGS_CLOSE,
    ]
}

fn base_interaction_ids() -> &'static [i32] {
    &[
        ID_SETTINGS_BUTTON,
        ID_TRAY_BUTTON,
        ID_SCALE_EDIT,
        ID_SCALE_UP,
        ID_SCALE_DOWN,
        ID_POINTER_SCALE_UP,
        ID_POINTER_SCALE_DOWN,
        ID_POINTER_WIDTH_EDIT,
        ID_POINTER_HEIGHT_EDIT,
        ID_POINTER_SCALE_EDIT,
        ID_POINTER_COLOR_TOGGLE,
        ID_POINTER_CURSOR_TOGGLE,
    ]
}

fn legacy_action_button_ids() -> &'static [i32] {
    &[
        ID_HOTKEY_CHANGE,
        ID_POINTER_HOTKEY_CHANGE,
        ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE,
        ID_POINTER_SCREENSHOT_HOTKEY_CHANGE,
        ID_POINTER_COLOR_HOTKEY_CHANGE,
        ID_POINTER_COLOR_COPY_HOTKEY_CHANGE,
        ID_POINTER_CURSOR_HOTKEY_CHANGE,
    ]
}

fn hotkey_panel_ids() -> &'static [i32] {
    &[
        ID_HOTKEY_PANEL_BG,
        ID_HOTKEY_PANEL_TITLE,
        ID_HOTKEY_HELP,
        ID_HOTKEY_CURRENT_LABEL,
        ID_HOTKEY_CURRENT_VALUE,
        ID_HOTKEY_NEW_LABEL,
        ID_HOTKEY_NEW_VALUE,
        ID_HOTKEY_APPLY,
        ID_HOTKEY_CANCEL,
    ]
}

fn modal_covered_base_control_ids() -> &'static [i32] {
    &[
        ID_HOTKEY_ICON,
        ID_HOTKEY_LABEL,
        ID_HOTKEY_MOD_PRIMARY,
        ID_HOTKEY_MOD_SECONDARY,
        ID_HOTKEY_KEY,
        ID_HOTKEY_CHANGE,
        ID_SCALE_ICON,
        ID_SCALE_LABEL,
        ID_SCALE_EDIT,
        ID_SCALE_PERCENT,
        ID_SCALE_UP,
        ID_SCALE_DOWN,
        ID_POINTER_LABEL,
        ID_POINTER_HOTKEY_VALUE,
        ID_POINTER_HOTKEY_CHANGE,
        ID_POINTER_RANGE_LABEL,
        ID_POINTER_WIDTH_EDIT,
        ID_POINTER_X_LABEL,
        ID_POINTER_HEIGHT_EDIT,
        ID_POINTER_SCALE_LABEL,
        ID_POINTER_SCALE_EDIT,
        ID_POINTER_PERCENT,
        ID_POINTER_SCALE_UP,
        ID_POINTER_SCALE_DOWN,
        ID_POINTER_RANGE_HELP,
        ID_SCREENSHOT_ICON,
        ID_SCREENSHOT_TITLE,
        ID_WINDOW_SCREENSHOT_LABEL,
        ID_WINDOW_SCREENSHOT_HOTKEY_VALUE,
        ID_WINDOW_SCREENSHOT_HOTKEY_CHANGE,
        ID_POINTER_SCREENSHOT_LABEL,
        ID_POINTER_SCREENSHOT_HOTKEY_VALUE,
        ID_POINTER_SCREENSHOT_HOTKEY_CHANGE,
        ID_POINTER_COLOR_LABEL,
        ID_POINTER_COLOR_HOTKEY_VALUE,
        ID_POINTER_COLOR_HOTKEY_CHANGE,
        ID_POINTER_COLOR_COPY_LABEL,
        ID_POINTER_COLOR_COPY_HOTKEY_VALUE,
        ID_POINTER_COLOR_COPY_HOTKEY_CHANGE,
        ID_POINTER_CURSOR_LABEL,
        ID_POINTER_CURSOR_HOTKEY_VALUE,
        ID_POINTER_CURSOR_HOTKEY_CHANGE,
        ID_POINTER_COLOR_TOGGLE_LABEL,
        ID_POINTER_COLOR_TOGGLE,
        ID_POINTER_CURSOR_TOGGLE_LABEL,
        ID_POINTER_CURSOR_TOGGLE,
    ]
}

fn show_child(parent: HWND, id: i32, visible: bool) {
    if id == 0 {
        return;
    }
    let child = get(parent, id);
    if child.0.is_null() {
        return;
    }
    unsafe {
        let _ = ShowWindow(child, if visible { SW_SHOW } else { SW_HIDE });
        let _ = EnableWindow(child, visible);
    }
}

fn hide_legacy_action_buttons(parent: HWND) {
    for id in legacy_action_button_ids() {
        show_child(parent, *id, false);
        set_child_enabled(parent, *id, false);
    }
}

fn update_modal_base_enabled(state: &SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let modal_active = state.settings_panel_visible || state.hotkey_panel_visible;
    let cover_rect = if modal_active {
        let layout = current_layout(hwnd);
        let mut rect = if state.settings_panel_visible {
            layout.settings_panel
        } else {
            layout.hotkey_panel
        };
        rect.right += 8;
        rect.bottom += 8;
        Some(rect)
    } else {
        None
    };
    for id in modal_covered_base_control_ids() {
        let covered_by_modal = cover_rect
            .as_ref()
            .and_then(|modal| {
                child_frame_rect(hwnd, *id, 0, 0).map(|child| rects_intersect(&child, modal))
            })
            .unwrap_or(false);
        show_child(hwnd, *id, !covered_by_modal);
        set_child_enabled(hwnd, *id, !modal_active && !covered_by_modal);
    }
    for id in base_interaction_ids() {
        let enabled = !modal_active && (*id != ID_DELETE_PROFILE || state.selected_index > 0);
        if *id == ID_DELETE_PROFILE {
            show_child(hwnd, *id, false);
            set_child_enabled(hwnd, *id, false);
            continue;
        }
        set_child_enabled(hwnd, *id, enabled);
    }
    hide_legacy_action_buttons(hwnd);
}

fn rects_intersect(a: &RECT, b: &RECT) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

fn set_child_enabled(parent: HWND, id: i32, enabled: bool) {
    let child = get(parent, id);
    if child.0.is_null() {
        return;
    }
    unsafe {
        let _ = EnableWindow(child, enabled);
    }
}

fn raise_panel_children(parent: HWND, ids: &[i32]) {
    for id in ids {
        if *id == ID_SETTINGS_PANEL_BG || *id == ID_HOTKEY_PANEL_BG {
            raise_child(parent, *id);
        }
    }
    for id in ids {
        if *id == ID_SETTINGS_PANEL_BG || *id == ID_HOTKEY_PANEL_BG {
            continue;
        }
        raise_child(parent, *id);
    }
}

fn raise_child(parent: HWND, id: i32) {
    let child = get(parent, id);
    if child.0.is_null() {
        return;
    }
    unsafe {
        let _ = SetWindowPos(
            child,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SET_WINDOW_POS_FLAGS(SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_NOACTIVATE.0),
        );
    }
}

fn hide_to_tray(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
    push_event(SettingsUiEvent::WindowHiddenToTray);
}

fn invalidate(hwnd: HWND) {
    unsafe {
        let _ = InvalidateRect(Some(hwnd), None, false);
    }
}

fn get(parent: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(parent), id).unwrap_or_default() }
}

fn send(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))).0 }
}

fn set_redraw(hwnd: HWND, enabled: bool) {
    if hwnd.0.is_null() {
        return;
    }
    let _ = send(hwnd, WM_SETREDRAW_MSG, usize::from(enabled), 0);
}

fn set_text(hwnd: HWND, text: &str) {
    let text = wide_null(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
        let _ = RedrawWindow(Some(hwnd), None, None, RDW_INVALIDATE);
    }
}

fn get_text(hwnd: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(hwnd) };
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize + 1];
    let read = unsafe { GetWindowTextW(hwnd, &mut buf) };
    String::from_utf16_lossy(&buf[..read as usize])
}

fn loword(value: usize) -> u16 {
    (value & 0xffff) as u16
}

fn hiword(value: usize) -> u16 {
    ((value >> 16) & 0xffff) as u16
}

fn hwnd_from_raw(raw: isize) -> HWND {
    HWND(raw as *mut _)
}

fn raw_from_hwnd(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[allow(dead_code)]
fn show_error(hwnd: HWND, message: &str) {
    let message = wide_null(message);
    let title = wide_null("Dodbogi");
    unsafe {
        let _ = MessageBoxW(
            Some(hwnd),
            PCWSTR(message.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}
