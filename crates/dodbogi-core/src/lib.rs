//! Core domain types for the Rust-first Magpie-parity window upscaler.
//!
//! This crate must stay independent from a specific UI shell. WinUI 3, if ever
//! introduced, is a shell-only fallback and must call into this Rust-owned core.

use std::{
    env, fmt, fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub const APP_NAME: &str = "Dodbogi";
pub const PARITY_TARGET: &str = "Magpie v0.12.1";
pub const MIN_WINDOWS_BUILD: u32 = 18362;
pub const MIN_DIRECTX_FEATURE_LEVEL: &str = "11_0";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportEnvelope {
    pub min_windows_build: u32,
    pub min_directx_feature_level: &'static str,
    pub description: &'static str,
}

impl Default for SupportEnvelope {
    fn default() -> Self {
        Self {
            min_windows_build: MIN_WINDOWS_BUILD,
            min_directx_feature_level: MIN_DIRECTX_FEATURE_LEVEL,
            description: "Windows 10 v1903+ / Windows 11 and DirectX feature level 11+",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckStatus {
    Passed,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupCheck {
    pub name: &'static str,
    pub status: CheckStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupReport {
    pub target: &'static str,
    pub envelope: SupportEnvelope,
    pub checks: Vec<StartupCheck>,
}

impl StartupReport {
    pub fn has_failures(&self) -> bool {
        self.checks
            .iter()
            .any(|check| check.status == CheckStatus::Failed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub root: PathBuf,
    pub config_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub settings_file: PathBuf,
    pub log_file: PathBuf,
}

impl RuntimePaths {
    pub fn discover() -> Self {
        let root = env::var_os("DODBOGI_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join(APP_NAME)))
            .unwrap_or_else(|| PathBuf::from(".").join(".dodbogi"));

        let config_dir = root.join("config");
        let logs_dir = root.join("logs");
        let cache_dir = root.join("cache");
        let settings_file = config_dir.join("settings.toml");
        let log_file = logs_dir.join("dodbogi.log");

        Self {
            root,
            config_dir,
            logs_dir,
            cache_dir,
            settings_file,
            log_file,
        }
    }

    pub fn ensure(&self) -> io::Result<()> {
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.logs_dir)?;
        fs::create_dir_all(&self.cache_dir)?;
        Ok(())
    }
}

pub fn write_default_settings_if_missing(paths: &RuntimePaths) -> io::Result<()> {
    if paths.settings_file.exists() {
        return Ok(());
    }

    fs::write(
        &paths.settings_file,
        DodbogiSettings::default().to_toml_string(),
    )
}

pub fn append_log_line(path: &Path, message: &str) -> io::Result<()> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut file| {
            use std::io::Write;
            writeln!(file, "{timestamp} {message}")
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl PhysicalRect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }

    pub fn height(self) -> i32 {
        self.bottom - self.top
    }

    pub fn is_empty(self) -> bool {
        self.width() <= 0 || self.height() <= 0
    }

    pub fn area(self) -> i64 {
        if self.is_empty() {
            0
        } else {
            i64::from(self.width()) * i64::from(self.height())
        }
    }

    pub fn center(self) -> (i32, i32) {
        (self.left + self.width() / 2, self.top + self.height() / 2)
    }

    pub fn intersect(self, other: Self) -> Option<Self> {
        let rect = Self {
            left: self.left.max(other.left),
            top: self.top.max(other.top),
            right: self.right.min(other.right),
            bottom: self.bottom.min(other.bottom),
        };
        (!rect.is_empty()).then_some(rect)
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            left: self.left.min(other.left),
            top: self.top.min(other.top),
            right: self.right.max(other.right),
            bottom: self.bottom.max(other.bottom),
        }
    }

    pub fn translated(self, dx: i32, dy: i32) -> Self {
        Self {
            left: self.left + dx,
            top: self.top + dy,
            right: self.right + dx,
            bottom: self.bottom + dy,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceWindow {
    pub hwnd: isize,
    pub rect: PhysicalRect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalingMode {
    Windowed,
    Fullscreen,
}

impl ScalingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Windowed => "windowed",
            Self::Fullscreen => "fullscreen",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "windowed" => Some(Self::Windowed),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dpi {
    pub x: u32,
    pub y: u32,
}

impl Default for Dpi {
    fn default() -> Self {
        Self { x: 96, y: 96 }
    }
}

impl Dpi {
    pub fn from_percent(percent: u32) -> Self {
        let dpi = ((u64::from(percent) * 96) / 100) as u32;
        Self { x: dpi, y: dpi }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorGeometry {
    pub id: u32,
    pub bounds: PhysicalRect,
    pub work_area: PhysicalRect,
    pub dpi: Dpi,
    pub primary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonitorSelectionMode {
    Closest,
    Intersected,
    All,
}

impl MonitorSelectionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Closest => "closest",
            Self::Intersected => "intersected",
            Self::All => "all",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "closest" => Some(Self::Closest),
            "intersected" => Some(Self::Intersected),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    WindowsGraphicsCapture,
    DesktopDuplication,
    Gdi,
    DwmFrameBounds,
}

impl CaptureMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WindowsGraphicsCapture => "windows_graphics_capture",
            Self::DesktopDuplication => "desktop_duplication",
            Self::Gdi => "gdi",
            Self::DwmFrameBounds => "dwm_frame_bounds",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "windows_graphics_capture" => Some(Self::WindowsGraphicsCapture),
            "desktop_duplication" => Some(Self::DesktopDuplication),
            "gdi" => Some(Self::Gdi),
            "dwm_frame_bounds" => Some(Self::DwmFrameBounds),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMatchRule {
    pub executable_name: Option<String>,
    pub window_class: Option<String>,
    pub title_contains: Option<String>,
}

impl ProfileMatchRule {
    pub fn empty() -> Self {
        Self {
            executable_name: None,
            window_class: None,
            title_contains: None,
        }
    }

    pub fn for_executable(executable_name: impl Into<String>) -> Self {
        Self {
            executable_name: Some(executable_name.into()),
            window_class: None,
            title_contains: None,
        }
    }

    pub fn score(&self, context: &ProfileMatchContext) -> Option<u32> {
        let mut score = 0;
        let mut has_rule = false;

        if let Some(expected) = &self.executable_name {
            has_rule = true;
            if context
                .executable_name
                .as_deref()
                .is_some_and(|actual| eq_ignore_ascii_case(actual, expected))
            {
                score += 100;
            } else {
                return None;
            }
        }

        if let Some(expected) = &self.window_class {
            has_rule = true;
            if context
                .window_class
                .as_deref()
                .is_some_and(|actual| eq_ignore_ascii_case(actual, expected))
            {
                score += 50;
            } else {
                return None;
            }
        }

        if let Some(expected) = &self.title_contains {
            has_rule = true;
            if context.title.as_deref().is_some_and(|actual| {
                actual
                    .to_ascii_lowercase()
                    .contains(&expected.to_ascii_lowercase())
            }) {
                score += 10;
            } else {
                return None;
            }
        }

        has_rule.then_some(score)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileMatchContext {
    pub executable_name: Option<String>,
    pub window_class: Option<String>,
    pub title: Option<String>,
}

impl ProfileMatchContext {
    pub fn for_executable(executable_name: impl Into<String>) -> Self {
        Self {
            executable_name: Some(executable_name.into()),
            window_class: None,
            title: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppProfile {
    pub id: String,
    pub display_name: String,
    pub match_rule: ProfileMatchRule,
    pub scaling_mode: ScalingMode,
    pub capture_method: CaptureMethod,
    pub windowed_scale_percent: u32,
    pub monitor_selection: MonitorSelectionMode,
    pub effect_chain: Vec<String>,
    pub capture_title_bar: bool,
    pub draw_cursor: bool,
    pub auto_scale: bool,
}

impl AppProfile {
    pub fn default_profile() -> Self {
        Self {
            id: "default".to_string(),
            display_name: "Default profile".to_string(),
            match_rule: ProfileMatchRule::empty(),
            scaling_mode: ScalingMode::Windowed,
            capture_method: CaptureMethod::WindowsGraphicsCapture,
            windowed_scale_percent: 200,
            monitor_selection: MonitorSelectionMode::Closest,
            effect_chain: vec!["bilinear".to_string()],
            capture_title_bar: true,
            draw_cursor: true,
            auto_scale: false,
        }
    }

    pub fn per_app_profile(
        id: impl Into<String>,
        display_name: impl Into<String>,
        executable_name: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            match_rule: ProfileMatchRule::for_executable(executable_name),
            ..Self::default_profile()
        }
    }

    pub fn windowed_scale_factor(&self) -> f32 {
        self.windowed_scale_percent as f32 / 100.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSet {
    pub active_profile_id: String,
    pub default_profile: AppProfile,
    pub per_app_profiles: Vec<AppProfile>,
}

impl Default for ProfileSet {
    fn default() -> Self {
        Self {
            active_profile_id: "default".to_string(),
            default_profile: AppProfile::default_profile(),
            per_app_profiles: Vec::new(),
        }
    }
}

impl ProfileSet {
    pub fn resolve<'a>(&'a self, context: &ProfileMatchContext) -> ProfileResolution<'a> {
        let mut best: Option<(&AppProfile, u32)> = None;
        for profile in &self.per_app_profiles {
            if let Some(score) = profile.match_rule.score(context) {
                match best {
                    Some((_, best_score)) if best_score >= score => {}
                    _ => best = Some((profile, score)),
                }
            }
        }

        if let Some((profile, score)) = best {
            ProfileResolution {
                profile,
                source: ProfileResolutionSource::PerApp,
                score,
            }
        } else {
            ProfileResolution {
                profile: &self.default_profile,
                source: ProfileResolutionSource::Default,
                score: 0,
            }
        }
    }

    pub fn all_profiles(&self) -> impl Iterator<Item = &AppProfile> {
        std::iter::once(&self.default_profile).chain(self.per_app_profiles.iter())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileResolutionSource {
    Default,
    PerApp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileResolution<'a> {
    pub profile: &'a AppProfile,
    pub source: ProfileResolutionSource,
    pub score: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotkeySettings {
    pub windowed_toggle: String,
    pub fullscreen_toggle: String,
    pub open_settings: String,
    pub screenshot: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            windowed_toggle: "Ctrl+Alt+Q".to_string(),
            fullscreen_toggle: "Ctrl+Alt+A".to_string(),
            open_settings: "Ctrl+Alt+S".to_string(),
            screenshot: "Ctrl+Alt+P".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsConfig {
    pub log_level: String,
    pub enable_stats_overlay: bool,
    pub keep_recent_logs: u32,
    pub screenshot_dir_name: String,
}

impl Default for DiagnosticsConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            enable_stats_overlay: false,
            keep_recent_logs: 10,
            screenshot_dir_name: "screenshots".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionKind {
    PortableZip,
}

impl DistributionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PortableZip => "portable_zip",
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value {
            "portable_zip" => Some(Self::PortableZip),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagingPlan {
    pub distribution: DistributionKind,
    pub binary_name: String,
    pub target_arch: String,
    pub manifest_embedded: bool,
    pub data_root_policy: String,
    pub reference_package_bundled: bool,
}

impl Default for PackagingPlan {
    fn default() -> Self {
        Self {
            distribution: DistributionKind::PortableZip,
            binary_name: "dodbogi.exe".to_string(),
            target_arch: "x86_64-pc-windows-msvc".to_string(),
            manifest_embedded: true,
            data_root_policy:
                "%LOCALAPPDATA%/Dodbogi by default; DODBOGI_DATA_DIR override for tests".to_string(),
            reference_package_bundled: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DodbogiSettings {
    pub version: u32,
    pub profiles: ProfileSet,
    pub hotkeys: HotkeySettings,
    pub diagnostics: DiagnosticsConfig,
    pub packaging: PackagingPlan,
}

impl Default for DodbogiSettings {
    fn default() -> Self {
        Self {
            version: 1,
            profiles: ProfileSet::default(),
            hotkeys: HotkeySettings::default(),
            diagnostics: DiagnosticsConfig::default(),
            packaging: PackagingPlan::default(),
        }
    }
}

impl DodbogiSettings {
    pub fn resolve_profile(&self, context: &ProfileMatchContext) -> ProfileResolution<'_> {
        self.profiles.resolve(context)
    }

    pub fn to_toml_string(&self) -> String {
        let mut output = String::new();
        output.push_str("# Dodbogi settings v1\n");
        push_kv(&mut output, "version", &self.version.to_string());
        push_kv_quoted(
            &mut output,
            "active_profile_id",
            &self.profiles.active_profile_id,
        );
        push_kv_quoted(
            &mut output,
            "hotkey_windowed",
            &self.hotkeys.windowed_toggle,
        );
        push_kv_quoted(
            &mut output,
            "hotkey_fullscreen",
            &self.hotkeys.fullscreen_toggle,
        );
        push_kv_quoted(
            &mut output,
            "hotkey_open_settings",
            &self.hotkeys.open_settings,
        );
        push_kv_quoted(&mut output, "hotkey_screenshot", &self.hotkeys.screenshot);
        push_kv_quoted(&mut output, "log_level", &self.diagnostics.log_level);
        push_kv(
            &mut output,
            "enable_stats_overlay",
            bool_setting(self.diagnostics.enable_stats_overlay),
        );
        push_kv(
            &mut output,
            "keep_recent_logs",
            &self.diagnostics.keep_recent_logs.to_string(),
        );
        push_kv_quoted(
            &mut output,
            "screenshot_dir_name",
            &self.diagnostics.screenshot_dir_name,
        );
        push_kv_quoted(
            &mut output,
            "distribution",
            self.packaging.distribution.as_str(),
        );
        push_kv_quoted(&mut output, "binary_name", &self.packaging.binary_name);
        push_kv_quoted(&mut output, "target_arch", &self.packaging.target_arch);
        push_kv(
            &mut output,
            "manifest_embedded",
            bool_setting(self.packaging.manifest_embedded),
        );
        push_kv_quoted(
            &mut output,
            "data_root_policy",
            &self.packaging.data_root_policy,
        );
        push_kv(
            &mut output,
            "reference_package_bundled",
            bool_setting(self.packaging.reference_package_bundled),
        );

        for profile in self.profiles.all_profiles() {
            output.push_str("\n[[profile]]\n");
            push_kv_quoted(&mut output, "id", &profile.id);
            push_kv_quoted(&mut output, "display_name", &profile.display_name);
            push_kv_quoted(
                &mut output,
                "executable_name",
                profile.match_rule.executable_name.as_deref().unwrap_or(""),
            );
            push_kv_quoted(
                &mut output,
                "window_class",
                profile.match_rule.window_class.as_deref().unwrap_or(""),
            );
            push_kv_quoted(
                &mut output,
                "title_contains",
                profile.match_rule.title_contains.as_deref().unwrap_or(""),
            );
            push_kv_quoted(&mut output, "scaling_mode", profile.scaling_mode.as_str());
            push_kv_quoted(
                &mut output,
                "capture_method",
                profile.capture_method.as_str(),
            );
            push_kv(
                &mut output,
                "windowed_scale_percent",
                &profile.windowed_scale_percent.to_string(),
            );
            push_kv_quoted(
                &mut output,
                "monitor_selection",
                profile.monitor_selection.as_str(),
            );
            push_kv_quoted(&mut output, "effect_chain", &profile.effect_chain.join(","));
            push_kv(
                &mut output,
                "capture_title_bar",
                bool_setting(profile.capture_title_bar),
            );
            push_kv(
                &mut output,
                "draw_cursor",
                bool_setting(profile.draw_cursor),
            );
            push_kv(&mut output, "auto_scale", bool_setting(profile.auto_scale));
        }
        output
    }

    pub fn from_toml_str(raw: &str) -> Result<Self, SettingsParseError> {
        parse_settings(raw)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsParseError {
    pub line: usize,
    pub detail: String,
}

impl fmt::Display for SettingsParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "settings parse error on line {}: {}",
            self.line, self.detail
        )
    }
}

impl std::error::Error for SettingsParseError {}

pub fn save_settings_to_path(settings: &DodbogiSettings, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, settings.to_toml_string())
}

pub fn load_settings_from_path(path: &Path) -> io::Result<DodbogiSettings> {
    let raw = fs::read_to_string(path)?;
    DodbogiSettings::from_toml_str(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn export_settings_to_path(settings: &DodbogiSettings, path: &Path) -> io::Result<()> {
    save_settings_to_path(settings, path)
}

pub fn import_settings_from_path(path: &Path) -> io::Result<DodbogiSettings> {
    load_settings_from_path(path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsUiSection {
    pub id: &'static str,
    pub covered: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsUiCoverageReport {
    pub sections: Vec<SettingsUiSection>,
}

impl SettingsUiCoverageReport {
    pub fn all_required_covered(&self) -> bool {
        self.sections.iter().all(|section| section.covered)
    }
}

pub fn settings_ui_coverage(settings: &DodbogiSettings) -> SettingsUiCoverageReport {
    let default = &settings.profiles.default_profile;
    SettingsUiCoverageReport {
        sections: vec![
            SettingsUiSection {
                id: "profiles.default",
                covered: !default.id.is_empty(),
                detail: default.display_name.clone(),
            },
            SettingsUiSection {
                id: "profiles.per_app",
                covered: true,
                detail: format!("{} configured", settings.profiles.per_app_profiles.len()),
            },
            SettingsUiSection {
                id: "scaling.mode",
                covered: true,
                detail: default.scaling_mode.as_str().to_string(),
            },
            SettingsUiSection {
                id: "capture.method",
                covered: true,
                detail: default.capture_method.as_str().to_string(),
            },
            SettingsUiSection {
                id: "effects.chain",
                covered: !default.effect_chain.is_empty(),
                detail: default.effect_chain.join(","),
            },
            SettingsUiSection {
                id: "hotkeys",
                covered: !settings.hotkeys.windowed_toggle.is_empty()
                    && !settings.hotkeys.fullscreen_toggle.is_empty(),
                detail: format!(
                    "{}/{}",
                    settings.hotkeys.windowed_toggle, settings.hotkeys.fullscreen_toggle
                ),
            },
            SettingsUiSection {
                id: "diagnostics",
                covered: !settings.diagnostics.log_level.is_empty(),
                detail: settings.diagnostics.log_level.clone(),
            },
            SettingsUiSection {
                id: "tray.menu",
                covered: true,
                detail: "start/stop, profile, screenshot, settings, diagnostics, exit".to_string(),
            },
            SettingsUiSection {
                id: "packaging",
                covered: settings.packaging.manifest_embedded,
                detail: settings.packaging.distribution.as_str().to_string(),
            },
        ],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSnapshot {
    pub profile_count: usize,
    pub per_app_profile_count: usize,
    pub settings_file: PathBuf,
    pub log_file: PathBuf,
    pub cache_dir: PathBuf,
    pub support_envelope: String,
}

impl DiagnosticsSnapshot {
    pub fn capture(paths: &RuntimePaths, settings: &DodbogiSettings) -> Self {
        Self {
            profile_count: settings.profiles.all_profiles().count(),
            per_app_profile_count: settings.profiles.per_app_profiles.len(),
            settings_file: paths.settings_file.clone(),
            log_file: paths.log_file.clone(),
            cache_dir: paths.cache_dir.clone(),
            support_envelope: SupportEnvelope::default().description.to_string(),
        }
    }
}

fn bool_setting(value: bool) -> &'static str {
    if value {
        "true"
    } else {
        "false"
    }
}

fn push_kv(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = ");
    output.push_str(value);
    output.push('\n');
}

fn push_kv_quoted(output: &mut String, key: &str, value: &str) {
    output.push_str(key);
    output.push_str(" = \"");
    output.push_str(&quote_setting(value));
    output.push_str("\"\n");
}

fn quote_setting(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn unquote_setting(value: &str) -> String {
    let trimmed = value.trim();
    let unquoted = trimmed
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(trimmed);
    let mut out = String::new();
    let mut escaped = false;
    for ch in unquoted.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_bool(value: &str, line: usize) -> Result<bool, SettingsParseError> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        other => Err(SettingsParseError {
            line,
            detail: format!("invalid boolean {other}"),
        }),
    }
}

fn parse_u32(value: &str, line: usize, key: &str) -> Result<u32, SettingsParseError> {
    value
        .trim()
        .parse::<u32>()
        .map_err(|error| SettingsParseError {
            line,
            detail: format!("invalid integer for {key}: {error}"),
        })
}

fn parse_settings(raw: &str) -> Result<DodbogiSettings, SettingsParseError> {
    let mut settings = DodbogiSettings::default();
    let mut profiles = Vec::<AppProfile>::new();
    let mut current_profile: Option<AppProfile> = None;

    for (index, line) in raw.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed == "[[profile]]" {
            if let Some(profile) = current_profile.take() {
                profiles.push(profile);
            }
            current_profile = Some(AppProfile::default_profile());
            continue;
        }
        let (key, value) = trimmed.split_once('=').ok_or_else(|| SettingsParseError {
            line: line_no,
            detail: "expected key = value".to_string(),
        })?;
        let key = key.trim();
        let value = value.trim();

        if let Some(profile) = current_profile.as_mut() {
            parse_profile_key(profile, key, value, line_no)?;
        } else {
            parse_global_key(&mut settings, key, value, line_no)?;
        }
    }

    if let Some(profile) = current_profile.take() {
        profiles.push(profile);
    }

    if !profiles.is_empty() {
        let default_index = profiles
            .iter()
            .position(|profile| profile.id == "default")
            .unwrap_or(0);
        settings.profiles.default_profile = profiles.remove(default_index);
        settings.profiles.per_app_profiles = profiles
            .into_iter()
            .filter(|profile| profile.id != settings.profiles.default_profile.id)
            .collect();
    }

    Ok(settings)
}

fn parse_global_key(
    settings: &mut DodbogiSettings,
    key: &str,
    value: &str,
    line: usize,
) -> Result<(), SettingsParseError> {
    match key {
        "version" => settings.version = parse_u32(value, line, key)?,
        "active_profile_id" => settings.profiles.active_profile_id = unquote_setting(value),
        "hotkey_windowed" => settings.hotkeys.windowed_toggle = unquote_setting(value),
        "hotkey_fullscreen" => settings.hotkeys.fullscreen_toggle = unquote_setting(value),
        "hotkey_open_settings" => settings.hotkeys.open_settings = unquote_setting(value),
        "hotkey_screenshot" => settings.hotkeys.screenshot = unquote_setting(value),
        "log_level" => settings.diagnostics.log_level = unquote_setting(value),
        "enable_stats_overlay" => {
            settings.diagnostics.enable_stats_overlay = parse_bool(value, line)?
        }
        "keep_recent_logs" => settings.diagnostics.keep_recent_logs = parse_u32(value, line, key)?,
        "screenshot_dir_name" => settings.diagnostics.screenshot_dir_name = unquote_setting(value),
        "distribution" => {
            settings.packaging.distribution =
                DistributionKind::from_setting(&unquote_setting(value)).ok_or_else(|| {
                    SettingsParseError {
                        line,
                        detail: "unknown distribution".to_string(),
                    }
                })?
        }
        "binary_name" => settings.packaging.binary_name = unquote_setting(value),
        "target_arch" => settings.packaging.target_arch = unquote_setting(value),
        "manifest_embedded" => settings.packaging.manifest_embedded = parse_bool(value, line)?,
        "data_root_policy" => settings.packaging.data_root_policy = unquote_setting(value),
        "reference_package_bundled" => {
            settings.packaging.reference_package_bundled = parse_bool(value, line)?
        }
        other => {
            return Err(SettingsParseError {
                line,
                detail: format!("unknown global settings key {other}"),
            })
        }
    }
    Ok(())
}

fn parse_profile_key(
    profile: &mut AppProfile,
    key: &str,
    value: &str,
    line: usize,
) -> Result<(), SettingsParseError> {
    match key {
        "id" => profile.id = unquote_setting(value),
        "display_name" => profile.display_name = unquote_setting(value),
        "executable_name" => {
            profile.match_rule.executable_name = non_empty_string(unquote_setting(value))
        }
        "window_class" => {
            profile.match_rule.window_class = non_empty_string(unquote_setting(value))
        }
        "title_contains" => {
            profile.match_rule.title_contains = non_empty_string(unquote_setting(value))
        }
        "scaling_mode" => {
            profile.scaling_mode =
                ScalingMode::from_setting(&unquote_setting(value)).ok_or_else(|| {
                    SettingsParseError {
                        line,
                        detail: "unknown scaling_mode".to_string(),
                    }
                })?
        }
        "capture_method" => {
            profile.capture_method = CaptureMethod::from_setting(&unquote_setting(value))
                .ok_or_else(|| SettingsParseError {
                    line,
                    detail: "unknown capture_method".to_string(),
                })?
        }
        "windowed_scale_percent" => {
            profile.windowed_scale_percent = parse_u32(value, line, key)?;
        }
        "monitor_selection" => {
            profile.monitor_selection = MonitorSelectionMode::from_setting(&unquote_setting(value))
                .ok_or_else(|| SettingsParseError {
                    line,
                    detail: "unknown monitor_selection".to_string(),
                })?
        }
        "effect_chain" => {
            profile.effect_chain = unquote_setting(value)
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .collect();
        }
        "capture_title_bar" => profile.capture_title_bar = parse_bool(value, line)?,
        "draw_cursor" => profile.draw_cursor = parse_bool(value, line)?,
        "auto_scale" => profile.auto_scale = parse_bool(value, line)?,
        other => {
            return Err(SettingsParseError {
                line,
                detail: format!("unknown profile settings key {other}"),
            })
        }
    }
    Ok(())
}

fn non_empty_string(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn eq_ignore_ascii_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutRequest {
    pub mode: ScalingMode,
    pub monitor_selection: MonitorSelectionMode,
    pub windowed_scale: f32,
}

impl Default for LayoutRequest {
    fn default() -> Self {
        Self {
            mode: ScalingMode::Windowed,
            monitor_selection: MonitorSelectionMode::Closest,
            windowed_scale: 2.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalingLayout {
    pub source: PhysicalRect,
    pub destination: PhysicalRect,
    pub monitor_ids: Vec<u32>,
    pub dpi: Dpi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryError {
    EmptySourceRect,
    NoMonitors,
    InvalidWindowedScale,
}

pub fn logical_to_physical_px(logical: i32, dpi: u32) -> i32 {
    ((f64::from(logical) * f64::from(dpi)) / 96.0).round() as i32
}

pub fn physical_to_logical_px(physical: i32, dpi: u32) -> i32 {
    ((f64::from(physical) * 96.0) / f64::from(dpi)).round() as i32
}

pub fn compute_scaling_layout(
    source: PhysicalRect,
    monitors: &[MonitorGeometry],
    request: LayoutRequest,
) -> Result<ScalingLayout, GeometryError> {
    if source.is_empty() {
        return Err(GeometryError::EmptySourceRect);
    }
    if monitors.is_empty() {
        return Err(GeometryError::NoMonitors);
    }

    let closest = closest_monitor(source, monitors)?;
    let selected = selected_monitors(source, monitors, request.monitor_selection, closest.id);
    let monitor_ids = selected
        .iter()
        .map(|monitor| monitor.id)
        .collect::<Vec<_>>();
    let dpi = closest.dpi;

    let destination = match request.mode {
        ScalingMode::Fullscreen => selected
            .iter()
            .map(|monitor| monitor.bounds)
            .reduce(PhysicalRect::union)
            .ok_or(GeometryError::NoMonitors)?,
        ScalingMode::Windowed => {
            if !request.windowed_scale.is_finite() || request.windowed_scale <= 0.0 {
                return Err(GeometryError::InvalidWindowedScale);
            }
            let work_area = selected
                .iter()
                .map(|monitor| monitor.work_area)
                .reduce(PhysicalRect::union)
                .ok_or(GeometryError::NoMonitors)?;
            let width = (source.width() as f32 * request.windowed_scale).round() as i32;
            let height = (source.height() as f32 * request.windowed_scale).round() as i32;
            fit_rect_within_area(
                PhysicalRect {
                    left: source.left,
                    top: source.top,
                    right: source.left + width,
                    bottom: source.top + height,
                },
                work_area,
            )
        }
    };

    Ok(ScalingLayout {
        source,
        destination,
        monitor_ids,
        dpi,
    })
}

pub fn windowed_work_area_for_source(
    source: PhysicalRect,
    monitors: &[MonitorGeometry],
    monitor_selection: MonitorSelectionMode,
) -> Result<PhysicalRect, GeometryError> {
    if source.is_empty() {
        return Err(GeometryError::EmptySourceRect);
    }
    if monitors.is_empty() {
        return Err(GeometryError::NoMonitors);
    }

    let closest = closest_monitor(source, monitors)?;
    selected_monitors(source, monitors, monitor_selection, closest.id)
        .iter()
        .map(|monitor| monitor.work_area)
        .reduce(PhysicalRect::union)
        .ok_or(GeometryError::NoMonitors)
}

fn selected_monitors(
    source: PhysicalRect,
    monitors: &[MonitorGeometry],
    mode: MonitorSelectionMode,
    closest_id: u32,
) -> Vec<&MonitorGeometry> {
    match mode {
        MonitorSelectionMode::Closest => monitors
            .iter()
            .filter(|monitor| monitor.id == closest_id)
            .collect(),
        MonitorSelectionMode::Intersected => {
            let intersected = monitors
                .iter()
                .filter(|monitor| source.intersect(monitor.bounds).is_some())
                .collect::<Vec<_>>();
            if intersected.is_empty() {
                monitors
                    .iter()
                    .filter(|monitor| monitor.id == closest_id)
                    .collect()
            } else {
                intersected
            }
        }
        MonitorSelectionMode::All => monitors.iter().collect(),
    }
}

fn closest_monitor(
    source: PhysicalRect,
    monitors: &[MonitorGeometry],
) -> Result<&MonitorGeometry, GeometryError> {
    let source_center = source.center();
    monitors
        .iter()
        .max_by(|left, right| {
            let left_intersection = source.intersect(left.bounds).map_or(0, PhysicalRect::area);
            let right_intersection = source.intersect(right.bounds).map_or(0, PhysicalRect::area);
            left_intersection
                .cmp(&right_intersection)
                .then_with(|| {
                    distance_score(source_center, right.bounds.center())
                        .cmp(&distance_score(source_center, left.bounds.center()))
                })
                .then_with(|| left.primary.cmp(&right.primary))
                .then_with(|| right.id.cmp(&left.id))
        })
        .ok_or(GeometryError::NoMonitors)
}

fn distance_score(left: (i32, i32), right: (i32, i32)) -> i64 {
    let dx = i64::from(left.0 - right.0);
    let dy = i64::from(left.1 - right.1);
    dx * dx + dy * dy
}

fn fit_rect_within_area(rect: PhysicalRect, area: PhysicalRect) -> PhysicalRect {
    let width = rect.width().min(area.width()).max(1);
    let height = rect.height().min(area.height()).max(1);
    let mut adjusted = PhysicalRect {
        left: rect.left,
        top: rect.top,
        right: rect.left + width,
        bottom: rect.top + height,
    };

    if adjusted.right > area.right {
        adjusted = adjusted.translated(area.right - adjusted.right, 0);
    }
    if adjusted.bottom > area.bottom {
        adjusted = adjusted.translated(0, area.bottom - adjusted.bottom);
    }
    if adjusted.left < area.left {
        adjusted = adjusted.translated(area.left - adjusted.left, 0);
    }
    if adjusted.top < area.top {
        adjusted = adjusted.translated(0, area.top - adjusted.top);
    }

    adjusted
}

/// Preserve a windowed scaling window's current placement when the source
/// window only moved.
///
/// Magpie keeps the scaling window size stable during a source title-bar drag
/// and moves it by the same source delta instead of recomputing the initial
/// windowed layout and clamping it back into the monitor work area every frame.
/// Returning `None` for size changes keeps true resizes on the normal layout
/// recomputation path.
pub fn translate_destination_for_source_move(
    previous_source: PhysicalRect,
    current_source: PhysicalRect,
    previous_destination: PhysicalRect,
) -> Option<PhysicalRect> {
    if previous_source.is_empty()
        || current_source.is_empty()
        || previous_destination.is_empty()
        || previous_source.width() != current_source.width()
        || previous_source.height() != current_source.height()
        || previous_source == current_source
    {
        return None;
    }

    Some(previous_destination.translated(
        current_source.left - previous_source.left,
        current_source.top - previous_source.top,
    ))
}

/// Preserve the current windowed scaling placement across source moves and
/// source edge/corner resizes.
///
/// The initial layout path may clamp a newly-created windowed destination into
/// the work area, but live source changes must not repeat that initial clamp.
/// Repeating it causes 1px source resize deltas at a bottom/right edge to jump
/// the whole enlarged destination back upward/leftward. Instead, keep the
/// previous scale and anchor unchanged source edges:
///
/// - move-only: translate the destination by the source delta;
/// - right/bottom resize: keep left/top fixed;
/// - left/top resize: keep right/bottom fixed;
/// - ambiguous resize+move: keep the previous center relation.
pub fn preserve_windowed_destination_for_source_change(
    previous_source: PhysicalRect,
    current_source: PhysicalRect,
    previous_destination: PhysicalRect,
) -> Option<PhysicalRect> {
    if previous_source.is_empty()
        || current_source.is_empty()
        || previous_destination.is_empty()
        || previous_source == current_source
    {
        return None;
    }

    if previous_source.width() == current_source.width()
        && previous_source.height() == current_source.height()
    {
        return translate_destination_for_source_move(
            previous_source,
            current_source,
            previous_destination,
        );
    }

    let scale_x = previous_destination.width() as f64 / previous_source.width() as f64;
    let scale_y = previous_destination.height() as f64 / previous_source.height() as f64;
    if !scale_x.is_finite() || !scale_y.is_finite() || scale_x <= 0.0 || scale_y <= 0.0 {
        return None;
    }

    let new_width = ((current_source.width() as f64 * scale_x).round() as i32).max(1);
    let new_height = ((current_source.height() as f64 * scale_y).round() as i32).max(1);

    let (left, right) = anchored_axis(
        previous_source.left,
        previous_source.right,
        current_source.left,
        current_source.right,
        previous_destination.left,
        previous_destination.right,
        new_width,
        scale_x,
    );
    let (top, bottom) = anchored_axis(
        previous_source.top,
        previous_source.bottom,
        current_source.top,
        current_source.bottom,
        previous_destination.top,
        previous_destination.bottom,
        new_height,
        scale_y,
    );

    Some(PhysicalRect {
        left,
        top,
        right,
        bottom,
    })
}

/// Preserve the Magpie-like live-resize anchor, but keep the visible scaling
/// surface inside the selected work area when the source grows beyond the
/// monitor. This is intentionally a separate step from
/// `preserve_windowed_destination_for_source_change`: normal edge drags should
/// not be repeatedly clamped, but once the enlarged destination would leave the
/// visible work area Windows clips the right/bottom side. In that case the only
/// lossless visible policy is to reduce the live scale uniformly and translate
/// the destination back into the work area.
pub fn preserve_windowed_destination_for_source_change_in_work_area(
    previous_source: PhysicalRect,
    current_source: PhysicalRect,
    previous_destination: PhysicalRect,
    work_area: PhysicalRect,
) -> Option<(PhysicalRect, bool)> {
    let preserved = preserve_windowed_destination_for_source_change(
        previous_source,
        current_source,
        previous_destination,
    )?;
    let fitted = fit_preserved_windowed_destination_to_work_area(
        previous_source,
        current_source,
        previous_destination,
        preserved,
        work_area,
    );
    Some((fitted, fitted != preserved))
}

fn fit_preserved_windowed_destination_to_work_area(
    previous_source: PhysicalRect,
    current_source: PhysicalRect,
    previous_destination: PhysicalRect,
    preserved_destination: PhysicalRect,
    work_area: PhysicalRect,
) -> PhysicalRect {
    if previous_source.is_empty()
        || current_source.is_empty()
        || previous_destination.is_empty()
        || preserved_destination.is_empty()
        || work_area.is_empty()
    {
        return preserved_destination;
    }

    if preserved_destination.left >= work_area.left
        && preserved_destination.top >= work_area.top
        && preserved_destination.right <= work_area.right
        && preserved_destination.bottom <= work_area.bottom
        && preserved_destination.width() <= work_area.width()
        && preserved_destination.height() <= work_area.height()
    {
        return preserved_destination;
    }

    let previous_scale_x = previous_destination.width() as f64 / previous_source.width() as f64;
    let previous_scale_y = previous_destination.height() as f64 / previous_source.height() as f64;
    let fit_scale_x = work_area.width() as f64 / current_source.width() as f64;
    let fit_scale_y = work_area.height() as f64 / current_source.height() as f64;
    let scale = previous_scale_x
        .min(previous_scale_y)
        .min(fit_scale_x)
        .min(fit_scale_y);

    if !scale.is_finite() || scale <= 0.0 {
        return fit_rect_within_area(preserved_destination, work_area);
    }

    let new_width =
        ((current_source.width() as f64 * scale).round() as i32).clamp(1, work_area.width().max(1));
    let new_height = ((current_source.height() as f64 * scale).round() as i32)
        .clamp(1, work_area.height().max(1));

    let (left, right) = anchored_axis(
        previous_source.left,
        previous_source.right,
        current_source.left,
        current_source.right,
        previous_destination.left,
        previous_destination.right,
        new_width,
        scale,
    );
    let (top, bottom) = anchored_axis(
        previous_source.top,
        previous_source.bottom,
        current_source.top,
        current_source.bottom,
        previous_destination.top,
        previous_destination.bottom,
        new_height,
        scale,
    );

    fit_rect_within_area(
        PhysicalRect {
            left,
            top,
            right,
            bottom,
        },
        work_area,
    )
}

fn anchored_axis(
    previous_start: i32,
    previous_end: i32,
    current_start: i32,
    current_end: i32,
    previous_destination_start: i32,
    previous_destination_end: i32,
    new_destination_size: i32,
    scale: f64,
) -> (i32, i32) {
    let previous_size = previous_end - previous_start;
    let current_size = current_end - current_start;

    if previous_size == current_size {
        let delta = current_start - previous_start;
        let start = previous_destination_start + delta;
        return (start, start + new_destination_size);
    }

    if current_start == previous_start {
        let start = previous_destination_start;
        return (start, start + new_destination_size);
    }

    if current_end == previous_end {
        let end = previous_destination_end;
        return (end - new_destination_size, end);
    }

    let previous_center = (previous_start as f64 + previous_end as f64) / 2.0;
    let current_center = (current_start as f64 + current_end as f64) / 2.0;
    let previous_destination_center =
        (previous_destination_start as f64 + previous_destination_end as f64) / 2.0;
    let center = previous_destination_center + (current_center - previous_center) * scale;
    let start = (center - new_destination_size as f64 / 2.0).round() as i32;
    (start, start + new_destination_size)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    UserToggle,
    SourceClosed,
    SourceMinimized,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceWindowEvent {
    Unchanged,
    MovedOrResized { rect: PhysicalRect },
    Minimized,
    Closed,
    FocusLost,
    PopupOpened,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayLifecycleAction {
    KeepCurrentLayout,
    RecomputeLayout { rect: PhysicalRect },
    Stop { reason: StopReason },
    PreserveNoActivateFocus,
    AllowPopupWithoutActivation,
}

pub fn evaluate_source_window_event(
    current: SourceWindow,
    event: SourceWindowEvent,
) -> OverlayLifecycleAction {
    match event {
        SourceWindowEvent::Unchanged => OverlayLifecycleAction::KeepCurrentLayout,
        SourceWindowEvent::MovedOrResized { rect } => {
            if rect == current.rect {
                OverlayLifecycleAction::KeepCurrentLayout
            } else {
                OverlayLifecycleAction::RecomputeLayout { rect }
            }
        }
        SourceWindowEvent::Minimized => OverlayLifecycleAction::Stop {
            reason: StopReason::SourceMinimized,
        },
        SourceWindowEvent::Closed => OverlayLifecycleAction::Stop {
            reason: StopReason::SourceClosed,
        },
        SourceWindowEvent::FocusLost => OverlayLifecycleAction::PreserveNoActivateFocus,
        SourceWindowEvent::PopupOpened => OverlayLifecycleAction::AllowPopupWithoutActivation,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    WaitingForSource {
        mode: ScalingMode,
    },
    Scaling {
        mode: ScalingMode,
        source: SourceWindow,
    },
    Stopped {
        reason: StopReason,
    },
    Failed {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalingSession {
    state: SessionState,
}

impl Default for ScalingSession {
    fn default() -> Self {
        Self {
            state: SessionState::Idle,
        }
    }
}

impl ScalingSession {
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    pub fn begin_waiting(&mut self, mode: ScalingMode) -> Result<(), &'static str> {
        match self.state {
            SessionState::Idle | SessionState::Stopped { .. } | SessionState::Failed { .. } => {
                self.state = SessionState::WaitingForSource { mode };
                Ok(())
            }
            _ => Err("session is already active"),
        }
    }

    pub fn start_scaling(&mut self, source: SourceWindow) -> Result<(), &'static str> {
        let mode = match self.state {
            SessionState::WaitingForSource { mode } => mode,
            _ => return Err("session is not waiting for a source"),
        };

        if source.hwnd == 0 || source.rect.is_empty() {
            return Err("source window is invalid");
        }

        self.state = SessionState::Scaling { mode, source };
        Ok(())
    }

    pub fn stop(&mut self, reason: StopReason) {
        self.state = SessionState::Stopped { reason };
    }

    pub fn fail(&mut self, detail: impl Into<String>) {
        self.state = SessionState::Failed {
            detail: detail.into(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_envelope_matches_reference_floor() {
        let envelope = SupportEnvelope::default();
        assert_eq!(envelope.min_windows_build, 18362);
        assert_eq!(envelope.min_directx_feature_level, "11_0");
    }

    #[test]
    fn scaling_session_transitions_idle_waiting_scaling_stopped() {
        let mut session = ScalingSession::default();
        session.begin_waiting(ScalingMode::Windowed).unwrap();
        assert_eq!(
            session.state(),
            &SessionState::WaitingForSource {
                mode: ScalingMode::Windowed
            }
        );

        session
            .start_scaling(SourceWindow {
                hwnd: 100,
                rect: PhysicalRect {
                    left: 0,
                    top: 0,
                    right: 640,
                    bottom: 480,
                },
            })
            .unwrap();

        assert!(matches!(session.state(), SessionState::Scaling { .. }));
        session.stop(StopReason::UserToggle);
        assert_eq!(
            session.state(),
            &SessionState::Stopped {
                reason: StopReason::UserToggle
            }
        );
    }

    #[test]
    fn scaling_session_rejects_invalid_source() {
        let mut session = ScalingSession::default();
        session.begin_waiting(ScalingMode::Fullscreen).unwrap();
        let result = session.start_scaling(SourceWindow {
            hwnd: 0,
            rect: PhysicalRect {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            },
        });
        assert_eq!(result, Err("source window is invalid"));
    }

    #[test]
    fn dpi_conversion_preserves_physical_pixels_for_common_scales() {
        assert_eq!(logical_to_physical_px(800, 96), 800);
        assert_eq!(logical_to_physical_px(800, 120), 1000);
        assert_eq!(logical_to_physical_px(800, 144), 1200);
        assert_eq!(physical_to_logical_px(1200, 144), 800);
    }

    #[test]
    fn closest_monitor_prefers_largest_intersection_then_distance() {
        let monitors = fixture_monitors();
        let source = PhysicalRect {
            left: 1800,
            top: 100,
            right: 2300,
            bottom: 500,
        };

        let layout = compute_scaling_layout(
            source,
            &monitors,
            LayoutRequest {
                mode: ScalingMode::Fullscreen,
                monitor_selection: MonitorSelectionMode::Closest,
                windowed_scale: 2.0,
            },
        )
        .unwrap();

        assert_eq!(layout.monitor_ids, vec![2]);
        assert_eq!(layout.destination, monitors[1].bounds);
    }

    #[test]
    fn fullscreen_intersected_uses_union_of_intersected_monitors() {
        let monitors = fixture_monitors();
        let source = PhysicalRect {
            left: 1800,
            top: 100,
            right: 2300,
            bottom: 500,
        };

        let layout = compute_scaling_layout(
            source,
            &monitors,
            LayoutRequest {
                mode: ScalingMode::Fullscreen,
                monitor_selection: MonitorSelectionMode::Intersected,
                windowed_scale: 2.0,
            },
        )
        .unwrap();

        assert_eq!(layout.monitor_ids, vec![1, 2]);
        assert_eq!(
            layout.destination,
            monitors[0].bounds.union(monitors[1].bounds)
        );
    }

    #[test]
    fn windowed_layout_scales_two_x_and_stays_in_work_area() {
        let monitors = fixture_monitors();
        let source = PhysicalRect {
            left: 1750,
            top: 850,
            right: 1950,
            bottom: 1000,
        };

        let layout = compute_scaling_layout(source, &monitors, LayoutRequest::default()).unwrap();

        assert_eq!(layout.destination.width(), 400);
        assert_eq!(layout.destination.height(), 300);
        assert_eq!(layout.destination.right, monitors[0].work_area.right);
        assert_eq!(layout.destination.bottom, monitors[0].work_area.bottom);
    }

    #[test]
    fn source_move_translation_preserves_vertical_delta_after_initial_clamp() {
        let previous_source = PhysicalRect {
            left: 35,
            top: 308,
            right: 999,
            bottom: 888,
        };
        let current_source = PhysicalRect {
            left: 35,
            top: 345,
            right: 999,
            bottom: 925,
        };
        let previous_destination = PhysicalRect {
            left: 0,
            top: 0,
            right: 1856,
            bottom: 1080,
        };

        let translated = translate_destination_for_source_move(
            previous_source,
            current_source,
            previous_destination,
        )
        .expect("move-only source change should translate the destination");

        assert_eq!(translated.top, 37);
        assert_eq!(translated.bottom, 1117);
        assert_eq!(translated.width(), previous_destination.width());
        assert_eq!(translated.height(), previous_destination.height());
    }

    #[test]
    fn source_move_translation_rejects_resize() {
        let previous_source = PhysicalRect {
            left: 35,
            top: 308,
            right: 999,
            bottom: 888,
        };
        let resized_source = PhysicalRect {
            left: 35,
            top: 308,
            right: 1009,
            bottom: 888,
        };
        let previous_destination = PhysicalRect {
            left: 0,
            top: 0,
            right: 1856,
            bottom: 1080,
        };

        assert_eq!(
            translate_destination_for_source_move(
                previous_source,
                resized_source,
                previous_destination
            ),
            None
        );
    }

    #[test]
    fn windowed_resize_preserves_bottom_right_anchor_without_work_area_reclamp() {
        let previous_source = PhysicalRect {
            left: 327,
            top: 420,
            right: 962,
            bottom: 867,
        };
        let current_source = PhysicalRect {
            left: 327,
            top: 420,
            right: 962,
            bottom: 866,
        };
        let previous_destination = PhysicalRect {
            left: 327,
            top: 72,
            right: 1597,
            bottom: 966,
        };

        let preserved = preserve_windowed_destination_for_source_change(
            previous_source,
            current_source,
            previous_destination,
        )
        .expect("edge resize should preserve the previous destination anchor");

        assert_eq!(preserved.left, previous_destination.left);
        assert_eq!(preserved.top, previous_destination.top);
        assert_eq!(preserved.right, previous_destination.right);
        assert_eq!(preserved.bottom, 964);
        assert_eq!(preserved.height(), current_source.height() * 2);
    }

    #[test]
    fn windowed_resize_from_top_left_preserves_opposite_edges() {
        let previous_source = PhysicalRect {
            left: 300,
            top: 300,
            right: 900,
            bottom: 700,
        };
        let current_source = PhysicalRect {
            left: 320,
            top: 330,
            right: 900,
            bottom: 700,
        };
        let previous_destination = PhysicalRect {
            left: 100,
            top: 80,
            right: 1300,
            bottom: 880,
        };

        let preserved = preserve_windowed_destination_for_source_change(
            previous_source,
            current_source,
            previous_destination,
        )
        .expect("top-left resize should keep bottom-right destination anchored");

        assert_eq!(preserved.right, previous_destination.right);
        assert_eq!(preserved.bottom, previous_destination.bottom);
        assert_eq!(preserved.width(), current_source.width() * 2);
        assert_eq!(preserved.height(), current_source.height() * 2);
    }

    #[test]
    fn windowed_resize_fit_prevents_right_side_work_area_clip() {
        let previous_source = PhysicalRect {
            left: 327,
            top: 420,
            right: 962,
            bottom: 867,
        };
        let current_source = PhysicalRect {
            left: 327,
            top: 420,
            right: 1300,
            bottom: 867,
        };
        let previous_destination = PhysicalRect {
            left: 327,
            top: 72,
            right: 1597,
            bottom: 966,
        };
        let work_area = PhysicalRect {
            left: 0,
            top: 0,
            right: 1856,
            bottom: 1080,
        };

        let preserved = preserve_windowed_destination_for_source_change(
            previous_source,
            current_source,
            previous_destination,
        )
        .expect("right-edge resize should produce a preserved destination first");
        assert!(
            preserved.right > work_area.right,
            "fixture must reproduce the off-screen clipping case"
        );

        let (fitted, fitted_to_work_area) =
            preserve_windowed_destination_for_source_change_in_work_area(
                previous_source,
                current_source,
                previous_destination,
                work_area,
            )
            .expect("live resize should still preserve a destination");

        assert!(fitted_to_work_area);
        assert!(fitted.left >= work_area.left);
        assert!(fitted.top >= work_area.top);
        assert!(fitted.right <= work_area.right);
        assert!(fitted.bottom <= work_area.bottom);
        assert_eq!(fitted.width(), work_area.width());
        assert!(
            (fitted.width() * current_source.height() - fitted.height() * current_source.width())
                .abs()
                <= current_source.width(),
            "fit should preserve source aspect ratio within integer rounding"
        );
    }

    #[test]
    fn windowed_resize_fit_leaves_visible_preservation_unchanged() {
        let previous_source = PhysicalRect {
            left: 300,
            top: 300,
            right: 900,
            bottom: 700,
        };
        let current_source = PhysicalRect {
            left: 300,
            top: 300,
            right: 920,
            bottom: 710,
        };
        let previous_destination = PhysicalRect {
            left: 100,
            top: 80,
            right: 1300,
            bottom: 880,
        };
        let work_area = PhysicalRect {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };

        let preserved = preserve_windowed_destination_for_source_change(
            previous_source,
            current_source,
            previous_destination,
        )
        .unwrap();
        let (fitted, fitted_to_work_area) =
            preserve_windowed_destination_for_source_change_in_work_area(
                previous_source,
                current_source,
                previous_destination,
                work_area,
            )
            .unwrap();

        assert!(!fitted_to_work_area);
        assert_eq!(fitted, preserved);
    }

    #[test]
    fn all_monitor_mode_uses_virtual_desktop_bounds() {
        let monitors = fixture_monitors();
        let layout = compute_scaling_layout(
            SourceWindow {
                hwnd: 1,
                rect: PhysicalRect {
                    left: 0,
                    top: 0,
                    right: 400,
                    bottom: 300,
                },
            }
            .rect,
            &monitors,
            LayoutRequest {
                mode: ScalingMode::Fullscreen,
                monitor_selection: MonitorSelectionMode::All,
                windowed_scale: 2.0,
            },
        )
        .unwrap();

        assert_eq!(layout.monitor_ids, vec![1, 2]);
        assert_eq!(
            layout.destination,
            monitors[0].bounds.union(monitors[1].bounds)
        );
    }

    #[test]
    fn source_lifecycle_policy_covers_close_minimize_focus_popup_and_resize() {
        let source = SourceWindow {
            hwnd: 1,
            rect: PhysicalRect {
                left: 0,
                top: 0,
                right: 640,
                bottom: 480,
            },
        };
        assert_eq!(
            evaluate_source_window_event(source, SourceWindowEvent::Closed),
            OverlayLifecycleAction::Stop {
                reason: StopReason::SourceClosed
            }
        );
        assert_eq!(
            evaluate_source_window_event(source, SourceWindowEvent::Minimized),
            OverlayLifecycleAction::Stop {
                reason: StopReason::SourceMinimized
            }
        );
        assert_eq!(
            evaluate_source_window_event(source, SourceWindowEvent::FocusLost),
            OverlayLifecycleAction::PreserveNoActivateFocus
        );
        assert_eq!(
            evaluate_source_window_event(source, SourceWindowEvent::PopupOpened),
            OverlayLifecycleAction::AllowPopupWithoutActivation
        );
        assert!(matches!(
            evaluate_source_window_event(
                source,
                SourceWindowEvent::MovedOrResized {
                    rect: PhysicalRect {
                        left: 0,
                        top: 0,
                        right: 800,
                        bottom: 600
                    }
                }
            ),
            OverlayLifecycleAction::RecomputeLayout { .. }
        ));
    }

    #[test]
    fn profile_resolution_uses_per_app_profile_before_default() {
        let mut settings = DodbogiSettings::default();
        let mut notepad = AppProfile::per_app_profile("notepad", "Notepad", "notepad.exe");
        notepad.effect_chain = vec!["bilinear".to_string(), "adaptive_sharpen".to_string()];
        settings.profiles.per_app_profiles.push(notepad);

        let default = settings.resolve_profile(&ProfileMatchContext {
            executable_name: Some("calc.exe".to_string()),
            window_class: None,
            title: None,
        });
        assert_eq!(default.source, ProfileResolutionSource::Default);
        assert_eq!(default.profile.id, "default");

        let matched = settings.resolve_profile(&ProfileMatchContext {
            executable_name: Some("NOTEPAD.EXE".to_string()),
            window_class: None,
            title: Some("Untitled - Notepad".to_string()),
        });
        assert_eq!(matched.source, ProfileResolutionSource::PerApp);
        assert_eq!(matched.profile.id, "notepad");
        assert!(matched.score >= 100);
    }

    #[test]
    fn settings_export_import_roundtrips_profiles_and_hotkeys() {
        let mut settings = DodbogiSettings::default();
        settings.hotkeys.windowed_toggle = "Ctrl+Shift+W".to_string();
        settings.diagnostics.enable_stats_overlay = true;
        settings
            .profiles
            .per_app_profiles
            .push(AppProfile::per_app_profile(
                "terminal",
                "Windows Terminal",
                "WindowsTerminal.exe",
            ));

        let raw = settings.to_toml_string();
        let parsed = DodbogiSettings::from_toml_str(&raw).expect("settings should parse");
        assert_eq!(parsed.hotkeys.windowed_toggle, "Ctrl+Shift+W");
        assert!(parsed.diagnostics.enable_stats_overlay);
        assert_eq!(parsed.profiles.per_app_profiles.len(), 1);
        assert_eq!(
            parsed.profiles.per_app_profiles[0]
                .match_rule
                .executable_name
                .as_deref(),
            Some("WindowsTerminal.exe")
        );
    }

    #[test]
    fn default_hotkeys_match_observed_magpie_scale_shortcuts() {
        let settings = DodbogiSettings::default();
        assert_eq!(settings.hotkeys.fullscreen_toggle, "Ctrl+Alt+A");
        assert_eq!(settings.hotkeys.windowed_toggle, "Ctrl+Alt+Q");
    }

    #[test]
    fn settings_parser_rejects_unknown_keys() {
        let raw = "version = 1\nunknown_key = \"value\"\n";
        let error = DodbogiSettings::from_toml_str(raw).expect_err("unknown key should fail");
        assert!(error.detail.contains("unknown global settings key"));
    }

    #[test]
    fn settings_coverage_and_diagnostics_are_complete() {
        let settings = DodbogiSettings::default();
        let coverage = settings_ui_coverage(&settings);
        assert!(coverage.all_required_covered());
        assert!(coverage
            .sections
            .iter()
            .any(|section| section.id == "packaging"));

        let paths = RuntimePaths {
            root: PathBuf::from("root"),
            config_dir: PathBuf::from("root/config"),
            logs_dir: PathBuf::from("root/logs"),
            cache_dir: PathBuf::from("root/cache"),
            settings_file: PathBuf::from("root/config/settings.toml"),
            log_file: PathBuf::from("root/logs/dodbogi.log"),
        };
        let snapshot = DiagnosticsSnapshot::capture(&paths, &settings);
        assert_eq!(snapshot.profile_count, 1);
        assert_eq!(snapshot.per_app_profile_count, 0);
        assert!(snapshot.support_envelope.contains("Windows 10 v1903+"));
    }

    fn fixture_monitors() -> Vec<MonitorGeometry> {
        vec![
            MonitorGeometry {
                id: 1,
                bounds: PhysicalRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
                work_area: PhysicalRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1040,
                },
                dpi: Dpi { x: 96, y: 96 },
                primary: true,
            },
            MonitorGeometry {
                id: 2,
                bounds: PhysicalRect {
                    left: 1920,
                    top: 0,
                    right: 4480,
                    bottom: 1440,
                },
                work_area: PhysicalRect {
                    left: 1920,
                    top: 0,
                    right: 4480,
                    bottom: 1400,
                },
                dpi: Dpi { x: 144, y: 144 },
                primary: false,
            },
        ]
    }
}
