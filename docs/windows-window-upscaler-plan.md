# Windows Window Upscaler Plan

## Goal

Build a Windows-only desktop utility that behaves like Magpie from the user's point of view: it makes another program appear larger by capturing the source window, scaling the captured image, and displaying the result in a non-activating overlay window.

The target is behavioral parity with Magpie, not code parity. Magpie is the reference for feature behavior, edge cases, architecture, and Windows API choices. The implementation should be independently written and should not copy Magpie source code, shaders, comments, or file structure.

## Product Scope

The long-term product scope is Magpie-like behavior on Windows. The first usable milestone can be smaller, but the roadmap and tests should be organized around reaching Magpie parity.

Primary use cases:

- Make small legacy applications easier to read.
- Enlarge browser, document, utility, or productivity windows.
- Provide simple scaling ratios such as `1.5x`, `2x`, and `3x`.
- Keep interaction feeling like the original window is still being used.

Out of scope for the product:

- Frame generation.
- FSR 2/3 style temporal upscaling.
- macOS/Linux support.
- Direct reuse of Magpie GPL source code or shaders.
- Anti-cheat bypass or behavior designed to evade game protections.

Long-term, most Magpie features are technically implementable on Windows. The constraint is not feasibility; it is implementation time, polish, and edge-case coverage. The first version should validate the core interaction, then add Magpie-compatible behavior in phases.

## Parity Definition

"Same behavior as Magpie" means the implementation should match Magpie's user-visible behavior for the same input scenario and settings.

Behavioral parity includes:

- Same scaling modes at the product level: fullscreen scaling and windowed scaling.
- Same style of source-window selection through global shortcuts and foreground-window detection.
- Same practical behavior around focus: the scaled view should not feel like a separate active app.
- Same practical behavior around source movement, minimize, close, resize, and focus loss.
- Same capture-method categories over time: Windows Graphics Capture, Desktop Duplication, GDI, and DWM shared surface where practical.
- Same broad cursor behavior: cursor visibility, coordinate mapping, scaling, and optional cursor speed adjustment.
- Same profile concept: default profile, per-app profiles, scaling mode, capture method, scaling flags, and hotkeys.
- Same multi-monitor intent: closest monitor, intersected monitors, and all monitors.
- Same overlay/tooling concept: toolbar, screenshot/debug/profiling support where useful.
- Same ability to add scaling effects, but independently implemented shaders and effect runtime.

Behavioral parity does not mean:

- Copying Magpie source.
- Matching internal class names or file layout.
- Matching every implementation detail.
- Reusing Magpie's HLSL effect files.

## Core Idea

The original program is not actually resized or modified. Instead:

1. Select a source window.
2. Capture that window continuously.
3. Scale the captured frame.
4. Draw the scaled result into a separate borderless overlay window.
5. Keep the overlay aligned with the source window.
6. Keep the visual overlay input-transparent, move the real OS cursor into source-window coordinates while scaling is active, and draw the visible cursor at the scaled coordinates.
7. Keep keyboard focus on the source window where possible.

The user should perceive this as "the original program became larger", even though the visible large image is an overlay.

## Recommended MVP

### MVP Features

- Pick the current foreground window with a global hotkey.
- Start and stop scaling with the same hotkey.
- Capture source window using Windows Graphics Capture.
- Display scaled output in a borderless always-on-top overlay.
- Support fixed scale factors: `1.5x`, `2x`, `3x`.
- Use one simple scaler first: bilinear or bicubic.
- Keep overlay above the source window without stealing focus, and make the render surface layered + mouse-transparent so real pointer events can still reach the source.
- Track source window movement and stop or realign when it moves.
- Basic Magpie-style mouse interaction: layered transparent overlay, cursor capture into source coordinates, visible cursor redraw at scaled coordinates, and temporary cursor-speed compensation while captured.
- Basic keyboard focus forwarding by returning focus to source.
- Simple tray/menu or minimal control window.

### MVP Non-Goals

- Multiple capture backends.
- HLSL effect compatibility with Magpie.
- Exotic cursor-theme / animated-cursor edge cases and per-app cursor-speed policy beyond Magpie-style default compensation. Hotspot-aware cursor redraw, cursor-shape refresh, duplicate-cursor prevention, and source-window interaction parity are not optional for the usable MVP.
- Windowed resizing handles.
- Multi-monitor edge cases beyond basic support.
- DPI-perfect handling for every app type.
- UWP/Chromium/WPF special cases beyond best effort.

## Architecture

```text
App
  -> HotkeyService
  -> WindowPicker
  -> ScalingSession
       -> SourceWindowTracker
       -> CaptureBackend
       -> RenderBackend
       -> OverlayWindow
       -> InputMapper
       -> CursorController
  -> SettingsStore
```

## Components

### HotkeyService

Responsibilities:

- Register a global hotkey.
- Start scaling the foreground window.
- Stop the active session.

Initial API choices:

- `RegisterHotKey`
- Optional later fallback: `WH_KEYBOARD_LL`

Recommended MVP:

- Use only `RegisterHotKey` first.
- Add low-level keyboard hook only if hotkey conflicts become a real problem.

### WindowPicker

Responsibilities:

- Get foreground window.
- Reject invalid windows.
- Reject this app's own windows.
- Return source `HWND`.

Initial API choices:

- `GetForegroundWindow`
- `IsWindow`
- `IsWindowVisible`
- `GetWindowLongPtr`
- `GetWindowThreadProcessId`

### SourceWindowTracker

Responsibilities:

- Track source window rectangle.
- Track client/content rectangle.
- Detect minimize, close, move, resize, focus change.
- Handle DPI-aware coordinate conversion.

Initial API choices:

- `GetWindowRect`
- `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)`
- `GetClientRect`
- `ClientToScreen`
- `GetDpiForWindow`
- `IsIconic`
- `IsWindowVisible`

MVP policy:

- Start with client-area capture.
- Stop scaling if source window size changes.
- Realign overlay if source window only moves.
- Do not treat transient move/resize modal-loop states as fatal runtime errors. While the foreground window is in a Win32 move/size loop, defer cursor capture, but keep overlay/layout tracking live so the scaled view follows the source during the drag.
- Add live resize support later.

### 2026-06-29 Windowed Move/Resize Correction

Manual parity testing showed that deferring all layout decisions during
`GUI_INMOVESIZE` makes the scaled view freeze during title-bar dragging and
jump only after the drag ends. Magpie does not behave that way: it keeps window
and renderer rectangles updated through the move/size message path.

Updated policy:

- Cursor capture may still defer during a Win32 move/size modal loop to avoid
  cursor warps.
- Overlay/layout tracking must not defer during a move/size modal loop; it must
  keep following the source rectangle while the user drags.
- Source geometry should prefer `DWMWA_EXTENDED_FRAME_BOUNDS` when valid,
  because `GetWindowRect` can include invisible resize borders that become
  visible scaled margins.
- Rendering must sample the WGC frame `ContentSize`, not the full backing
  surface, because the backing surface can contain unused black right/bottom
  regions.
- Overlay repositioning/resizing should use `SWP_NOCOPYBITS` to avoid copied
  border/client trails during DWM move/resize.

Implementation/evidence: `.omx/evidence/g015/magpie-windowed-pipeline-root-cause-fix.md`.

### CaptureBackend

Responsibilities:

- Capture frames from the source window.
- Provide frame texture to renderer.

Recommended MVP:

- Implement only Windows Graphics Capture.

Initial API choices:

- `GraphicsCaptureItem`
- `IGraphicsCaptureItemInterop::CreateForWindow`
- `Direct3D11CaptureFramePool`
- `GraphicsCaptureSession`

Later optional backends:

- Desktop Duplication for monitor capture.
- GDI fallback for old apps.

### RenderBackend

Responsibilities:

- Scale captured frames.
- Present to overlay window.

Recommended MVP:

- Use Direct3D 11.
- Use one or two simple HLSL compute or pixel shaders.
- Start with bilinear/bicubic.

Possible rendering paths:

- Simple D3D11 swap chain for the overlay window.
- DirectComposition later if flicker or resize issues appear.

Shader plan:

- Do not reuse Magpie HLSL files.
- Write independent minimal HLSL shaders.
- Add Lanczos only after the pipeline is stable.

### OverlayWindow

Responsibilities:

- Show the scaled frame.
- Sit above the source window.
- Avoid stealing focus.
- Hide from taskbar if needed.

Initial API choices:

- `CreateWindowEx`
- `WS_POPUP`
- `WS_EX_NOACTIVATE`
- `WS_EX_TOOLWINDOW`
- `WS_EX_TOPMOST`
- `SetWindowPos`

MVP behavior:

- Borderless overlay.
- Cover the intended destination rectangle.
- Topmost while source is focused.
- Hide or stop when source loses focus, depending on selected product behavior.

### InputMapper

Responsibilities:

- Convert between scaled-overlay coordinates and source coordinates.
- Provide deterministic mapping data to the cursor controller and controlled probes.
- Avoid product-runtime click-time `SetCursorPos` + `SendInput` forwarding for pointer events.

MVP approach:

- Keep keyboard focus on source window.
- For mouse, make the visual overlay input-transparent with the layered-window + transparent-window style combination and let the cursor controller keep the real OS cursor in source coordinates while drawing a separate cursor at the scaled overlay position.
- Disable Windows Graphics Capture cursor inclusion for the scaling session and hide the real system cursor with a global cursor-visibility API (`ShowSystemCursor` when available, Magnification `MagShowSystemCursor` fallback, then `ShowCursor` only as last resort). Relying only on `ShowCursor` is not sufficient because it is a process/thread display-count API and can leave the real cursor visible over the source window while the app also draws a scaled cursor.
- While captured, temporarily reduce the Windows cursor speed by the average destination/source scale and restore the original speed on release/stop. This is required so the visible scaled cursor does not move faster than the unscaled desktop.
- Draw the overlay cursor at `cursor_position - cursor_hotspot`, not at the raw cursor point. When the real cursor is moved into source coordinates, keep it clipped briefly at the target point, then force a source `WM_SETCURSOR` refresh so hover/titlebar/client-area cursor shapes do not remain stale.
- Runtime polling should log and continue through transient layout, cursor, or frame-capture errors where the session can still recover. A single WGC/cursor/readback failure during source move or resize must not terminate the process unless the source is actually invalid, closed, minimized, or intentionally stopped.
- Use `SendInput` only for controlled diagnostics or non-pointer paths, not as the product pointer-delivery mechanism.

Important caveat:

- Input parity is one of the hardest parts. Click/wheel/drag should work because the real OS cursor is already in the source window; text selection and complex gestures still need scenario testing.

### CursorController

Responsibilities:

- Keep cursor behavior visually consistent.
- Keep visible cursor movement speed consistent between the scaled surface and the normal desktop.

MVP options:

- Option A: Let system cursor remain visible. Simpler, but it breaks Magpie-like interaction when the scaled destination is larger than the source.
- Option B: Hide system cursor while captured and draw a separate cursor at the scaled position. More natural and closer to Magpie.

Recommendation:

- Use Option B for product-runtime interaction parity. A click-time restore hack is rejected because it still lets clicks warp the visible cursor. Option B must use layered passthrough, not `HTTRANSPARENT` alone, because source apps are normally different top-level windows/threads. It must also adjust and restore Windows cursor speed during capture; otherwise the drawn cursor moves too fast by the zoom scale. It must also hide the real cursor globally and keep WGC from embedding the source cursor in captured frames; otherwise users can see both the original cursor and the separately drawn scaled cursor. The drawn cursor must honor the source cursor hotspot and repaint on shape changes; after reliable cursor repositioning the source must receive a `WM_SETCURSOR`-style refresh so the cursor shape tracks the underlying control.

### SettingsStore

Responsibilities:

- Save scale factor.
- Save hotkey.
- Save preferred scaler.

MVP:

- JSON file in `%LOCALAPPDATA%`.

Later:

- Per-app profiles.
- Auto-scale rules by executable path and class name.

## Implementation Phases

### Phase 0: Prototype

Goal:

- Prove the basic visual loop.

Tasks:

- Create a Windows desktop app.
- Pick foreground window.
- Capture it with Windows Graphics Capture.
- Show frames in a separate overlay window.
- Scale with nearest or bilinear.

Success criteria:

- A normal app window can be enlarged and viewed with acceptable latency.

### Phase 1: Usable MVP

Goal:

- Make interaction usable enough for daily testing.

Tasks:

- Add global hotkey.
- Add clean start/stop lifecycle.
- Track source movement.
- Add `1.5x`, `2x`, `3x`.
- Add cursor capture/pass-through interaction so clicks happen on the source without click-time pointer injection.
- Use layered overlay passthrough so hover/click/drag events are delivered by Windows to the source rather than swallowed by the overlay.
- Add Magpie-style cursor speed compensation while capture is active.
- Keep keyboard focus on source.
- Add basic error messages.

Success criteria:

- User can scale a small normal app and click buttons accurately.

### Phase 2: Natural Window Behavior

Goal:

- Reduce the feeling that the overlay is a separate window.

Tasks:

- Improve Z-order handling.
- Handle focus changes.
- Handle source minimize/close.
- Handle multi-monitor basics.
- Improve DPI handling.
- Add optional custom cursor drawing.
- Add wheel and drag support.

Success criteria:

- Overlay follows the source reliably and feels natural in common apps.

### Phase 3: Quality Scaling

Goal:

- Improve image quality.

Tasks:

- Add bicubic.
- Add Lanczos.
- Add sharpening pass.
- Add presets.
- Add GPU timing/FPS overlay for debugging only.

Success criteria:

- Text and UI scaling quality is visibly better than naive scaling.

### Phase 4: App Profiles

Goal:

- Make repeat use convenient.

Tasks:

- Add per-executable settings.
- Add auto-scale for selected apps.
- Add profile import/export.

Success criteria:

- User can configure a program once and reuse the same behavior later.

## Key Technical Risks

### Input Mapping

Click-time forwarding with `SetCursorPos` + `SendInput` is easy to demo but causes visible cursor warps and diverges from Magpie. Drag, hover, context menus, text selection, and app-specific capture behavior still expose edge cases.

Mitigation:

- Implement cursor capture/pass-through first so pointer events naturally target the source.
- Use scenario tests for click, drag, wheel, and text selection.
- Include explicit regression checks for source-window move/drag, scaled-window move/drag, hover cursor-shape changes, titlebar resize cursors, and no duplicate cursor.
- Avoid pretending every app works until tested.

### DPI and Coordinates

Windows has per-monitor DPI, DPI virtualization, frame bounds, and client bounds. These can disagree.

Mitigation:

- Make the process per-monitor DPI aware.
- Store all internal coordinates in physical pixels.
- Build coordinate conversion tests with mixed-DPI monitors if available.

### Focus and Z-Order

The overlay must be visible but should not behave like the active app.

Mitigation:

- Use `WS_EX_NOACTIVATE`.
- Re-focus source window after overlay interactions.
- Keep topmost logic conservative.

### Capture Compatibility

Windows Graphics Capture is the best MVP path, but some windows may not capture correctly.

Mitigation:

- Start with WGC only.
- Add GDI fallback only after the main pipeline is stable.

### GPL Boundary

Magpie is GPLv3. Direct code reuse creates licensing obligations.

Mitigation:

- Treat Magpie as architectural research.
- Write independent code.
- Use Windows official docs for API implementation details.
- Do not copy Magpie source, shaders, or comments.

## Technology Choices

Recommended for Windows-only MVP:

- Language: Rust.
- Windowing: raw Win32.
- Capture: Windows Graphics Capture.
- Rendering: Direct3D 11 or `wgpu`.
- Shader language: HLSL, independently written.
- Settings: JSON.

Pragmatic recommendation:

- Use Rust for the application architecture, state management, settings, hotkeys, session lifecycle, and safety around async/threaded code.
- Use the `windows` crate for Win32, Windows Graphics Capture, WinRT, DXGI, and D3D interop.
- For rendering, start with either D3D11 through `windows` crate bindings or `wgpu`.
- If the goal is closest behavior to Magpie, D3D11 is more direct.
- If the goal is easier future portability and cleaner Rust ergonomics, `wgpu` is attractive, but capture texture interop may need more investigation.

Recommended first technical direction:

- Rust + `windows` crate.
- Windows Graphics Capture.
- D3D11 renderer first.
- Independent minimal HLSL shaders.

This keeps the hard Windows integration close to the native APIs while avoiding a C++ codebase.

## Feature Parity Direction

The project goal is Magpie-like coverage over time. The implementation should still be built in layers so each layer can be tested against Magpie before moving on. A practical parity ladder is:

1. P0: Core scaling session parity

   - Foreground-window selection.
   - Windows Graphics Capture.
   - Overlay display.
   - Fixed scale factors.
   - Basic cursor capture/pass-through input mapping.
   - Start/stop behavior comparable to Magpie's normal shortcut workflow.

2. P1: Window behavior parity

   - Source movement tracking.
   - Z-order and focus management.
   - DPI handling.
   - Multi-monitor basics.
   - Source minimize/close handling.
   - Fullscreen scaling and windowed scaling behavior.

3. P2: Interaction parity

   - Click, drag, wheel.
   - Cursor scaling/redraw.
   - Per-app/edge-case cursor speed policy beyond the default Magpie-style capture compensation.
   - Popup/menu handling where possible.
   - Toolbar shortcut behavior.

4. P3: Capture compatibility parity

   - GDI fallback.
   - Desktop Duplication fallback.
   - DWM shared surface fallback if still useful.
   - Capture title bar option.
   - App-specific capture profiles.

5. P4: Rendering/effect parity

   - Bilinear.
   - Bicubic.
   - Lanczos.
   - Sharpening.
   - Later: selected advanced algorithms rewritten independently.
   - Independent effect-chain runtime inspired by MagpieFX behavior.

6. P5: Product polish parity

   - Tray icon.
   - Profiles.
   - Auto-scale rules.
   - Import/export settings.
   - Diagnostics and logs.
   - Screenshots and overlay diagnostics.

Each category should have a parity checklist and should be tested against Magpie on the same machine, app, monitor setup, DPI setup, and selected settings.

## Parity Test Strategy

Testing should treat Magpie as the behavioral oracle. For each feature, run the same scenario in Magpie and in this project, then compare user-visible behavior.

### Test Levels

1. Reference behavior notes

   - Run Magpie manually.
   - Record what happens for the scenario.
   - Capture screenshots or short screen recordings where useful.
   - Convert observations into acceptance criteria.

2. Automated unit tests

   - Coordinate conversion.
   - Scale-factor math.
   - Window rectangle mapping.
   - Settings serialization.
   - Profile matching rules.

3. Integration tests with test windows

   - Use small controlled Win32 test apps.
   - Validate move, resize, minimize, focus loss, popup, and different border styles.
   - Validate click mapping by having the test app report received coordinates.

4. Visual comparison tests

   - Capture output screenshots.
   - Compare dimensions, alignment, black bars, and scaling placement.
   - Pixel-perfect equality is not required unless the same algorithm is intentionally implemented.

5. Real-app parity tests

   - Notepad.
   - Windows Terminal.
   - Chrome or Edge.
   - Electron app.
   - WPF app.
   - UWP/WinUI app.
   - A simple windowed game or rendering demo.

### Required Test Matrix

Every major feature should be tested across:

- Scaling mode: fullscreen, windowed.
- Capture method: WGC first; later Desktop Duplication, GDI, DWM shared surface.
- DPI: 100%, 125%, 150%; later mixed-DPI multi-monitor.
- Monitor setup: single monitor, two monitors, source partially offscreen.
- Window state: normal, moved, resized, minimized, restored, maximized if allowed.
- Focus state: source focused, popup focused, unrelated foreground window.
- Input: click, drag, wheel, keyboard.

### Parity Acceptance Rule

A feature is accepted only when:

- The intended scenario behaves the same as Magpie from the user's point of view.
- Known differences are documented.
- Any deliberate difference has a product reason, not just implementation convenience.
- Regression tests exist for coordinate math and lifecycle behavior.

### Test Artifacts

Create and maintain:

- `tests/reference/magpie-behavior.md`: observed Magpie behavior.
- `tests/manual/parity-checklist.md`: manual test checklist.
- `tests/fixtures/window_cases/`: small test windows for move, resize, popup, and input mapping.
- `tests/fixtures/screenshots/`: representative visual outputs.

## Feature Decisions Needed

Please decide these before implementation starts.

1. Target apps

   - Normal desktop apps only.
   - Include browsers/Electron apps.
   - Include old games/windowed games.

2. First interaction target

   - View-only magnifier.
   - Click support.
   - Click + drag + wheel support.

3. Overlay behavior

   - Fullscreen-like overlay.
   - Windowed enlarged overlay.
   - Replace source visually by covering it exactly.

4. Scaling quality

   - Fast/simple: bilinear first.
   - Better text/UI: bicubic or Lanczos early.
   - Shader presets later.

5. Cursor handling

   - Keep system cursor.
   - Hide and redraw custom cursor.

6. Source window behavior

   - Stop when source moves/resizes.
   - Follow when source moves, stop on resize.
   - Follow both move and resize.

7. Distribution goal

   - Personal tool.
   - Open-source app.
   - Closed-source/private app.

8. License tolerance

   - Must avoid GPL-derived implementation.
   - Magpie may be studied as a behavioral and architectural reference only.

9. Extra features beyond Magpie

   - Decide after the MVP proves that capture, overlay, and input mapping feel good.
   - New features should be added as independent modules, not mixed into the capture/render core.

## Proposed First Cut

Unless requirements say otherwise, build this first:

- Windows only.
- Rust + `windows` crate.
- D3D11 renderer through Windows bindings.
- Windows Graphics Capture only.
- Foreground-window hotkey selection.
- Borderless no-activate overlay.
- Fixed `2x` scaling first.
- Bilinear shader first.
- Stop on source resize.
- Follow source move.
- Cursor capture/pass-through for mouse interaction.
- Hide the system cursor while captured and draw the visible cursor separately.
- Use layered + transparent overlay passthrough rather than relying on top-level `HTTRANSPARENT`.
- Temporarily compensate Windows cursor speed during capture and restore it on release/stop.
- Hotspot-aware cursor overlay drawing, source `WM_SETCURSOR` refresh after reliable repositioning, and non-fatal recovery from transient source move/resize errors.
- No Magpie code reuse.

This is small enough to validate the product feel before investing in advanced edge cases.

## 2026-06-29 Parity Correction: Windowed Move and Cursor Edge Handling

User-observed regression:

- During source title-bar drag, horizontal movement followed but vertical movement was clamped.
- Cursor could disappear around source move/size and bottom/right destination edges.
- Runtime logs showed `move_size_active=true`, changing `source_rect.top`, but a fixed/clamped `destination.top`, plus transient `ClipCursor transition failed` errors.

Decision and rationale:

- Match Magpie's windowed move behavior: when the source rect only moves and does not resize, translate the existing scaling destination by the same source delta instead of recomputing the initial windowed layout and re-clamping it to the work area every frame.
- Keep full layout recomputation for real source size changes.
- Treat foreground move/size as a special cursor state: show the system cursor, hide the cursor overlay, and restore mouse speed while the source is being dragged.
- Clamp cursor overlay/restore coordinates to `destination.right - 1` and `destination.bottom - 1`, matching Magpie's edge handling.
- Treat `ClipCursor` transition denial as recoverable by falling back to `SetCursorPos`; only fail if the fallback also fails.

Implementation references:

- `crates/dodbogi-core/src/lib.rs`: `translate_destination_for_source_move`.
- `crates/dodbogi-app/src/main.rs`: `layout_policy=translate_source_move`, scaler output-size tracking, deferred scaler resize while `move_size_active`.
- `crates/dodbogi-win32/src/lib.rs`: cursor move/size handling, destination-edge clamping, `SetCursorPos` fallback.
- Evidence: `.omx/evidence/g016/cursor-and-vertical-source-move-fix.md`.

Validation:

- `cargo check --workspace`: passed.
- `cargo test --workspace`: passed.
- `cargo build --release -p dodbogi-app`: passed.
- Controlled Notepad source-move smoke: passed with move-preserving destination translation; current policy label is `layout_policy=preserve_source_move`.
- Controlled Notepad cursor-capture smoke: passed with `captured=true` and overlay/source positions recorded.

## 2026-06-29 Parity Correction: Windowed Resize Anchor Handling

User-observed regression:

- Dragging a scaled corner/edge to resize made the enlarged window jump to a new position.
- Cursor capture flickered at the resize edge.
- Runtime logs showed tiny source size deltas switching to `layout_policy=recompute_layout`, e.g. a 1px height change moved `destination.top` from `72` to `188`.
- Runtime logs also showed `CreateSwapChainForHwnd failed: 0x80070005` after resize.

Decision and rationale:

- Live source resize is not an initial placement event. It must preserve the previous windowed scaling relationship.
- For windowed source changes:
  - move-only: translate the destination;
  - right/bottom resize: keep left/top anchored;
  - left/top resize: keep right/bottom anchored;
  - ambiguous resize+move: keep center relation.
- Do not run work-area fit/clamp during live resize, because that causes visible position jumps.
- Hold cursor capture steady while the foreground source has mouse capture, so boundary jitter does not repeatedly release and recapture the cursor.
- Drop the old scaler/swapchain before creating the replacement swapchain for the same overlay HWND; otherwise flip-model swapchain creation can fail with `0x80070005`.

Implementation references:

- `crates/dodbogi-core/src/lib.rs`: `preserve_windowed_destination_for_source_change`.
- `crates/dodbogi-app/src/main.rs`: `layout_policy=preserve_source_resize`, settled scaler recreation, `Option<WgcEffectScaler>` drop-before-create lifecycle, `--g016-source-resize-smoke`.
- `crates/dodbogi-win32/src/lib.rs`: foreground capture guard for cursor stability, `resize_window_for_probe`.
- Evidence: `.omx/evidence/g017/source-resize-anchor-and-swapchain-fix.md`.

Validation:

- `cargo check --workspace`: passed.
- `cargo test --workspace`: passed, 57 tests.
- `cargo build --release -p dodbogi-app`: passed.
- Controlled Notepad resize smoke: passed with `layout_policy=preserve_source_resize`, followed by `layout_policy=deferred_scaler_resize recreated_scaler=true`.
- Controlled Notepad move smoke: passed with `layout_policy=preserve_source_move`.
- Controlled Notepad cursor smoke: passed with `captured=true`.

## 2026-06-29 Parity Correction: Cursor Edge Half-Open Mapping

User-observed regression:

- Cursor still flickered when hovering at the scaled window corner/edge.
- Logs showed repeated `captured=false` / `captured=true` near source/destination right and bottom edges.

Decision and rationale:

- Treat all source/destination cursor hit-tests as Win32 half-open rectangles: `left <= x < right`, `top <= y < bottom`.
- Do not map cursor pixels using simple scale + `round()`, because the last destination pixel can round to `source.right`, which is outside the source rect.
- Use Magpie-style endpoint pixel mapping for cursor capture:
  - `dest.right - 1` maps to `src.right - 1`;
  - `src.right - 1` maps to `dest.right - 1`.

Implementation references:

- `crates/dodbogi-input/src/lib.rs`: half-open overlay containment, `overlay_to_source_pixel`, `source_to_overlay_pixel`.
- `crates/dodbogi-win32/src/lib.rs`: cursor capture uses endpoint pixel mapping for real cursor repositioning and overlay cursor drawing.
- `crates/dodbogi-app/src/main.rs`: `--g018-cursor-edge-smoke`.
- Evidence: `.omx/evidence/g018/cursor-edge-half-open-mapping-fix.md`.

Validation:

- `cargo check --workspace`: passed.
- `cargo test --workspace`: passed, 59 tests.
- `cargo build --release -p dodbogi-app`: passed.
- Controlled Notepad cursor-edge smoke: passed with `edge_point=(1457, 1079)`, `first_captured=true`, and `second=None` (no release/flicker on the next update).
- Controlled Notepad resize smoke still passed.

## 2026-06-29 Parity Correction: Resize Cursor Overlay and Cursor-Speed Guard

Problem observed during manual Magpie-parity testing:

- When resizing the source window by dragging a scaled edge/corner, the visible cursor jumped back to the original unscaled source-window border.
- After the runtime was no longer running, Windows mouse speed could remain at the temporary low value used for scaled cursor capture.

Decision:

- Treat native foreground move/size as an active captured-cursor state, not as a reason to reveal the hardware cursor. The hardware cursor remains in source-window coordinates for Windows to keep native resize/move behavior working, while Dodbogi draws the visible cursor at the mapped scaled-overlay coordinate.
- Restore temporary cursor speed during foreground move/size, and add both a console shutdown handler and a persistent cache guard file so abnormal exits do not leave the adjusted `SPI_SETMOUSESPEED` value as the user's active mouse speed.

Rationale:

- Magpie's user-visible model is that the scaled window owns the visible cursor while source input continues underneath. Showing the hardware cursor during native resize exposes the implementation detail and makes it look like the cursor teleported to the original window.
- Cursor speed adjustment is global Windows state. Any implementation that changes it must have more than ordinary Rust `Drop` cleanup because console close/interrupt paths can bypass normal session teardown.

Validation evidence:

- Evidence file: `.omx/evidence/g019/cursor-resize-speed-guard-fix.md`
- `cargo check --workspace` passed.
- `cargo test --workspace` passed: 59 tests.
- `--g019-cursor-speed-guard-smoke` proved `10 -> 1 -> 10` recovery.
- Direct Windows readback after smoke: `mouse_speed_current=10`.
- Existing `--g018-cursor-edge-smoke` and `--g016-source-resize-smoke` still passed.

## 2026-06-29 Parity Correction: Live Resize Swapchain and Drag Speed

This supersedes the earlier resize-speed note that restored cursor speed during foreground move/size.

User-observed regressions:

- During corner/edge resize, the visible scaled edge/cursor moved faster than normal pointer movement.
- During resize, text in the scaled window could become horizontally or vertically stretched.

Decisions:

- Keep temporary cursor-speed compensation active during native foreground move/size. In Dodbogi's current cursor architecture, restoring speed during resize makes the source resize delta unscaled while the visible overlay delta remains scaled, so the visible drag speed jumps by the windowed scale factor.
- Resize the DXGI swapchain output immediately when the overlay destination size changes. Do not resize the overlay HWND and then defer scaler output resizing for the whole move/size loop.
- Use `IDXGISwapChain::ResizeBuffers`, after unbinding/flushing the D3D11 render target, and recreate only the render target view for live output-size changes. Keep full scaler recreation as a fallback, not the normal resize path.
- Skip presenting if scaler output size and overlay destination size are mismatched, so Dodbogi does not keep presenting an old-size backbuffer into a new-size overlay.

Implementation references:

- `crates/dodbogi-win32/src/lib.rs`: native move/size branch keeps `adjust_cursor_speed` active while drawing the overlay cursor.
- `crates/dodbogi-render-d3d11/src/lib.rs`: `WgcEffectScaler::resize_output` uses `ResizeBuffers` and recreates the RTV.
- `crates/dodbogi-app/src/main.rs`: layout refresh logs `resized_scaler=true` and no longer defers ordinary output resizing during live resize.
- Evidence: `.omx/evidence/g020/live-resize-swapchain-speed-fix.md`.

Validation:

- `cargo check`: passed.
- `cargo test`: passed.
- `cargo build --release`: passed.
- Release `--g016-source-resize-smoke`: passed with `resized_scaler=true`, `scaler_resize_deferred=false`, `settle_event=None`, `frame_presented=1`.
- Release `--g019-cursor-speed-guard-smoke`: passed with mouse speed restored to `10`.
- Root `dodbogi.exe` and `Dodbogi-v0.1.0-x64\dodbogi.exe` were refreshed from the release build.


## 2026-06-29 Parity Correction: Wide Live Resize Right-Side Clipping

This narrows the earlier G017 rule. Dodbogi still must not blindly re-run initial placement/clamping on every live resize frame, but if the Magpie-like preserved destination would leave the selected monitor work area, letting it stay off-screen produces visible right/bottom clipping.

User-observed regression:

- Stretching the source window horizontally could cut off the right side of the scaled output.

Decisions:

- Keep the existing live resize anchor policy while the destination fits.
- If a preserved live-resize destination exceeds the selected work area, uniformly reduce the live scale and translate the overlay back inside the work area. This avoids the previous distortion problem because width and height use the same fitted scale.
- Recreate the WGC frame pool when a captured frame reports `ContentSize` larger than the current frame-pool dimensions, then skip the stale frame and present the next correctly-sized capture frame.

Implementation references:

- `crates/dodbogi-core/src/lib.rs`: `windowed_work_area_for_source`, `preserve_windowed_destination_for_source_change_in_work_area`, and right-clip regression tests.
- `crates/dodbogi-app/src/main.rs`: windowed resize uses `layout_policy=preserve_source_resize_fit` when work-area fitting is required.
- `crates/dodbogi-render-d3d11/src/lib.rs`: WGC frame-pool growth path calls `Direct3D11CaptureFramePool::Recreate`.
- Evidence: `.omx/evidence/g021/wide-resize-right-clip-fix.md`.

Validation:

- `cargo fmt --all`: passed.
- `cargo check`: passed.
- `cargo test`: passed, 61 tests.
- `cargo build --release`: passed.
- Release `--g016-source-resize-smoke`: passed with `resized_scaler=true`, `scaler_resize_deferred=false`, `settle_event=None`, `frame_presented=1`.
- Release `--g019-cursor-speed-guard-smoke`: passed with mouse speed restored to `10`.
- Root `dodbogi.exe` and `Dodbogi-v0.1.0-x64\dodbogi.exe` were refreshed and smoke-tested.
