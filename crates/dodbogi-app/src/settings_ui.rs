use dodbogi_core::{
    load_settings_from_path, save_settings_to_path, AppProfile, DodbogiSettings, RuntimePaths,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex, OnceLock,
    },
};
use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{
            GetLastError, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::Gdi::{
            AddFontMemResourceEx, BeginPaint, CreateFontW, CreatePen, CreateSolidBrush,
            DeleteObject, EndPaint, FillRect, GetStockObject, InvalidateRect, Rectangle,
            RedrawWindow, RoundRect, SelectObject, SetBkMode, SetTextColor, TextOutW, UpdateWindow,
            CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_GUI_FONT, DEFAULT_PITCH, DEFAULT_QUALITY,
            HBRUSH, HDC, HGDIOBJ, HOLLOW_BRUSH, OUT_DEFAULT_PRECIS, PAINTSTRUCT, PS_SOLID,
            RDW_ALLCHILDREN, RDW_ERASE, RDW_ERASENOW, RDW_INVALIDATE, RDW_UPDATENOW, TRANSPARENT,
            WHITE_BRUSH,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Input::KeyboardAndMouse::{
                EnableWindow, GetAsyncKeyState, GetKeyState, SetFocus, VK_CONTROL, VK_LWIN,
                VK_MENU, VK_RWIN, VK_SHIFT,
            },
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, GetClientRect, GetDlgCtrlID, GetDlgItem,
                GetWindowTextLengthW, GetWindowTextW, KillTimer, LoadCursorW, LoadImageW,
                MessageBoxW, RegisterClassW, SendMessageW, SetForegroundWindow, SetTimer,
                SetWindowPos, SetWindowTextW, ShowWindow, BM_GETCHECK, BM_SETCHECK, BN_CLICKED,
                BS_AUTOCHECKBOX, CBN_SELCHANGE, CBS_DROPDOWNLIST, CB_ADDSTRING, CB_GETCURSEL,
                CB_RESETCONTENT, CB_SETCURSEL, CS_DBLCLKS, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT,
                EN_CHANGE, EN_KILLFOCUS, ES_AUTOHSCROLL, HMENU, HWND_TOP, IDC_ARROW, IDYES,
                IMAGE_BITMAP, LBN_DBLCLK, LBN_SELCHANGE, LBS_NOTIFY, LB_ADDSTRING, LB_GETCURSEL,
                LB_RESETCONTENT, LB_SETCURSEL, LR_LOADFROMFILE, MB_ICONERROR, MB_ICONQUESTION,
                MB_OK, MB_YESNO, MINMAXINFO, SET_WINDOW_POS_FLAGS, STM_SETIMAGE, SWP_NOACTIVATE,
                SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_RESTORE, SW_SHOW,
                WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORSTATIC,
                WM_DESTROY, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_KEYDOWN, WM_NCCREATE, WM_PAINT,
                WM_SETFONT, WM_SIZE, WM_SYSKEYDOWN, WM_TIMER, WNDCLASSW, WS_CHILD, WS_CLIPSIBLINGS,
                WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
            },
        },
    },
};

const MIN_TRACK_WIDTH: i32 = 720;
const MIN_TRACK_HEIGHT: i32 = 460;
const ID_LIVE_APPLY_TIMER: usize = 2001;

const HOTKEY_ICON_BMP: &[u8] = include_bytes!("../assets/icons/40/hotkey.bmp");
const SCALE_ICON_BMP: &[u8] = include_bytes!("../assets/icons/40/scale.bmp");
const SETTINGS_ICON_BMP: &[u8] = include_bytes!("../assets/icons/24/settings.bmp");
const TRAY_ICON_BMP: &[u8] = include_bytes!("../assets/icons/24/minimize-to-tray.bmp");
const UI_FONT_TTF: &[u8] = include_bytes!("../assets/fonts/NeoDunggeunmoPro-Regular.ttf");
const UI_FONT_FACE: &str = "NeoDunggeunmo Pro";
const ROW_ICON_SIZE: i32 = 40;
const SS_BITMAP_STYLE: i32 = 0x000E;
const SS_WHITERECT_STYLE: i32 = 0x0006;
const BS_OWNERDRAW_STYLE: i32 = 0x000B;
const LBS_OWNERDRAWFIXED_STYLE: i32 = 0x0010;
const LBS_HASSTRINGS_STYLE: i32 = 0x0040;
const LBS_NOINTEGRALHEIGHT_STYLE: i32 = 0x0100;
const WM_DRAWITEM_MSG: u32 = 0x002B;
const WM_MEASUREITEM_MSG: u32 = 0x002C;
const LB_GETTEXT_MSG: u32 = 0x0189;
const ODS_SELECTED_FLAG: u32 = 0x0001;
const ODS_DISABLED_FLAG: u32 = 0x0004;
const UI_STROKE_WIDTH: i32 = 2;
const UI_RADIUS: i32 = 8;

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

const ID_SETTINGS_PANEL_BG: i32 = 1098;
const ID_SETTINGS_PANEL_TITLE: i32 = 1099;
const ID_SETTINGS_CLOSE: i32 = 1100;
const ID_LANGUAGE_COMBO: i32 = 1101;
const ID_RESET_BUTTON: i32 = 1102;
const ID_LOG_CHECK: i32 = 1103;
const ID_SETTINGS_LANGUAGE_LABEL: i32 = 1104;
const ID_HOTKEY_PANEL_BG: i32 = 1198;
const ID_HOTKEY_PANEL_TITLE: i32 = 1199;
const ID_HOTKEY_APPLY: i32 = 1202;
const ID_HOTKEY_CANCEL: i32 = 1203;
const ID_HOTKEY_HELP: i32 = 1204;
const ID_HOTKEY_CURRENT_LABEL: i32 = 1205;
const ID_HOTKEY_CURRENT_VALUE: i32 = 1206;
const ID_HOTKEY_NEW_LABEL: i32 = 1207;
const ID_HOTKEY_NEW_VALUE: i32 = 1208;

#[derive(Clone, Copy)]
enum UiString {
    WindowTitle,
    Profiles,
    AddProfile,
    Hotkey,
    Change,
    Scale,
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
    DeleteProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsUiEvent {
    HotkeysChanged,
    ProfileChanged,
    GlobalSettingsChanged,
    WindowHiddenToTray,
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
                    SET_WINDOW_POS_FLAGS(
                        SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_NOACTIVATE.0 | SWP_SHOWWINDOW.0,
                    ),
                );
                let _ = SetForegroundWindow(hwnd);
                let _ = SetFocus(Some(hwnd));
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
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                760,
                500,
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
    pending_hotkey: Option<String>,
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
            pending_hotkey: None,
        }
    }
}

static SETTINGS_UI_STATE: OnceLock<Mutex<Option<SettingsUiState>>> = OnceLock::new();
static SETTINGS_UI_EVENTS: OnceLock<Mutex<Vec<SettingsUiEvent>>> = OnceLock::new();
static SETTINGS_PANEL_PAINT_VISIBLE: AtomicBool = AtomicBool::new(false);
static HOTKEY_PANEL_PAINT_VISIBLE: AtomicBool = AtomicBool::new(false);

fn state_slot() -> &'static Mutex<Option<SettingsUiState>> {
    SETTINGS_UI_STATE.get_or_init(|| Mutex::new(None))
}

fn event_slot() -> &'static Mutex<Vec<SettingsUiEvent>> {
    SETTINGS_UI_EVENTS.get_or_init(|| Mutex::new(Vec::new()))
}

fn ui_text(lang: &str, key: UiString) -> &'static str {
    let english = lang.eq_ignore_ascii_case("en");
    match (english, key) {
        (true, UiString::WindowTitle) => "Dodbogi Settings",
        (false, UiString::WindowTitle) => "Dodbogi 설정",
        (true, UiString::Profiles) => "Profiles",
        (false, UiString::Profiles) => "프로파일",
        (true, UiString::AddProfile) => "+ New profile",
        (false, UiString::AddProfile) => "+새 프로파일",
        (true, UiString::Hotkey) => "Hotkey",
        (false, UiString::Hotkey) => "단축키",
        (true, UiString::Change) => "Change",
        (false, UiString::Change) => "변경",
        (true, UiString::Scale) => "Scale",
        (false, UiString::Scale) => "배율",
        (true, UiString::Settings) => "Settings",
        (false, UiString::Settings) => "설정",
        (true, UiString::Language) => "Language",
        (false, UiString::Language) => "언어",
        (true, UiString::ResetDefaults) => "Reset to defaults",
        (false, UiString::ResetDefaults) => "기본값으로 초기화",
        (true, UiString::LogOutput) => "Log output",
        (false, UiString::LogOutput) => "로그 출력",
        (true, UiString::Close) => "Close",
        (false, UiString::Close) => "닫기",
        (true, UiString::Apply) => "Apply",
        (false, UiString::Apply) => "적용",
        (true, UiString::Cancel) => "Cancel",
        (false, UiString::Cancel) => "취소",
        (true, UiString::HotkeyChange) => "Change hotkey",
        (false, UiString::HotkeyChange) => "단축키 변경",
        (true, UiString::HotkeyHelp) => "Press the shortcut you want to use.",
        (false, UiString::HotkeyHelp) => "사용할 단축키를 누르세요.",
        (true, UiString::CurrentHotkey) => "Current",
        (false, UiString::CurrentHotkey) => "현재",
        (true, UiString::NewHotkey) => "New",
        (false, UiString::NewHotkey) => "새 단축키",
        (true, UiString::ResetQuestion) => "Reset settings to defaults?",
        (false, UiString::ResetQuestion) => "설정을 기본값으로 초기화할까요?",
        (true, UiString::NewProfile) => "New profile",
        (false, UiString::NewProfile) => "새 프로파일",
        (true, UiString::DeleteProfile) => "Delete",
        (false, UiString::DeleteProfile) => "삭제",
    }
}

fn ensure_icon_files(paths: &RuntimePaths) -> Result<PathBuf, String> {
    let icon_dir = paths.cache_dir.join("ui-icons");
    fs::create_dir_all(&icon_dir).map_err(|error| format!("icon cache create failed: {error}"))?;
    for (name, bytes) in [
        ("hotkey.bmp", HOTKEY_ICON_BMP),
        ("scale.bmp", SCALE_ICON_BMP),
        ("settings.bmp", SETTINGS_ICON_BMP),
        ("minimize-to-tray.bmp", TRAY_ICON_BMP),
    ] {
        let path = icon_dir.join(name);
        fs::write(&path, bytes)
            .map_err(|error| format!("icon cache write failed for {}: {error}", path.display()))?;
    }
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
            WM_COMMAND => {
                handle_command(hwnd, wparam);
                LRESULT(0)
            }
            WM_TIMER => {
                if wparam.0 == ID_LIVE_APPLY_TIMER {
                    let _ = poll_hotkey_capture(hwnd);
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
                hide_to_tray(hwnd);
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
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
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
    hotkey_row: RECT,
    scale_row: RECT,
}

fn current_layout(hwnd: HWND) -> UiLayout {
    let mut client = RECT::default();
    let _ = unsafe { GetClientRect(hwnd, &mut client) };
    let client_w = (client.right - client.left).max(1);
    let client_h = (client.bottom - client.top).max(1);
    let margin = 24.min((client_w / 24).max(12));
    let sidebar_w = (client_w / 5).clamp(164, 220).min((client_w / 3).max(140));
    let sidebar_x = margin;
    let sidebar_y = 84;
    let profile_button_stack_h = 74;
    let sidebar_h = (client_h - sidebar_y - profile_button_stack_h - margin * 2).max(180);
    let content_x = sidebar_x + sidebar_w + margin;
    let content_y = 64;
    let content_w = (client_w - content_x - margin).max(360);
    let content_h = (client_h - content_y - margin).max(300);
    let content_panel = RECT {
        left: content_x,
        top: content_y,
        right: content_x + content_w,
        bottom: content_y + content_h,
    };
    let row_pad = 48.min((content_w / 12).max(18));
    let row_left = content_x + row_pad;
    let row_right = content_panel.right - row_pad;
    let row_h = 66;
    let row1_top = content_y + 70;
    let hotkey_row = RECT {
        left: row_left,
        top: row1_top,
        right: row_right,
        bottom: row1_top + row_h,
    };
    let scale_row = RECT {
        left: row_left,
        top: row1_top + row_h + 16,
        right: row_right,
        bottom: row1_top + row_h * 2 + 16,
    };
    let modal_w = (content_w - 48).max(340).clamp(340, 440);
    let settings_w = modal_w;
    let settings_h = 238;
    let modal_top = (row1_top - 18).clamp(content_y + 26, content_panel.bottom - settings_h - 18);
    let settings_left = content_x + ((content_w - settings_w) / 2).max(24);
    let settings_panel = RECT {
        left: settings_left,
        top: modal_top,
        right: settings_left + settings_w,
        bottom: modal_top + settings_h,
    };
    let hotkey_w = (content_w - 48).max(360).clamp(360, 460);
    let hotkey_h = 238;
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
        hotkey_row,
        scale_row,
    }
}

fn layout_controls(hwnd: HWND) {
    let layout = current_layout(hwnd);
    let toolbar_y = 16;
    let tray_x = layout.content_panel.right - 72;
    let settings_x = tray_x - 50;
    move_child(
        hwnd,
        ID_PROFILE_TITLE,
        layout.sidebar_x,
        layout.margin + 34,
        140,
        24,
    );
    move_child(
        hwnd,
        ID_PROFILE_LIST,
        layout.sidebar_x + 4,
        layout.sidebar_y + 4,
        layout.sidebar_w - 8,
        layout.sidebar_h - 8,
    );
    let bottom_y = layout.sidebar_y + layout.sidebar_h + 16;
    move_child(
        hwnd,
        ID_ADD_PROFILE,
        layout.sidebar_x,
        bottom_y,
        layout.sidebar_w,
        34,
    );
    move_child(
        hwnd,
        ID_DELETE_PROFILE,
        layout.sidebar_x,
        bottom_y + 40,
        layout.sidebar_w,
        34,
    );
    move_child(hwnd, ID_SETTINGS_BUTTON, settings_x, toolbar_y, 44, 30);
    move_child(hwnd, ID_TRAY_BUTTON, tray_x, toolbar_y, 44, 30);

    let hotkey = layout.hotkey_row;
    let scale = layout.scale_row;
    let icon_x = hotkey.left + 20;
    let label_x = icon_x + 52;
    let label_w = 78;
    let value_x = label_x + 88;
    let action_x = hotkey.right - 100;
    let hotkey_value_w = (action_x - value_x - 18).clamp(116, 240);
    let scale_edit_frame = scale_edit_frame_rect(&layout);

    move_child(
        hwnd,
        ID_HOTKEY_ICON,
        icon_x,
        hotkey.top + 13,
        ROW_ICON_SIZE,
        ROW_ICON_SIZE,
    );
    move_child(hwnd, ID_HOTKEY_LABEL, label_x, hotkey.top + 18, label_w, 24);
    move_child(
        hwnd,
        ID_HOTKEY_MOD_PRIMARY,
        value_x,
        hotkey.top + 18,
        hotkey_value_w,
        24,
    );
    move_child(
        hwnd,
        ID_HOTKEY_MOD_SECONDARY,
        value_x,
        hotkey.top + 18,
        1,
        1,
    );
    move_child(hwnd, ID_HOTKEY_KEY, value_x, hotkey.top + 18, 1, 1);
    move_child(hwnd, ID_HOTKEY_CHANGE, action_x, hotkey.top + 13, 84, 32);

    move_child(
        hwnd,
        ID_SCALE_ICON,
        icon_x,
        scale.top + 13,
        ROW_ICON_SIZE,
        ROW_ICON_SIZE,
    );
    move_child(hwnd, ID_SCALE_LABEL, label_x, scale.top + 18, label_w, 24);
    move_child(
        hwnd,
        ID_SCALE_EDIT,
        scale_edit_frame.left + 8,
        scale_edit_frame.top + 5,
        scale_edit_frame.right - scale_edit_frame.left - 16,
        scale_edit_frame.bottom - scale_edit_frame.top - 10,
    );
    move_child(
        hwnd,
        ID_SCALE_PERCENT,
        scale_edit_frame.right + 12,
        scale.top + 18,
        40,
        24,
    );
    move_child(hwnd, ID_SCALE_UP, action_x, scale.top + 13, 38, 28);
    move_child(hwnd, ID_SCALE_DOWN, action_x + 46, scale.top + 13, 38, 28);

    let sp = layout.settings_panel;
    move_child(
        hwnd,
        ID_SETTINGS_PANEL_BG,
        sp.left,
        sp.top,
        sp.right - sp.left,
        sp.bottom - sp.top,
    );
    move_child(
        hwnd,
        ID_SETTINGS_PANEL_TITLE,
        sp.left + 26,
        sp.top + 22,
        sp.right - sp.left - 52,
        24,
    );
    move_child(
        hwnd,
        ID_SETTINGS_LANGUAGE_LABEL,
        sp.left + 28,
        sp.top + 66,
        86,
        24,
    );
    move_child(hwnd, ID_LANGUAGE_COMBO, sp.left + 126, sp.top + 62, 190, 88);
    move_child(hwnd, ID_RESET_BUTTON, sp.left + 28, sp.top + 110, 186, 34);
    move_child(hwnd, ID_LOG_CHECK, sp.left + 28, sp.top + 154, 160, 28);
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
        hp.left,
        hp.top,
        hp.right - hp.left,
        hp.bottom - hp.top,
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

fn scale_edit_frame_rect(layout: &UiLayout) -> RECT {
    let hotkey = layout.hotkey_row;
    let scale = layout.scale_row;
    let icon_x = hotkey.left + 20;
    let label_x = icon_x + 44;
    let value_x = label_x + 100;
    let action_x = hotkey.right - 116;
    let scale_edit_w = (action_x - value_x - 58).clamp(72, 108);
    RECT {
        left: value_x - 7,
        top: scale.top + 10,
        right: value_x + scale_edit_w + 7,
        bottom: scale.top + 42,
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
        let modal_active = settings_panel_visible || hotkey_panel_visible;
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
        if !modal_active {
            let _ = FillRect(hdc, &layout.hotkey_row, card_brush);
            let _ = FillRect(hdc, &layout.scale_row, card_brush);
            sketch_round_rect(hdc, &layout.hotkey_row, UI_RADIUS, UI_STROKE_WIDTH);
            sketch_round_rect(hdc, &layout.scale_row, UI_RADIUS, UI_STROKE_WIDTH);
            let scale_edit_frame = scale_edit_frame_rect(&layout);
            let _ = FillRect(hdc, &scale_edit_frame, white_brush);
            sketch_round_rect(hdc, &scale_edit_frame, UI_RADIUS, UI_STROKE_WIDTH);
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

fn create_controls(hwnd: HWND, icon_dir: &Path) -> Result<(), String> {
    create_static(hwnd, "프로파일", 24, 32, 120, 22, ID_PROFILE_TITLE)?;
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

    create_listbox(hwnd, ID_PROFILE_LIST, 13, 76, 150, 290)?;
    create_button(hwnd, "+새 프로파일", ID_ADD_PROFILE, 13, 389, 150, 30)?;
    create_button(hwnd, "삭제", ID_DELETE_PROFILE, 13, 389, 72, 30)?;
    create_edit(hwnd, ID_NAME_EDIT, 13, 76, 150, 24)?;
    show_child(hwnd, ID_NAME_EDIT, false);

    create_bitmap_static(hwnd, ID_HOTKEY_ICON, 272, 63, &icon_dir.join("hotkey.bmp"))?;
    create_static(hwnd, "단축키", 376, 66, 70, 24, ID_HOTKEY_LABEL)?;
    create_static(hwnd, "Ctrl", 478, 66, 72, 24, ID_HOTKEY_MOD_PRIMARY)?;
    create_static(hwnd, "Alt", 603, 66, 72, 24, ID_HOTKEY_MOD_SECONDARY)?;
    create_static(hwnd, "Q", 765, 66, 72, 24, ID_HOTKEY_KEY)?;
    create_button(hwnd, "변경", ID_HOTKEY_CHANGE, 867, 60, 80, 30)?;

    create_bitmap_static(hwnd, ID_SCALE_ICON, 272, 105, &icon_dir.join("scale.bmp"))?;
    create_static(hwnd, "배율", 376, 108, 70, 24, ID_SCALE_LABEL)?;
    create_edit(hwnd, ID_SCALE_EDIT, 575, 102, 84, 28)?;
    create_static(hwnd, "%", 765, 108, 40, 24, ID_SCALE_PERCENT)?;
    create_button(hwnd, "▲", ID_SCALE_UP, 867, 98, 36, 26)?;
    create_button(hwnd, "▼", ID_SCALE_DOWN, 911, 98, 36, 26)?;

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

fn create_global_settings_panel(hwnd: HWND) -> Result<(), String> {
    create_panel_background(hwnd, ID_SETTINGS_PANEL_BG, 610, 90, 380, 218)?;
    create_static(hwnd, "설정", 636, 112, 220, 24, ID_SETTINGS_PANEL_TITLE)?;
    create_static(hwnd, "언어", 628, 104, 70, 24, ID_SETTINGS_LANGUAGE_LABEL)?;
    create_combo(hwnd, ID_LANGUAGE_COMBO, 700, 100, 190, 88)?;
    create_button(
        hwnd,
        "기본값으로 초기화",
        ID_RESET_BUTTON,
        628,
        142,
        160,
        30,
    )?;
    create_checkbox(hwnd, "로그 출력", ID_LOG_CHECK, 628, 180, 140, 30)?;
    create_button(hwnd, "닫기", ID_SETTINGS_CLOSE, 808, 216, 70, 28)?;
    for id in settings_panel_ids() {
        show_child(hwnd, *id, false);
    }
    Ok(())
}

fn create_hotkey_panel(hwnd: HWND) -> Result<(), String> {
    create_panel_background(hwnd, ID_HOTKEY_PANEL_BG, 330, 290, 420, 218)?;
    create_static(
        hwnd,
        "단축키 변경",
        356,
        312,
        220,
        24,
        ID_HOTKEY_PANEL_TITLE,
    )?;
    create_static(
        hwnd,
        "사용할 단축키를 누르세요.",
        368,
        314,
        300,
        24,
        ID_HOTKEY_HELP,
    )?;
    create_static(hwnd, "현재", 368, 344, 90, 24, ID_HOTKEY_CURRENT_LABEL)?;
    create_static(
        hwnd,
        "Ctrl + Alt + Q",
        470,
        344,
        180,
        24,
        ID_HOTKEY_CURRENT_VALUE,
    )?;
    create_static(hwnd, "새 단축키", 368, 374, 90, 24, ID_HOTKEY_NEW_LABEL)?;
    create_static(
        hwnd,
        "Ctrl + Alt + Q",
        470,
        374,
        180,
        24,
        ID_HOTKEY_NEW_VALUE,
    )?;
    create_button(hwnd, "적용", ID_HOTKEY_APPLY, 470, 312, 70, 30)?;
    create_button(hwnd, "취소", ID_HOTKEY_CANCEL, 552, 312, 70, 30)?;
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
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
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
    let _ = path;
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

fn create_checkbox(
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
        style(WS_CHILD | WS_VISIBLE | WS_TABSTOP, &[BS_AUTOCHECKBOX]),
        x,
        y,
        w,
        h,
        id,
    )
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

fn create_combo(hwnd: HWND, id: i32, x: i32, y: i32, w: i32, h: i32) -> Result<HWND, String> {
    create_child(
        hwnd,
        w!("COMBOBOX"),
        "",
        style(WS_CHILD | WS_VISIBLE | WS_TABSTOP, &[CBS_DROPDOWNLIST]),
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
    apply_scale_from_edit(state);
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
        item.item_height = 28;
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
    let text = listbox_item_text(item.hwnd_item, item.item_id);
    draw_text_left(
        item.hdc,
        &text,
        rect.left + 8,
        rect.top + 4,
        if selected {
            rgb(0, 0, 0)
        } else {
            rgb(35, 35, 35)
        },
    );
}

fn draw_owner_button(item: &OwnerDrawItem) {
    let id = item.ctl_id as i32;
    let disabled = (item.item_state & ODS_DISABLED_FLAG) != 0;
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
    if draw_toolbar_icon(item.hdc, &rect, id, disabled) {
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

fn owner_button_label(id: i32, text: &str) -> String {
    match id {
        ID_SCALE_UP => "▲".to_string(),
        ID_SCALE_DOWN => "▼".to_string(),
        _ => text.to_string(),
    }
}

fn draw_toolbar_icon(hdc: HDC, rect: &RECT, id: i32, disabled: bool) -> bool {
    if id != ID_SETTINGS_BUTTON && id != ID_TRAY_BUTTON {
        return false;
    }
    let pattern = if id == ID_SETTINGS_BUTTON {
        pixel_icon_settings()
    } else {
        pixel_icon_tray()
    };
    draw_pixel_pattern(hdc, rect, pattern, disabled);
    true
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
        "..........BBBB..........",
        "..........BBBB..........",
        "..........BBBB..........",
        ".....BB...BBBB...BB.....",
        "....BBBB.BBBBBB.BBBB....",
        "....BBBBBBBBBBBBBBBB....",
        ".....BBBBBBBBBBBBBB.....",
        "......BBBB....BBBB......",
        ".....BBBB......BBBB.....",
        ".BBBBBBB........BBBBBBB.",
        ".BBBBBBB........BBBBBBB.",
        ".BBBBBBB........BBBBBBB.",
        ".BBBBBBB........BBBBBBB.",
        ".....BBBB......BBBB.....",
        "......BBBB....BBBB......",
        ".....BBBBBBBBBBBBBB.....",
        ".....BBBBBBBBBBBBBB.....",
        "....BBBB.BBBBBB.BBBB....",
        ".....BB...BBBB...BB.....",
        "..........BBBB..........",
        "..........BBBB..........",
        "..........BBBB..........",
        "........................",
    ]
}

fn pixel_icon_tray() -> &'static [&'static str; 24] {
    &[
        "........................",
        "...BBBBBBBBBBBBBBBBBB...",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBBBBBBBBBBB.B..B.BB..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..B..................B..",
        "..B..................B..",
        "..B..................B..",
        "..B........BB........B..",
        "..B........BB........B..",
        "..B........BB........B..",
        "..B........BB........B..",
        "..B........BB........B..",
        "..B.....BBBBBBBB.....B..",
        "..B......BBBBB.......B..",
        "..B........BB........B..",
        "..B........BB........B..",
        "........................",
        "...BBBB..........BBBB...",
        "..BBBBBB........BBBBBB..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBBBBBBBBBBBBBBBBBBB..",
        "..BBBBBBBBBBBBBBBBBBBB..",
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

fn draw_text_center(hdc: HDC, text: &str, rect: &RECT, color: COLORREF) {
    let text_width = approximate_text_width(text);
    let x = rect.left + ((rect.right - rect.left - text_width) / 2).max(4);
    let y = rect.top + ((rect.bottom - rect.top - 16) / 2).max(2);
    draw_text_left(hdc, text, x, y, color);
}

fn approximate_text_width(text: &str) -> i32 {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_whitespace() {
                5
            } else if ch.is_ascii() {
                8
            } else {
                13
            }
        })
        .sum()
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

fn handle_command(hwnd: HWND, wparam: WPARAM) {
    let id = loword(wparam.0) as i32;
    let code = hiword(wparam.0) as u32;
    let Ok(mut slot) = state_slot().try_lock() else {
        return;
    };
    let Some(state) = slot.as_mut() else {
        return;
    };
    if state.loading {
        return;
    }

    match id {
        ID_PROFILE_LIST if code == LBN_SELCHANGE => {
            let selected = send(get(hwnd, ID_PROFILE_LIST), LB_GETCURSEL, 0, 0);
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
                refresh_profile_controls(state);
            }
        }
        ID_PROFILE_LIST if code == LBN_DBLCLK => show_profile_rename_edit(state),
        ID_ADD_PROFILE if code == BN_CLICKED => {
            add_profile(state);
            let _ = save_settings(state);
            push_event(SettingsUiEvent::HotkeysChanged);
            push_event(SettingsUiEvent::ProfileChanged);
            refresh_all_controls(state);
        }
        ID_DELETE_PROFILE if code == BN_CLICKED => {
            if delete_selected_profile(state) {
                let _ = save_settings(state);
                push_event(SettingsUiEvent::HotkeysChanged);
                push_event(SettingsUiEvent::ProfileChanged);
                refresh_all_controls(state);
            }
        }
        ID_NAME_EDIT if code == EN_CHANGE || code == EN_KILLFOCUS => {
            let name = get_text(get(hwnd, ID_NAME_EDIT)).trim().to_string();
            if !name.is_empty() {
                if let Some(profile) =
                    selected_profile_mut(&mut state.settings, state.selected_index)
                {
                    profile.display_name = name;
                    let _ = save_settings(state);
                    refresh_profile_list(state);
                }
            }
            if code == EN_KILLFOCUS {
                show_child(hwnd, ID_NAME_EDIT, false);
            }
        }
        ID_HOTKEY_CHANGE if code == BN_CLICKED => show_hotkey_panel(state, true),
        ID_SCALE_EDIT => {
            apply_scale_from_edit(state);
        }
        ID_SCALE_UP if code == BN_CLICKED => adjust_scale(state, 10),
        ID_SCALE_DOWN if code == BN_CLICKED => adjust_scale(state, -10),
        ID_SETTINGS_BUTTON if code == BN_CLICKED => toggle_settings_panel(state),
        ID_TRAY_BUTTON if code == BN_CLICKED => hide_to_tray(hwnd),
        ID_SETTINGS_CLOSE if code == BN_CLICKED => show_settings_panel(state, false),
        ID_LANGUAGE_COMBO if code == CBN_SELCHANGE => {
            let selection = send(get(hwnd, ID_LANGUAGE_COMBO), CB_GETCURSEL, 0, 0);
            state.settings.ui.language = if selection == 1 { "en" } else { "ko" }.to_string();
            let _ = save_settings(state);
            refresh_localized_texts(state);
            refresh_global_controls(state);
            push_event(SettingsUiEvent::GlobalSettingsChanged);
        }
        ID_RESET_BUTTON if code == BN_CLICKED => reset_settings(hwnd, state),
        ID_LOG_CHECK if code == BN_CLICKED => {
            let checked = send(get(hwnd, ID_LOG_CHECK), BM_GETCHECK, 0, 0) == 1;
            state.settings.ui.log_output_enabled = checked;
            let _ = save_settings(state);
            push_event(SettingsUiEvent::GlobalSettingsChanged);
        }
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

fn refresh_all_controls(state: &mut SettingsUiState) {
    refresh_localized_texts(state);
    refresh_profile_list(state);
    refresh_profile_controls(state);
    refresh_global_controls(state);
    show_settings_panel(state, state.settings_panel_visible);
    show_hotkey_panel(state, state.hotkey_panel_visible);
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
    set_text(
        get(hwnd, ID_ADD_PROFILE),
        ui_text(lang, UiString::AddProfile),
    );
    set_text(
        get(hwnd, ID_DELETE_PROFILE),
        ui_text(lang, UiString::DeleteProfile),
    );
    set_text(get(hwnd, ID_HOTKEY_LABEL), ui_text(lang, UiString::Hotkey));
    set_text(get(hwnd, ID_HOTKEY_CHANGE), ui_text(lang, UiString::Change));
    set_text(get(hwnd, ID_SCALE_LABEL), ui_text(lang, UiString::Scale));
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
    set_text(get(hwnd, ID_LOG_CHECK), ui_text(lang, UiString::LogOutput));
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
    let current = profile_at(&state.settings, state.selected_index)
        .map(|profile| profile.windowed_hotkey.clone())
        .unwrap_or_else(|| state.settings.hotkeys.windowed_toggle.clone());
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
    let _ = send(list, LB_RESETCONTENT, 0, 0);
    for profile in profiles(&state.settings) {
        let name = wide_null(&profile.display_name);
        let _ = send(list, LB_ADDSTRING, 0, name.as_ptr() as isize);
    }
    let _ = send(list, LB_SETCURSEL, state.selected_index, 0);
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
    set_child_enabled(hwnd, ID_DELETE_PROFILE, state.selected_index > 0);
    state.loading = false;
}

fn show_profile_rename_edit(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let Some(profile) = profile_at(&state.settings, state.selected_index).cloned() else {
        return;
    };
    let layout = current_layout(hwnd);
    let y = layout.sidebar_y + (state.selected_index as i32 * 18).clamp(0, layout.sidebar_h - 28);
    let edit = get(hwnd, ID_NAME_EDIT);
    state.loading = true;
    unsafe {
        let _ = SetWindowPos(
            edit,
            None,
            layout.sidebar_x,
            y,
            layout.sidebar_w,
            24,
            SET_WINDOW_POS_FLAGS(SWP_NOZORDER.0),
        );
    }
    set_text(edit, &profile.display_name);
    show_child(hwnd, ID_NAME_EDIT, true);
    state.loading = false;
    unsafe {
        let _ = SetFocus(Some(edit));
    }
}

fn refresh_global_controls(state: &mut SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let combo = get(hwnd, ID_LANGUAGE_COMBO);
    state.loading = true;
    let korean = wide_null("한국어");
    let english = wide_null("English");
    let _ = send(combo, CB_RESETCONTENT, 0, 0);
    let _ = send(combo, CB_ADDSTRING, 0, korean.as_ptr() as isize);
    let _ = send(combo, CB_ADDSTRING, 0, english.as_ptr() as isize);
    let selected = if state.settings.ui.language.eq_ignore_ascii_case("en") {
        1
    } else {
        0
    };
    let _ = send(combo, CB_SETCURSEL, selected, 0);
    let checked = if state.settings.ui.log_output_enabled {
        1
    } else {
        0
    };
    let _ = send(get(hwnd, ID_LOG_CHECK), BM_SETCHECK, checked, 0);
    state.loading = false;
}

fn show_settings_panel(state: &mut SettingsUiState, visible: bool) {
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
    }
    for id in settings_panel_ids() {
        show_child(hwnd, *id, visible);
    }
    if visible {
        raise_panel_children(hwnd, settings_panel_ids());
    }
    update_modal_base_enabled(state);
    refresh_localized_texts(state);
    invalidate(hwnd);
}

fn toggle_settings_panel(state: &mut SettingsUiState) {
    let visible = !state.settings_panel_visible;
    show_settings_panel(state, visible);
}

fn show_hotkey_panel(state: &mut SettingsUiState, visible: bool) {
    state.hotkey_panel_visible = visible;
    HOTKEY_PANEL_PAINT_VISIBLE.store(visible, Ordering::Relaxed);
    let hwnd = hwnd_from_raw(state.hwnd);
    if visible {
        state.settings_panel_visible = false;
        SETTINGS_PANEL_PAINT_VISIBLE.store(false, Ordering::Relaxed);
        for id in settings_panel_ids() {
            show_child(hwnd, *id, false);
        }
        state.pending_hotkey = profile_at(&state.settings, state.selected_index)
            .map(|profile| profile.windowed_hotkey.clone());
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
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        profile.windowed_hotkey = hotkey.clone();
    }
    state.settings.hotkeys.windowed_toggle = hotkey;
    let _ = save_settings(state);
    show_hotkey_panel(state, false);
    refresh_profile_controls(state);
    push_event(SettingsUiEvent::HotkeysChanged);
}

fn apply_scale_from_edit(state: &mut SettingsUiState) {
    let raw = get_text(get(hwnd_from_raw(state.hwnd), ID_SCALE_EDIT));
    let Ok(value) = raw.trim().parse::<u32>() else {
        return;
    };
    if !(50..=500).contains(&value) {
        return;
    }
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        if profile.windowed_scale_percent != value {
            profile.windowed_scale_percent = value;
            let _ = save_settings(state);
            push_event(SettingsUiEvent::ProfileChanged);
        }
    }
}

fn adjust_scale(state: &mut SettingsUiState, delta: i32) {
    let value = profile_at(&state.settings, state.selected_index)
        .map(|profile| profile.windowed_scale_percent as i32)
        .unwrap_or(200);
    let next = (value + delta).clamp(50, 500) as u32;
    if let Some(profile) = selected_profile_mut(&mut state.settings, state.selected_index) {
        profile.windowed_scale_percent = next;
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
            if let Some(profile) = profile_at(&state.settings, state.selected_index) {
                state.settings.hotkeys.windowed_toggle = profile.windowed_hotkey.clone();
            }
            break;
        }
        next += 1;
    }
}

fn delete_selected_profile(state: &mut SettingsUiState) -> bool {
    if state.selected_index == 0 {
        return false;
    }
    let remove_index = state.selected_index - 1;
    if remove_index >= state.settings.profiles.per_app_profiles.len() {
        state.selected_index = selected_index_for_settings(&state.settings);
        return false;
    }

    state
        .settings
        .profiles
        .per_app_profiles
        .remove(remove_index);
    let profile_count = profiles(&state.settings).len();
    state.selected_index = if profile_count == 0 {
        0
    } else {
        state.selected_index.min(profile_count - 1)
    };
    if let Some(profile) = profile_at(&state.settings, state.selected_index).cloned() {
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
    true
}

fn save_settings(state: &SettingsUiState) -> Result<(), String> {
    save_settings_to_path(&state.settings, &state.paths.settings_file)
        .map_err(|error| format!("settings save failed: {error}"))
}

fn normalize_loaded_settings(settings: &mut DodbogiSettings) -> bool {
    let mut changed = false;
    if settings.profiles.default_profile.display_name.trim() == "Default profile" {
        settings.profiles.default_profile.display_name = "기본 프로파일".to_string();
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
    if !(50..=500).contains(&settings.profiles.default_profile.windowed_scale_percent) {
        settings.profiles.default_profile.windowed_scale_percent = 200;
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
        ID_SETTINGS_PANEL_BG,
        ID_SETTINGS_PANEL_TITLE,
        ID_LANGUAGE_COMBO,
        ID_RESET_BUTTON,
        ID_LOG_CHECK,
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
        ID_SETTINGS_PANEL_TITLE,
        ID_SETTINGS_LANGUAGE_LABEL,
        ID_LANGUAGE_COMBO,
        ID_RESET_BUTTON,
        ID_LOG_CHECK,
        ID_SETTINGS_CLOSE,
    ]
}

fn base_interaction_ids() -> &'static [i32] {
    &[
        ID_PROFILE_LIST,
        ID_ADD_PROFILE,
        ID_DELETE_PROFILE,
        ID_NAME_EDIT,
        ID_SETTINGS_BUTTON,
        ID_TRAY_BUTTON,
        ID_HOTKEY_CHANGE,
        ID_SCALE_EDIT,
        ID_SCALE_UP,
        ID_SCALE_DOWN,
    ]
}

fn hotkey_panel_ids() -> &'static [i32] {
    &[
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

fn update_modal_base_enabled(state: &SettingsUiState) {
    let hwnd = hwnd_from_raw(state.hwnd);
    let modal_active = state.settings_panel_visible || state.hotkey_panel_visible;
    for id in modal_covered_base_control_ids() {
        show_child(hwnd, *id, !modal_active);
    }
    for id in base_interaction_ids() {
        let enabled = !modal_active && (*id != ID_DELETE_PROFILE || state.selected_index > 0);
        set_child_enabled(hwnd, *id, enabled);
    }
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
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN | RDW_ERASENOW | RDW_UPDATENOW,
        );
        let _ = InvalidateRect(Some(hwnd), None, true);
        let _ = UpdateWindow(hwnd);
    }
}

fn get(parent: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(parent), id).unwrap_or_default() }
}

fn send(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    unsafe { SendMessageW(hwnd, msg, Some(WPARAM(wparam)), Some(LPARAM(lparam))).0 }
}

fn set_text(hwnd: HWND, text: &str) {
    let text = wide_null(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(text.as_ptr()));
        let _ = RedrawWindow(
            Some(hwnd),
            None,
            None,
            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
        );
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
