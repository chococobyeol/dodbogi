//! D3D11 renderer boundary.
//!
//! Stage C introduces the first WGC texture presentation path; Stage G expands
//! this into the independent effect renderer.

use dodbogi_effects::ShaderStage;
use std::{error::Error, fmt, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendererPlan {
    pub api: &'static str,
    pub min_feature_level: &'static str,
    pub bgra_required: bool,
}

impl Default for RendererPlan {
    fn default() -> Self {
        Self {
            api: "Direct3D 11",
            min_feature_level: "11_0",
            bgra_required: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselinePresentReport {
    pub width: u32,
    pub height: u32,
    pub feature_level: String,
    pub presented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureEffectPresentReport {
    pub effect_id: String,
    pub capture_width: u32,
    pub capture_height: u32,
    pub output_width: u32,
    pub output_height: u32,
    pub poll_attempts: u32,
    pub frames_observed: u32,
    pub surfaces_observed: u32,
    pub effect_draws: u32,
    pub presented_frames: u32,
    pub last_frame_size: Option<(u32, u32)>,
    pub last_surface_size: Option<(u32, u32)>,
    pub last_poll_error: Option<String>,
}

impl TextureEffectPresentReport {
    fn empty(effect_id: impl Into<String>, output_width: u32, output_height: u32) -> Self {
        Self {
            effect_id: effect_id.into(),
            capture_width: 0,
            capture_height: 0,
            output_width,
            output_height,
            poll_attempts: 0,
            frames_observed: 0,
            surfaces_observed: 0,
            effect_draws: 0,
            presented_frames: 0,
            last_frame_size: None,
            last_surface_size: None,
            last_poll_error: None,
        }
    }

    pub fn absorb(&mut self, other: Self) {
        if other.capture_width > 0 {
            self.capture_width = other.capture_width;
        }
        if other.capture_height > 0 {
            self.capture_height = other.capture_height;
        }
        self.poll_attempts += other.poll_attempts;
        self.frames_observed += other.frames_observed;
        self.surfaces_observed += other.surfaces_observed;
        self.effect_draws += other.effect_draws;
        self.presented_frames += other.presented_frames;
        self.last_frame_size = other.last_frame_size.or(self.last_frame_size);
        self.last_surface_size = other.last_surface_size.or(self.last_surface_size);
        self.last_poll_error = other.last_poll_error.or(self.last_poll_error.take());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HlslCompileReport {
    pub effect_id: String,
    pub stage: ShaderStage,
    pub entry_point: String,
    pub target: String,
    pub byte_len: usize,
    pub cache_hit: bool,
    pub cache_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectPipelineCompileSummary {
    pub compiler: &'static str,
    pub reports: Vec<HlslCompileReport>,
}

impl EffectPipelineCompileSummary {
    pub fn total_programs(&self) -> usize {
        self.reports.len()
    }

    pub fn cache_hits(&self) -> usize {
        self.reports
            .iter()
            .filter(|report| report.cache_hit)
            .count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    InvalidSwapchainSize { width: u32, height: u32 },
    InvalidWindowHandle,
    Api(String),
    NotImplemented(&'static str),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSwapchainSize { width, height } => {
                write!(f, "invalid swapchain size {width}x{height}")
            }
            Self::InvalidWindowHandle => f.write_str("invalid HWND"),
            Self::Api(detail) => f.write_str(detail),
            Self::NotImplemented(detail) => f.write_str(detail),
        }
    }
}

impl Error for RenderError {}

#[cfg(windows)]
mod d3d11 {
    use super::{
        BaselinePresentReport, EffectPipelineCompileSummary, HlslCompileReport, RenderError,
        TextureEffectPresentReport,
    };
    use dodbogi_effects::{builtin_effects, HlslProgram, ShaderCache, ShaderCacheKey, ShaderStage};
    use std::{
        ffi::{c_void, CString},
        fs::File,
        io::{BufWriter, Write},
        mem::{size_of, size_of_val},
        path::Path,
        ptr::null_mut,
        slice, thread,
        time::{Duration, Instant},
    };
    use windows::core::{Interface, PCSTR};
    use windows::{
        Graphics::{
            Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
            DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
            SizeInt32,
        },
        Win32::{
            Foundation::{HMODULE, HWND},
            Graphics::{
                Direct3D::{
                    Fxc::D3DCompile, ID3DBlob, ID3DInclude, D3D_DRIVER_TYPE_HARDWARE,
                    D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
                    D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Buffer, ID3D11ClassLinkage, ID3D11Device,
                    ID3D11DeviceContext, ID3D11InputLayout, ID3D11PixelShader,
                    ID3D11RenderTargetView, ID3D11Resource, ID3D11SamplerState,
                    ID3D11ShaderResourceView, ID3D11Texture2D, ID3D11VertexShader,
                    D3D11_BIND_SHADER_RESOURCE, D3D11_BIND_VERTEX_BUFFER, D3D11_BUFFER_DESC,
                    D3D11_COMPARISON_NEVER, D3D11_CPU_ACCESS_READ, D3D11_CPU_ACCESS_WRITE,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    D3D11_FLOAT32_MAX, D3D11_INPUT_ELEMENT_DESC, D3D11_INPUT_PER_VERTEX_DATA,
                    D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_MAP_WRITE_DISCARD,
                    D3D11_SAMPLER_DESC, D3D11_SDK_VERSION, D3D11_SUBRESOURCE_DATA,
                    D3D11_TEXTURE2D_DESC, D3D11_TEXTURE_ADDRESS_CLAMP, D3D11_USAGE_DEFAULT,
                    D3D11_USAGE_DYNAMIC, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
                },
                Dxgi::{
                    Common::{
                        DXGI_ALPHA_MODE_IGNORE, DXGI_FORMAT_B8G8R8A8_UNORM,
                        DXGI_FORMAT_R32G32_FLOAT, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
                    },
                    CreateDXGIFactory2, IDXGIAdapter, IDXGIDevice, IDXGIFactory2, IDXGIOutput,
                    IDXGISwapChain1, DXGI_CREATE_FACTORY_FLAGS, DXGI_PRESENT, DXGI_SCALING_STRETCH,
                    DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                    DXGI_USAGE_RENDER_TARGET_OUTPUT,
                },
                Hlsl::D3DCOMPILE_OPTIMIZATION_LEVEL2,
            },
            System::WinRT::Direct3D11::{
                CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
            },
        },
    };

    fn hwnd_from_raw(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    fn is_null_hwnd(hwnd: HWND) -> bool {
        hwnd.0.is_null()
    }

    fn feature_level_name(level: D3D_FEATURE_LEVEL) -> &'static str {
        match level {
            D3D_FEATURE_LEVEL_11_1 => "11_1",
            D3D_FEATURE_LEVEL_11_0 => "11_0",
            _ => "unknown",
        }
    }

    struct HardwareDevice {
        device: ID3D11Device,
        context: ID3D11DeviceContext,
        feature_level: D3D_FEATURE_LEVEL,
    }

    impl HardwareDevice {
        fn create() -> Result<Self, RenderError> {
            let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
            let mut device: Option<ID3D11Device> = None;
            let mut context: Option<ID3D11DeviceContext> = None;
            let mut selected = D3D_FEATURE_LEVEL(0);

            unsafe {
                D3D11CreateDevice(
                    None::<&IDXGIAdapter>,
                    D3D_DRIVER_TYPE_HARDWARE,
                    HMODULE(null_mut()),
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT,
                    Some(&levels),
                    D3D11_SDK_VERSION,
                    Some(&mut device),
                    Some(&mut selected),
                    Some(&mut context),
                )
            }
            .map_err(|error| RenderError::Api(format!("D3D11CreateDevice failed: {error:?}")))?;

            let device = device.ok_or_else(|| {
                RenderError::Api("D3D11CreateDevice returned no ID3D11Device".to_string())
            })?;
            let context = context.ok_or_else(|| {
                RenderError::Api("D3D11CreateDevice returned no immediate context".to_string())
            })?;

            Ok(Self {
                device,
                context,
                feature_level: selected,
            })
        }

        fn winrt_device(&self) -> Result<IDirect3DDevice, RenderError> {
            let dxgi_device: IDXGIDevice = self
                .device
                .cast()
                .map_err(|error| RenderError::Api(format!("IDXGIDevice cast failed: {error:?}")))?;
            let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
                .map_err(|error| {
                    RenderError::Api(format!(
                        "CreateDirect3D11DeviceFromDXGIDevice failed: {error:?}"
                    ))
                })?;
            inspectable.cast::<IDirect3DDevice>().map_err(|error| {
                RenderError::Api(format!("IDirect3DDevice cast failed: {error:?}"))
            })
        }
    }

    pub struct BaselinePresenter {
        d3d: HardwareDevice,
        swap_chain: IDXGISwapChain1,
        render_target: ID3D11RenderTargetView,
        width: u32,
        height: u32,
    }

    impl BaselinePresenter {
        pub fn create_for_hwnd(hwnd: isize, width: u32, height: u32) -> Result<Self, RenderError> {
            if width == 0 || height == 0 {
                return Err(RenderError::InvalidSwapchainSize { width, height });
            }
            let hwnd = hwnd_from_raw(hwnd);
            if is_null_hwnd(hwnd) {
                return Err(RenderError::InvalidWindowHandle);
            }

            let d3d = HardwareDevice::create()?;
            let factory: IDXGIFactory2 = unsafe {
                CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0))
            }
            .map_err(|error| RenderError::Api(format!("CreateDXGIFactory2 failed: {error:?}")))?;
            let desc = DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                Stereo: false.into(),
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                Flags: 0,
            };
            let swap_chain = unsafe {
                factory.CreateSwapChainForHwnd(&d3d.device, hwnd, &desc, None, None::<&IDXGIOutput>)
            }
            .map_err(|error| {
                RenderError::Api(format!("CreateSwapChainForHwnd failed: {error:?}"))
            })?;
            let back_buffer: ID3D11Texture2D =
                unsafe { swap_chain.GetBuffer(0) }.map_err(|error| {
                    RenderError::Api(format!("IDXGISwapChain::GetBuffer failed: {error:?}"))
                })?;
            let mut render_target: Option<ID3D11RenderTargetView> = None;
            unsafe {
                d3d.device
                    .CreateRenderTargetView(&back_buffer, None, Some(&mut render_target))
            }
            .map_err(|error| {
                RenderError::Api(format!("CreateRenderTargetView failed: {error:?}"))
            })?;
            let render_target = render_target.ok_or_else(|| {
                RenderError::Api("CreateRenderTargetView returned no render target".to_string())
            })?;

            Ok(Self {
                d3d,
                swap_chain,
                render_target,
                width,
                height,
            })
        }

        pub fn present_baseline_clear(
            &self,
            color: [f32; 4],
        ) -> Result<BaselinePresentReport, RenderError> {
            unsafe {
                self.d3d
                    .context
                    .OMSetRenderTargets(Some(&[Some(self.render_target.clone())]), None);
                self.d3d
                    .context
                    .ClearRenderTargetView(&self.render_target, &color);
                self.swap_chain
                    .Present(1, DXGI_PRESENT(0))
                    .ok()
                    .map_err(|error| {
                        RenderError::Api(format!("IDXGISwapChain::Present failed: {error:?}"))
                    })?;
            }

            Ok(BaselinePresentReport {
                width: self.width,
                height: self.height,
                feature_level: feature_level_name(self.d3d.feature_level).to_string(),
                presented: true,
            })
        }
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FullscreenVertex {
        position: [f32; 2],
        uv: [f32; 2],
    }

    struct FullscreenEffectPipeline {
        effect_id: String,
        input_layout: ID3D11InputLayout,
        vertex_shader: ID3D11VertexShader,
        pixel_shader: ID3D11PixelShader,
        sampler: ID3D11SamplerState,
        vertex_buffer: ID3D11Buffer,
    }

    fn fullscreen_vertices(uv_right: f32, uv_bottom: f32) -> [FullscreenVertex; 6] {
        [
            FullscreenVertex {
                position: [-1.0, -1.0],
                uv: [0.0, uv_bottom],
            },
            FullscreenVertex {
                position: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            FullscreenVertex {
                position: [1.0, -1.0],
                uv: [uv_right, uv_bottom],
            },
            FullscreenVertex {
                position: [1.0, -1.0],
                uv: [uv_right, uv_bottom],
            },
            FullscreenVertex {
                position: [-1.0, 1.0],
                uv: [0.0, 0.0],
            },
            FullscreenVertex {
                position: [1.0, 1.0],
                uv: [uv_right, 0.0],
            },
        ]
    }

    fn compiled_effect_pair(effect_id: &str) -> Result<(Vec<u8>, Vec<u8>), RenderError> {
        let effect = builtin_effects()
            .into_iter()
            .find(|effect| effect.id == effect_id)
            .ok_or_else(|| RenderError::Api(format!("unknown effect id: {effect_id}")))?;
        let vertex = effect
            .programs
            .iter()
            .find(|program| program.stage == ShaderStage::Vertex)
            .ok_or_else(|| RenderError::Api(format!("effect {effect_id} has no vertex shader")))?;
        let pixel = effect
            .programs
            .iter()
            .find(|program| program.stage == ShaderStage::Pixel)
            .ok_or_else(|| RenderError::Api(format!("effect {effect_id} has no pixel shader")))?;
        Ok((compile_hlsl_program(vertex)?, compile_hlsl_program(pixel)?))
    }

    fn create_fullscreen_effect_pipeline(
        d3d: &HardwareDevice,
        effect_id: &str,
    ) -> Result<FullscreenEffectPipeline, RenderError> {
        let (vertex_bytes, pixel_bytes) = compiled_effect_pair(effect_id)?;

        let mut vertex_shader = None;
        unsafe {
            d3d.device.CreateVertexShader(
                &vertex_bytes,
                None::<&ID3D11ClassLinkage>,
                Some(&mut vertex_shader),
            )
        }
        .map_err(|error| RenderError::Api(format!("CreateVertexShader failed: {error:?}")))?;
        let vertex_shader = vertex_shader
            .ok_or_else(|| RenderError::Api("CreateVertexShader returned no shader".to_string()))?;

        let mut pixel_shader = None;
        unsafe {
            d3d.device.CreatePixelShader(
                &pixel_bytes,
                None::<&ID3D11ClassLinkage>,
                Some(&mut pixel_shader),
            )
        }
        .map_err(|error| RenderError::Api(format!("CreatePixelShader failed: {error:?}")))?;
        let pixel_shader = pixel_shader
            .ok_or_else(|| RenderError::Api("CreatePixelShader returned no shader".to_string()))?;

        static POSITION_SEMANTIC: &[u8] = b"POSITION\0";
        static TEXCOORD_SEMANTIC: &[u8] = b"TEXCOORD\0";
        let input_elements = [
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(POSITION_SEMANTIC.as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 0,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
            D3D11_INPUT_ELEMENT_DESC {
                SemanticName: PCSTR(TEXCOORD_SEMANTIC.as_ptr()),
                SemanticIndex: 0,
                Format: DXGI_FORMAT_R32G32_FLOAT,
                InputSlot: 0,
                AlignedByteOffset: 8,
                InputSlotClass: D3D11_INPUT_PER_VERTEX_DATA,
                InstanceDataStepRate: 0,
            },
        ];
        let mut input_layout = None;
        unsafe {
            d3d.device
                .CreateInputLayout(&input_elements, &vertex_bytes, Some(&mut input_layout))
        }
        .map_err(|error| RenderError::Api(format!("CreateInputLayout failed: {error:?}")))?;
        let input_layout = input_layout
            .ok_or_else(|| RenderError::Api("CreateInputLayout returned no layout".to_string()))?;

        let vertices = fullscreen_vertices(1.0, 1.0);
        let buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of_val(&vertices) as u32,
            Usage: D3D11_USAGE_DYNAMIC,
            BindFlags: D3D11_BIND_VERTEX_BUFFER.0 as u32,
            CPUAccessFlags: D3D11_CPU_ACCESS_WRITE.0 as u32,
            MiscFlags: 0,
            StructureByteStride: size_of::<FullscreenVertex>() as u32,
        };
        let initial_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: vertices.as_ptr().cast::<c_void>(),
            SysMemPitch: 0,
            SysMemSlicePitch: 0,
        };
        let mut vertex_buffer = None;
        unsafe {
            d3d.device
                .CreateBuffer(&buffer_desc, Some(&initial_data), Some(&mut vertex_buffer))
        }
        .map_err(|error| RenderError::Api(format!("CreateBuffer failed: {error:?}")))?;
        let vertex_buffer = vertex_buffer
            .ok_or_else(|| RenderError::Api("CreateBuffer returned no buffer".to_string()))?;

        let sampler_desc = D3D11_SAMPLER_DESC {
            Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
            AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
            AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
            MipLODBias: 0.0,
            MaxAnisotropy: 1,
            ComparisonFunc: D3D11_COMPARISON_NEVER,
            BorderColor: [0.0, 0.0, 0.0, 0.0],
            MinLOD: 0.0,
            MaxLOD: D3D11_FLOAT32_MAX,
        };
        let mut sampler = None;
        unsafe {
            d3d.device
                .CreateSamplerState(&sampler_desc, Some(&mut sampler))
        }
        .map_err(|error| RenderError::Api(format!("CreateSamplerState failed: {error:?}")))?;
        let sampler = sampler.ok_or_else(|| {
            RenderError::Api("CreateSamplerState returned no sampler".to_string())
        })?;

        Ok(FullscreenEffectPipeline {
            effect_id: effect_id.to_string(),
            input_layout,
            vertex_shader,
            pixel_shader,
            sampler,
            vertex_buffer,
        })
    }

    fn create_swapchain_render_target(
        d3d: &HardwareDevice,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> Result<(IDXGISwapChain1, ID3D11RenderTargetView), RenderError> {
        let factory: IDXGIFactory2 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS(0)) }
            .map_err(|error| RenderError::Api(format!("CreateDXGIFactory2 failed: {error:?}")))?;
        let desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: width,
            Height: height,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            Stereo: false.into(),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            Scaling: DXGI_SCALING_STRETCH,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
            AlphaMode: DXGI_ALPHA_MODE_IGNORE,
            Flags: 0,
        };
        let swap_chain = unsafe {
            factory.CreateSwapChainForHwnd(&d3d.device, hwnd, &desc, None, None::<&IDXGIOutput>)
        }
        .map_err(|error| RenderError::Api(format!("CreateSwapChainForHwnd failed: {error:?}")))?;
        let render_target = create_render_target_from_swap_chain(d3d, &swap_chain)?;
        Ok((swap_chain, render_target))
    }

    fn create_render_target_from_swap_chain(
        d3d: &HardwareDevice,
        swap_chain: &IDXGISwapChain1,
    ) -> Result<ID3D11RenderTargetView, RenderError> {
        let back_buffer: ID3D11Texture2D = unsafe { swap_chain.GetBuffer(0) }.map_err(|error| {
            RenderError::Api(format!("IDXGISwapChain::GetBuffer failed: {error:?}"))
        })?;
        let mut render_target = None;
        unsafe {
            d3d.device
                .CreateRenderTargetView(&back_buffer, None, Some(&mut render_target))
        }
        .map_err(|error| RenderError::Api(format!("CreateRenderTargetView failed: {error:?}")))?;
        let render_target = render_target.ok_or_else(|| {
            RenderError::Api("CreateRenderTargetView returned no render target".to_string())
        })?;
        Ok(render_target)
    }

    fn copyable_shader_resource_texture(
        d3d: &HardwareDevice,
        source: &ID3D11Texture2D,
        source_desc: D3D11_TEXTURE2D_DESC,
    ) -> Result<ID3D11Texture2D, RenderError> {
        if (source_desc.BindFlags & D3D11_BIND_SHADER_RESOURCE.0 as u32) != 0 {
            return Ok(source.clone());
        }

        let desc = D3D11_TEXTURE2D_DESC {
            Width: source_desc.Width,
            Height: source_desc.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: source_desc.Format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        unsafe { d3d.device.CreateTexture2D(&desc, None, Some(&mut texture)) }.map_err(
            |error| RenderError::Api(format!("CreateTexture2D SRV copy failed: {error:?}")),
        )?;
        let texture = texture.ok_or_else(|| {
            RenderError::Api("CreateTexture2D returned no SRV copy texture".to_string())
        })?;
        unsafe { d3d.context.CopyResource(&texture, source) };
        Ok(texture)
    }

    fn create_screenshot_parent_dir(path: &Path) -> Result<(), RenderError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                RenderError::Api(format!(
                    "failed creating screenshot directory {}: {error}",
                    parent.display()
                ))
            })?;
        }
        Ok(())
    }

    fn read_texture_to_bgra8(
        d3d: &HardwareDevice,
        source: &ID3D11Texture2D,
    ) -> Result<(u32, u32, Vec<u8>), RenderError> {
        let mut source_desc = D3D11_TEXTURE2D_DESC::default();
        unsafe { source.GetDesc(&mut source_desc) };
        let desc = D3D11_TEXTURE2D_DESC {
            Width: source_desc.Width,
            Height: source_desc.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: source_desc.Format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging = None;
        unsafe { d3d.device.CreateTexture2D(&desc, None, Some(&mut staging)) }.map_err(
            |error| {
                RenderError::Api(format!(
                    "CreateTexture2D screenshot staging failed: {error:?}"
                ))
            },
        )?;
        let staging = staging.ok_or_else(|| {
            RenderError::Api("CreateTexture2D returned no screenshot staging texture".to_string())
        })?;
        unsafe { d3d.context.CopyResource(&staging, source) };

        let resource: ID3D11Resource = staging
            .cast()
            .map_err(|error| RenderError::Api(format!("ID3D11Resource cast failed: {error:?}")))?;
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            d3d.context
                .Map(&resource, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(|error| RenderError::Api(format!("Map screenshot staging failed: {error:?}")))?;

        let result = (|| -> Result<Vec<u8>, RenderError> {
            if mapped.pData.is_null() {
                return Err(RenderError::Api(
                    "Map screenshot staging returned null data".to_string(),
                ));
            }
            let row_pitch = mapped.RowPitch as usize;
            let row_bytes = desc.Width as usize * 4;
            if row_pitch < row_bytes {
                return Err(RenderError::Api(format!(
                    "screenshot row pitch {row_pitch} is smaller than row bytes {row_bytes}"
                )));
            }
            let mut pixels = Vec::with_capacity(row_bytes * desc.Height as usize);
            for y in 0..desc.Height as usize {
                let row = unsafe {
                    slice::from_raw_parts(mapped.pData.cast::<u8>().add(y * row_pitch), row_bytes)
                };
                pixels.extend_from_slice(row);
            }
            Ok(pixels)
        })();

        unsafe { d3d.context.Unmap(&resource, 0) };
        Ok((desc.Width, desc.Height, result?))
    }

    fn bgra8_to_rgb8(bgra: &[u8]) -> Vec<u8> {
        let mut rgb = Vec::with_capacity(bgra.len() / 4 * 3);
        for pixel in bgra.chunks_exact(4) {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
        rgb
    }

    fn write_texture_to_ppm(
        d3d: &HardwareDevice,
        source: &ID3D11Texture2D,
        path: impl AsRef<Path>,
    ) -> Result<(), RenderError> {
        let path = path.as_ref();
        create_screenshot_parent_dir(path)?;
        let (width, height, pixels) = read_texture_to_bgra8(d3d, source)?;
        let mut file = File::create(path).map_err(|error| {
            RenderError::Api(format!(
                "failed creating screenshot {}: {error}",
                path.display()
            ))
        })?;
        write!(file, "P6\n{} {}\n255\n", width, height).map_err(|error| {
            RenderError::Api(format!("failed writing screenshot header: {error}"))
        })?;
        for pixel in pixels.chunks_exact(4) {
            file.write_all(&[pixel[2], pixel[1], pixel[0]])
                .map_err(|error| {
                    RenderError::Api(format!("failed writing screenshot pixels: {error}"))
                })?;
        }
        Ok(())
    }

    fn write_texture_to_png(
        d3d: &HardwareDevice,
        source: &ID3D11Texture2D,
        path: impl AsRef<Path>,
    ) -> Result<(), RenderError> {
        let path = path.as_ref();
        create_screenshot_parent_dir(path)?;
        let (width, height, pixels) = read_texture_to_bgra8(d3d, source)?;
        let rgb = bgra8_to_rgb8(&pixels);
        let file = File::create(path).map_err(|error| {
            RenderError::Api(format!(
                "failed creating screenshot {}: {error}",
                path.display()
            ))
        })?;
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| RenderError::Api(format!("failed writing PNG header: {error}")))?;
        writer
            .write_image_data(&rgb)
            .map_err(|error| RenderError::Api(format!("failed writing PNG pixels: {error}")))?;
        Ok(())
    }

    pub struct WgcEffectScaler {
        d3d: HardwareDevice,
        winrt_device: IDirect3DDevice,
        pool: Direct3D11CaptureFramePool,
        session: GraphicsCaptureSession,
        swap_chain: IDXGISwapChain1,
        render_target: Option<ID3D11RenderTargetView>,
        pipeline: FullscreenEffectPipeline,
        output_width: u32,
        output_height: u32,
        capture_width: u32,
        capture_height: u32,
        frame_pool_width: u32,
        frame_pool_height: u32,
        closed: bool,
    }

    impl WgcEffectScaler {
        pub fn create_for_hwnd_and_item(
            hwnd: isize,
            width: u32,
            height: u32,
            item: &GraphicsCaptureItem,
            effect_id: &str,
        ) -> Result<Self, RenderError> {
            if width == 0 || height == 0 {
                return Err(RenderError::InvalidSwapchainSize { width, height });
            }
            let hwnd = hwnd_from_raw(hwnd);
            if is_null_hwnd(hwnd) {
                return Err(RenderError::InvalidWindowHandle);
            }

            let item_size = item.Size().map_err(|error| {
                RenderError::Api(format!("GraphicsCaptureItem::Size failed: {error:?}"))
            })?;
            if item_size.Width <= 0 || item_size.Height <= 0 {
                return Err(RenderError::InvalidSwapchainSize {
                    width: item_size.Width.max(0) as u32,
                    height: item_size.Height.max(0) as u32,
                });
            }

            let d3d = HardwareDevice::create()?;
            let winrt_device = d3d.winrt_device()?;
            let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
                &winrt_device,
                DirectXPixelFormat::B8G8R8A8UIntNormalized,
                2,
                item_size,
            )
            .map_err(|error| {
                RenderError::Api(format!(
                    "Direct3D11CaptureFramePool::CreateFreeThreaded failed: {error:?}"
                ))
            })?;
            let session = pool.CreateCaptureSession(item).map_err(|error| {
                RenderError::Api(format!("CreateCaptureSession failed: {error:?}"))
            })?;
            let _ = session.SetIsCursorCaptureEnabled(false);
            let _ = session.SetIsBorderRequired(false);
            session
                .StartCapture()
                .map_err(|error| RenderError::Api(format!("StartCapture failed: {error:?}")))?;

            let (swap_chain, render_target) =
                create_swapchain_render_target(&d3d, hwnd, width, height)?;
            let pipeline = create_fullscreen_effect_pipeline(&d3d, effect_id)?;

            Ok(Self {
                d3d,
                winrt_device,
                pool,
                session,
                swap_chain,
                render_target: Some(render_target),
                pipeline,
                output_width: width,
                output_height: height,
                capture_width: item_size.Width as u32,
                capture_height: item_size.Height as u32,
                frame_pool_width: item_size.Width as u32,
                frame_pool_height: item_size.Height as u32,
                closed: false,
            })
        }

        pub fn resize_output(&mut self, width: u32, height: u32) -> Result<(), RenderError> {
            if width == 0 || height == 0 {
                return Err(RenderError::InvalidSwapchainSize { width, height });
            }
            if self.output_width == width && self.output_height == height {
                return Ok(());
            }

            let empty_targets: [Option<ID3D11RenderTargetView>; 1] = [None];
            unsafe {
                self.d3d
                    .context
                    .OMSetRenderTargets(Some(&empty_targets), None);
                self.d3d.context.Flush();
            }
            self.render_target = None;

            unsafe {
                self.swap_chain.ResizeBuffers(
                    0,
                    width,
                    height,
                    DXGI_FORMAT_UNKNOWN,
                    DXGI_SWAP_CHAIN_FLAG(0),
                )
            }
            .map_err(|error| {
                RenderError::Api(format!("IDXGISwapChain::ResizeBuffers failed: {error:?}"))
            })?;

            self.render_target = Some(create_render_target_from_swap_chain(
                &self.d3d,
                &self.swap_chain,
            )?);
            self.output_width = width;
            self.output_height = height;
            Ok(())
        }

        fn recreate_capture_frame_pool(
            &mut self,
            width: u32,
            height: u32,
        ) -> Result<(), RenderError> {
            if width == 0 || height == 0 {
                return Err(RenderError::InvalidSwapchainSize { width, height });
            }

            self.pool
                .Recreate(
                    &self.winrt_device,
                    DirectXPixelFormat::B8G8R8A8UIntNormalized,
                    2,
                    SizeInt32 {
                        Width: width as i32,
                        Height: height as i32,
                    },
                )
                .map_err(|error| {
                    RenderError::Api(format!(
                        "Direct3D11CaptureFramePool::Recreate failed: {error:?}"
                    ))
                })?;
            self.capture_width = width;
            self.capture_height = height;
            self.frame_pool_width = width;
            self.frame_pool_height = height;
            Ok(())
        }

        pub fn present_frames(
            &mut self,
            target_frames: u32,
            timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            let mut aggregate = TextureEffectPresentReport::empty(
                self.pipeline.effect_id.clone(),
                self.output_width,
                self.output_height,
            );
            aggregate.capture_width = self.capture_width;
            aggregate.capture_height = self.capture_height;
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline && aggregate.presented_frames < target_frames.max(1) {
                let remaining = deadline.saturating_duration_since(Instant::now());
                let slice = remaining.min(Duration::from_millis(50));
                let report = self.present_next_frame(slice)?;
                let observed = report.presented_frames > 0;
                aggregate.absorb(report);
                if !observed {
                    thread::sleep(Duration::from_millis(5));
                }
            }
            Ok(aggregate)
        }

        pub fn present_next_frame(
            &mut self,
            timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            let mut report = TextureEffectPresentReport::empty(
                self.pipeline.effect_id.clone(),
                self.output_width,
                self.output_height,
            );
            report.capture_width = self.capture_width;
            report.capture_height = self.capture_height;
            let deadline = Instant::now() + timeout;

            while Instant::now() <= deadline {
                report.poll_attempts += 1;
                match self.pool.TryGetNextFrame() {
                    Ok(frame) => {
                        let content_size = frame.ContentSize().map_err(|error| {
                            RenderError::Api(format!(
                                "Direct3D11CaptureFrame::ContentSize failed: {error:?}"
                            ))
                        })?;
                        let content_width = content_size.Width.max(0) as u32;
                        let content_height = content_size.Height.max(0) as u32;
                        report.frames_observed = 1;
                        report.last_frame_size = Some((content_width, content_height));
                        if content_width > self.frame_pool_width
                            || content_height > self.frame_pool_height
                        {
                            self.recreate_capture_frame_pool(
                                content_width.max(1),
                                content_height.max(1),
                            )?;
                            report.capture_width = self.capture_width;
                            report.capture_height = self.capture_height;
                            report.last_poll_error =
                                Some("frame_pool_recreated_for_source_growth".to_string());
                            return Ok(report);
                        }
                        self.capture_width = content_width;
                        self.capture_height = content_height;
                        report.capture_width = content_width;
                        report.capture_height = content_height;

                        let surface = frame.Surface().map_err(|error| {
                            RenderError::Api(format!(
                                "Direct3D11CaptureFrame::Surface failed: {error:?}"
                            ))
                        })?;
                        let surface_description = surface.Description().map_err(|error| {
                            RenderError::Api(format!(
                                "IDirect3DSurface::Description failed: {error:?}"
                            ))
                        })?;
                        report.surfaces_observed = 1;
                        report.last_surface_size = Some((
                            surface_description.Width.max(0) as u32,
                            surface_description.Height.max(0) as u32,
                        ));

                        let access: IDirect3DDxgiInterfaceAccess =
                            surface.cast().map_err(|error| {
                                RenderError::Api(format!(
                                    "IDirect3DDxgiInterfaceAccess cast failed: {error:?}"
                                ))
                            })?;
                        let texture: ID3D11Texture2D =
                            unsafe { access.GetInterface() }.map_err(|error| {
                                RenderError::Api(format!(
                                    "captured surface ID3D11Texture2D access failed: {error:?}"
                                ))
                            })?;
                        self.draw_texture(&texture, content_width, content_height)?;
                        report.effect_draws = 1;
                        report.presented_frames = 1;
                        report.last_poll_error = None;
                        return Ok(report);
                    }
                    Err(error) => {
                        report.last_poll_error = Some(format!("{error:?}"));
                        if timeout.is_zero() || Instant::now() >= deadline {
                            return Ok(report);
                        }
                        thread::sleep(Duration::from_millis(2));
                    }
                }
            }
            Ok(report)
        }

        pub fn write_presented_frame_ppm(
            &mut self,
            path: impl AsRef<Path>,
            timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            let report = self.present_next_frame(timeout)?;
            if report.presented_frames == 0 {
                return Err(RenderError::Api(format!(
                    "no presented frame available for screenshot; last_poll_error={:?}",
                    report.last_poll_error
                )));
            }
            self.write_current_backbuffer_ppm(path)?;
            Ok(report)
        }

        pub fn write_current_backbuffer_ppm(
            &self,
            path: impl AsRef<Path>,
        ) -> Result<(), RenderError> {
            let back_buffer: ID3D11Texture2D =
                unsafe { self.swap_chain.GetBuffer(0) }.map_err(|error| {
                    RenderError::Api(format!("IDXGISwapChain::GetBuffer failed: {error:?}"))
                })?;
            write_texture_to_ppm(&self.d3d, &back_buffer, path)
        }

        pub fn write_presented_frame_png(
            &mut self,
            path: impl AsRef<Path>,
            timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            let report = self.present_next_frame(timeout)?;
            if report.presented_frames == 0 {
                return Err(RenderError::Api(format!(
                    "no presented frame available for screenshot; last_poll_error={:?}",
                    report.last_poll_error
                )));
            }
            self.write_current_backbuffer_png(path)?;
            Ok(report)
        }

        pub fn write_current_backbuffer_png(
            &self,
            path: impl AsRef<Path>,
        ) -> Result<(), RenderError> {
            let back_buffer: ID3D11Texture2D =
                unsafe { self.swap_chain.GetBuffer(0) }.map_err(|error| {
                    RenderError::Api(format!("IDXGISwapChain::GetBuffer failed: {error:?}"))
                })?;
            write_texture_to_png(&self.d3d, &back_buffer, path)
        }

        fn update_fullscreen_vertices(
            &self,
            content_width: u32,
            content_height: u32,
            texture_width: u32,
            texture_height: u32,
        ) -> Result<(), RenderError> {
            let uv_right = if texture_width == 0 || content_width == 0 {
                1.0
            } else {
                (content_width.min(texture_width) as f32 / texture_width as f32).clamp(0.0, 1.0)
            };
            let uv_bottom = if texture_height == 0 || content_height == 0 {
                1.0
            } else {
                (content_height.min(texture_height) as f32 / texture_height as f32).clamp(0.0, 1.0)
            };
            let vertices = fullscreen_vertices(uv_right, uv_bottom);
            let resource: ID3D11Resource = self.pipeline.vertex_buffer.cast().map_err(|error| {
                RenderError::Api(format!(
                    "vertex buffer ID3D11Resource cast failed: {error:?}"
                ))
            })?;
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.d3d
                    .context
                    .Map(&resource, 0, D3D11_MAP_WRITE_DISCARD, 0, Some(&mut mapped))
            }
            .map_err(|error| {
                RenderError::Api(format!("Map fullscreen vertices failed: {error:?}"))
            })?;

            unsafe {
                std::ptr::copy_nonoverlapping(
                    vertices.as_ptr().cast::<u8>(),
                    mapped.pData.cast::<u8>(),
                    size_of_val(&vertices),
                );
                self.d3d.context.Unmap(&resource, 0);
            }
            Ok(())
        }

        fn draw_texture(
            &self,
            texture: &ID3D11Texture2D,
            content_width: u32,
            content_height: u32,
        ) -> Result<(), RenderError> {
            let mut source_desc = D3D11_TEXTURE2D_DESC::default();
            unsafe { texture.GetDesc(&mut source_desc) };
            self.update_fullscreen_vertices(
                content_width,
                content_height,
                source_desc.Width,
                source_desc.Height,
            )?;
            let texture = copyable_shader_resource_texture(&self.d3d, texture, source_desc)?;
            let mut shader_resource: Option<ID3D11ShaderResourceView> = None;
            unsafe {
                self.d3d
                    .device
                    .CreateShaderResourceView(&texture, None, Some(&mut shader_resource))
            }
            .map_err(|error| {
                RenderError::Api(format!("CreateShaderResourceView failed: {error:?}"))
            })?;
            let shader_resource = shader_resource.ok_or_else(|| {
                RenderError::Api("CreateShaderResourceView returned no view".to_string())
            })?;

            let viewport = D3D11_VIEWPORT {
                TopLeftX: 0.0,
                TopLeftY: 0.0,
                Width: self.output_width as f32,
                Height: self.output_height as f32,
                MinDepth: 0.0,
                MaxDepth: 1.0,
            };
            let vertex_buffers = [Some(self.pipeline.vertex_buffer.clone())];
            let strides = [size_of::<FullscreenVertex>() as u32];
            let offsets = [0u32];
            let samplers = [Some(self.pipeline.sampler.clone())];
            let resources = [Some(shader_resource)];
            let empty_resources: [Option<ID3D11ShaderResourceView>; 1] = [None];
            let render_target = self.render_target.as_ref().ok_or_else(|| {
                RenderError::Api("render target is unavailable after output resize".to_string())
            })?;

            unsafe {
                self.d3d
                    .context
                    .OMSetRenderTargets(Some(&[Some(render_target.clone())]), None);
                self.d3d
                    .context
                    .ClearRenderTargetView(render_target, &[0.0, 0.0, 0.0, 1.0]);
                self.d3d.context.RSSetViewports(Some(&[viewport]));
                self.d3d
                    .context
                    .IASetInputLayout(&self.pipeline.input_layout);
                self.d3d
                    .context
                    .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
                self.d3d.context.IASetVertexBuffers(
                    0,
                    1,
                    Some(vertex_buffers.as_ptr()),
                    Some(strides.as_ptr()),
                    Some(offsets.as_ptr()),
                );
                self.d3d
                    .context
                    .VSSetShader(&self.pipeline.vertex_shader, None);
                self.d3d
                    .context
                    .PSSetShader(&self.pipeline.pixel_shader, None);
                self.d3d.context.PSSetSamplers(0, Some(&samplers));
                self.d3d.context.PSSetShaderResources(0, Some(&resources));
                self.d3d.context.Draw(6, 0);
                self.d3d
                    .context
                    .PSSetShaderResources(0, Some(&empty_resources));
                self.swap_chain
                    .Present(1, DXGI_PRESENT(0))
                    .ok()
                    .map_err(|error| {
                        RenderError::Api(format!("IDXGISwapChain::Present failed: {error:?}"))
                    })?;
            }
            Ok(())
        }

        pub fn close(&mut self) {
            if !self.closed {
                let _ = self.session.Close();
                let _ = self.pool.Close();
                self.closed = true;
            }
        }
    }

    impl Drop for WgcEffectScaler {
        fn drop(&mut self) {
            self.close();
        }
    }

    pub fn probe_d3d11_hardware_feature_level() -> Result<String, RenderError> {
        HardwareDevice::create().map(|device| feature_level_name(device.feature_level).to_string())
    }

    fn cstring(value: &str, label: &str) -> Result<CString, RenderError> {
        CString::new(value)
            .map_err(|error| RenderError::Api(format!("{label} contains NUL byte: {error}")))
    }

    fn blob_to_bytes(blob: &ID3DBlob) -> Vec<u8> {
        unsafe {
            let len = blob.GetBufferSize();
            let ptr = blob.GetBufferPointer().cast::<u8>();
            if ptr.is_null() || len == 0 {
                Vec::new()
            } else {
                slice::from_raw_parts(ptr, len).to_vec()
            }
        }
    }

    fn blob_to_message(blob: &Option<ID3DBlob>) -> String {
        blob.as_ref()
            .map(blob_to_bytes)
            .filter(|bytes| !bytes.is_empty())
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .trim_matches(char::from(0))
                    .trim()
                    .to_string()
            })
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| "no compiler diagnostic blob".to_string())
    }

    fn compile_hlsl_program(program: &HlslProgram) -> Result<Vec<u8>, RenderError> {
        let source_name = cstring("dodbogi-stage-g-clean-room.hlsl", "source name")?;
        let entry = cstring(program.entry_point, "entry point")?;
        let target = cstring(program.target, "target profile")?;
        let mut bytecode: Option<ID3DBlob> = None;
        let mut diagnostics: Option<ID3DBlob> = None;

        let result = unsafe {
            D3DCompile(
                program.source.as_ptr().cast(),
                program.source.len(),
                PCSTR(source_name.as_ptr().cast()),
                None,
                None::<&ID3DInclude>,
                PCSTR(entry.as_ptr().cast()),
                PCSTR(target.as_ptr().cast()),
                D3DCOMPILE_OPTIMIZATION_LEVEL2,
                0,
                &mut bytecode,
                Some(&mut diagnostics),
            )
        };

        if let Err(error) = result {
            return Err(RenderError::Api(format!(
                "D3DCompile failed for entry={} target={}: {error:?}; {}",
                program.entry_point,
                program.target,
                blob_to_message(&diagnostics)
            )));
        }

        let blob = bytecode.ok_or_else(|| {
            RenderError::Api(format!(
                "D3DCompile returned no bytecode for entry={} target={}",
                program.entry_point, program.target
            ))
        })?;
        let bytes = blob_to_bytes(&blob);
        if bytes.is_empty() {
            return Err(RenderError::Api(format!(
                "D3DCompile returned empty bytecode for entry={} target={}",
                program.entry_point, program.target
            )));
        }
        Ok(bytes)
    }

    pub fn compile_builtin_effects_with_cache(
        cache_root: impl AsRef<Path>,
    ) -> Result<EffectPipelineCompileSummary, RenderError> {
        let cache = ShaderCache::new(cache_root.as_ref().to_path_buf());
        let mut reports = Vec::new();

        for effect in builtin_effects() {
            for program in &effect.programs {
                let key = ShaderCacheKey::for_program(effect.id, program);
                let cache_path = cache.path_for_key(&key);
                if let Some(bytes) = cache.load(&key).map_err(|error| {
                    RenderError::Api(format!(
                        "failed reading shader cache {}: {error}",
                        cache_path.display()
                    ))
                })? {
                    reports.push(HlslCompileReport {
                        effect_id: effect.id.to_string(),
                        stage: program.stage,
                        entry_point: program.entry_point.to_string(),
                        target: program.target.to_string(),
                        byte_len: bytes.len(),
                        cache_hit: true,
                        cache_path,
                    });
                    continue;
                }

                let bytes = compile_hlsl_program(program)?;
                let record = cache.store(key, &bytes, false).map_err(|error| {
                    RenderError::Api(format!(
                        "failed writing shader cache {}: {error}",
                        cache_path.display()
                    ))
                })?;
                reports.push(HlslCompileReport {
                    effect_id: effect.id.to_string(),
                    stage: program.stage,
                    entry_point: program.entry_point.to_string(),
                    target: program.target.to_string(),
                    byte_len: record.byte_len,
                    cache_hit: record.cache_hit,
                    cache_path: record.path,
                });
            }
        }

        Ok(EffectPipelineCompileSummary {
            compiler: "D3DCompile/d3dcompiler_47",
            reports,
        })
    }
}

#[cfg(not(windows))]
mod d3d11 {
    use super::{
        BaselinePresentReport, EffectPipelineCompileSummary, RenderError,
        TextureEffectPresentReport,
    };
    use std::{path::Path, time::Duration};

    pub struct BaselinePresenter;

    impl BaselinePresenter {
        pub fn create_for_hwnd(_hwnd: isize, width: u32, height: u32) -> Result<Self, RenderError> {
            if width == 0 || height == 0 {
                return Err(RenderError::InvalidSwapchainSize { width, height });
            }
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn present_baseline_clear(
            &self,
            _color: [f32; 4],
        ) -> Result<BaselinePresentReport, RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }
    }

    pub struct WgcEffectScaler;

    impl WgcEffectScaler {
        pub fn create_for_hwnd_and_item(
            _hwnd: isize,
            width: u32,
            height: u32,
            _item: &(),
            _effect_id: &str,
        ) -> Result<Self, RenderError> {
            if width == 0 || height == 0 {
                return Err(RenderError::InvalidSwapchainSize { width, height });
            }
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn present_frames(
            &mut self,
            _target_frames: u32,
            _timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn present_next_frame(
            &mut self,
            _timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn write_presented_frame_ppm(
            &mut self,
            _path: impl AsRef<Path>,
            _timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn write_current_backbuffer_ppm(
            &self,
            _path: impl AsRef<Path>,
        ) -> Result<(), RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn write_presented_frame_png(
            &mut self,
            _path: impl AsRef<Path>,
            _timeout: Duration,
        ) -> Result<TextureEffectPresentReport, RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn write_current_backbuffer_png(
            &self,
            _path: impl AsRef<Path>,
        ) -> Result<(), RenderError> {
            Err(RenderError::NotImplemented("Windows-only"))
        }

        pub fn close(&mut self) {}
    }

    pub fn probe_d3d11_hardware_feature_level() -> Result<String, RenderError> {
        Err(RenderError::NotImplemented("Windows-only"))
    }

    pub fn compile_builtin_effects_with_cache(
        _cache_root: impl AsRef<Path>,
    ) -> Result<EffectPipelineCompileSummary, RenderError> {
        Err(RenderError::NotImplemented("Windows-only"))
    }
}

pub use d3d11::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_plan_preserves_stage_c_d3d11_floor() {
        let plan = RendererPlan::default();
        assert_eq!(plan.api, "Direct3D 11");
        assert_eq!(plan.min_feature_level, "11_0");
        assert!(plan.bgra_required);
    }

    #[test]
    fn compile_summary_counts_cache_hits() {
        let summary = EffectPipelineCompileSummary {
            compiler: "test",
            reports: vec![
                HlslCompileReport {
                    effect_id: "a".to_string(),
                    stage: ShaderStage::Pixel,
                    entry_point: "ps_main".to_string(),
                    target: "ps_5_0".to_string(),
                    byte_len: 4,
                    cache_hit: false,
                    cache_path: "a.cso".into(),
                },
                HlslCompileReport {
                    effect_id: "b".to_string(),
                    stage: ShaderStage::Pixel,
                    entry_point: "ps_main".to_string(),
                    target: "ps_5_0".to_string(),
                    byte_len: 4,
                    cache_hit: true,
                    cache_path: "b.cso".into(),
                },
            ],
        };
        assert_eq!(summary.total_programs(), 2);
        assert_eq!(summary.cache_hits(), 1);
    }
}
