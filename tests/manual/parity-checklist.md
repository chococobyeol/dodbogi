# Manual Parity Checklist

Run each scenario in Magpie first, record the expected behavior in `tests/reference/magpie-behavior.md`, then run this project and compare.

## P0 Core Scaling Session

- [ ] Start scaling the foreground Notepad window.
- [ ] Stop scaling with the same hotkey.
- [ ] Start scaling the foreground Windows Terminal window.
- [ ] Overlay appears above the source without stealing focus.
- [ ] 2x scaling produces the expected destination size.
- [ ] Output remains aligned with the source content.
- [ ] Source close stops scaling.
- [ ] Source minimize stops or hides scaling like Magpie.

## P1 Window Behavior

- [ ] Fullscreen scaling matches Magpie behavior.
- [ ] Windowed scaling matches Magpie behavior.
- [ ] Moving the source window behaves like Magpie.
- [ ] Resizing the source window behaves like Magpie.
- [ ] Moving the scaled window behaves like Magpie.
- [ ] Resizing the scaled window behaves like Magpie.
- [ ] Dragging or moving any involved window does not terminate the program unless Magpie also stops in the same scenario.
- [ ] During source/scaled-window move-size operations, scaling either follows/defer-updates like Magpie or stops only for the same user-visible reason Magpie does.
- [ ] Focus loss behaves like Magpie.
- [ ] Popup menu behavior matches Magpie.
- [ ] Alt+Tab/taskbar behavior matches Magpie.

## P2 Interaction

- [ ] Click mapping is accurate.
- [ ] Double-click mapping is accurate.
- [ ] Drag mapping is accurate.
- [ ] Wheel mapping is accurate.
- [ ] Text selection works in a text editor.
- [ ] Context menus open at expected positions.
- [ ] Keyboard input remains directed to the source.
- [ ] Cursor display behavior matches Magpie.
- [ ] Only one cursor is visible while captured: WGC cursor capture is not duplicated, the real system cursor is hidden, and the drawn cursor tracks the scaled position.
- [ ] Cursor shape changes on hover/titlebar/text/client areas match Magpie and do not leave stale shapes after click, drag, or movement.
- [ ] Cursor hotspot is correct: arrow tip, I-beam center, resize handles, and hand pointer visually hit the same point as Magpie.
- [ ] Cursor speed is consistent when entering/leaving the scaled area and while dragging.

## P3 Capture Methods

- [ ] Windows Graphics Capture behavior matches Magpie.
- [ ] Desktop Duplication behavior matches Magpie.
- [ ] GDI behavior matches Magpie.
- [ ] DWM shared surface behavior matches Magpie, if implemented.
- [ ] Capture title bar option matches Magpie.

## P4 DPI And Multi-Monitor

- [ ] 100% DPI single monitor.
- [ ] 125% DPI single monitor.
- [ ] 150% DPI single monitor.
- [ ] Mixed-DPI two-monitor setup.
- [ ] Source partly offscreen.
- [ ] Closest monitor mode.
- [ ] Intersected monitors mode.
- [ ] All monitors mode.

## P5 Profiles And Polish

- [ ] Default profile behavior matches Magpie.
- [ ] Per-app profile matching matches Magpie.
- [ ] Auto-scale behavior matches Magpie.
- [ ] Hotkey settings match Magpie-style behavior.
- [ ] Tray/menu behavior matches Magpie-style behavior.
- [ ] Screenshot behavior matches Magpie-style behavior.
- [ ] Diagnostics/log behavior is sufficient for debugging parity regressions.

## P6 Effects And Rendering

- [ ] Built-in scaling effects exposed by Magpie v0.12.1 have independently implemented equivalents or approved clean-room/legal differences.
- [ ] Effect parameter behavior matches Magpie-style behavior.
- [ ] Baseline nearest/linear rendering behavior matches Magpie where applicable.
- [ ] Debug/statistics overlay behavior matches Magpie where user-visible.
- [ ] Visual output differences are recorded as pass, bug, legal-difference, or unsupported-with-evidence.

## Acceptance Notes

A checklist item passes only if the result is the same from the user's point of view. If behavior differs, document whether it is a bug, missing feature, or deliberate product difference.
