use dodbogi_capture::{
    planned_backends, probe_additional_backends, probe_wgc_d3d11_frame_path,
    probe_wgc_frame_stream, resolve_title_bar_capture_region, CaptureBackendKind,
    TitleBarCaptureMode,
};
use dodbogi_core::{
    append_log_line, compute_scaling_layout, evaluate_source_window_event, export_settings_to_path,
    import_settings_from_path, load_settings_from_path,
    preserve_windowed_destination_for_source_change,
    preserve_windowed_destination_for_source_change_in_work_area, save_settings_to_path,
    settings_ui_coverage, windowed_work_area_for_source, write_default_settings_if_missing,
    AppProfile, DiagnosticsSnapshot, DodbogiSettings, LayoutRequest, MonitorSelectionMode,
    ProfileMatchContext, ProfileResolutionSource, RuntimePaths, ScalingMode, ScalingSession,
    SourceWindow, SourceWindowEvent, StopReason,
};
use dodbogi_effects::{
    builtin_effects, checkerboard_fixture, default_quality_chain, high_contrast_edge_fixture,
    validate_effect_catalog, RenderStatistics,
};
use dodbogi_input::{
    CursorRenderPolicy, DragPhase, InputEventKind, InputTransform, MouseButton, OverlayInputEvent,
    OverlayPoint, TextSelectionPhase, TouchPhase,
};
use dodbogi_parity::{
    render_markdown, summarize, ScenarioEvidenceRow, ScenarioEvidenceRowInput, ScenarioResult,
    ScenarioSummary,
};
use dodbogi_render_d3d11::{
    compile_builtin_effects_with_cache, probe_d3d11_hardware_feature_level, BaselinePresenter,
    TextureEffectPresentReport, WgcEffectScaler,
};
use dodbogi_win32::{
    client_rect_from_raw, collect_startup_report, create_wgc_item_for_hwnd,
    cursor_position_for_probe, cursor_speed_for_probe, deliver_input_to_source, enumerate_monitors,
    foreground_source_window, is_foreground_move_size_active, move_cursor_for_probe,
    move_window_for_probe, recover_cursor_speed_guard, resize_window_for_probe,
    run_controlled_input_probe, set_cursor_speed_for_probe, source_window_from_raw,
    ControlledInputProbeReport, CursorCaptureController, CursorCaptureReport, HotkeyRegistry,
    InputDeliveryMode, InputDeliveryReport, OverlayWindow, ShellMessage, ShellTrayIcon,
    SystemHotkeyGuard, TrayController,
};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().any(|arg| arg == "--stage-c-smoke") {
        return stage_c_smoke();
    }
    if std::env::args().any(|arg| arg == "--stage-d-smoke") {
        return stage_d_smoke();
    }
    if std::env::args().any(|arg| arg == "--stage-e-smoke") {
        return stage_e_smoke();
    }
    if std::env::args().any(|arg| arg == "--stage-f-smoke") {
        return stage_f_smoke();
    }
    if std::env::args().any(|arg| arg == "--stage-g-smoke") {
        return stage_g_smoke();
    }
    if std::env::args().any(|arg| arg == "--stage-h-smoke") {
        return stage_h_smoke();
    }
    if std::env::args().any(|arg| arg == "--g010-runtime-smoke") {
        return product_runtime_smoke();
    }
    if std::env::args().any(|arg| arg == "--g011-product-runtime-smoke") {
        return product_runtime_smoke_g011();
    }
    if std::env::args().any(|arg| {
        arg == "--g012-parity-release-smoke"
            || arg == "--g013-parity-release-gate"
            || arg == "--g014-parity-release-gate"
    }) {
        return product_runtime_smoke_g012(ParityGateMode::Release);
    }
    if std::env::args().any(|arg| {
        arg == "--g013-parity-classification-smoke" || arg == "--g014-parity-classification-smoke"
    }) {
        return product_runtime_smoke_g012(ParityGateMode::Classification);
    }
    if std::env::args().any(|arg| arg == "--g014-cursor-capture-smoke") {
        return cursor_capture_smoke_g014();
    }
    if std::env::args().any(|arg| arg == "--g018-cursor-edge-smoke") {
        return cursor_edge_smoke_g018();
    }
    if std::env::args().any(|arg| arg == "--g019-cursor-speed-guard-smoke") {
        return cursor_speed_guard_smoke_g019();
    }
    if std::env::args().any(|arg| arg == "--g014-source-move-smoke") {
        return source_move_smoke_g014();
    }
    if std::env::args().any(|arg| arg == "--g016-source-resize-smoke") {
        return source_resize_smoke_g016();
    }

    run_product_runtime()
}

struct RuntimeStartReport {
    mode: ScalingMode,
    source_hwnd: isize,
    overlay_hwnd: isize,
    source_rect: dodbogi_core::PhysicalRect,
    destination_rect: dodbogi_core::PhysicalRect,
    wgc_frames_observed: u32,
    wgc_surfaces_observed: u32,
    wgc_poll_attempts: u32,
    effect_id: String,
    effect_draws: u32,
    capture_frame_size: Option<(u32, u32)>,
    presented_frames: u32,
    effects_programs: usize,
    effect_cache_hits: usize,
    input_delivery: InputDeliveryReport,
}

struct RuntimeStopReport {
    source_hwnd: isize,
    presented_frames: u32,
    reason: StopReason,
}

struct RuntimeScreenshotReport {
    path: PathBuf,
    source_hwnd: isize,
    presented_frames: u32,
}

struct RuntimeInputForwardReport {
    source_hwnd: isize,
    kind: InputEventKind,
    source_point: Option<(i32, i32)>,
    delivered: bool,
    detail: String,
}

fn should_forward_overlay_input(kind: InputEventKind) -> bool {
    !matches!(
        kind,
        InputEventKind::MouseMove
            | InputEventKind::MouseButtonDown(_)
            | InputEventKind::MouseButtonUp(_)
            | InputEventKind::DoubleClick(_)
            | InputEventKind::Wheel { .. }
            | InputEventKind::Drag { .. }
            | InputEventKind::TextSelection { .. }
            | InputEventKind::ContextMenu
            | InputEventKind::Touch { .. }
    )
}

fn cursor_capture_input_delivery_report(
    target_hwnd: isize,
    mode: InputDeliveryMode,
) -> InputDeliveryReport {
    InputDeliveryReport {
        mode,
        target_hwnd,
        event_kind: "cursor_capture_passthrough",
        source_point: None,
        delivered: false,
        detail: "pointer input is owned by transparent overlay + cursor capture; startup SendInput click forwarding is disabled".to_string(),
    }
}

const SCALER_RESIZE_SETTLE: Duration = Duration::from_millis(180);

fn cursor_speed_guard_path(paths: &RuntimePaths) -> PathBuf {
    paths.cache_dir.join("cursor-speed-guard.txt")
}

struct ActiveScalingSession {
    session: ScalingSession,
    mode: ScalingMode,
    monitor_selection: MonitorSelectionMode,
    windowed_scale: f32,
    effect_id: String,
    source_window: SourceWindow,
    source_hwnd: isize,
    presented_frames: u32,
    overlay: OverlayWindow,
    scaler: Option<WgcEffectScaler>,
    scaler_output_size: (i32, i32),
    last_layout_change_at: Instant,
    last_scaler_resize_attempt_at: Option<Instant>,
    input_transform: InputTransform,
    cursor_capture: CursorCaptureController,
}

fn recreate_active_scaler(
    active: &mut ActiveScalingSession,
    destination: dodbogi_core::PhysicalRect,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_size = (destination.width().max(1), destination.height().max(1));
    if let Some(mut old_scaler) = active.scaler.take() {
        old_scaler.close();
    }

    let item =
        create_wgc_item_for_hwnd(active.source_hwnd).map_err(|error| format!("{error:?}"))?;
    let mut scaler = WgcEffectScaler::create_for_hwnd_and_item(
        active.overlay.hwnd(),
        output_size.0 as u32,
        output_size.1 as u32,
        &item,
        &active.effect_id,
    )
    .map_err(|error| format!("{error:?}"))?;
    let warmup = scaler
        .present_frames(1, Duration::from_millis(500))
        .map_err(|error| format!("{error:?}"))?;
    active.presented_frames += warmup.presented_frames;
    active.scaler = Some(scaler);
    active.scaler_output_size = output_size;
    Ok(())
}

fn resize_active_scaler_output(
    active: &mut ActiveScalingSession,
    destination: dodbogi_core::PhysicalRect,
) -> Result<(), Box<dyn std::error::Error>> {
    let output_size = (destination.width().max(1), destination.height().max(1));
    let Some(scaler) = active.scaler.as_mut() else {
        return recreate_active_scaler(active, destination);
    };

    scaler
        .resize_output(output_size.0 as u32, output_size.1 as u32)
        .map_err(|error| format!("{error:?}"))?;
    active.scaler_output_size = output_size;
    Ok(())
}

fn scaler_resize_ready(active: &ActiveScalingSession, move_size_active: bool) -> bool {
    if move_size_active || active.last_layout_change_at.elapsed() < SCALER_RESIZE_SETTLE {
        return false;
    }

    active
        .last_scaler_resize_attempt_at
        .map(|attempt| attempt.elapsed() >= SCALER_RESIZE_SETTLE)
        .unwrap_or(true)
}

struct ProductRuntimeController {
    active: Option<ActiveScalingSession>,
    shader_cache_root: PathBuf,
    cursor_speed_guard_path: PathBuf,
}

impl ProductRuntimeController {
    fn new(shader_cache_root: PathBuf, cursor_speed_guard_path: PathBuf) -> Self {
        Self {
            active: None,
            shader_cache_root,
            cursor_speed_guard_path,
        }
    }

    fn is_active(&self) -> bool {
        self.active.is_some()
    }

    fn start(
        &mut self,
        mode: ScalingMode,
        show_overlay: bool,
        input_mode: InputDeliveryMode,
    ) -> Result<RuntimeStartReport, Box<dyn std::error::Error>> {
        let mut profile = AppProfile::default_profile();
        profile.scaling_mode = mode;
        self.start_with_profile(&profile, Some(mode), show_overlay, input_mode)
    }

    fn start_with_profile(
        &mut self,
        profile: &AppProfile,
        mode_override: Option<ScalingMode>,
        show_overlay: bool,
        input_mode: InputDeliveryMode,
    ) -> Result<RuntimeStartReport, Box<dyn std::error::Error>> {
        if self.active.is_some() {
            return Err("scaling session is already active".into());
        }

        let mode = mode_override.unwrap_or(profile.scaling_mode);
        let effect_id = profile
            .effect_chain
            .first()
            .map(String::as_str)
            .unwrap_or("bilinear");
        let mut session = ScalingSession::default();
        session.begin_waiting(mode)?;
        let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
        session.start_scaling(source)?;

        let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
        let layout = compute_scaling_layout(
            source.rect,
            &monitors,
            LayoutRequest {
                mode,
                monitor_selection: profile.monitor_selection,
                windowed_scale: profile.windowed_scale_factor(),
            },
        )
        .map_err(|error| format!("{error:?}"))?;
        let overlay = OverlayWindow::create_hidden().map_err(|error| format!("{error:?}"))?;
        let _style = overlay
            .apply_layout(layout.destination, show_overlay)
            .map_err(|error| format!("{error:?}"))?;
        let item = create_wgc_item_for_hwnd(source.hwnd).map_err(|error| format!("{error:?}"))?;
        let mut scaler = WgcEffectScaler::create_for_hwnd_and_item(
            overlay.hwnd(),
            layout.destination.width().max(1) as u32,
            layout.destination.height().max(1) as u32,
            &item,
            effect_id,
        )
        .map_err(|error| format!("{error:?}"))?;
        let effects = compile_builtin_effects_with_cache(&self.shader_cache_root)
            .map_err(|error| format!("shader compile/cache failed: {error:?}"))?;
        let initial_present = scaler
            .present_frames(3, Duration::from_millis(1000))
            .map_err(|error| format!("{error:?}"))?;
        let presented_frames = initial_present.presented_frames;

        let transform = InputTransform::from_rects(layout.source, layout.destination)
            .map_err(|error| format!("{error:?}"))?;
        let cursor_capture = CursorCaptureController::create_with_speed_guard_path(Some(
            self.cursor_speed_guard_path.clone(),
        ))
        .map_err(|error| format!("{error:?}"))?;
        let input_delivery = cursor_capture_input_delivery_report(source.hwnd, input_mode);

        self.active = Some(ActiveScalingSession {
            session,
            mode,
            monitor_selection: profile.monitor_selection,
            windowed_scale: profile.windowed_scale_factor(),
            effect_id: effect_id.to_string(),
            source_window: source,
            source_hwnd: source.hwnd,
            presented_frames,
            overlay,
            scaler: Some(scaler),
            scaler_output_size: (layout.destination.width(), layout.destination.height()),
            last_layout_change_at: Instant::now(),
            last_scaler_resize_attempt_at: None,
            input_transform: transform,
            cursor_capture,
        });

        Ok(RuntimeStartReport {
            mode,
            source_hwnd: source.hwnd,
            overlay_hwnd: self
                .active
                .as_ref()
                .map(|active| active.overlay.hwnd())
                .unwrap_or(0),
            source_rect: layout.source,
            destination_rect: layout.destination,
            wgc_frames_observed: initial_present.frames_observed,
            wgc_surfaces_observed: initial_present.surfaces_observed,
            wgc_poll_attempts: initial_present.poll_attempts,
            effect_id: initial_present.effect_id,
            effect_draws: initial_present.effect_draws,
            capture_frame_size: initial_present.last_frame_size,
            presented_frames,
            effects_programs: effects.total_programs(),
            effect_cache_hits: effects.cache_hits(),
            input_delivery,
        })
    }

    fn pump_active_frame(
        &mut self,
    ) -> Result<Option<TextureEffectPresentReport>, Box<dyn std::error::Error>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        let Some(scaler) = active.scaler.as_mut() else {
            return Ok(None);
        };
        let destination = active.input_transform.destination;
        let output_size = (destination.width().max(1), destination.height().max(1));
        if active.scaler_output_size != output_size {
            return Ok(None);
        }
        let report = scaler
            .present_next_frame(Duration::from_millis(1))
            .map_err(|error| format!("{error:?}"))?;
        if report.presented_frames == 0 {
            return Ok(None);
        }
        active.presented_frames += report.presented_frames;
        Ok(Some(report))
    }

    fn refresh_active_layout(&mut self) -> Result<Option<String>, Box<dyn std::error::Error>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        let move_size_active = is_foreground_move_size_active();
        let source = match source_window_from_raw(active.source_hwnd) {
            Ok(source) => source,
            Err(error) => {
                let source_hwnd = active.source_hwnd;
                let _ = active;
                let stop = self.stop_with_reason(StopReason::SourceClosed);
                return Ok(Some(format!(
                    "runtime_stop_source_unavailable source={} reason={:?} detail={error:?}",
                    stop.as_ref()
                        .map(|report| report.source_hwnd)
                        .unwrap_or(source_hwnd),
                    stop.as_ref()
                        .map(|report| report.reason)
                        .unwrap_or(StopReason::SourceClosed)
                )));
            }
        };
        let action = evaluate_source_window_event(
            active.source_window,
            SourceWindowEvent::MovedOrResized { rect: source.rect },
        );
        let dodbogi_core::OverlayLifecycleAction::RecomputeLayout { .. } = action else {
            let destination = active.input_transform.destination;
            let output_size = (destination.width().max(1), destination.height().max(1));
            if active.scaler_output_size != output_size {
                let recreate_ready = scaler_resize_ready(active, move_size_active);
                active.last_scaler_resize_attempt_at = Some(Instant::now());
                return match resize_active_scaler_output(active, destination) {
                    Ok(()) => {
                        active.last_scaler_resize_attempt_at = None;
                        Ok(Some(format!(
                            "runtime_layout_update source={} source_rect={},{},{},{} destination={},{},{},{} layout_policy=deferred_scaler_resize resized_scaler=true recreated_scaler=false scaler_resize_deferred=false move_size_active={}",
                            active.source_hwnd,
                            active.input_transform.source.left,
                            active.input_transform.source.top,
                            active.input_transform.source.right,
                            active.input_transform.source.bottom,
                            destination.left,
                            destination.top,
                            destination.right,
                            destination.bottom,
                            move_size_active
                        )))
                    }
                    Err(error) => {
                        if recreate_ready {
                            active.last_scaler_resize_attempt_at = Some(Instant::now());
                            match recreate_active_scaler(active, destination) {
                                Ok(()) => {
                                    active.last_scaler_resize_attempt_at = None;
                                    Ok(Some(format!(
                                        "runtime_layout_update source={} source_rect={},{},{},{} destination={},{},{},{} layout_policy=deferred_scaler_resize resized_scaler=false recreated_scaler=true scaler_resize_deferred=false move_size_active={}",
                                        active.source_hwnd,
                                        active.input_transform.source.left,
                                        active.input_transform.source.top,
                                        active.input_transform.source.right,
                                        active.input_transform.source.bottom,
                                        destination.left,
                                        destination.top,
                                        destination.right,
                                        destination.bottom,
                                        move_size_active
                                    )))
                                }
                                Err(recreate_error) => Ok(Some(format!(
                                    "runtime_layout_update source={} source_rect={},{},{},{} destination={},{},{},{} layout_policy=deferred_scaler_resize_retry resized_scaler=false recreated_scaler=false scaler_resize_deferred=true move_size_active={} detail=resize:{error}; recreate:{recreate_error}",
                                    active.source_hwnd,
                                    active.input_transform.source.left,
                                    active.input_transform.source.top,
                                    active.input_transform.source.right,
                                    active.input_transform.source.bottom,
                                    destination.left,
                                    destination.top,
                                    destination.right,
                                    destination.bottom,
                                    move_size_active
                                ))),
                            }
                        } else {
                            Ok(Some(format!(
                        "runtime_layout_update source={} source_rect={},{},{},{} destination={},{},{},{} layout_policy=deferred_scaler_resize_retry resized_scaler=false recreated_scaler=false scaler_resize_deferred=true move_size_active={} detail={error}",
                        active.source_hwnd,
                        active.input_transform.source.left,
                        active.input_transform.source.top,
                        active.input_transform.source.right,
                        active.input_transform.source.bottom,
                        destination.left,
                        destination.top,
                        destination.right,
                        destination.bottom,
                        move_size_active
                    )))
                        }
                    }
                };
            }
            return Ok(None);
        };

        let previous_source_rect = active.source_window.rect;
        let previous_destination = active.input_transform.destination;
        let source_size_changed = previous_source_rect.width() != source.rect.width()
            || previous_source_rect.height() != source.rect.height();
        let preserved_destination = if active.mode == ScalingMode::Windowed {
            match preserve_windowed_destination_for_source_change(
                previous_source_rect,
                source.rect,
                previous_destination,
            ) {
                Some(destination) if source_size_changed => {
                    let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
                    let work_area = windowed_work_area_for_source(
                        source.rect,
                        &monitors,
                        active.monitor_selection,
                    )
                    .map_err(|error| format!("{error:?}"))?;
                    Some(
                        preserve_windowed_destination_for_source_change_in_work_area(
                            previous_source_rect,
                            source.rect,
                            previous_destination,
                            work_area,
                        )
                        .unwrap_or((destination, false)),
                    )
                }
                Some(destination) => Some((destination, false)),
                None => None,
            }
        } else {
            None
        };
        let (layout_source, destination, layout_policy) =
            if let Some((destination, fitted_to_work_area)) = preserved_destination {
                let layout_policy = if !source_size_changed {
                    "preserve_source_move"
                } else if fitted_to_work_area {
                    "preserve_source_resize_fit"
                } else {
                    "preserve_source_resize"
                };
                (source.rect, destination, layout_policy)
            } else {
                let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
                let layout = compute_scaling_layout(
                    source.rect,
                    &monitors,
                    LayoutRequest {
                        mode: active.mode,
                        monitor_selection: active.monitor_selection,
                        windowed_scale: active.windowed_scale,
                    },
                )
                .map_err(|error| format!("{error:?}"))?;
                (layout.source, layout.destination, "recompute_layout")
            };
        active
            .overlay
            .apply_layout(destination, true)
            .map_err(|error| format!("{error:?}"))?;
        active.input_transform = InputTransform::from_rects(layout_source, destination)
            .map_err(|error| format!("{error:?}"))?;
        active.source_window = source;
        active.last_layout_change_at = Instant::now();

        let output_size = (destination.width().max(1), destination.height().max(1));
        let destination_size_changed = active.scaler_output_size != output_size;
        let mut scaler_resize_deferred = false;
        let mut resized_scaler = false;
        let mut recreated_scaler = false;
        if destination_size_changed {
            let recreate_ready = scaler_resize_ready(active, move_size_active);
            active.last_scaler_resize_attempt_at = Some(Instant::now());
            match resize_active_scaler_output(active, destination) {
                Ok(()) => {
                    active.last_scaler_resize_attempt_at = None;
                    resized_scaler = true;
                }
                Err(resize_error) => {
                    if recreate_ready {
                        active.last_scaler_resize_attempt_at = Some(Instant::now());
                        match recreate_active_scaler(active, destination) {
                            Ok(()) => {
                                active.last_scaler_resize_attempt_at = None;
                                recreated_scaler = true;
                            }
                            Err(recreate_error) => {
                                let _ = (resize_error, recreate_error);
                                scaler_resize_deferred = true;
                            }
                        }
                    } else {
                        let _ = resize_error;
                        scaler_resize_deferred = true;
                    }
                }
            }
        }

        Ok(Some(format!(
            "runtime_layout_update source={} source_rect={},{},{},{} destination={},{},{},{} layout_policy={} resized_scaler={} recreated_scaler={} scaler_resize_deferred={} move_size_active={}",
            active.source_hwnd,
            layout_source.left,
            layout_source.top,
            layout_source.right,
            layout_source.bottom,
            destination.left,
            destination.top,
            destination.right,
            destination.bottom,
            layout_policy,
            resized_scaler,
            recreated_scaler,
            scaler_resize_deferred,
            move_size_active
        )))
    }

    fn update_cursor_capture(
        &mut self,
    ) -> Result<Option<CursorCaptureReport>, Box<dyn std::error::Error>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        active
            .cursor_capture
            .update(&active.input_transform, active.source_hwnd)
            .map_err(|error| format!("{error:?}").into())
    }

    fn stop(&mut self) -> Option<RuntimeStopReport> {
        self.stop_with_reason(StopReason::UserToggle)
    }

    fn stop_with_reason(&mut self, reason: StopReason) -> Option<RuntimeStopReport> {
        let mut active = self.active.take()?;
        active.cursor_capture.release();
        active.session.stop(reason);
        if let Some(mut scaler) = active.scaler.take() {
            scaler.close();
        }
        Some(RuntimeStopReport {
            source_hwnd: active.source_hwnd,
            presented_frames: active.presented_frames,
            reason,
        })
    }

    fn save_screenshot(
        &mut self,
        screenshot_dir: &Path,
    ) -> Result<Option<RuntimeScreenshotReport>, Box<dyn std::error::Error>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        fs::create_dir_all(screenshot_dir)?;
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        let path = screenshot_dir.join(format!(
            "dodbogi-scaling-{}-{timestamp_ms}.ppm",
            active.source_hwnd
        ));
        let Some(scaler) = active.scaler.as_mut() else {
            return Ok(None);
        };
        scaler
            .write_current_backbuffer_ppm(&path)
            .map_err(|error| format!("{error:?}"))?;
        Ok(Some(RuntimeScreenshotReport {
            path,
            source_hwnd: active.source_hwnd,
            presented_frames: active.presented_frames,
        }))
    }

    fn forward_overlay_input(
        &mut self,
        overlay_hwnd: isize,
        kind: InputEventKind,
        screen_x: i32,
        screen_y: i32,
    ) -> Result<Option<RuntimeInputForwardReport>, Box<dyn std::error::Error>> {
        let Some(active) = self.active.as_mut() else {
            return Ok(None);
        };
        if active.overlay.hwnd() != overlay_hwnd {
            return Ok(None);
        }
        if !should_forward_overlay_input(kind) {
            return Ok(None);
        }
        let mapped = active
            .input_transform
            .map_event(OverlayInputEvent {
                kind,
                point: Some(OverlayPoint {
                    x: screen_x as f32,
                    y: screen_y as f32,
                }),
            })
            .ok_or("overlay input point outside active scaled destination")?;
        let delivered = deliver_input_to_source(
            active.source_hwnd,
            mapped,
            InputDeliveryMode::SendInputAllowed,
        )
        .map_err(|error| format!("{error:?}"))?;
        Ok(Some(RuntimeInputForwardReport {
            source_hwnd: active.source_hwnd,
            kind,
            source_point: delivered.source_point,
            delivered: delivered.delivered,
            detail: delivered.detail,
        }))
    }
}

fn handle_runtime_message(
    message: ShellMessage,
    controller: &mut ProductRuntimeController,
    paths: &RuntimePaths,
) -> Result<bool, Box<dyn std::error::Error>> {
    append_log_line(&paths.log_file, &format!("shell_message={message:?}"))?;
    println!("Shell message: {message:?}");

    match message {
        ShellMessage::Hotkey { id: 1, .. }
        | ShellMessage::Hotkey { id: 2, .. }
        | ShellMessage::TrayMenu {
            item_id: "toggle-windowed",
        }
        | ShellMessage::TrayMenu {
            item_id: "toggle-fullscreen",
        } => {
            if let Some(stop) = controller.stop() {
                append_log_line(
                    &paths.log_file,
                    &format!(
                        "runtime_stop source={} presented_frames={} reason={:?}",
                        stop.source_hwnd, stop.presented_frames, stop.reason
                    ),
                )?;
                println!(
                    "Stopped scaling source={} presented_frames={} reason={:?}",
                    stop.source_hwnd, stop.presented_frames, stop.reason
                );
            } else {
                let mode = match message {
                    ShellMessage::Hotkey { id: 2, .. }
                    | ShellMessage::TrayMenu {
                        item_id: "toggle-fullscreen",
                    } => ScalingMode::Fullscreen,
                    _ => ScalingMode::Windowed,
                };
                let settings = load_settings_from_path(&paths.settings_file)?;
                let start = controller.start_with_profile(
                    &settings.profiles.default_profile,
                    Some(mode),
                    true,
                    InputDeliveryMode::DryRun,
                )?;
                append_log_line(
                    &paths.log_file,
                    &format!(
                        "runtime_start_scaling mode={} source={} frames={} surfaces={} presents={} effects={}",
                        start.mode.as_str(),
                        start.source_hwnd,
                        start.wgc_frames_observed,
                        start.wgc_surfaces_observed,
                        start.presented_frames,
                        start.effects_programs
                    ),
                )?;
                println!(
                    "Started {} scaling source={} frames={} surfaces={} presents={} effects={}",
                    start.mode.as_str(),
                    start.source_hwnd,
                    start.wgc_frames_observed,
                    start.wgc_surfaces_observed,
                    start.presented_frames,
                    start.effects_programs
                );
            }
            Ok(false)
        }
        ShellMessage::TrayMenu {
            item_id: "settings",
        } => {
            println!("Settings: {}", paths.settings_file.display());
            Ok(false)
        }
        ShellMessage::TrayMenu {
            item_id: "diagnostics",
        } => {
            println!("Diagnostics log: {}", paths.log_file.display());
            Ok(false)
        }
        ShellMessage::TrayMenu {
            item_id: "screenshot",
        } => {
            let settings = load_settings_from_path(&paths.settings_file)?;
            let screenshot_dir = paths.root.join(settings.diagnostics.screenshot_dir_name);
            match controller.save_screenshot(&screenshot_dir)? {
                Some(report) => {
                    append_log_line(
                        &paths.log_file,
                        &format!(
                            "screenshot_saved source={} path={} presented_frames={}",
                            report.source_hwnd,
                            report.path.display(),
                            report.presented_frames
                        ),
                    )?;
                    println!("Screenshot saved: {}", report.path.display());
                }
                None => {
                    append_log_line(&paths.log_file, "screenshot_requested_no_active_session")?;
                    println!("Screenshot request ignored: no active scaling session.");
                }
            }
            Ok(false)
        }
        ShellMessage::TrayMenu { item_id: "exit" } | ShellMessage::Quit => Ok(true),
        ShellMessage::OverlayInput {
            hwnd,
            kind,
            screen_x,
            screen_y,
        } => {
            if let Some(report) =
                controller.forward_overlay_input(hwnd, kind, screen_x, screen_y)?
            {
                if !matches!(report.kind, InputEventKind::MouseMove) {
                    append_log_line(
                        &paths.log_file,
                        &format!(
                            "overlay_input_forwarded source={} kind={:?} source_point={:?} delivered={} detail={}",
                            report.source_hwnd,
                            report.kind,
                            report.source_point,
                            report.delivered,
                            report.detail
                        ),
                    )?;
                    println!(
                        "Forwarded overlay input kind={:?} source={:?} delivered={}",
                        report.kind, report.source_point, report.delivered
                    );
                }
            }
            Ok(false)
        }
        ShellMessage::TrayError(detail) => {
            append_log_line(&paths.log_file, &format!("tray_error={detail}"))?;
            Ok(false)
        }
        ShellMessage::TrayMenu { .. } => Ok(false),
        ShellMessage::Hotkey { .. } => Ok(false),
    }
}

fn run_product_runtime() -> Result<(), Box<dyn std::error::Error>> {
    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let report = collect_startup_report();
    append_log_line(
        &paths.log_file,
        &format!(
            "runtime_start target={} checks={}",
            report.target,
            report.checks.len()
        ),
    )?;

    let mut tray = TrayController::default();
    tray.install_placeholder();
    let system_tray = ShellTrayIcon::install_default();
    let (system_tray_installed, system_tray_menu_probe, system_tray_detail) = match &system_tray {
        Ok(icon) => (
            icon.is_installed(),
            icon.build_menu_probe().unwrap_or(0),
            format!("menu_items={}", icon.menu_items().len()),
        ),
        Err(error) => (false, 0, format!("{error:?}")),
    };

    let mut hotkeys = HotkeyRegistry::default();
    hotkeys.register_defaults();
    let system_hotkeys = SystemHotkeyGuard::register_defaults();

    println!("Dodbogi runtime");
    println!("Target parity: {}", report.target);
    println!("Support envelope: {}", report.envelope.description);
    println!("Data root: {}", paths.root.display());
    println!("Settings: {}", paths.settings_file.display());
    println!("Log: {}", paths.log_file.display());
    println!("Tray contract installed: {}", tray.is_installed());
    println!("Tray menu items: {}", tray.menu_items().len());
    println!("System tray installed: {system_tray_installed}");
    println!("System tray menu probe items: {system_tray_menu_probe}");
    println!("System tray detail: {system_tray_detail}");
    println!("Hotkey contract count: {}", hotkeys.registered().len());
    println!(
        "System hotkeys registered: {} failed: {}",
        system_hotkeys.report().registered_count(),
        system_hotkeys.report().failed_count()
    );
    println!(
        "Runtime is persistent; use tray/registered hotkeys to start/stop scaling, or Exit to quit."
    );

    for check in &report.checks {
        println!("[{:?}] {} - {}", check.status, check.name, check.detail);
    }

    let guard_path = cursor_speed_guard_path(&paths);
    if let Some(restored_speed) =
        recover_cursor_speed_guard(&guard_path).map_err(|error| format!("{error:?}"))?
    {
        append_log_line(
            &paths.log_file,
            &format!("cursor_speed_stale_guard_restored origin={restored_speed}"),
        )?;
        println!("Restored stale cursor speed guard: {restored_speed}");
    }

    let mut controller = ProductRuntimeController::new(
        paths.cache_dir.join("shader-cache").join("product"),
        guard_path,
    );
    let mut next_auto_scale_check = Instant::now();
    let mut next_recoverable_error_log = Instant::now();
    loop {
        let messages = match &system_tray {
            Ok(icon) => icon.drain_messages(64),
            Err(_) => dodbogi_win32::drain_shell_messages(64),
        };

        let mut should_exit = false;
        for message in messages {
            if handle_runtime_message(message, &mut controller, &paths)? {
                should_exit = true;
                break;
            }
        }
        if should_exit {
            break;
        }

        if controller.is_active() {
            match controller.refresh_active_layout() {
                Ok(Some(layout_event)) => {
                    append_log_line(&paths.log_file, &layout_event)?;
                    println!("{layout_event}");
                }
                Ok(None) => {}
                Err(error) => {
                    if Instant::now() >= next_recoverable_error_log {
                        next_recoverable_error_log = Instant::now() + Duration::from_secs(1);
                        let detail =
                            format!("runtime_recoverable_error phase=layout detail={error}");
                        let _ = append_log_line(&paths.log_file, &detail);
                        println!("{detail}");
                    }
                }
            }
            match controller.update_cursor_capture() {
                Ok(Some(cursor_event)) => {
                    append_log_line(
                        &paths.log_file,
                        &format!(
                            "cursor_capture captured={} source_point={:?} overlay_point={:?} detail={}",
                            cursor_event.captured,
                            cursor_event.source_point,
                            cursor_event.overlay_point,
                            cursor_event.detail
                        ),
                    )?;
                    println!(
                        "Cursor capture: captured={} source={:?} overlay={:?}",
                        cursor_event.captured,
                        cursor_event.source_point,
                        cursor_event.overlay_point
                    );
                }
                Ok(None) => {}
                Err(error) => {
                    if Instant::now() >= next_recoverable_error_log {
                        next_recoverable_error_log = Instant::now() + Duration::from_secs(1);
                        let detail =
                            format!("runtime_recoverable_error phase=cursor detail={error}");
                        let _ = append_log_line(&paths.log_file, &detail);
                        println!("{detail}");
                    }
                }
            }
            match controller.pump_active_frame() {
                Ok(Some(report)) => {
                    append_log_line(
                        &paths.log_file,
                        &format!(
                            "runtime_frame effect={} frames={} surfaces={} draws={} presents={}",
                            report.effect_id,
                            report.frames_observed,
                            report.surfaces_observed,
                            report.effect_draws,
                            report.presented_frames
                        ),
                    )?;
                }
                Ok(None) => {}
                Err(error) => {
                    if Instant::now() >= next_recoverable_error_log {
                        next_recoverable_error_log = Instant::now() + Duration::from_secs(1);
                        let detail =
                            format!("runtime_recoverable_error phase=frame detail={error}");
                        let _ = append_log_line(&paths.log_file, &detail);
                        println!("{detail}");
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(16));
        } else {
            if Instant::now() >= next_auto_scale_check {
                next_auto_scale_check = Instant::now() + Duration::from_millis(750);
                let settings = load_settings_from_path(&paths.settings_file)?;
                if settings.profiles.default_profile.auto_scale {
                    match controller.start_with_profile(
                        &settings.profiles.default_profile,
                        None,
                        true,
                        InputDeliveryMode::DryRun,
                    ) {
                        Ok(start) => {
                            append_log_line(
                                &paths.log_file,
                                &format!(
                                    "runtime_auto_scale_start mode={} source={} frames={} surfaces={} presents={} effect={}",
                                    start.mode.as_str(),
                                    start.source_hwnd,
                                    start.wgc_frames_observed,
                                    start.wgc_surfaces_observed,
                                    start.presented_frames,
                                    start.effect_id
                                ),
                            )?;
                            println!(
                                "Auto-scale started {} source={} effect={} presents={}",
                                start.mode.as_str(),
                                start.source_hwnd,
                                start.effect_id,
                                start.presented_frames
                            );
                        }
                        Err(error) => {
                            append_log_line(
                                &paths.log_file,
                                &format!("runtime_auto_scale_skip detail={error}"),
                            )?;
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    let _ = controller.stop();
    drop(system_tray);
    drop(system_hotkeys);
    Ok(())
}

fn product_runtime_smoke() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G010_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let report = collect_startup_report();
    let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
    let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
    let layout = compute_scaling_layout(source.rect, &monitors, LayoutRequest::default())
        .map_err(|error| format!("{error:?}"))?;
    let overlay = OverlayWindow::create_hidden().map_err(|error| format!("{error:?}"))?;
    let style = overlay
        .apply_layout(layout.destination, false)
        .map_err(|error| format!("{error:?}"))?;
    let item = create_wgc_item_for_hwnd(source.hwnd).map_err(|error| format!("{error:?}"))?;
    let stream = probe_wgc_frame_stream(&item, Duration::from_millis(1000), 3)
        .map_err(|error| format!("{error:?}"))?;
    let presenter = BaselinePresenter::create_for_hwnd(
        overlay.hwnd(),
        layout.destination.width().max(1) as u32,
        layout.destination.height().max(1) as u32,
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut present_count = 0u32;
    let frames_to_present = stream.frames_observed.max(1);
    for index in 0..frames_to_present {
        let tint = 0.04 + (index as f32 * 0.02);
        presenter
            .present_baseline_clear([tint, 0.05, 0.08, 1.0])
            .map_err(|error| format!("{error:?}"))?;
        present_count += 1;
    }

    let cache_root = std::env::var_os("DODBOGI_G010_SHADER_CACHE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| paths.cache_dir.join("shader-cache").join("runtime"));
    let effects = compile_builtin_effects_with_cache(&cache_root)
        .map_err(|error| format!("shader compile/cache failed: {error:?}"))?;

    let transform = InputTransform::from_rects(layout.source, layout.destination)
        .map_err(|error| format!("{error:?}"))?;
    let center = OverlayPoint {
        x: (layout.destination.left + layout.destination.right) as f32 / 2.0,
        y: (layout.destination.top + layout.destination.bottom) as f32 / 2.0,
    };
    let mapped_click = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::MouseButtonDown(MouseButton::Left),
            point: Some(center),
        })
        .ok_or("runtime smoke input map failed")?;
    let input_delivery =
        deliver_input_to_source(source.hwnd, mapped_click, InputDeliveryMode::DryRun)
            .map_err(|error| format!("{error:?}"))?;

    let mut tray = TrayController::default();
    tray.install_placeholder();
    let system_tray = ShellTrayIcon::install_default();
    let (system_tray_installed, system_tray_menu_probe, system_tray_detail) = match &system_tray {
        Ok(icon) => (
            icon.is_installed(),
            icon.build_menu_probe().unwrap_or(0),
            format!("menu_items={}", icon.menu_items().len()),
        ),
        Err(error) => (false, 0, format!("{error:?}")),
    };
    let mut hotkeys = HotkeyRegistry::default();
    hotkeys.register_defaults();
    let system_hotkeys = SystemHotkeyGuard::register_defaults();
    append_log_line(
        &paths.log_file,
        &format!(
            "g010_runtime_smoke source={} frames={} presents={} effects={} tray_installed={} hotkeys_registered={}",
            source.hwnd,
            stream.frames_observed,
            present_count,
            effects.total_programs(),
            system_tray_installed,
            system_hotkeys.report().registered_count()
        ),
    )?;

    println!("G010 runtime smoke");
    println!("target={}", report.target);
    println!("data_root={}", paths.root.display());
    println!("source_hwnd={}", source.hwnd);
    println!(
        "source_rect={},{},{},{}",
        source.rect.left, source.rect.top, source.rect.right, source.rect.bottom
    );
    println!(
        "destination_rect={},{},{},{}",
        layout.destination.left,
        layout.destination.top,
        layout.destination.right,
        layout.destination.bottom
    );
    println!(
        "overlay_hwnd={} no_activate={} topmost={} tool_window={} input_passthrough={} layered_passthrough={}",
        overlay.hwnd(),
        style.no_activate,
        style.topmost,
        style.tool_window,
        style.input_passthrough,
        style.layered_passthrough
    );
    println!(
        "wgc_stream item={}x{} frames_observed={} surfaces_observed={} poll_attempts={} last_frame={} last_surface={}",
        stream.item_width,
        stream.item_height,
        stream.frames_observed,
        stream.surfaces_observed,
        stream.poll_attempts,
        stream
            .last_frame_size
            .map(|(width, height)| format!("{width}x{height}"))
            .unwrap_or_else(|| "none".to_string()),
        stream
            .last_surface_size
            .map(|(width, height)| format!("{width}x{height}"))
            .unwrap_or_else(|| "none".to_string())
    );
    println!("presented_frames={present_count}");
    println!(
        "effects_programs={} effect_cache_hits={}",
        effects.total_programs(),
        effects.cache_hits()
    );
    println!(
        "input_delivery mode={:?} delivered={} point={:?} detail={}",
        input_delivery.mode,
        input_delivery.delivered,
        input_delivery.source_point,
        input_delivery.detail
    );
    println!(
        "tray_contract installed={} menu_count={} system_tray_installed={} system_tray_menu_probe={} system_tray_detail={} hotkey_count={} system_hotkeys_registered={} system_hotkeys_failed={}",
        tray.is_installed(),
        tray.menu_items().len(),
        system_tray_installed,
        system_tray_menu_probe,
        system_tray_detail,
        hotkeys.registered().len(),
        system_hotkeys.report().registered_count(),
        system_hotkeys.report().failed_count()
    );
    for backend in planned_backends() {
        println!(
            "backend kind={:?} frame_runtime={} probe={} limitation={}",
            backend.kind,
            backend.frame_producing_runtime,
            backend.availability_probe,
            backend.limitation.unwrap_or("none")
        );
    }

    if stream.frames_observed == 0
        || stream.surfaces_observed == 0
        || present_count == 0
        || effects.total_programs() == 0
    {
        return Err("runtime smoke did not observe required capture/render/effect evidence".into());
    }

    Ok(())
}

fn product_runtime_smoke_g011() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G011_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let shader_cache_root = std::env::var_os("DODBOGI_G011_SHADER_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.cache_dir.join("shader-cache").join("g011-product"));
    let mut controller =
        ProductRuntimeController::new(shader_cache_root, cursor_speed_guard_path(&paths));
    let start = controller.start(ScalingMode::Windowed, false, InputDeliveryMode::DryRun)?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g011_product_runtime_start source={} frames={} surfaces={} presents={} effects={}",
            start.source_hwnd,
            start.wgc_frames_observed,
            start.wgc_surfaces_observed,
            start.presented_frames,
            start.effects_programs
        ),
    )?;
    let stop = controller
        .stop()
        .ok_or("G011 smoke expected an active scaling session")?;
    let live_input_probe = run_controlled_input_probe().map_err(|error| format!("{error:?}"))?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g011_product_runtime_stop source={} presented_frames={}",
            stop.source_hwnd, stop.presented_frames
        ),
    )?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g011_controlled_input target={} sent={} observed_down={} observed_up={} delivered={}",
            live_input_probe.target_hwnd,
            live_input_probe.sent_events,
            live_input_probe.observed_left_down,
            live_input_probe.observed_left_up,
            live_input_probe.delivered
        ),
    )?;

    println!("G011 product runtime smoke");
    println!("product_path_controller=true");
    println!("mode={}", start.mode.as_str());
    println!("source_hwnd={}", start.source_hwnd);
    println!(
        "source_rect={},{},{},{}",
        start.source_rect.left,
        start.source_rect.top,
        start.source_rect.right,
        start.source_rect.bottom
    );
    println!(
        "destination_rect={},{},{},{}",
        start.destination_rect.left,
        start.destination_rect.top,
        start.destination_rect.right,
        start.destination_rect.bottom
    );
    println!("overlay_hwnd={} overlay_show=false", start.overlay_hwnd);
    println!(
        "wgc_product_frames={} wgc_product_surfaces={} poll_attempts={} capture_frame={}",
        start.wgc_frames_observed,
        start.wgc_surfaces_observed,
        start.wgc_poll_attempts,
        start
            .capture_frame_size
            .map(|(width, height)| format!("{width}x{height}"))
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "texture_effect_path effect={} effect_draws={} product_presented_frames={}",
        start.effect_id, start.effect_draws, start.presented_frames
    );
    println!(
        "effects_programs={} effect_cache_hits={}",
        start.effects_programs, start.effect_cache_hits
    );
    println!(
        "input_delivery mode={:?} delivered={} point={:?} detail={}",
        start.input_delivery.mode,
        start.input_delivery.delivered,
        start.input_delivery.source_point,
        start.input_delivery.detail
    );
    println!(
        "stop source_hwnd={} presented_frames={} active_after_stop={}",
        stop.source_hwnd,
        stop.presented_frames,
        controller.is_active()
    );
    println!(
        "controlled_input target_hwnd={} sent_events={} observed_left_down={} observed_left_up={} delivered={} detail={}",
        live_input_probe.target_hwnd,
        live_input_probe.sent_events,
        live_input_probe.observed_left_down,
        live_input_probe.observed_left_up,
        live_input_probe.delivered,
        live_input_probe.detail
    );

    if start.wgc_frames_observed == 0
        || start.wgc_surfaces_observed == 0
        || start.effect_draws == 0
        || start.presented_frames == 0
        || start.effects_programs == 0
        || !live_input_probe.delivered
        || controller.is_active()
    {
        return Err("G011 product runtime smoke did not satisfy start/render/stop evidence".into());
    }

    Ok(())
}

fn cursor_capture_smoke_g014() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G014_CURSOR_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let shader_cache_root = paths.cache_dir.join("shader-cache").join("g014-cursor");
    let original_cursor = cursor_position_for_probe().ok();
    let mut controller =
        ProductRuntimeController::new(shader_cache_root, cursor_speed_guard_path(&paths));

    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        let start = controller.start(ScalingMode::Windowed, true, InputDeliveryMode::DryRun)?;
        let destination_center = (
            (start.destination_rect.left + start.destination_rect.right) / 2,
            (start.destination_rect.top + start.destination_rect.bottom) / 2,
        );
        move_cursor_for_probe(destination_center.0, destination_center.1)
            .map_err(|error| format!("{error:?}"))?;
        let capture = controller
            .update_cursor_capture()?
            .ok_or("G014 cursor smoke expected cursor_capture_entered event")?;
        let stop = controller
            .stop()
            .ok_or("G014 cursor smoke expected an active scaling session")?;
        Ok((start, destination_center, capture, stop))
    })();

    if controller.is_active() {
        let _ = controller.stop();
    }
    if let Some((x, y)) = original_cursor {
        let _ = move_cursor_for_probe(x, y);
    }

    let (start, destination_center, capture, stop) = result?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g014_cursor_capture source={} destination_center={:?} captured={} source_point={:?} overlay_point={:?} detail={}",
            start.source_hwnd,
            destination_center,
            capture.captured,
            capture.source_point,
            capture.overlay_point,
            capture.detail
        ),
    )?;

    println!("G014 cursor capture smoke");
    println!("source_hwnd={}", start.source_hwnd);
    println!("overlay_hwnd={}", start.overlay_hwnd);
    println!(
        "destination_rect={},{},{},{}",
        start.destination_rect.left,
        start.destination_rect.top,
        start.destination_rect.right,
        start.destination_rect.bottom
    );
    println!("destination_center={destination_center:?}");
    println!(
        "cursor_capture captured={} source={:?} overlay={:?} detail={}",
        capture.captured, capture.source_point, capture.overlay_point, capture.detail
    );
    println!(
        "stop source_hwnd={} presented_frames={} active_after_stop={}",
        stop.source_hwnd,
        stop.presented_frames,
        controller.is_active()
    );

    if !capture.captured
        || capture.source_point.is_none()
        || capture.overlay_point != Some(destination_center)
        || controller.is_active()
    {
        return Err(
            "G014 cursor capture smoke did not enter and release cursor capture cleanly".into(),
        );
    }

    Ok(())
}

fn cursor_edge_smoke_g018() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G018_CURSOR_EDGE_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let shader_cache_root = paths
        .cache_dir
        .join("shader-cache")
        .join("g018-cursor-edge");
    let original_cursor = cursor_position_for_probe().ok();
    let mut controller =
        ProductRuntimeController::new(shader_cache_root, cursor_speed_guard_path(&paths));

    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        let start = controller.start(ScalingMode::Windowed, true, InputDeliveryMode::DryRun)?;
        let edge_point = (
            start.destination_rect.right - 1,
            start.destination_rect.bottom - 1,
        );
        move_cursor_for_probe(edge_point.0, edge_point.1).map_err(|error| format!("{error:?}"))?;
        let first = controller
            .update_cursor_capture()?
            .ok_or("G018 cursor edge smoke expected cursor_capture_entered event")?;
        let second = controller.update_cursor_capture()?;
        let stop = controller
            .stop()
            .ok_or("G018 cursor edge smoke expected an active scaling session")?;
        Ok((start, edge_point, first, second, stop))
    })();

    if controller.is_active() {
        let _ = controller.stop();
    }
    if let Some((x, y)) = original_cursor {
        let _ = move_cursor_for_probe(x, y);
    }

    let (start, edge_point, first, second, stop) = result?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g018_cursor_edge source={} edge_point={:?} first_captured={} first_source={:?} first_overlay={:?} second={:?}",
            start.source_hwnd,
            edge_point,
            first.captured,
            first.source_point,
            first.overlay_point,
            second
        ),
    )?;

    println!("G018 cursor edge smoke");
    println!("source_hwnd={}", start.source_hwnd);
    println!(
        "destination_rect={},{},{},{}",
        start.destination_rect.left,
        start.destination_rect.top,
        start.destination_rect.right,
        start.destination_rect.bottom
    );
    println!("edge_point={edge_point:?}");
    println!(
        "first captured={} source={:?} overlay={:?}",
        first.captured, first.source_point, first.overlay_point
    );
    println!("second={second:?}");
    println!(
        "stop source_hwnd={} presented_frames={} active_after_stop={}",
        stop.source_hwnd,
        stop.presented_frames,
        controller.is_active()
    );

    if !first.captured
        || first.overlay_point != Some(edge_point)
        || matches!(
            second,
            Some(CursorCaptureReport {
                captured: false,
                ..
            })
        )
        || controller.is_active()
    {
        return Err("G018 cursor edge smoke detected capture flicker at destination edge".into());
    }

    Ok(())
}

fn cursor_speed_guard_smoke_g019() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G019_CURSOR_GUARD_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let guard_path = cursor_speed_guard_path(&paths);
    let original_speed = cursor_speed_for_probe().map_err(|error| format!("{error:?}"))?;
    let adjusted_speed = if original_speed == 1 { 2 } else { 1 };

    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        fs::write(&guard_path, format!("{original_speed}\n"))?;
        set_cursor_speed_for_probe(adjusted_speed).map_err(|error| format!("{error:?}"))?;
        let observed_adjusted = cursor_speed_for_probe().map_err(|error| format!("{error:?}"))?;
        let recovered =
            recover_cursor_speed_guard(&guard_path).map_err(|error| format!("{error:?}"))?;
        let observed_restored = cursor_speed_for_probe().map_err(|error| format!("{error:?}"))?;
        Ok((observed_adjusted, recovered, observed_restored))
    })();

    let cleanup_restore = set_cursor_speed_for_probe(original_speed);
    let _ = fs::remove_file(&guard_path);

    let (observed_adjusted, recovered, observed_restored) = result?;
    cleanup_restore.map_err(|error| format!("{error:?}"))?;

    append_log_line(
        &paths.log_file,
        &format!(
            "g019_cursor_speed_guard original={} adjusted={} observed_adjusted={} recovered={:?} observed_restored={}",
            original_speed, adjusted_speed, observed_adjusted, recovered, observed_restored
        ),
    )?;

    println!("G019 cursor speed guard smoke");
    println!("original_speed={original_speed}");
    println!("adjusted_speed={adjusted_speed}");
    println!("observed_adjusted={observed_adjusted}");
    println!("recovered={recovered:?}");
    println!("observed_restored={observed_restored}");

    if observed_adjusted != adjusted_speed
        || recovered != Some(original_speed)
        || observed_restored != original_speed
    {
        return Err("G019 cursor speed guard did not restore the original mouse speed".into());
    }

    Ok(())
}

fn source_move_smoke_g014() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G014_MOVE_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let shader_cache_root = paths
        .cache_dir
        .join("shader-cache")
        .join("g014-source-move");
    let mut controller =
        ProductRuntimeController::new(shader_cache_root, cursor_speed_guard_path(&paths));
    let start = controller.start(ScalingMode::Windowed, true, InputDeliveryMode::DryRun)?;
    let original_rect = start.source_rect;

    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        let moved_source = move_window_for_probe(start.source_hwnd, 24, 18)
            .map_err(|error| format!("{error:?}"))?;
        std::thread::sleep(Duration::from_millis(120));
        let layout_event = controller
            .refresh_active_layout()?
            .ok_or("G014 source move smoke expected a layout update")?;
        let frame = controller.pump_active_frame()?;
        let stop = controller
            .stop()
            .ok_or("G014 source move smoke expected an active scaling session")?;
        Ok((moved_source, layout_event, frame, stop))
    })();

    let restore_dx = original_rect.left
        - source_window_from_raw(start.source_hwnd)
            .map(|source| source.rect.left)
            .unwrap_or(original_rect.left);
    let restore_dy = original_rect.top
        - source_window_from_raw(start.source_hwnd)
            .map(|source| source.rect.top)
            .unwrap_or(original_rect.top);
    let _ = move_window_for_probe(start.source_hwnd, restore_dx, restore_dy);
    if controller.is_active() {
        let _ = controller.stop();
    }

    let (moved_source, layout_event, frame, stop) = result?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g014_source_move source={} original={},{},{},{} moved={},{},{},{} layout_event={} frame_presented={}",
            start.source_hwnd,
            original_rect.left,
            original_rect.top,
            original_rect.right,
            original_rect.bottom,
            moved_source.rect.left,
            moved_source.rect.top,
            moved_source.rect.right,
            moved_source.rect.bottom,
            layout_event,
            frame.as_ref().map(|report| report.presented_frames).unwrap_or(0)
        ),
    )?;

    println!("G014 source move smoke");
    println!("source_hwnd={}", start.source_hwnd);
    println!(
        "original_rect={},{},{},{}",
        original_rect.left, original_rect.top, original_rect.right, original_rect.bottom
    );
    println!(
        "moved_rect={},{},{},{}",
        moved_source.rect.left,
        moved_source.rect.top,
        moved_source.rect.right,
        moved_source.rect.bottom
    );
    println!("layout_event={layout_event}");
    println!(
        "frame_presented={}",
        frame
            .as_ref()
            .map(|report| report.presented_frames)
            .unwrap_or(0)
    );
    println!(
        "stop source_hwnd={} presented_frames={} active_after_stop={}",
        stop.source_hwnd,
        stop.presented_frames,
        controller.is_active()
    );

    if moved_source.rect.left == original_rect.left
        || !layout_event.contains("runtime_layout_update")
        || controller.is_active()
    {
        return Err("G014 source move smoke did not prove move-follow lifecycle".into());
    }

    Ok(())
}

fn source_resize_smoke_g016() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_G016_RESIZE_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;

    let shader_cache_root = paths
        .cache_dir
        .join("shader-cache")
        .join("g016-source-resize");
    let mut controller =
        ProductRuntimeController::new(shader_cache_root, cursor_speed_guard_path(&paths));
    let start = controller.start(ScalingMode::Windowed, true, InputDeliveryMode::DryRun)?;
    let original_rect = start.source_rect;

    let result = (|| -> Result<_, Box<dyn std::error::Error>> {
        let resized_source = resize_window_for_probe(start.source_hwnd, -80, -60)
            .map_err(|error| format!("{error:?}"))?;
        std::thread::sleep(Duration::from_millis(120));
        let resize_event = controller
            .refresh_active_layout()?
            .ok_or("G016 source resize smoke expected a resize layout update")?;
        std::thread::sleep(SCALER_RESIZE_SETTLE + Duration::from_millis(80));
        let settle_event = controller.refresh_active_layout()?;
        let frame = controller.pump_active_frame()?;
        let stop = controller
            .stop()
            .ok_or("G016 source resize smoke expected an active scaling session")?;
        Ok((resized_source, resize_event, settle_event, frame, stop))
    })();

    if let Ok(current) = source_window_from_raw(start.source_hwnd) {
        let _ = resize_window_for_probe(
            start.source_hwnd,
            original_rect.width() - current.rect.width(),
            original_rect.height() - current.rect.height(),
        );
    }
    if controller.is_active() {
        let _ = controller.stop();
    }

    let (resized_source, resize_event, settle_event, frame, stop) = result?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g016_source_resize source={} original={},{},{},{} resized={},{},{},{} resize_event={} settle_event={:?} frame_presented={}",
            start.source_hwnd,
            original_rect.left,
            original_rect.top,
            original_rect.right,
            original_rect.bottom,
            resized_source.rect.left,
            resized_source.rect.top,
            resized_source.rect.right,
            resized_source.rect.bottom,
            resize_event,
            settle_event,
            frame.as_ref().map(|report| report.presented_frames).unwrap_or(0)
        ),
    )?;

    println!("G016 source resize smoke");
    println!("source_hwnd={}", start.source_hwnd);
    println!(
        "original_rect={},{},{},{}",
        original_rect.left, original_rect.top, original_rect.right, original_rect.bottom
    );
    println!(
        "resized_rect={},{},{},{}",
        resized_source.rect.left,
        resized_source.rect.top,
        resized_source.rect.right,
        resized_source.rect.bottom
    );
    println!("resize_event={resize_event}");
    println!("settle_event={settle_event:?}");
    println!(
        "frame_presented={}",
        frame
            .as_ref()
            .map(|report| report.presented_frames)
            .unwrap_or(0)
    );
    println!(
        "stop source_hwnd={} presented_frames={} active_after_stop={}",
        stop.source_hwnd,
        stop.presented_frames,
        controller.is_active()
    );

    if resized_source.rect.width() >= original_rect.width()
        || !resize_event.contains("layout_policy=preserve_source_resize")
        || controller.is_active()
    {
        return Err(
            "G016 source resize smoke did not prove anchored resize-follow lifecycle".into(),
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParityGateMode {
    Classification,
    Release,
}

fn product_runtime_smoke_g012(mode: ParityGateMode) -> Result<(), Box<dyn std::error::Error>> {
    let parity_label = if std::env::args().any(|arg| arg.contains("g014")) {
        "G014"
    } else {
        "G013"
    };
    let parity_slug = parity_label.to_ascii_lowercase();

    if let Some(data_dir) = std::env::var_os("DODBOGI_G014_DATA_DIR")
        .or_else(|| std::env::var_os("DODBOGI_G013_DATA_DIR"))
        .or_else(|| std::env::var_os("DODBOGI_G012_DATA_DIR"))
    {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    write_default_settings_if_missing(&paths)?;
    let matrix_out = std::env::var_os("DODBOGI_G014_MATRIX_OUT")
        .or_else(|| std::env::var_os("DODBOGI_G013_MATRIX_OUT"))
        .or_else(|| std::env::var_os("DODBOGI_G012_MATRIX_OUT"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                ".omx/evidence/{parity_slug}/{parity_slug}-parity-matrix-generated.md"
            ))
        });
    if let Some(parent) = matrix_out.parent() {
        fs::create_dir_all(parent)?;
    }

    let reference_exe = PathBuf::from("Magpie-v0.12.1-x64/Magpie.exe");
    let reference_log = PathBuf::from("Magpie-v0.12.1-x64/logs/magpie.log");
    let reference_evidence = build_reference_evidence(&reference_exe, &reference_log)?;

    let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
    let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
    let windowed = compute_scaling_layout(source.rect, &monitors, LayoutRequest::default())
        .map_err(|error| format!("{error:?}"))?;
    let fullscreen_closest = compute_scaling_layout(
        source.rect,
        &monitors,
        LayoutRequest {
            mode: ScalingMode::Fullscreen,
            monitor_selection: MonitorSelectionMode::Closest,
            windowed_scale: 2.0,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let fullscreen_intersected = compute_scaling_layout(
        source.rect,
        &monitors,
        LayoutRequest {
            mode: ScalingMode::Fullscreen,
            monitor_selection: MonitorSelectionMode::Intersected,
            windowed_scale: 2.0,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let fullscreen_all = compute_scaling_layout(
        source.rect,
        &monitors,
        LayoutRequest {
            mode: ScalingMode::Fullscreen,
            monitor_selection: MonitorSelectionMode::All,
            windowed_scale: 2.0,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let overlay = OverlayWindow::create_hidden().map_err(|error| format!("{error:?}"))?;
    let style = overlay
        .apply_layout(windowed.destination, false)
        .map_err(|error| format!("{error:?}"))?;
    let item = create_wgc_item_for_hwnd(source.hwnd).map_err(|error| format!("{error:?}"))?;
    let backend_report = probe_additional_backends(source.hwnd);
    let client_rect = client_rect_from_raw(source.hwnd).map_err(|error| format!("{error:?}"))?;
    let include_title = resolve_title_bar_capture_region(
        source.rect,
        client_rect,
        TitleBarCaptureMode::IncludeTitleBar,
    )?;
    let client_only = resolve_title_bar_capture_region(
        source.rect,
        client_rect,
        TitleBarCaptureMode::ClientOnly,
    )?;

    let shader_cache_root = std::env::var_os("DODBOGI_G014_SHADER_CACHE")
        .or_else(|| std::env::var_os("DODBOGI_G013_SHADER_CACHE"))
        .or_else(|| std::env::var_os("DODBOGI_G012_SHADER_CACHE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            paths
                .cache_dir
                .join("shader-cache")
                .join(format!("{parity_slug}-parity"))
        });
    let compile_summary = compile_builtin_effects_with_cache(&shader_cache_root)
        .map_err(|error| format!("shader compile/cache failed: {error:?}"))?;
    let effect_catalog = builtin_effects();
    let mut effect_reports = Vec::new();
    let active_frame_screenshot = paths
        .root
        .join("screenshots")
        .join(format!("{parity_slug}-active-frame-proof.ppm"));
    for effect in &effect_catalog {
        let mut scaler = WgcEffectScaler::create_for_hwnd_and_item(
            overlay.hwnd(),
            windowed.destination.width().max(1) as u32,
            windowed.destination.height().max(1) as u32,
            &item,
            effect.id,
        )
        .map_err(|error| format!("{error:?}"))?;
        let report = scaler
            .present_frames(1, Duration::from_millis(1000))
            .map_err(|error| format!("{error:?}"))?;
        scaler.close();
        println!(
            "effect_runtime id={} frames={} surfaces={} draws={} presents={} capture={}x{} output={}x{}",
            report.effect_id,
            report.frames_observed,
            report.surfaces_observed,
            report.effect_draws,
            report.presented_frames,
            report.capture_width,
            report.capture_height,
            report.output_width,
            report.output_height
        );
        if effect.id == "bilinear" {
            scaler
                .write_current_backbuffer_ppm(&active_frame_screenshot)
                .map_err(|error| format!("{error:?}"))?;
            append_log_line(
                &paths.log_file,
                &format!(
                    "{parity_slug}_active_frame_screenshot path={} effect={} frames={} presents={}",
                    active_frame_screenshot.display(),
                    report.effect_id,
                    report.frames_observed,
                    report.presented_frames
                ),
            )?;
        }
        effect_reports.push(report);
    }
    let effect_runtime_all = effect_reports.len() == effect_catalog.len()
        && effect_reports.iter().all(|report| {
            report.frames_observed > 0
                && report.surfaces_observed > 0
                && report.effect_draws > 0
                && report.presented_frames > 0
        });
    let effect_runtime_total_presents: u32 = effect_reports
        .iter()
        .map(|report| report.presented_frames)
        .sum();

    let live_input_probe = optional_controlled_input_probe();

    let transform = InputTransform::from_rects(windowed.source, windowed.destination)
        .map_err(|error| format!("{error:?}"))?;
    let center = OverlayPoint {
        x: (windowed.destination.left + windowed.destination.right) as f32 / 2.0,
        y: (windowed.destination.top + windowed.destination.bottom) as f32 / 2.0,
    };
    let input_events = [
        OverlayInputEvent {
            kind: InputEventKind::MouseButtonDown(MouseButton::Left),
            point: Some(center),
        },
        OverlayInputEvent {
            kind: InputEventKind::DoubleClick(MouseButton::Left),
            point: Some(center),
        },
        OverlayInputEvent {
            kind: InputEventKind::Wheel { delta: 120 },
            point: Some(center),
        },
        OverlayInputEvent {
            kind: InputEventKind::ContextMenu,
            point: Some(center),
        },
        OverlayInputEvent {
            kind: InputEventKind::KeyboardFocus,
            point: None,
        },
    ];
    let expected_mapped_input_count = input_events.len();
    let mapped_input_count = input_events
        .iter()
        .filter_map(|event| transform.map_event(*event))
        .count();

    let mut settings = DodbogiSettings::default();
    settings.diagnostics.enable_stats_overlay = true;
    settings.profiles.default_profile.auto_scale = true;
    let mut notepad = AppProfile::per_app_profile("notepad", "Notepad", "notepad.exe");
    notepad.effect_chain = vec!["bilinear".to_string(), "adaptive_sharpen".to_string()];
    let mut terminal =
        AppProfile::per_app_profile("terminal", "Windows Terminal", "WindowsTerminal.exe");
    terminal.scaling_mode = ScalingMode::Fullscreen;
    terminal.monitor_selection = MonitorSelectionMode::Intersected;
    terminal.match_rule.window_class = Some("CASCADIA_HOSTING_WINDOW_CLASS".to_string());
    terminal.match_rule.title_contains = Some("Terminal".to_string());
    terminal.effect_chain = vec!["lanczos3".to_string(), "adaptive_sharpen".to_string()];
    settings.profiles.per_app_profiles.push(notepad);
    settings.profiles.per_app_profiles.push(terminal);
    save_settings_to_path(&settings, &paths.settings_file)?;
    let loaded = load_settings_from_path(&paths.settings_file)?;
    let export_path = paths
        .config_dir
        .join(format!("settings-export-{parity_slug}.toml"));
    export_settings_to_path(&loaded, &export_path)?;
    let imported = import_settings_from_path(&export_path)?;
    let coverage = settings_ui_coverage(&imported);
    let diagnostics = DiagnosticsSnapshot::capture(&paths, &imported);
    let default_resolution = imported.resolve_profile(&ProfileMatchContext {
        executable_name: Some("calc.exe".to_string()),
        window_class: None,
        title: None,
    });
    let notepad_resolution =
        imported.resolve_profile(&ProfileMatchContext::for_executable("NOTEPAD.EXE"));
    let terminal_resolution = imported.resolve_profile(&ProfileMatchContext {
        executable_name: Some("WindowsTerminal.exe".to_string()),
        window_class: Some("CASCADIA_HOSTING_WINDOW_CLASS".to_string()),
        title: Some("Terminal".to_string()),
    });

    let screenshot_dir = paths.root.join(&imported.diagnostics.screenshot_dir_name);
    fs::create_dir_all(&screenshot_dir)?;
    let screenshot_fixture =
        checkerboard_fixture(64, 64, 8).map_err(|error| format!("{error:?}"))?;
    let screenshot_meta = screenshot_fixture
        .write_ppm(screenshot_dir.join(format!("{parity_slug}-screenshot-proof.ppm")))?;
    append_log_line(
        &paths.log_file,
        &format!(
            "g012_screenshot path={} size={}x{}",
            screenshot_meta.path.display(),
            screenshot_meta.width,
            screenshot_meta.height
        ),
    )?;

    let mut tray = TrayController::default();
    tray.install_placeholder();
    let system_hotkeys = SystemHotkeyGuard::register_defaults();

    let artifact_bundle = format!(
        "matrix={}; log={}; settings={}; export={}; screenshot={}; active_frame_screenshot={}; shader_cache={}",
        matrix_out.display(),
        paths.log_file.display(),
        paths.settings_file.display(),
        export_path.display(),
        screenshot_meta.path.display(),
        active_frame_screenshot.display(),
        shader_cache_root.display()
    );
    let geometry_artifacts = format!(
        "{}; source={},{},{},{} windowed={},{},{},{} fullscreen_closest={},{},{},{} fullscreen_intersected={},{},{},{} fullscreen_all={},{},{},{} monitors={}",
        artifact_bundle,
        source.rect.left,
        source.rect.top,
        source.rect.right,
        source.rect.bottom,
        windowed.destination.left,
        windowed.destination.top,
        windowed.destination.right,
        windowed.destination.bottom,
        fullscreen_closest.destination.left,
        fullscreen_closest.destination.top,
        fullscreen_closest.destination.right,
        fullscreen_closest.destination.bottom,
        fullscreen_intersected.destination.left,
        fullscreen_intersected.destination.top,
        fullscreen_intersected.destination.right,
        fullscreen_intersected.destination.bottom,
        fullscreen_all.destination.left,
        fullscreen_all.destination.top,
        fullscreen_all.destination.right,
        fullscreen_all.destination.bottom,
        monitors.len()
    );
    let effect_artifacts = format!(
        "{}; effect_count={} runtime_effects={} runtime_presents={} compiled_programs={} cache_hits={}",
        artifact_bundle,
        effect_catalog.len(),
        effect_reports.len(),
        effect_runtime_total_presents,
        compile_summary.total_programs(),
        compile_summary.cache_hits()
    );
    let input_artifacts = format!(
        "{}; mapped_input_events={} controlled_input_delivered={} observed_down={} observed_up={}",
        artifact_bundle,
        mapped_input_count,
        live_input_probe.delivered,
        live_input_probe.observed_left_down,
        live_input_probe.observed_left_up
    );
    let shell_artifacts = format!(
        "{}; tray_items={} registered_hotkeys={} failed_hotkeys={} overlay_no_activate={} topmost={} tool_window={} input_passthrough={} layered_passthrough={} taskbar_entry={} alt_tab_entry={}",
        artifact_bundle,
        tray.menu_items().len(),
        system_hotkeys.report().registered_count(),
        system_hotkeys.report().failed_count(),
        style.no_activate,
        style.topmost,
        style.tool_window,
        style.input_passthrough,
        style.layered_passthrough,
        style.taskbar_entry,
        style.alt_tab_entry
    );
    let settings_artifacts = format!(
        "{}; profiles={} per_app={} coverage_all={} auto_scale_enabled={} default_source={:?} notepad_source={:?} terminal_source={:?}",
        artifact_bundle,
        diagnostics.profile_count,
        diagnostics.per_app_profile_count,
        coverage.all_required_covered(),
        imported.profiles.default_profile.auto_scale,
        default_resolution.source,
        notepad_resolution.source,
        terminal_resolution.source
    );
    let capture_artifacts = format!(
        "{}; include_title={},{},{},{} client_only={},{},{},{} backend_probes={}",
        artifact_bundle,
        include_title.left,
        include_title.top,
        include_title.right,
        include_title.bottom,
        client_only.left,
        client_only.top,
        client_only.right,
        client_only.bottom,
        backend_report.probes.len()
    );

    let backend_status = |kind: CaptureBackendKind| {
        backend_report
            .probes
            .iter()
            .find(|probe| probe.kind == kind)
            .map(|_probe| ScenarioResult::Partial)
            .unwrap_or(ScenarioResult::Partial)
    };
    let backend_note = |kind: CaptureBackendKind| {
        backend_report
            .probes
            .iter()
            .find(|probe| probe.kind == kind)
            .map(|probe| probe.detail.clone())
            .unwrap_or_else(|| "backend probe did not return a row".to_string())
    };

    let scenario_artifact_dir = std::env::var_os("DODBOGI_G014_SCENARIO_ARTIFACT_DIR")
        .or_else(|| std::env::var_os("DODBOGI_G013_SCENARIO_ARTIFACT_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.root.join("scenario-artifacts").join(&parity_slug));
    fs::create_dir_all(&scenario_artifact_dir)?;

    let rows = vec![
        row(&scenario_artifact_dir, "P0-NOTEPAD-START-001", "P0", "launch,lifecycle", &reference_evidence, ScenarioResult::Partial, &artifact_bundle, "exact", "Foreground-window start path is product-proven; app-specific Notepad visual observation remains a human parity confirmation.")?, 
        row(&scenario_artifact_dir, "P0-HOTKEY-STOP-001", "P0", "launch,lifecycle,hotkey", &reference_evidence, ScenarioResult::Partial, &shell_artifacts, "exact", "Registered system hotkeys and controller stop path are proven; global hotkey injection was not run because an unrelated user Magpie process is active.")?, 
        row(&scenario_artifact_dir, "P0-TERMINAL-START-001", "P0", "launch,lifecycle", &reference_evidence, ScenarioResult::Partial, &settings_artifacts, "exact", "Terminal profile matching is proven; app-specific Terminal visual observation remains a human parity confirmation.")?, 
        row(&scenario_artifact_dir, "P0-NOACTIVATE-OVERLAY-001", "P0", "window,z-order,focus", &reference_evidence, ScenarioResult::Pass, &shell_artifacts, "exact", "Overlay style contract has no-activate/topmost/tool-window and no taskbar/Alt+Tab entry.")?, 
        row(&scenario_artifact_dir, "P0-DESTINATION-SIZE-2X-001", "P0", "geometry,rendering", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "Windowed 2x destination is computed in physical pixels and used by the WGC effect swapchain.")?, 
        row(&scenario_artifact_dir, "P0-SOURCE-CLOSE-001", "P0", "lifecycle", &reference_evidence, ScenarioResult::Pass, &artifact_bundle, "exact", "Core lifecycle policy stops on source close.")?, 
        row(&scenario_artifact_dir, "P0-SOURCE-MINIMIZE-001", "P0", "lifecycle", &reference_evidence, ScenarioResult::Pass, &artifact_bundle, "exact", "Core lifecycle policy stops on source minimize.")?, 
        row(&scenario_artifact_dir, "P1-FULLSCREEN-001", "P1", "geometry,window", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "Fullscreen closest-monitor destination is computed from live monitor geometry.")?, 
        row(&scenario_artifact_dir, "P1-WINDOWED-001", "P1", "geometry,window", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "Windowed destination is computed and used by product runtime.")?, 
        row(&scenario_artifact_dir, "P1-SOURCE-MOVE-001", "P1", "lifecycle,geometry", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "Geometry recomputation primitives are proven; live move-follow observation remains manual.")?, 
        row(&scenario_artifact_dir, "P1-SOURCE-RESIZE-001", "P1", "lifecycle,geometry", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "Geometry recomputation primitives are proven; live resize-follow observation remains manual.")?, 
        row(&scenario_artifact_dir, "P1-SCALED-MOVE-001", "P1", "window,geometry", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "Overlay layout is applied by HWND; drag interaction observation remains manual.")?, 
        row(&scenario_artifact_dir, "P1-SCALED-RESIZE-001", "P1", "window,geometry", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "Overlay layout can be resized by policy; live user resize observation remains manual.")?, 
        row(&scenario_artifact_dir, "P1-FOCUS-LOSS-001", "P1", "focus,input", &reference_evidence, ScenarioResult::Pass, &shell_artifacts, "exact", "No-activate overlay and focus-loss policy are implemented.")?, 
        row(&scenario_artifact_dir, "P1-POPUP-MENU-001", "P1", "window,input", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Context-menu input maps to source coordinates and right-button delivery is represented.")?, 
        row(&scenario_artifact_dir, "P1-ALTTAB-TASKBAR-001", "P1", "shell,window", &reference_evidence, ScenarioResult::Pass, &shell_artifacts, "exact", "Overlay is tool/no-activate and excluded from taskbar/Alt+Tab contract.")?, 
        row(&scenario_artifact_dir, "P2-CLICK-001", "P2", "input", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Controlled HWND observed SendInput left down/up.")?, 
        row(&scenario_artifact_dir, "P2-DOUBLECLICK-001", "P2", "input", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Double-click maps through overlay-to-source transform and has a SendInput sequence.")?, 
        row(&scenario_artifact_dir, "P2-DRAG-001", "P2", "input", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Drag phases map through overlay-to-source transform and have SendInput sequences.")?, 
        row(&scenario_artifact_dir, "P2-WHEEL-001", "P2", "input", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Wheel maps through overlay-to-source transform and has a SendInput sequence.")?, 
        row(&scenario_artifact_dir, "P2-TEXT-SELECTION-001", "P2", "input", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Text-selection phases map and use mouse down/move/up sequences.")?, 
        row(&scenario_artifact_dir, "P2-CONTEXT-MENU-001", "P2", "input,window", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "exact", "Context-menu maps and uses right-click SendInput sequence.")?, 
        row(&scenario_artifact_dir, "P2-KEYBOARD-FOCUS-001", "P2", "input,focus", &reference_evidence, ScenarioResult::Partial, &input_artifacts, "exact", "Keyboard focus request is implemented; Windows foreground policy may deny arbitrary targets.")?, 
        row(&scenario_artifact_dir, "P2-CURSOR-DISPLAY-001", "P2", "cursor", &reference_evidence, ScenarioResult::Pass, &input_artifacts, "visual-equivalent", "Cursor overlay policy derives visibility/draw/scale from the active transform.")?, 
        row(&scenario_artifact_dir, "P3-WGC-001", "P3", "capture", &reference_evidence, ScenarioResult::Pass, &effect_artifacts, "exact", "WGC produces captured surfaces consumed by every clean-room effect in this smoke.")?, 
        row(&scenario_artifact_dir, "P3-DDA-001", "P3", "capture", &reference_evidence, backend_status(CaptureBackendKind::DesktopDuplication), &capture_artifacts, "exact", format!("Desktop Duplication classified by probe; runtime remains WGC-first. {}", backend_note(CaptureBackendKind::DesktopDuplication)))?, 
        row(&scenario_artifact_dir, "P3-GDI-001", "P3", "capture", &reference_evidence, backend_status(CaptureBackendKind::Gdi), &capture_artifacts, "exact", format!("GDI classified by BitBlt probe; runtime remains WGC-first. {}", backend_note(CaptureBackendKind::Gdi)))?, 
        row(&scenario_artifact_dir, "P3-DWM-001", "P3", "capture", &reference_evidence, ScenarioResult::Partial, &capture_artifacts, "exact", format!("Public DWM frame-bounds metadata is available, but practical frame capture support or an approved unsupported classification remains unresolved. {}", backend_note(CaptureBackendKind::DwmSharedSurface)))?, 
        row(&scenario_artifact_dir, "P3-TITLEBAR-001", "P3", "capture,window", &reference_evidence, ScenarioResult::Pass, &capture_artifacts, "exact", "Include-title-bar and client-only capture regions are computed from live HWND rectangles.")?, 
        row(&scenario_artifact_dir, "P4-DPI-100-001", "P4", "dpi,geometry", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "Live monitor DPI is recorded and physical-pixel geometry is used.")?, 
        row(&scenario_artifact_dir, "P4-DPI-125-001", "P4", "dpi,geometry", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "125% DPI math is covered by unit/stage evidence; current live monitor may not be 125%.")?, 
        row(&scenario_artifact_dir, "P4-DPI-150-001", "P4", "dpi,geometry", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "150% DPI math is covered by unit/stage evidence; current live monitor may not be 150%.")?, 
        row(&scenario_artifact_dir, "P4-MIXED-DPI-001", "P4", "dpi,monitor", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "Mixed-DPI algorithm is covered; hardware layout may not contain mixed-DPI monitors.")?, 
        row(&scenario_artifact_dir, "P4-PARTLY-OFFSCREEN-001", "P4", "geometry,monitor", &reference_evidence, ScenarioResult::Partial, &geometry_artifacts, "exact", "Partly offscreen handling is covered by geometry algorithms; live offscreen source not forced in smoke.")?, 
        row(&scenario_artifact_dir, "P4-CLOSEST-MONITOR-001", "P4", "monitor", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "Closest monitor selection is computed from live monitor geometry.")?, 
        row(&scenario_artifact_dir, "P4-INTERSECTED-MONITOR-001", "P4", "monitor", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "Intersected monitor selection is computed from live monitor geometry.")?, 
        row(&scenario_artifact_dir, "P4-ALL-MONITORS-001", "P4", "monitor", &reference_evidence, ScenarioResult::Pass, &geometry_artifacts, "exact", "All-monitors destination is computed from live monitor geometry.")?, 
        row(&scenario_artifact_dir, "P5-DEFAULT-PROFILE-001", "P5", "profile,settings", &reference_evidence, ScenarioResult::Pass, &settings_artifacts, "exact", "Default profile resolves for unmatched apps.")?, 
        row(&scenario_artifact_dir, "P5-PERAPP-PROFILE-001", "P5", "profile,settings", &reference_evidence, ScenarioResult::Pass, &settings_artifacts, "exact", "Notepad and Terminal per-app matching resolve with scores.")?, 
        row(&scenario_artifact_dir, "P5-AUTOSCALE-001", "P5", "profile,lifecycle", &reference_evidence, ScenarioResult::Pass, &settings_artifacts, "exact", "Default-profile auto-scale setting is persisted and product idle loop starts foreground scaling when enabled.")?, 
        row(&scenario_artifact_dir, "P5-HOTKEY-SETTINGS-001", "P5", "settings,hotkey", &reference_evidence, ScenarioResult::Pass, &shell_artifacts, "exact", "Default hotkey settings and system registration report are generated.")?, 
        row(&scenario_artifact_dir, "P5-TRAY-MENU-001", "P5", "tray,shell", &reference_evidence, ScenarioResult::Pass, &shell_artifacts, "exact", "Tray menu contract contains start/stop, profile, screenshot, settings, diagnostics, and exit items.")?, 
        row(&scenario_artifact_dir, "P5-SCREENSHOT-001", "P5", "screenshot,diagnostics", &reference_evidence, ScenarioResult::Pass, &artifact_bundle, "exact", "Screenshot artifact path and active WGC/effect swapchain readback proof are recorded.")?, 
        row(&scenario_artifact_dir, "P5-LOGGING-001", "P5", "diagnostics", &reference_evidence, ScenarioResult::Pass, &settings_artifacts, "exact", "Diagnostics snapshot and runtime log are written.")?, 
        row(&scenario_artifact_dir, "P6-EFFECTS-COVERAGE-001", "P6", "effects", &reference_evidence, ScenarioResult::Pass, &effect_artifacts, "visual-equivalent", "Every built-in clean-room effect compiled and presented at least one WGC frame.")?, 
        row(&scenario_artifact_dir, "P6-EFFECT-PARAMS-001", "P6", "effects,settings", &reference_evidence, ScenarioResult::Pass, &effect_artifacts, "exact", "Effect descriptors and profile chains validate parameters without Magpie source reuse.")?, 
        row(&scenario_artifact_dir, "P6-BASELINE-RENDERING-001", "P6", "rendering", &reference_evidence, ScenarioResult::Pass, &effect_artifacts, "visual-equivalent", "WGC texture path renders through all effect shaders instead of clear-only presentation.")?, 
        row(&scenario_artifact_dir, "P6-STATS-OVERLAY-001", "P6", "rendering,diagnostics", &reference_evidence, ScenarioResult::Pass, &settings_artifacts, "exact", "Stats overlay formatting and diagnostics setting are enabled in the smoke profile.")?, 
        row(&scenario_artifact_dir, "P6-VISUAL-DIFF-001", "P6", "effects,rendering", &reference_evidence, ScenarioResult::Partial, &effect_artifacts, "visual-equivalent", "Runtime effect presentation and fixture screenshot are captured; side-by-side Magpie pixel diff still requires manual screen capture.")?, 
    ];
    let summary = summarize(&rows);
    fs::write(
        &matrix_out,
        render_markdown(
            &format!("{parity_label} full scenario classification matrix"),
            &rows,
        ),
    )?;
    append_log_line(
        &paths.log_file,
        &format!(
            "{}_parity_smoke rows={} classified={} pass={} partial={} unsupported={} matrix={}",
            parity_slug,
            summary.total,
            summary.classified,
            summary.pass,
            summary.partial,
            summary.unsupported,
            matrix_out.display()
        ),
    )?;

    println!(
        "{}",
        match mode {
            ParityGateMode::Classification => {
                if parity_label == "G014" {
                    "G014 parity classification smoke"
                } else {
                    "G013 parity classification smoke"
                }
            }
            ParityGateMode::Release => {
                if parity_label == "G014" {
                    "G014 parity release gate"
                } else {
                    "G013 parity release gate"
                }
            }
        }
    );
    println!("reference_evidence={reference_evidence}");
    println!("source_hwnd={}", source.hwnd);
    println!("monitor_count={}", monitors.len());
    println!(
        "overlay_style no_activate={} topmost={} tool_window={} input_passthrough={} layered_passthrough={} taskbar_entry={} alt_tab_entry={}",
        style.no_activate,
        style.topmost,
        style.tool_window,
        style.input_passthrough,
        style.layered_passthrough,
        style.taskbar_entry,
        style.alt_tab_entry
    );
    println!(
        "effect_runtime_all={} effect_count={} presented_total={}",
        effect_runtime_all,
        effect_catalog.len(),
        effect_runtime_total_presents
    );
    println!(
        "input controlled_delivered={} mapped_events={}",
        live_input_probe.delivered, mapped_input_count
    );
    println!("input_probe detail={}", live_input_probe.detail);
    println!(
        "settings profiles={} per_app={} coverage_all={} tray_items={} hotkeys_registered={} hotkeys_failed={}",
        diagnostics.profile_count,
        diagnostics.per_app_profile_count,
        coverage.all_required_covered(),
        tray.menu_items().len(),
        system_hotkeys.report().registered_count(),
        system_hotkeys.report().failed_count()
    );
    for probe in &backend_report.probes {
        println!(
            "backend_probe kind={:?} available={} detail={}",
            probe.kind, probe.available, probe.detail
        );
    }
    println!(
        "scenario_summary total={} classified={} pass={} partial={} unsupported={} blocked={} fail={} pending={} unapproved_release_blockers={} release_clean={}",
        summary.total,
        summary.classified,
        summary.pass,
        summary.partial,
        summary.unsupported,
        summary.blocked,
        summary.fail,
        summary.pending,
        summary.unapproved_release_blockers(),
        summary.release_clean_without_exceptions()
    );
    println!("matrix={}", matrix_out.display());
    println!("screenshot={}", screenshot_meta.path.display());

    let classification_ready = classification_runtime_ready(
        summary,
        effect_runtime_all,
        mapped_input_count,
        expected_mapped_input_count,
        coverage.all_required_covered(),
        !tray.menu_items().is_empty(),
    );
    if mode == ParityGateMode::Release && !summary.release_clean_without_exceptions() {
        return Err(format!(
            "{parity_label} parity release gate blocked: {} unapproved release blockers remain (partial={} unsupported={} blocked={} fail={} pending={})",
            summary.unapproved_release_blockers(),
            summary.partial,
            summary.unsupported,
            summary.blocked,
            summary.fail,
            summary.pending
        )
        .into());
    }
    if !classification_ready {
        return Err(
            "parity classification smoke did not classify all rows or missed required runtime evidence"
                .into(),
        );
    }

    Ok(())
}

fn optional_controlled_input_probe() -> ControlledInputProbeReport {
    run_controlled_input_probe().unwrap_or_else(|error| ControlledInputProbeReport {
        target_hwnd: 0,
        sent_events: 0,
        observed_left_down: 0,
        observed_left_up: 0,
        delivered: false,
        detail: format!("controlled SendInput probe unavailable in this session: {error:?}"),
    })
}

fn classification_runtime_ready(
    summary: ScenarioSummary,
    effect_runtime_all: bool,
    mapped_input_count: usize,
    expected_mapped_input_count: usize,
    settings_coverage_all: bool,
    tray_has_items: bool,
) -> bool {
    summary.all_classified()
        && effect_runtime_all
        && mapped_input_count == expected_mapped_input_count
        && settings_coverage_all
        && tray_has_items
}

#[allow(clippy::too_many_arguments)]
fn row(
    scenario_artifact_dir: &Path,
    scenario_id: impl Into<String>,
    priority: &'static str,
    feature_area: &'static str,
    reference_evidence: &str,
    result: ScenarioResult,
    artifacts: &str,
    tolerance: &'static str,
    verifier_notes: impl Into<String>,
) -> Result<ScenarioEvidenceRow, Box<dyn std::error::Error>> {
    let scenario_id = scenario_id.into();
    let artifacts = write_dodbogi_scenario_artifact(
        scenario_artifact_dir,
        &scenario_id,
        priority,
        feature_area,
        artifacts,
    )?;
    let mut result = result;
    let mut verifier_notes = verifier_notes.into();
    let has_magpie_artifact =
        has_concrete_magpie_scenario_artifact(&scenario_id, reference_evidence);
    let has_dodbogi_artifact = has_concrete_dodbogi_scenario_artifact(&scenario_id, &artifacts);
    let has_pass_verdict = has_scenario_pass_verdict(&scenario_id, reference_evidence);
    let has_required_runtime_evidence =
        dodbogi_runtime_evidence_supports_scenario(feature_area, &artifacts);
    let release_ready = has_magpie_artifact
        && has_dodbogi_artifact
        && has_pass_verdict
        && has_required_runtime_evidence;
    if result == ScenarioResult::Partial && release_ready {
        result = ScenarioResult::Pass;
        verifier_notes = format!(
            "Scenario-keyed Magpie artifact, Dodbogi artifact, and explicit pass verdict are present. {verifier_notes}"
        );
    } else if result == ScenarioResult::Pass && !release_ready {
        result = ScenarioResult::Partial;
        let missing = missing_release_evidence(
            has_magpie_artifact,
            has_dodbogi_artifact,
            has_pass_verdict,
            has_required_runtime_evidence,
        );
        verifier_notes =
            format!("Dodbogi implementation evidence is passing, but {missing}. {verifier_notes}");
    }

    Ok(ScenarioEvidenceRow::new(ScenarioEvidenceRowInput {
        magpie_settings: scenario_magpie_settings(&scenario_id, feature_area),
        environment: scenario_environment(),
        magpie_observation: scenario_magpie_observation(&scenario_id, feature_area),
        magpie_artifacts: reference_evidence.to_string(),
        dodbogi_result: result,
        dodbogi_artifacts: artifacts,
        owner: scenario_owner(feature_area),
        scenario_id,
        priority,
        feature_area,
        tolerance,
        verifier_notes,
    }))
}

fn build_reference_evidence(
    reference_exe: &Path,
    reference_log: &Path,
) -> Result<String, Box<dyn std::error::Error>> {
    let reference_log_len = fs::metadata(reference_log)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let mut evidence = format!(
        "Magpie reference package observed before Dodbogi smoke; exe_exists={} log_exists={} log_bytes={} log={}",
        reference_exe.is_file(),
        reference_log.is_file(),
        reference_log_len,
        reference_log.display()
    );

    if let Some(raw) = std::env::var_os("DODBOGI_G014_MAGPIE_EVIDENCE") {
        evidence.push_str("; ");
        evidence.push_str(&raw.to_string_lossy());
    }
    if let Some(path) = std::env::var_os("DODBOGI_G014_MAGPIE_EVIDENCE_FILE") {
        let path = PathBuf::from(path);
        evidence.push_str("; magpie_evidence_file=");
        evidence.push_str(&path.display().to_string());
        evidence.push_str("; ");
        evidence.push_str(&fs::read_to_string(path)?);
    }

    Ok(evidence)
}

fn write_dodbogi_scenario_artifact(
    artifact_dir: &Path,
    scenario_id: &str,
    priority: &str,
    feature_area: &str,
    artifacts: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    fs::create_dir_all(artifact_dir)?;
    let artifact_path = artifact_dir.join(format!("{scenario_id}.txt"));
    fs::write(
        &artifact_path,
        format!(
            "scenario_id={scenario_id}\npriority={priority}\nfeature_area={feature_area}\nsource=Dodbogi parity smoke\nartifacts={artifacts}\n"
        ),
    )?;
    Ok(format!(
        "dodbogi_artifact[{scenario_id}]={}; {artifacts}",
        artifact_path.display()
    ))
}

fn has_concrete_magpie_scenario_artifact(scenario_id: &str, reference_evidence: &str) -> bool {
    concrete_artifact_paths(
        scenario_id,
        reference_evidence,
        &["scenario_artifact", "magpie_video", "magpie_screenshot"],
    )
    .iter()
    .any(|path| Path::new(path).is_file())
}

fn has_concrete_dodbogi_scenario_artifact(scenario_id: &str, artifacts: &str) -> bool {
    concrete_artifact_paths(scenario_id, artifacts, &["dodbogi_artifact"])
        .iter()
        .any(|path| Path::new(path).is_file())
}

fn has_scenario_pass_verdict(scenario_id: &str, reference_evidence: &str) -> bool {
    scenario_marker_values(
        scenario_id,
        reference_evidence,
        &["scenario_verdict", "parity_verdict"],
    )
    .iter()
    .any(|value| value.eq_ignore_ascii_case("pass"))
}

fn dodbogi_runtime_evidence_supports_scenario(feature_area: &str, artifacts: &str) -> bool {
    if !feature_area
        .split(',')
        .map(str::trim)
        .any(|area| area == "input")
    {
        return true;
    }

    evidence_bool_marker(artifacts, "controlled_input_delivered").unwrap_or(false)
        && evidence_usize_marker(artifacts, "mapped_input_events").unwrap_or(0) > 0
}

fn evidence_bool_marker(evidence: &str, key: &str) -> Option<bool> {
    match evidence_marker_value(evidence, key)?
        .to_ascii_lowercase()
        .as_str()
    {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn evidence_usize_marker(evidence: &str, key: &str) -> Option<usize> {
    evidence_marker_value(evidence, key)?.parse().ok()
}

fn evidence_marker_value(evidence: &str, key: &str) -> Option<String> {
    let marker = format!("{key}=");
    evidence
        .split(|ch: char| ch == ';' || ch.is_whitespace())
        .find_map(|token| token.strip_prefix(&marker).map(str::to_string))
        .filter(|value| !value.is_empty())
}

fn missing_release_evidence(
    has_magpie_artifact: bool,
    has_dodbogi_artifact: bool,
    has_pass_verdict: bool,
    has_required_runtime_evidence: bool,
) -> String {
    let mut missing = Vec::new();
    if !has_magpie_artifact {
        missing.push("Magpie-first scenario artifact");
    }
    if !has_dodbogi_artifact {
        missing.push("Dodbogi-second scenario artifact");
    }
    if !has_pass_verdict {
        missing.push("explicit scenario pass verdict");
    }
    if !has_required_runtime_evidence {
        missing.push("Dodbogi runtime evidence required for this feature area");
    }
    format!("{} still pending", missing.join(", "))
}

fn concrete_artifact_paths(
    scenario_id: &str,
    evidence: &str,
    marker_prefixes: &[&str],
) -> Vec<String> {
    scenario_marker_values(scenario_id, evidence, marker_prefixes)
}

fn scenario_marker_values(
    scenario_id: &str,
    evidence: &str,
    marker_prefixes: &[&str],
) -> Vec<String> {
    let mut values = Vec::new();
    for prefix in marker_prefixes {
        let marker = format!("{prefix}[{scenario_id}]=");
        let mut remaining = evidence;
        while let Some(index) = remaining.find(&marker) {
            let after_marker = &remaining[index + marker.len()..];
            if let Some(value) = parse_marker_value(after_marker) {
                values.push(value);
            }
            remaining = &after_marker[after_marker.len().min(1)..];
        }
    }
    values
}

fn parse_marker_value(value: &str) -> Option<String> {
    let value = value.trim_start();
    if let Some(rest) = value.strip_prefix('"') {
        return rest.find('"').map(|end| rest[..end].trim().to_string());
    }

    let end = value.find([';', '\n', '\r']).unwrap_or(value.len());
    let path = value[..end].trim();
    (!path.is_empty()).then(|| path.to_string())
}

fn scenario_environment() -> String {
    "stage-a=.omx/evidence/stage-a/environment.json; dxdiag=.omx/evidence/stage-a/dxdiag.txt; live host details are emitted by this smoke run".to_string()
}

fn scenario_magpie_settings(scenario_id: &str, feature_area: &str) -> String {
    format!(
        "Magpie v0.12.1 equivalent settings for {scenario_id}; feature_area={feature_area}; exact capture/effect/profile values require Magpie-first scenario observation"
    )
}

fn scenario_magpie_observation(scenario_id: &str, feature_area: &str) -> String {
    format!(
        "tests/reference/magpie-behavior.md#{anchor}; tests/reference/magpie-v0.12.1-scenario-evidence.md#{scenario_id}",
        anchor = feature_area
            .split(',')
            .next()
            .unwrap_or("scenario")
            .replace(' ', "-")
    )
}

fn scenario_owner(feature_area: &str) -> &'static str {
    if feature_area.contains("capture") {
        "capture"
    } else if feature_area.contains("input")
        || feature_area.contains("cursor")
        || feature_area.contains("focus")
    {
        "input,win32"
    } else if feature_area.contains("effect") || feature_area.contains("rendering") {
        "render-d3d11,effects"
    } else if feature_area.contains("profile")
        || feature_area.contains("settings")
        || feature_area.contains("tray")
        || feature_area.contains("diagnostics")
    {
        "app,core"
    } else if feature_area.contains("window")
        || feature_area.contains("geometry")
        || feature_area.contains("dpi")
        || feature_area.contains("monitor")
    {
        "core,win32"
    } else {
        "app"
    }
}

fn stage_c_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let mut session = ScalingSession::default();
    session.begin_waiting(ScalingMode::Windowed)?;

    let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
    session.start_scaling(source)?;

    let feature_level =
        probe_d3d11_hardware_feature_level().map_err(|error| format!("{error:?}"))?;
    let overlay = OverlayWindow::create_hidden().map_err(|error| format!("{error:?}"))?;
    let item = create_wgc_item_for_hwnd(source.hwnd).map_err(|error| format!("{error:?}"))?;
    let wgc_report = probe_wgc_d3d11_frame_path(&item, Duration::from_millis(750))
        .map_err(|error| format!("{error:?}"))?;
    let presenter = BaselinePresenter::create_for_hwnd(overlay.hwnd(), 640, 480)
        .map_err(|error| format!("{error:?}"))?;
    let present_report = presenter
        .present_baseline_clear([0.02, 0.04, 0.08, 1.0])
        .map_err(|error| format!("{error:?}"))?;

    println!("Stage C smoke");
    println!("source_hwnd={}", source.hwnd);
    println!(
        "source_rect={},{},{},{}",
        source.rect.left, source.rect.top, source.rect.right, source.rect.bottom
    );
    println!("d3d11_feature_level={feature_level}");
    println!("overlay_hwnd={}", overlay.hwnd());
    println!("wgc_item=created");
    println!(
        "wgc_item_size={}x{}",
        wgc_report.item_width, wgc_report.item_height
    );
    println!("wgc_frame_pool_created={}", wgc_report.frame_pool_created);
    println!("wgc_session_started={}", wgc_report.session_started);
    println!(
        "wgc_first_frame={}",
        wgc_report
            .first_frame_size
            .map(|(width, height)| format!("{width}x{height}"))
            .unwrap_or_else(|| "not-observed-within-750ms".to_string())
    );
    println!("wgc_poll_attempts={}", wgc_report.poll_attempts);
    if let Some(error) = &wgc_report.last_poll_error {
        println!("wgc_last_poll_error={error}");
    }
    println!(
        "baseline_presented={} size={}x{} feature_level={}",
        present_report.presented,
        present_report.width,
        present_report.height,
        present_report.feature_level
    );

    session.stop(StopReason::UserToggle);
    println!("session_state={:?}", session.state());
    Ok(())
}

fn stage_d_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
    let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
    let windowed = compute_scaling_layout(source.rect, &monitors, LayoutRequest::default())
        .map_err(|error| format!("{error:?}"))?;
    let fullscreen_closest = compute_scaling_layout(
        source.rect,
        &monitors,
        LayoutRequest {
            mode: ScalingMode::Fullscreen,
            monitor_selection: MonitorSelectionMode::Closest,
            windowed_scale: 2.0,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let fullscreen_intersected = compute_scaling_layout(
        source.rect,
        &monitors,
        LayoutRequest {
            mode: ScalingMode::Fullscreen,
            monitor_selection: MonitorSelectionMode::Intersected,
            windowed_scale: 2.0,
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let fullscreen_all = compute_scaling_layout(
        source.rect,
        &monitors,
        LayoutRequest {
            mode: ScalingMode::Fullscreen,
            monitor_selection: MonitorSelectionMode::All,
            windowed_scale: 2.0,
        },
    )
    .map_err(|error| format!("{error:?}"))?;

    let overlay = OverlayWindow::create_hidden().map_err(|error| format!("{error:?}"))?;
    let style = overlay
        .apply_layout(windowed.destination, false)
        .map_err(|error| format!("{error:?}"))?;
    let minimize_policy = evaluate_source_window_event(source, SourceWindowEvent::Minimized);
    let focus_policy = evaluate_source_window_event(source, SourceWindowEvent::FocusLost);
    let popup_policy = evaluate_source_window_event(source, SourceWindowEvent::PopupOpened);

    println!("Stage D smoke");
    println!("source_hwnd={}", source.hwnd);
    println!(
        "source_rect={},{},{},{}",
        source.rect.left, source.rect.top, source.rect.right, source.rect.bottom
    );
    println!("monitor_count={}", monitors.len());
    for monitor in &monitors {
        println!(
            "monitor id={} primary={} bounds={},{},{},{} work_area={},{},{},{} dpi={}x{}",
            monitor.id,
            monitor.primary,
            monitor.bounds.left,
            monitor.bounds.top,
            monitor.bounds.right,
            monitor.bounds.bottom,
            monitor.work_area.left,
            monitor.work_area.top,
            monitor.work_area.right,
            monitor.work_area.bottom,
            monitor.dpi.x,
            monitor.dpi.y
        );
    }
    println!(
        "windowed_destination={},{},{},{} monitors={:?} dpi={}x{}",
        windowed.destination.left,
        windowed.destination.top,
        windowed.destination.right,
        windowed.destination.bottom,
        windowed.monitor_ids,
        windowed.dpi.x,
        windowed.dpi.y
    );
    println!(
        "fullscreen_closest_destination={},{},{},{} monitors={:?}",
        fullscreen_closest.destination.left,
        fullscreen_closest.destination.top,
        fullscreen_closest.destination.right,
        fullscreen_closest.destination.bottom,
        fullscreen_closest.monitor_ids
    );
    println!(
        "fullscreen_intersected_destination={},{},{},{} monitors={:?}",
        fullscreen_intersected.destination.left,
        fullscreen_intersected.destination.top,
        fullscreen_intersected.destination.right,
        fullscreen_intersected.destination.bottom,
        fullscreen_intersected.monitor_ids
    );
    println!(
        "fullscreen_all_destination={},{},{},{} monitors={:?}",
        fullscreen_all.destination.left,
        fullscreen_all.destination.top,
        fullscreen_all.destination.right,
        fullscreen_all.destination.bottom,
        fullscreen_all.monitor_ids
    );
    println!("overlay_hwnd={}", overlay.hwnd());
    println!(
        "overlay_style no_activate={} topmost={} tool_window={} input_passthrough={} layered_passthrough={} taskbar_entry={} alt_tab_entry={}",
        style.no_activate,
        style.topmost,
        style.tool_window,
        style.input_passthrough,
        style.layered_passthrough,
        style.taskbar_entry,
        style.alt_tab_entry
    );
    println!("minimize_policy={:?}", minimize_policy);
    println!("focus_policy={:?}", focus_policy);
    println!("popup_policy={:?}", popup_policy);
    Ok(())
}

fn stage_e_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
    let monitors = enumerate_monitors().map_err(|error| format!("{error:?}"))?;
    let layout = compute_scaling_layout(source.rect, &monitors, LayoutRequest::default())
        .map_err(|error| format!("{error:?}"))?;
    let transform = InputTransform::from_rects(layout.source, layout.destination)
        .map_err(|error| format!("{error:?}"))?;
    let cursor = CursorRenderPolicy::from_transform(&transform);

    let center = OverlayPoint {
        x: (layout.destination.left + layout.destination.right) as f32 / 2.0,
        y: (layout.destination.top + layout.destination.bottom) as f32 / 2.0,
    };
    let click = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::MouseButtonDown(MouseButton::Left),
            point: Some(center),
        })
        .ok_or("click did not map")?;
    let double_click = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::DoubleClick(MouseButton::Left),
            point: Some(center),
        })
        .ok_or("double click did not map")?;
    let wheel = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::Wheel { delta: 120 },
            point: Some(center),
        })
        .ok_or("wheel did not map")?;
    let drag = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::Drag {
                button: MouseButton::Left,
                phase: DragPhase::Move,
            },
            point: Some(center),
        })
        .ok_or("drag did not map")?;
    let context_menu = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::ContextMenu,
            point: Some(center),
        })
        .ok_or("context menu did not map")?;
    let text_selection = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::TextSelection {
                phase: TextSelectionPhase::Update,
            },
            point: Some(center),
        })
        .ok_or("text selection did not map")?;
    let key_focus = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::KeyboardFocus,
            point: None,
        })
        .ok_or("keyboard focus did not map")?;
    let touch = transform
        .map_event(OverlayInputEvent {
            kind: InputEventKind::Touch {
                id: 1,
                phase: TouchPhase::Move,
            },
            point: Some(center),
        })
        .ok_or("touch did not map")?;

    println!("Stage E smoke");
    println!("source_hwnd={}", source.hwnd);
    println!(
        "source_rect={},{},{},{}",
        source.rect.left, source.rect.top, source.rect.right, source.rect.bottom
    );
    println!(
        "destination_rect={},{},{},{}",
        layout.destination.left,
        layout.destination.top,
        layout.destination.right,
        layout.destination.bottom
    );
    println!("scale={}x{}", transform.scale_x, transform.scale_y);
    println!("click_source={:?}", click.point);
    println!("double_click_source={:?}", double_click.point);
    println!("wheel_source={:?} kind={:?}", wheel.point, wheel.kind);
    println!("drag_source={:?} kind={:?}", drag.point, drag.kind);
    println!(
        "context_menu_source={:?} kind={:?}",
        context_menu.point, context_menu.kind
    );
    println!(
        "text_selection_source={:?} kind={:?}",
        text_selection.point, text_selection.kind
    );
    println!("keyboard_focus={:?}", key_focus.kind);
    println!("touch_source={:?} kind={:?}", touch.point, touch.kind);
    println!(
        "cursor visible={} draw_in_overlay={} scale={} speed_mode={:?} touch_support={:?}",
        cursor.visible,
        cursor.draw_in_overlay,
        cursor.scale,
        cursor.speed_mode,
        cursor.touch_support
    );
    Ok(())
}

fn stage_f_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let source = foreground_source_window().map_err(|error| format!("{error:?}"))?;
    let client_rect = client_rect_from_raw(source.hwnd).map_err(|error| format!("{error:?}"))?;
    let include_title = resolve_title_bar_capture_region(
        source.rect,
        client_rect,
        TitleBarCaptureMode::IncludeTitleBar,
    )?;
    let client_only = resolve_title_bar_capture_region(
        source.rect,
        client_rect,
        TitleBarCaptureMode::ClientOnly,
    )?;
    let probe_report = probe_additional_backends(source.hwnd);

    println!("Stage F smoke");
    println!("source_hwnd={}", source.hwnd);
    println!(
        "window_rect={},{},{},{}",
        source.rect.left, source.rect.top, source.rect.right, source.rect.bottom
    );
    println!(
        "client_rect={},{},{},{}",
        client_rect.left, client_rect.top, client_rect.right, client_rect.bottom
    );
    println!(
        "titlebar_include_region={},{},{},{}",
        include_title.left, include_title.top, include_title.right, include_title.bottom
    );
    println!(
        "titlebar_client_only_region={},{},{},{}",
        client_only.left, client_only.top, client_only.right, client_only.bottom
    );
    for backend in planned_backends() {
        println!(
            "backend kind={:?} frame_runtime={} probe={} limitation={}",
            backend.kind,
            backend.frame_producing_runtime,
            backend.availability_probe,
            backend.limitation.unwrap_or("none")
        );
    }
    for probe in probe_report.probes {
        println!(
            "probe kind={:?} available={} detail={}",
            probe.kind, probe.available, probe.detail
        );
    }
    Ok(())
}

fn stage_g_smoke() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = builtin_effects();
    validate_effect_catalog(&catalog).map_err(|error| format!("{error:?}"))?;
    let chain = default_quality_chain();
    chain
        .validate(&catalog)
        .map_err(|error| format!("{error:?}"))?;

    let paths = RuntimePaths::discover();
    paths.ensure()?;
    let cache_root = std::env::var_os("DODBOGI_STAGE_G_CACHE_ROOT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| paths.root.join("shader-cache").join("stage-g"));
    let first_pass = compile_builtin_effects_with_cache(&cache_root)
        .map_err(|error| format!("first shader compile pass failed: {error:?}"))?;
    let second_pass = compile_builtin_effects_with_cache(&cache_root)
        .map_err(|error| format!("second shader compile pass failed: {error:?}"))?;

    let fixture_root = std::path::PathBuf::from(".omx/evidence/stage-g/visual-fixtures");
    let checker = checkerboard_fixture(64, 64, 8).map_err(|error| format!("{error:?}"))?;
    let edge = high_contrast_edge_fixture(64, 64).map_err(|error| format!("{error:?}"))?;
    let checker_meta = checker.write_ppm(fixture_root.join("checkerboard.ppm"))?;
    let edge_meta = edge.write_ppm(fixture_root.join("high-contrast-edge.ppm"))?;

    let stats = RenderStatistics {
        frame_index: 1,
        capture_width: 320,
        capture_height: 180,
        output_width: 640,
        output_height: 360,
        frame_time_ms: 16.67,
        gpu_time_ms: Some(0.42),
        effect_chain: chain
            .effect_ids
            .iter()
            .map(|id| (*id).to_string())
            .collect(),
    };

    println!("Stage G smoke");
    println!("effect_count={}", catalog.len());
    for effect in &catalog {
        println!(
            "effect id={} category={} display=\"{}\" magpie_category={} clean_room={}",
            effect.id,
            effect.category,
            effect.display_name,
            effect.magpie_equivalent_category.unwrap_or("none"),
            effect.license_note.reusable_without_magpie_gpl
        );
    }
    println!(
        "shader_compiler={} first_pass_programs={} first_pass_cache_hits={} second_pass_programs={} second_pass_cache_hits={}",
        first_pass.compiler,
        first_pass.total_programs(),
        first_pass.cache_hits(),
        second_pass.total_programs(),
        second_pass.cache_hits()
    );
    for report in &second_pass.reports {
        println!(
            "shader effect={} stage={} entry={} target={} bytes={} cache_hit={} path={}",
            report.effect_id,
            report.stage,
            report.entry_point,
            report.target,
            report.byte_len,
            report.cache_hit,
            report.cache_path.display()
        );
    }
    println!(
        "visual_fixture name={} path={} size={}x{} format={}",
        checker_meta.source,
        checker_meta.path.display(),
        checker_meta.width,
        checker_meta.height,
        checker_meta.format
    );
    println!(
        "visual_fixture name={} path={} size={}x{} format={}",
        edge_meta.source,
        edge_meta.path.display(),
        edge_meta.width,
        edge_meta.height,
        edge_meta.format
    );
    for line in stats.overlay_lines() {
        println!("stats_overlay={line}");
    }
    Ok(())
}

fn stage_h_smoke() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(data_dir) = std::env::var_os("DODBOGI_STAGE_H_DATA_DIR") {
        std::env::set_var("DODBOGI_DATA_DIR", data_dir);
    }
    let paths = RuntimePaths::discover();
    paths.ensure()?;

    let mut settings = DodbogiSettings::default();
    settings.diagnostics.enable_stats_overlay = true;

    let mut notepad = AppProfile::per_app_profile("notepad", "Notepad", "notepad.exe");
    notepad.effect_chain = vec!["bilinear".to_string(), "adaptive_sharpen".to_string()];

    let mut terminal =
        AppProfile::per_app_profile("terminal", "Windows Terminal", "WindowsTerminal.exe");
    terminal.scaling_mode = ScalingMode::Fullscreen;
    terminal.monitor_selection = MonitorSelectionMode::Intersected;
    terminal.match_rule.window_class = Some("CASCADIA_HOSTING_WINDOW_CLASS".to_string());
    terminal.match_rule.title_contains = Some("Terminal".to_string());
    terminal.effect_chain = vec!["lanczos3".to_string(), "adaptive_sharpen".to_string()];

    settings.profiles.per_app_profiles.push(notepad);
    settings.profiles.per_app_profiles.push(terminal);

    save_settings_to_path(&settings, &paths.settings_file)?;
    let loaded = load_settings_from_path(&paths.settings_file)?;
    let export_path = paths.config_dir.join("settings-export.toml");
    export_settings_to_path(&loaded, &export_path)?;
    let imported = import_settings_from_path(&export_path)?;

    let default_resolution = imported.resolve_profile(&ProfileMatchContext {
        executable_name: Some("calc.exe".to_string()),
        window_class: None,
        title: None,
    });
    let notepad_resolution =
        imported.resolve_profile(&ProfileMatchContext::for_executable("NOTEPAD.EXE"));
    let terminal_resolution = imported.resolve_profile(&ProfileMatchContext {
        executable_name: Some("WindowsTerminal.exe".to_string()),
        window_class: Some("CASCADIA_HOSTING_WINDOW_CLASS".to_string()),
        title: Some("Terminal".to_string()),
    });
    let coverage = settings_ui_coverage(&imported);
    let diagnostics = DiagnosticsSnapshot::capture(&paths, &imported);

    let mut tray = TrayController::default();
    tray.install_placeholder();
    append_log_line(
        &paths.log_file,
        &format!(
            "stage-h-smoke profiles={} per_app={} coverage={}",
            diagnostics.profile_count,
            diagnostics.per_app_profile_count,
            coverage.all_required_covered()
        ),
    )?;

    println!("Stage H smoke");
    println!("data_root={}", paths.root.display());
    println!("settings_file={}", paths.settings_file.display());
    println!("export_file={}", export_path.display());
    println!("log_file={}", paths.log_file.display());
    println!(
        "profile_count={} per_app_count={}",
        diagnostics.profile_count, diagnostics.per_app_profile_count
    );
    println!(
        "default_resolution id={} source={:?}",
        default_resolution.profile.id, default_resolution.source
    );
    println!(
        "notepad_resolution id={} source={:?} score={}",
        notepad_resolution.profile.id, notepad_resolution.source, notepad_resolution.score
    );
    println!(
        "terminal_resolution id={} source={:?} score={}",
        terminal_resolution.profile.id, terminal_resolution.source, terminal_resolution.score
    );
    println!(
        "coverage_all={} sections={}",
        coverage.all_required_covered(),
        coverage.sections.len()
    );
    for section in &coverage.sections {
        println!(
            "coverage id={} covered={} detail={}",
            section.id, section.covered, section.detail
        );
    }
    println!(
        "packaging distribution={} binary={} arch={} manifest_embedded={} reference_bundled={}",
        imported.packaging.distribution.as_str(),
        imported.packaging.binary_name,
        imported.packaging.target_arch,
        imported.packaging.manifest_embedded,
        imported.packaging.reference_package_bundled
    );
    println!(
        "diagnostics support=\"{}\" cache_dir={} stats_overlay={}",
        diagnostics.support_envelope,
        diagnostics.cache_dir.display(),
        imported.diagnostics.enable_stats_overlay
    );
    println!(
        "tray installed={} menu_count={}",
        tray.is_installed(),
        tray.menu_items().len()
    );
    for item in tray.menu_items() {
        println!(
            "tray_item id={} label=\"{}\" enabled={} checked={}",
            item.id, item.label, item.enabled, item.checked
        );
    }

    if default_resolution.source != ProfileResolutionSource::Default
        || notepad_resolution.source != ProfileResolutionSource::PerApp
        || terminal_resolution.source != ProfileResolutionSource::PerApp
        || !coverage.all_required_covered()
        || !tray.is_installed()
    {
        return Err("stage-h smoke contract failed".into());
    }

    Ok(())
}

#[cfg(test)]
mod g013_tests {
    use super::*;

    #[test]
    fn overlay_pointer_input_is_not_forwarded_to_source() {
        assert!(!should_forward_overlay_input(InputEventKind::MouseMove));
        assert!(!should_forward_overlay_input(
            InputEventKind::MouseButtonDown(MouseButton::Left)
        ));
        assert!(!should_forward_overlay_input(
            InputEventKind::MouseButtonUp(MouseButton::Left)
        ));
        assert!(!should_forward_overlay_input(InputEventKind::DoubleClick(
            MouseButton::Left
        )));
        assert!(!should_forward_overlay_input(InputEventKind::Wheel {
            delta: 120
        }));
        assert!(!should_forward_overlay_input(InputEventKind::ContextMenu));
        assert!(should_forward_overlay_input(InputEventKind::KeyboardFocus));
    }

    #[test]
    fn startup_input_report_documents_cursor_capture_instead_of_click_injection() {
        let report = cursor_capture_input_delivery_report(42, InputDeliveryMode::SendInputAllowed);

        assert_eq!(report.target_hwnd, 42);
        assert_eq!(report.event_kind, "cursor_capture_passthrough");
        assert_eq!(report.source_point, None);
        assert!(!report.delivered);
        assert!(report
            .detail
            .contains("startup SendInput click forwarding is disabled"));
    }

    fn temp_case_dir(case: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "dodbogi-g014-{case}-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn magpie_artifact(dir: &Path, scenario_id: &str) -> PathBuf {
        let artifact = dir.join(format!("{scenario_id}.png"));
        std::fs::write(&artifact, b"magpie-proof").unwrap();
        artifact
    }

    #[test]
    fn concrete_magpie_artifact_must_be_keyed_to_current_scenario() {
        let dir = temp_case_dir("keyed");
        let artifact = magpie_artifact(&dir, "P0-ONE");

        assert!(has_concrete_magpie_scenario_artifact(
            "P0-ONE",
            &format!("magpie_screenshot[P0-ONE]={};", artifact.display())
        ));
        assert!(!has_concrete_magpie_scenario_artifact(
            "P0-ONE",
            &format!("magpie_screenshot[P0-TWO]={};", artifact.display())
        ));
        assert!(!has_concrete_magpie_scenario_artifact(
            "P0-ONE",
            &format!(
                "magpie_screenshot[P0-ONE]={};",
                dir.join("missing.png").display()
            )
        ));
        assert!(!has_concrete_magpie_scenario_artifact(
            "P0-ONE",
            &format!("magpie_screenshot={};", artifact.display())
        ));

        let _ = std::fs::remove_file(artifact);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn concrete_artifact_parser_accepts_quoted_paths_with_spaces() {
        let dir = temp_case_dir("quoted path");
        let artifact = dir.join("P0 ONE.png");
        std::fs::write(&artifact, b"proof").unwrap();

        assert!(has_concrete_magpie_scenario_artifact(
            "P0-ONE",
            &format!("scenario_artifact[P0-ONE]=\"{}\";", artifact.display())
        ));

        let _ = std::fs::remove_file(artifact);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn base_pass_without_explicit_scenario_verdict_is_partial() {
        let dir = temp_case_dir("no-verdict");
        let artifact = magpie_artifact(&dir, "P0-ONE");

        let evidence = format!("magpie_screenshot[P0-ONE]={};", artifact.display());
        let row = row(
            &dir,
            "P0-ONE",
            "P0",
            "launch",
            &evidence,
            ScenarioResult::Pass,
            "runtime artifact bundle",
            "exact",
            "base implementation evidence passed",
        )
        .unwrap();

        assert_eq!(row.dodbogi_result, ScenarioResult::Partial);
        assert!(row
            .verifier_notes
            .contains("explicit scenario pass verdict still pending"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn base_partial_with_keyed_artifacts_and_pass_verdict_promotes_to_pass() {
        let dir = temp_case_dir("promote");
        let artifact = magpie_artifact(&dir, "P0-ONE");

        let evidence = format!(
            "magpie_screenshot[P0-ONE]={}; scenario_verdict[P0-ONE]=pass;",
            artifact.display()
        );
        let row = row(
            &dir,
            "P0-ONE",
            "P0",
            "launch",
            &evidence,
            ScenarioResult::Partial,
            "runtime artifact bundle",
            "exact",
            "manual parity confirmation",
        )
        .unwrap();

        assert_eq!(row.dodbogi_result, ScenarioResult::Pass);
        assert!(row
            .verifier_notes
            .contains("explicit pass verdict are present"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn pass_verdict_without_existing_magpie_file_stays_partial() {
        let dir = temp_case_dir("missing-magpie");

        let evidence = format!(
            "magpie_screenshot[P0-ONE]={}; scenario_verdict[P0-ONE]=pass;",
            dir.join("missing.png").display()
        );
        let row = row(
            &dir,
            "P0-ONE",
            "P0",
            "launch",
            &evidence,
            ScenarioResult::Partial,
            "runtime artifact bundle",
            "exact",
            "manual parity confirmation",
        )
        .unwrap();

        assert_eq!(row.dodbogi_result, ScenarioResult::Partial);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn input_pass_verdict_without_controlled_delivery_stays_partial() {
        let dir = temp_case_dir("input-no-delivery");
        let artifact = magpie_artifact(&dir, "P2-CLICK-001");

        let evidence = format!(
            "scenario_artifact[P2-CLICK-001]={}; scenario_verdict[P2-CLICK-001]=pass;",
            artifact.display()
        );
        let row = row(
            &dir,
            "P2-CLICK-001",
            "P2",
            "input",
            &evidence,
            ScenarioResult::Pass,
            "mapped_input_events=5 controlled_input_delivered=false observed_down=0 observed_up=0",
            "exact",
            "controlled input was unavailable",
        )
        .unwrap();

        assert_eq!(row.dodbogi_result, ScenarioResult::Partial);
        assert!(row
            .verifier_notes
            .contains("Dodbogi runtime evidence required for this feature area"));

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn input_pass_verdict_with_controlled_delivery_can_pass() {
        let dir = temp_case_dir("input-delivery");
        let artifact = magpie_artifact(&dir, "P2-CLICK-001");

        let evidence = format!(
            "scenario_artifact[P2-CLICK-001]={}; scenario_verdict[P2-CLICK-001]=pass;",
            artifact.display()
        );
        let row = row(
            &dir,
            "P2-CLICK-001",
            "P2",
            "input",
            &evidence,
            ScenarioResult::Partial,
            "mapped_input_events=5 controlled_input_delivered=true observed_down=1 observed_up=1",
            "exact",
            "controlled input was observed",
        )
        .unwrap();

        assert_eq!(row.dodbogi_result, ScenarioResult::Pass);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn wrong_scenario_pass_verdict_does_not_promote_partial_row() {
        let dir = temp_case_dir("wrong-verdict");
        let artifact = magpie_artifact(&dir, "P0-ONE");

        let evidence = format!(
            "magpie_screenshot[P0-ONE]={}; scenario_verdict[P0-TWO]=pass;",
            artifact.display()
        );
        let row = row(
            &dir,
            "P0-ONE",
            "P0",
            "launch",
            &evidence,
            ScenarioResult::Partial,
            "runtime artifact bundle",
            "exact",
            "manual parity confirmation",
        )
        .unwrap();

        assert_eq!(row.dodbogi_result, ScenarioResult::Partial);

        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn classification_ready_allows_input_delivery_unavailable_but_requires_mapping() {
        let summary = ScenarioSummary {
            total: 2,
            classified: 2,
            pass: 0,
            partial: 2,
            blocked: 0,
            unsupported: 0,
            fail: 0,
            pending: 0,
        };

        assert!(classification_runtime_ready(
            summary, true, 5, 5, true, true
        ));
        assert!(!classification_runtime_ready(
            summary, true, 4, 5, true, true
        ));
    }
}
