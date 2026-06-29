//! Independent effect pipeline boundary.
//!
//! This crate owns clean-room effect descriptors, HLSL source text, shader cache
//! keys, visual fixtures, screenshot metadata, and diagnostic overlay formatting.
//! Do not copy Magpie HLSL. Every built-in effect below is independently written
//! for this project and carries an explicit license note.

use std::{
    collections::HashSet,
    error::Error,
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

const CLEAN_ROOM_LICENSE: &str = "independent clean-room implementation; no Magpie source, shader, comment, asset, or file-structure reuse";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectCategory {
    Nearest,
    Bilinear,
    Bicubic,
    Lanczos,
    Sharpen,
    Diagnostic,
}

impl fmt::Display for EffectCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nearest => f.write_str("nearest"),
            Self::Bilinear => f.write_str("bilinear"),
            Self::Bicubic => f.write_str("bicubic"),
            Self::Lanczos => f.write_str("lanczos"),
            Self::Sharpen => f.write_str("sharpen"),
            Self::Diagnostic => f.write_str("diagnostic"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Vertex,
    Pixel,
}

impl ShaderStage {
    pub fn default_target(self) -> &'static str {
        match self {
            Self::Vertex => "vs_5_0",
            Self::Pixel => "ps_5_0",
        }
    }
}

impl fmt::Display for ShaderStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Vertex => f.write_str("vertex"),
            Self::Pixel => f.write_str("pixel"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectParameter {
    pub name: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

impl EffectParameter {
    pub fn validate(&self) -> Result<(), EffectError> {
        if self.min > self.max || self.default < self.min || self.default > self.max {
            return Err(EffectError::InvalidParameter {
                name: self.name.to_string(),
                min: self.min,
                max: self.max,
                default: self.default,
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectLicenseNote {
    pub origin: &'static str,
    pub license: &'static str,
    pub reusable_without_magpie_gpl: bool,
    pub note: &'static str,
}

impl EffectLicenseNote {
    pub fn clean_room(note: &'static str) -> Self {
        Self {
            origin: "dodbogi-clean-room",
            license: CLEAN_ROOM_LICENSE,
            reusable_without_magpie_gpl: true,
            note,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlslProgram {
    pub stage: ShaderStage,
    pub entry_point: &'static str,
    pub target: &'static str,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EffectDescriptor {
    pub id: &'static str,
    pub display_name: &'static str,
    pub category: EffectCategory,
    pub magpie_equivalent_category: Option<&'static str>,
    pub description: &'static str,
    pub parameters: Vec<EffectParameter>,
    pub programs: Vec<HlslProgram>,
    pub license_note: EffectLicenseNote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectChain {
    pub id: &'static str,
    pub display_name: &'static str,
    pub effect_ids: Vec<&'static str>,
}

impl EffectChain {
    pub fn validate(&self, catalog: &[EffectDescriptor]) -> Result<(), EffectError> {
        if self.effect_ids.is_empty() {
            return Err(EffectError::EmptyChain(self.id.to_string()));
        }
        let available: HashSet<&str> = catalog.iter().map(|effect| effect.id).collect();
        for id in &self.effect_ids {
            if !available.contains(*id) {
                return Err(EffectError::UnknownEffect((*id).to_string()));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EffectError {
    DuplicateEffectId(String),
    EmptyChain(String),
    InvalidFixture {
        name: String,
        width: u32,
        height: u32,
        cell: u32,
    },
    InvalidParameter {
        name: String,
        min: f32,
        max: f32,
        default: f32,
    },
    MissingProgram(String),
    MissingCleanRoomLicense(String),
    UnknownEffect(String),
}

impl fmt::Display for EffectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateEffectId(id) => write!(f, "duplicate effect id: {id}"),
            Self::EmptyChain(id) => write!(f, "effect chain is empty: {id}"),
            Self::InvalidFixture {
                name,
                width,
                height,
                cell,
            } => write!(
                f,
                "invalid fixture {name}: width={width} height={height} cell={cell}"
            ),
            Self::InvalidParameter {
                name,
                min,
                max,
                default,
            } => write!(
                f,
                "invalid parameter {name}: min={min} max={max} default={default}"
            ),
            Self::MissingProgram(id) => write!(f, "effect has no HLSL programs: {id}"),
            Self::MissingCleanRoomLicense(id) => {
                write!(f, "effect lacks a reusable clean-room license note: {id}")
            }
            Self::UnknownEffect(id) => write!(f, "unknown effect id in chain: {id}"),
        }
    }
}

impl Error for EffectError {}

const FULLSCREEN_TRIANGLE_VS: &str = r#"
struct VsInput {
    float2 position : POSITION;
    float2 uv : TEXCOORD0;
};

struct VsOutput {
    float4 position : SV_POSITION;
    float2 uv : TEXCOORD0;
};

VsOutput vs_main(VsInput input) {
    VsOutput output;
    output.position = float4(input.position.xy, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}
"#;

const NEAREST_PS: &str = r#"
Texture2D sourceTexture : register(t0);
SamplerState pointSampler : register(s0);

float4 ps_main(float4 position : SV_POSITION, float2 uv : TEXCOORD0) : SV_TARGET {
    return sourceTexture.Sample(pointSampler, uv);
}
"#;

const BILINEAR_PS: &str = r#"
Texture2D sourceTexture : register(t0);
SamplerState linearSampler : register(s0);

float4 ps_main(float4 position : SV_POSITION, float2 uv : TEXCOORD0) : SV_TARGET {
    return sourceTexture.Sample(linearSampler, uv);
}
"#;

const BICUBIC_PS: &str = r#"
Texture2D sourceTexture : register(t0);
SamplerState linearSampler : register(s0);

cbuffer ScaleInfo : register(b0) {
    float2 sourceSize;
    float2 inverseSourceSize;
    float2 outputSize;
    float sharpenStrength;
};

float cubic_weight(float x) {
    x = abs(x);
    if (x <= 1.0) {
        return (1.5 * x - 2.5) * x * x + 1.0;
    }
    if (x < 2.0) {
        return ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0;
    }
    return 0.0;
}

float4 ps_main(float4 position : SV_POSITION, float2 uv : TEXCOORD0) : SV_TARGET {
    float2 texel = uv * sourceSize - 0.5;
    float2 base = floor(texel);
    float2 fraction = texel - base;
    float4 accum = 0.0;
    float totalWeight = 0.0;

    [unroll]
    for (int y = -1; y <= 2; ++y) {
        float wy = cubic_weight((float)y - fraction.y);
        [unroll]
        for (int x = -1; x <= 2; ++x) {
            float wx = cubic_weight((float)x - fraction.x);
            float weight = wx * wy;
            float2 sampleUv = (base + float2((float)x, (float)y) + 0.5) * inverseSourceSize;
            accum += sourceTexture.SampleLevel(linearSampler, sampleUv, 0.0) * weight;
            totalWeight += weight;
        }
    }

    return accum / max(totalWeight, 0.00001);
}
"#;

const LANCZOS_PS: &str = r#"
Texture2D sourceTexture : register(t0);
SamplerState linearSampler : register(s0);

cbuffer ScaleInfo : register(b0) {
    float2 sourceSize;
    float2 inverseSourceSize;
    float2 outputSize;
    float sharpenStrength;
};

static const float PI_VALUE = 3.14159265358979323846;

float sinc_weight(float x) {
    x = abs(x);
    if (x < 0.0001) {
        return 1.0;
    }
    float pix = PI_VALUE * x;
    return sin(pix) / pix;
}

float lanczos_weight(float x) {
    x = abs(x);
    if (x >= 3.0) {
        return 0.0;
    }
    return sinc_weight(x) * sinc_weight(x / 3.0);
}

float4 ps_main(float4 position : SV_POSITION, float2 uv : TEXCOORD0) : SV_TARGET {
    float2 texel = uv * sourceSize - 0.5;
    float2 base = floor(texel);
    float2 fraction = texel - base;
    float4 accum = 0.0;
    float totalWeight = 0.0;

    [unroll]
    for (int y = -2; y <= 3; ++y) {
        float wy = lanczos_weight((float)y - fraction.y);
        [unroll]
        for (int x = -2; x <= 3; ++x) {
            float wx = lanczos_weight((float)x - fraction.x);
            float weight = wx * wy;
            float2 sampleUv = (base + float2((float)x, (float)y) + 0.5) * inverseSourceSize;
            accum += sourceTexture.SampleLevel(linearSampler, sampleUv, 0.0) * weight;
            totalWeight += weight;
        }
    }

    return saturate(accum / max(totalWeight, 0.00001));
}
"#;

const SHARPEN_PS: &str = r#"
Texture2D sourceTexture : register(t0);
SamplerState linearSampler : register(s0);

cbuffer ScaleInfo : register(b0) {
    float2 sourceSize;
    float2 inverseSourceSize;
    float2 outputSize;
    float sharpenStrength;
};

float4 ps_main(float4 position : SV_POSITION, float2 uv : TEXCOORD0) : SV_TARGET {
    float2 stepUv = inverseSourceSize;
    float4 center = sourceTexture.Sample(linearSampler, uv);
    float4 blur = 0.0;
    blur += sourceTexture.Sample(linearSampler, uv + float2(-stepUv.x, 0.0));
    blur += sourceTexture.Sample(linearSampler, uv + float2(stepUv.x, 0.0));
    blur += sourceTexture.Sample(linearSampler, uv + float2(0.0, -stepUv.y));
    blur += sourceTexture.Sample(linearSampler, uv + float2(0.0, stepUv.y));
    blur *= 0.25;
    return saturate(center + (center - blur) * sharpenStrength);
}
"#;

fn vertex_program() -> HlslProgram {
    HlslProgram {
        stage: ShaderStage::Vertex,
        entry_point: "vs_main",
        target: ShaderStage::Vertex.default_target(),
        source: FULLSCREEN_TRIANGLE_VS,
    }
}

fn pixel_program(source: &'static str) -> HlslProgram {
    HlslProgram {
        stage: ShaderStage::Pixel,
        entry_point: "ps_main",
        target: ShaderStage::Pixel.default_target(),
        source,
    }
}

pub fn builtin_effects() -> Vec<EffectDescriptor> {
    vec![
        EffectDescriptor {
            id: "nearest",
            display_name: "Nearest / point scaler",
            category: EffectCategory::Nearest,
            magpie_equivalent_category: Some("nearest or pixel-preserving scaler category"),
            description: "Point-sampled scaling for pixel-art or exact texel stepping.",
            parameters: vec![],
            programs: vec![vertex_program(), pixel_program(NEAREST_PS)],
            license_note: EffectLicenseNote::clean_room(
                "Uses a standard point sampler and contains no reference shader code.",
            ),
        },
        EffectDescriptor {
            id: "bilinear",
            display_name: "Bilinear scaler",
            category: EffectCategory::Bilinear,
            magpie_equivalent_category: Some("linear scaler category"),
            description: "Linear texture filtering for the default fast scaler path.",
            parameters: vec![],
            programs: vec![vertex_program(), pixel_program(BILINEAR_PS)],
            license_note: EffectLicenseNote::clean_room(
                "Uses Direct3D linear sampling; independently authored source.",
            ),
        },
        EffectDescriptor {
            id: "bicubic_catmull_rom",
            display_name: "Bicubic Catmull-Rom scaler",
            category: EffectCategory::Bicubic,
            magpie_equivalent_category: Some("bicubic scaler category"),
            description: "A compact Catmull-Rom bicubic scaler for UI/text quality comparisons.",
            parameters: vec![],
            programs: vec![vertex_program(), pixel_program(BICUBIC_PS)],
            license_note: EffectLicenseNote::clean_room(
                "Formula implemented from standard interpolation math, not from Magpie shaders.",
            ),
        },
        EffectDescriptor {
            id: "lanczos3",
            display_name: "Lanczos3 scaler",
            category: EffectCategory::Lanczos,
            magpie_equivalent_category: Some("Lanczos scaler category"),
            description: "Windowed-sinc Lanczos3 sampling for sharper high-quality scaling tests.",
            parameters: vec![],
            programs: vec![vertex_program(), pixel_program(LANCZOS_PS)],
            license_note: EffectLicenseNote::clean_room(
                "Windowed-sinc implementation is independently authored from public math.",
            ),
        },
        EffectDescriptor {
            id: "adaptive_sharpen",
            display_name: "Simple adaptive sharpen",
            category: EffectCategory::Sharpen,
            magpie_equivalent_category: Some("sharpening/post-process category"),
            description: "A small unsharp-mask style post-process used after scaling when enabled.",
            parameters: vec![EffectParameter {
                name: "strength",
                label: "Strength",
                min: 0.0,
                max: 1.0,
                default: 0.25,
            }],
            programs: vec![vertex_program(), pixel_program(SHARPEN_PS)],
            license_note: EffectLicenseNote::clean_room(
                "Simple four-neighbor sharpen pass; no copied shader source.",
            ),
        },
    ]
}

pub fn baseline_effects() -> Vec<EffectDescriptor> {
    builtin_effects()
}

pub fn default_quality_chain() -> EffectChain {
    EffectChain {
        id: "default_quality",
        display_name: "Default quality chain",
        effect_ids: vec!["bilinear"],
    }
}

pub fn validate_effect_catalog(catalog: &[EffectDescriptor]) -> Result<(), EffectError> {
    let mut ids = HashSet::new();
    for effect in catalog {
        if !ids.insert(effect.id) {
            return Err(EffectError::DuplicateEffectId(effect.id.to_string()));
        }
        if effect.programs.is_empty() {
            return Err(EffectError::MissingProgram(effect.id.to_string()));
        }
        for parameter in &effect.parameters {
            parameter.validate()?;
        }
        if !effect.license_note.reusable_without_magpie_gpl {
            return Err(EffectError::MissingCleanRoomLicense(effect.id.to_string()));
        }
    }
    Ok(())
}

pub fn hlsl_source_hash(program: &HlslProgram) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in program
        .entry_point
        .as_bytes()
        .iter()
        .chain(program.target.as_bytes())
        .chain(program.source.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderCacheKey {
    pub effect_id: String,
    pub stage: ShaderStage,
    pub entry_point: String,
    pub target: String,
    pub source_hash: u64,
}

impl ShaderCacheKey {
    pub fn for_program(effect_id: &str, program: &HlslProgram) -> Self {
        Self {
            effect_id: effect_id.to_string(),
            stage: program.stage,
            entry_point: program.entry_point.to_string(),
            target: program.target.to_string(),
            source_hash: hlsl_source_hash(program),
        }
    }

    pub fn file_name(&self) -> String {
        format!(
            "{}-{}-{}-{:016x}.cso",
            sanitize_cache_component(&self.effect_id),
            self.stage,
            sanitize_cache_component(&self.target),
            self.source_hash
        )
    }
}

fn sanitize_cache_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderCacheRecord {
    pub key: ShaderCacheKey,
    pub path: PathBuf,
    pub byte_len: usize,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShaderCache {
    root: PathBuf,
}

impl ShaderCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path_for_key(&self, key: &ShaderCacheKey) -> PathBuf {
        self.root.join(key.file_name())
    }

    pub fn load(&self, key: &ShaderCacheKey) -> io::Result<Option<Vec<u8>>> {
        let path = self.path_for_key(key);
        if !path.exists() {
            return Ok(None);
        }
        fs::read(path).map(Some)
    }

    pub fn store(
        &self,
        key: ShaderCacheKey,
        bytes: &[u8],
        cache_hit: bool,
    ) -> io::Result<ShaderCacheRecord> {
        fs::create_dir_all(&self.root)?;
        let path = self.path_for_key(&key);
        fs::write(&path, bytes)?;
        Ok(ShaderCacheRecord {
            key,
            path,
            byte_len: bytes.len(),
            cache_hit,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenshotFormat {
    PpmRgb8,
}

impl fmt::Display for ScreenshotFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PpmRgb8 => f.write_str("ppm-rgb8"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotMetadata {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub format: ScreenshotFormat,
    pub source: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualFixture {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub pixels_rgb: Vec<[u8; 3]>,
}

impl VisualFixture {
    pub fn write_ppm(&self, path: impl AsRef<Path>) -> io::Result<ScreenshotMetadata> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::File::create(path)?;
        writeln!(file, "P6")?;
        writeln!(file, "# dodbogi stage-g visual fixture: {}", self.name)?;
        writeln!(file, "{} {}", self.width, self.height)?;
        writeln!(file, "255")?;
        for pixel in &self.pixels_rgb {
            file.write_all(pixel)?;
        }
        Ok(ScreenshotMetadata {
            path: path.to_path_buf(),
            width: self.width,
            height: self.height,
            format: ScreenshotFormat::PpmRgb8,
            source: self.name,
        })
    }
}

pub fn checkerboard_fixture(
    width: u32,
    height: u32,
    cell: u32,
) -> Result<VisualFixture, EffectError> {
    if width == 0 || height == 0 || cell == 0 {
        return Err(EffectError::InvalidFixture {
            name: "checkerboard".to_string(),
            width,
            height,
            cell,
        });
    }
    let mut pixels = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let light = ((x / cell) + (y / cell)).is_multiple_of(2);
            pixels.push(if light { [236, 236, 236] } else { [36, 36, 36] });
        }
    }
    Ok(VisualFixture {
        name: "checkerboard",
        width,
        height,
        pixels_rgb: pixels,
    })
}

pub fn high_contrast_edge_fixture(width: u32, height: u32) -> Result<VisualFixture, EffectError> {
    if width == 0 || height == 0 {
        return Err(EffectError::InvalidFixture {
            name: "high_contrast_edge".to_string(),
            width,
            height,
            cell: 1,
        });
    }
    let mut pixels = Vec::with_capacity((width * height) as usize);
    let split = width / 2;
    for y in 0..height {
        for x in 0..width {
            let stripe = y % 8 < 4;
            let left = x < split;
            let pixel = match (left, stripe) {
                (true, true) => [255, 255, 255],
                (true, false) => [0, 0, 0],
                (false, true) => [255, 80, 40],
                (false, false) => [20, 80, 255],
            };
            pixels.push(pixel);
        }
    }
    Ok(VisualFixture {
        name: "high_contrast_edge",
        width,
        height,
        pixels_rgb: pixels,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderStatistics {
    pub frame_index: u64,
    pub capture_width: u32,
    pub capture_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub frame_time_ms: f32,
    pub gpu_time_ms: Option<f32>,
    pub effect_chain: Vec<String>,
}

impl RenderStatistics {
    pub fn overlay_lines(&self) -> Vec<String> {
        let gpu = self
            .gpu_time_ms
            .map(|value| format!("{value:.2} ms"))
            .unwrap_or_else(|| "not measured".to_string());
        vec![
            format!("frame {}", self.frame_index),
            format!(
                "capture {}x{} -> output {}x{}",
                self.capture_width, self.capture_height, self.output_width, self.output_height
            ),
            format!("frame {:.2} ms, gpu {gpu}", self.frame_time_ms),
            format!("effects {}", self.effect_chain.join(" -> ")),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn builtin_catalog_covers_required_quality_categories() {
        let catalog = builtin_effects();
        validate_effect_catalog(&catalog).expect("catalog should be valid");
        let categories: HashSet<EffectCategory> =
            catalog.iter().map(|effect| effect.category).collect();
        assert!(categories.contains(&EffectCategory::Nearest));
        assert!(categories.contains(&EffectCategory::Bilinear));
        assert!(categories.contains(&EffectCategory::Bicubic));
        assert!(categories.contains(&EffectCategory::Lanczos));
        assert!(categories.contains(&EffectCategory::Sharpen));
        assert!(catalog
            .iter()
            .all(|effect| effect.license_note.reusable_without_magpie_gpl));
    }

    #[test]
    fn default_quality_chain_references_catalog_entries() {
        let catalog = builtin_effects();
        default_quality_chain()
            .validate(&catalog)
            .expect("default chain should validate");
    }

    #[test]
    fn cache_key_changes_when_shader_source_changes() {
        let program_a = HlslProgram {
            stage: ShaderStage::Pixel,
            entry_point: "ps_main",
            target: "ps_5_0",
            source: "float4 ps_main() : SV_TARGET { return 1; }",
        };
        let program_b = HlslProgram {
            source: "float4 ps_main() : SV_TARGET { return 0; }",
            ..program_a.clone()
        };
        assert_ne!(hlsl_source_hash(&program_a), hlsl_source_hash(&program_b));
    }

    #[test]
    fn shader_cache_roundtrips_bytes() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("dodbogi-effect-cache-test-{unique}"));
        let cache = ShaderCache::new(&root);
        let program = builtin_effects()[0].programs[0].clone();
        let key = ShaderCacheKey::for_program("nearest", &program);
        assert!(cache.load(&key).expect("cache read should work").is_none());
        let record = cache
            .store(key.clone(), &[1, 2, 3, 4], false)
            .expect("cache write should work");
        assert_eq!(record.byte_len, 4);
        assert_eq!(
            cache.load(&key).expect("cache read should work"),
            Some(vec![1, 2, 3, 4])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fixtures_have_expected_pixel_count_and_write_ppm() {
        let fixture = checkerboard_fixture(8, 4, 2).expect("fixture should build");
        assert_eq!(fixture.pixels_rgb.len(), 32);
        let path = std::env::temp_dir().join("dodbogi-checkerboard-test.ppm");
        let metadata = fixture.write_ppm(&path).expect("fixture should write");
        assert_eq!(metadata.width, 8);
        assert_eq!(metadata.height, 4);
        assert!(path.exists());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn stats_overlay_formats_diagnostic_lines() {
        let stats = RenderStatistics {
            frame_index: 7,
            capture_width: 320,
            capture_height: 180,
            output_width: 640,
            output_height: 360,
            frame_time_ms: 16.67,
            gpu_time_ms: Some(0.42),
            effect_chain: vec!["bilinear".to_string(), "adaptive_sharpen".to_string()],
        };
        let lines = stats.overlay_lines();
        assert!(lines
            .iter()
            .any(|line| line.contains("320x180 -> output 640x360")));
        assert!(lines
            .iter()
            .any(|line| line.contains("bilinear -> adaptive_sharpen")));
    }
}
