# Magpie Behavior Reference

This document records user-visible Magpie behavior that this project should match with an independent implementation.

Use this as the behavioral oracle. Do not copy Magpie source code or shaders.

## Reference Environment

Stage A initial reference data was recorded on 2026-06-28 from the runnable package and local machine evidence. Full scenario observations remain pending until each matrix row is run in Magpie first.

- Magpie version/commit: v0.12.1 x64 reference package
- Reference executable: `D:\Projects\dodbogi\Magpie-v0.12.1-x64\Magpie.exe`
- Magpie.exe SHA256: `ADDF4A3E4EC1595B968E3FD8E8ECF338F5C8A57E5B858539676DCECFC0C8C28D`
- Windows version: Microsoft Windows 10 Pro 10.0.19045 build 19045
- GPU: NVIDIA GeForce GTX 1660 SUPER, driver 32.0.15.6094
- DirectX: DirectX 12; DxDiag reported feature levels include 12_1, 12_0, 11_1, and 11_0
- Monitor setup: one primary display, 1920x1080, 32 bpp, 59 Hz
- DPI setup: not yet manually varied; Stage A recorded current display topology only
- Capture method: pending per-scenario observation
- Scaling mode: pending per-scenario observation
- Source app: pending per-scenario observation
- Evidence files: `.omx/evidence/stage-a/reference-package-hashes.json`, `.omx/evidence/stage-a/environment.json`, `.omx/evidence/stage-a/dxdiag.txt`, `.omx/evidence/stage-a/magpie-smoke-run.json`

## Initial Smoke Observation

- Smoke launch: attempted with `Magpie.exe` from the reference package.
- Runtime artifact: `Magpie-v0.12.1-x64\logs\magpie.log` was updated during the smoke run.
- Observation limit: this smoke run proves the package is runnable enough to initialize and write runtime logs, but it is not a substitute for the scenario matrix. Each parity scenario below must still be observed interactively in Magpie before this project can claim a match.
- Clean-room note: behavior notes in this file must be written from runtime observation/artifacts and public/official docs, not copied from Magpie source wording.

## Behavior Categories

### Start And Stop

- Starting from foreground window:
- Starting while another scaling session is active:
- Stopping with the same shortcut:
- Stopping when source closes:
- Stopping when source is minimized:
- Behavior when source is invalid:

### Fullscreen Scaling

- Destination monitor selection:
- Source window movement before scaling:
- Z-order:
- Focus:
- Alt+Tab behavior:
- Popup/menu behavior:

### Windowed Scaling

- Initial overlay size:
- Window frame behavior:
- Moving source window:
- Moving scaled window:
- Resizing scaled window:
- Source resize behavior:
- Caption/border behavior:

### Capture Methods

- Windows Graphics Capture:
- Desktop Duplication:
- GDI:
- DWM shared surface:
- Capture title bar option:

### Cursor And Input

- Cursor visibility:
- Cursor scale:
- Click mapping:
- Drag mapping:
- Wheel mapping:
- Text selection:
- Context menu:
- Cursor speed adjustment:

### DPI And Multi-Monitor

- Same-DPI single monitor:
- Mixed-DPI monitors:
- Moving source between monitors:
- Source partly offscreen:
- Intersected monitor mode:
- All monitor mode:

### Profiles And Settings

- Default profile behavior:
- Per-app profile matching:
- Auto-scale behavior:
- Hotkey behavior:
- Scaling mode selection:
- Settings persistence:

### Effects And Rendering

- Scale placement:
- Black bars/fit/fill behavior:
- Bilinear/Bicubic/Lanczos behavior:
- Effect chain behavior:
- Screenshot behavior:
- FPS/diagnostic overlay behavior:

## Known Deliberate Differences

Document any difference from Magpie here. A difference should have a product reason.

- None yet.
