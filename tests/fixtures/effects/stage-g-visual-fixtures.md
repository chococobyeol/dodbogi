# Stage G Visual Fixtures

These fixtures are clean-room test inputs for Dodbogi's independent effect pipeline. They are not copied from Magpie assets or shaders.

## Generated artifacts

- `.omx/evidence/stage-g/visual-fixtures/checkerboard.ppm`
  - 64x64 RGB8 checkerboard for nearest/linear edge behavior.
- `.omx/evidence/stage-g/visual-fixtures/high-contrast-edge.ppm`
  - 64x64 RGB8 high-contrast split/stripe pattern for bicubic, Lanczos, and sharpening checks.

## Usage

Run:

```powershell
$env:DODBOGI_STAGE_G_CACHE_ROOT = (Resolve-Path '.omx/evidence/stage-g/shader-cache-smoke').Path
cargo run -p dodbogi-app -- --stage-g-smoke
```

The smoke command regenerates the fixtures and prints the shader compile/cache and diagnostic overlay evidence.
