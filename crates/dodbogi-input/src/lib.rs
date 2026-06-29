//! Input and cursor mapping boundary.
//!
//! The input crate owns deterministic overlay-to-source coordinate conversion
//! and user-input normalization. Platform-specific message injection remains in
//! the Win32 boundary so this crate can be tested without sending real input.

use dodbogi_core::PhysicalRect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourcePoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleTransform {
    pub source_x: f32,
    pub source_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
}

impl ScaleTransform {
    pub fn overlay_to_source(&self, overlay_x: f32, overlay_y: f32) -> (f32, f32) {
        (
            self.source_x + overlay_x / self.scale_x,
            self.source_y + overlay_y / self.scale_y,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InputTransform {
    pub source: PhysicalRect,
    pub destination: PhysicalRect,
    pub scale_x: f32,
    pub scale_y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMappingError {
    EmptySourceRect,
    EmptyDestinationRect,
    InvalidScale,
}

impl InputTransform {
    pub fn from_rects(
        source: PhysicalRect,
        destination: PhysicalRect,
    ) -> Result<Self, InputMappingError> {
        if source.is_empty() {
            return Err(InputMappingError::EmptySourceRect);
        }
        if destination.is_empty() {
            return Err(InputMappingError::EmptyDestinationRect);
        }

        let scale_x = destination.width() as f32 / source.width() as f32;
        let scale_y = destination.height() as f32 / source.height() as f32;
        if !scale_x.is_finite() || !scale_y.is_finite() || scale_x <= 0.0 || scale_y <= 0.0 {
            return Err(InputMappingError::InvalidScale);
        }

        Ok(Self {
            source,
            destination,
            scale_x,
            scale_y,
        })
    }

    pub fn contains_overlay_point(&self, point: OverlayPoint) -> bool {
        point.x >= self.destination.left as f32
            && point.x < self.destination.right as f32
            && point.y >= self.destination.top as f32
            && point.y < self.destination.bottom as f32
    }

    pub fn overlay_to_source_point(&self, point: OverlayPoint) -> Option<SourcePoint> {
        if !self.contains_overlay_point(point) {
            return None;
        }
        Some(SourcePoint {
            x: self.source.left as f32 + (point.x - self.destination.left as f32) / self.scale_x,
            y: self.source.top as f32 + (point.y - self.destination.top as f32) / self.scale_y,
        })
    }

    pub fn overlay_to_source_pixel(&self, point: OverlayPoint) -> Option<(i32, i32)> {
        if !self.contains_overlay_point(point) {
            return None;
        }

        Some((
            map_half_open_pixel_axis(
                point.x,
                self.destination.left,
                self.destination.right,
                self.source.left,
                self.source.right,
            ),
            map_half_open_pixel_axis(
                point.y,
                self.destination.top,
                self.destination.bottom,
                self.source.top,
                self.source.bottom,
            ),
        ))
    }

    pub fn source_to_overlay_point(&self, point: SourcePoint) -> OverlayPoint {
        OverlayPoint {
            x: self.destination.left as f32 + (point.x - self.source.left as f32) * self.scale_x,
            y: self.destination.top as f32 + (point.y - self.source.top as f32) * self.scale_y,
        }
    }

    pub fn source_to_overlay_pixel(&self, point: SourcePoint) -> (i32, i32) {
        (
            map_half_open_pixel_axis(
                point.x,
                self.source.left,
                self.source.right,
                self.destination.left,
                self.destination.right,
            ),
            map_half_open_pixel_axis(
                point.y,
                self.source.top,
                self.source.bottom,
                self.destination.top,
                self.destination.bottom,
            ),
        )
    }

    pub fn map_event(&self, event: OverlayInputEvent) -> Option<SourceInputEvent> {
        let point = match event.point {
            Some(point) => Some(self.overlay_to_source_point(point)?),
            None => None,
        };
        Some(SourceInputEvent {
            kind: event.kind,
            point,
        })
    }
}

fn map_half_open_pixel_axis(
    value: f32,
    from_start: i32,
    from_end: i32,
    to_start: i32,
    to_end: i32,
) -> i32 {
    let from_last = from_end.saturating_sub(1).max(from_start);
    let to_last = to_end.saturating_sub(1).max(to_start);
    let from_span = from_last - from_start;
    let to_span = to_last - to_start;
    if from_span <= 0 || to_span <= 0 {
        return to_start;
    }

    let clamped = (value as f64).clamp(from_start as f64, from_last as f64);
    let pos = (clamped - from_start as f64) / from_span as f64;
    (to_start as f64 + pos * to_span as f64).round() as i32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragPhase {
    Start,
    Move,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextSelectionPhase {
    Start,
    Update,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEventKind {
    MouseMove,
    MouseButtonDown(MouseButton),
    MouseButtonUp(MouseButton),
    DoubleClick(MouseButton),
    Wheel {
        delta: i32,
    },
    Drag {
        button: MouseButton,
        phase: DragPhase,
    },
    TextSelection {
        phase: TextSelectionPhase,
    },
    ContextMenu,
    KeyboardFocus,
    KeyDown {
        virtual_key: u16,
    },
    KeyUp {
        virtual_key: u16,
    },
    TextInput {
        ch: char,
    },
    Touch {
        id: u32,
        phase: TouchPhase,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayInputEvent {
    pub kind: InputEventKind,
    pub point: Option<OverlayPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourceInputEvent {
    pub kind: InputEventKind,
    pub point: Option<SourcePoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorSpeedMode {
    Normal,
    PendingReferenceObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchSupport {
    Mapped,
    PendingReferenceObservation,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorRenderPolicy {
    pub visible: bool,
    pub draw_in_overlay: bool,
    pub scale: f32,
    pub speed_mode: CursorSpeedMode,
    pub touch_support: TouchSupport,
}

impl CursorRenderPolicy {
    pub fn from_transform(transform: &InputTransform) -> Self {
        Self {
            visible: true,
            draw_in_overlay: true,
            scale: (transform.scale_x + transform.scale_y) / 2.0,
            speed_mode: CursorSpeedMode::PendingReferenceObservation,
            touch_support: TouchSupport::Mapped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_to_source_inverts_scale() {
        let transform = ScaleTransform {
            source_x: 10.0,
            source_y: 20.0,
            scale_x: 2.0,
            scale_y: 2.0,
        };
        assert_eq!(transform.overlay_to_source(40.0, 60.0), (30.0, 50.0));
    }

    #[test]
    fn input_transform_maps_overlay_center_to_source_center() {
        let transform = fixture_transform();
        let mapped = transform
            .overlay_to_source_point(OverlayPoint { x: 400.0, y: 200.0 })
            .unwrap();
        assert_eq!(mapped, SourcePoint { x: 300.0, y: 200.0 });
    }

    #[test]
    fn input_transform_rejects_points_outside_overlay() {
        let transform = fixture_transform();
        assert!(transform
            .overlay_to_source_point(OverlayPoint { x: 900.0, y: 200.0 })
            .is_none());
    }

    #[test]
    fn maps_click_double_click_wheel_context_and_text_selection_events() {
        let transform = fixture_transform();
        let point = Some(OverlayPoint { x: 100.0, y: 50.0 });
        let cases = [
            InputEventKind::MouseButtonDown(MouseButton::Left),
            InputEventKind::MouseButtonUp(MouseButton::Left),
            InputEventKind::DoubleClick(MouseButton::Left),
            InputEventKind::Wheel { delta: 120 },
            InputEventKind::ContextMenu,
            InputEventKind::TextSelection {
                phase: TextSelectionPhase::Start,
            },
            InputEventKind::TextSelection {
                phase: TextSelectionPhase::Update,
            },
            InputEventKind::TextSelection {
                phase: TextSelectionPhase::End,
            },
        ];

        for kind in cases {
            let mapped = transform
                .map_event(OverlayInputEvent { kind, point })
                .unwrap();
            assert_eq!(mapped.kind, kind);
            assert_eq!(mapped.point.unwrap(), SourcePoint { x: 150.0, y: 125.0 });
        }
    }

    #[test]
    fn maps_drag_sequence_to_source_points() {
        let transform = fixture_transform();
        let sequence = [
            (
                DragPhase::Start,
                OverlayPoint { x: 0.0, y: 0.0 },
                SourcePoint { x: 100.0, y: 100.0 },
            ),
            (
                DragPhase::Move,
                OverlayPoint { x: 200.0, y: 100.0 },
                SourcePoint { x: 200.0, y: 150.0 },
            ),
            (
                DragPhase::End,
                OverlayPoint { x: 799.0, y: 399.0 },
                SourcePoint { x: 499.5, y: 299.5 },
            ),
        ];

        for (phase, point, expected) in sequence {
            let mapped = transform
                .map_event(OverlayInputEvent {
                    kind: InputEventKind::Drag {
                        button: MouseButton::Left,
                        phase,
                    },
                    point: Some(point),
                })
                .unwrap();
            assert_eq!(mapped.point.unwrap(), expected);
        }
    }

    #[test]
    fn keyboard_focus_and_text_input_do_not_require_pointer_coordinates() {
        let transform = fixture_transform();
        let focus = transform
            .map_event(OverlayInputEvent {
                kind: InputEventKind::KeyboardFocus,
                point: None,
            })
            .unwrap();
        let text = transform
            .map_event(OverlayInputEvent {
                kind: InputEventKind::TextInput { ch: 'A' },
                point: None,
            })
            .unwrap();

        assert_eq!(focus.point, None);
        assert_eq!(text.kind, InputEventKind::TextInput { ch: 'A' });
    }

    #[test]
    fn cursor_policy_scales_with_destination() {
        let transform = fixture_transform();
        let cursor = CursorRenderPolicy::from_transform(&transform);
        assert!(cursor.visible);
        assert!(cursor.draw_in_overlay);
        assert_eq!(cursor.scale, 2.0);
        assert_eq!(
            cursor.speed_mode,
            CursorSpeedMode::PendingReferenceObservation
        );
        assert_eq!(cursor.touch_support, TouchSupport::Mapped);
    }

    #[test]
    fn input_transform_uses_half_open_overlay_bounds() {
        let transform = fixture_transform();
        assert!(transform
            .overlay_to_source_point(OverlayPoint { x: 799.0, y: 399.0 })
            .is_some());
        assert!(transform
            .overlay_to_source_point(OverlayPoint { x: 800.0, y: 399.0 })
            .is_none());
        assert!(transform
            .overlay_to_source_point(OverlayPoint { x: 799.0, y: 400.0 })
            .is_none());
    }

    #[test]
    fn cursor_pixel_mapping_keeps_last_pixels_inside_half_open_rects() {
        let transform = fixture_transform();

        assert_eq!(
            transform.overlay_to_source_pixel(OverlayPoint { x: 799.0, y: 399.0 }),
            Some((499, 299))
        );
        assert_eq!(
            transform.source_to_overlay_pixel(SourcePoint { x: 499.0, y: 299.0 }),
            (799, 399)
        );
        assert_eq!(
            transform.overlay_to_source_pixel(OverlayPoint { x: 800.0, y: 400.0 }),
            None
        );
    }

    fn fixture_transform() -> InputTransform {
        InputTransform::from_rects(
            PhysicalRect {
                left: 100,
                top: 100,
                right: 500,
                bottom: 300,
            },
            PhysicalRect {
                left: 0,
                top: 0,
                right: 800,
                bottom: 400,
            },
        )
        .unwrap()
    }
}
