//! Capture backend boundary.
//!
//! Stage C/F implement real Windows Graphics Capture, Desktop Duplication, GDI,
//! and practical DWM shared-surface backends behind this boundary.

use dodbogi_core::PhysicalRect;
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackendKind {
    WindowsGraphicsCapture,
    DesktopDuplication,
    Gdi,
    DwmSharedSurface,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureBackendDescriptor {
    pub kind: CaptureBackendKind,
    pub display_name: &'static str,
    pub frame_producing_runtime: bool,
    pub availability_probe: bool,
    pub limitation: Option<&'static str>,
}

pub fn planned_backends() -> Vec<CaptureBackendDescriptor> {
    vec![
        CaptureBackendDescriptor {
            kind: CaptureBackendKind::WindowsGraphicsCapture,
            display_name: "Windows Graphics Capture",
            frame_producing_runtime: true,
            availability_probe: true,
            limitation: None,
        },
        CaptureBackendDescriptor {
            kind: CaptureBackendKind::DesktopDuplication,
            display_name: "Desktop Duplication",
            frame_producing_runtime: false,
            availability_probe: true,
            limitation: Some("availability probe only in this build; display-level runtime must clip per-window/title-bar regions after capture"),
        },
        CaptureBackendDescriptor {
            kind: CaptureBackendKind::Gdi,
            display_name: "GDI",
            frame_producing_runtime: false,
            availability_probe: true,
            limitation: Some("availability probe only in this build; CPU BitBlt runtime may miss layered/accelerated/protected surfaces"),
        },
        CaptureBackendDescriptor {
            kind: CaptureBackendKind::DwmSharedSurface,
            display_name: "DWM shared surface",
            frame_producing_runtime: false,
            availability_probe: true,
            limitation: Some("public metadata/frame-bounds probe only; no private shared-surface API use"),
        },
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarCaptureMode {
    IncludeTitleBar,
    ClientOnly,
}

pub fn resolve_title_bar_capture_region(
    window_rect: PhysicalRect,
    client_rect_screen: PhysicalRect,
    mode: TitleBarCaptureMode,
) -> Result<PhysicalRect, CaptureError> {
    if window_rect.is_empty() {
        return Err(CaptureError::InvalidCaptureItemSize {
            width: window_rect.width(),
            height: window_rect.height(),
        });
    }
    if client_rect_screen.is_empty() {
        return Err(CaptureError::InvalidCaptureItemSize {
            width: client_rect_screen.width(),
            height: client_rect_screen.height(),
        });
    }

    Ok(match mode {
        TitleBarCaptureMode::IncludeTitleBar => window_rect,
        TitleBarCaptureMode::ClientOnly => client_rect_screen,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureBackendProbe {
    pub kind: CaptureBackendKind,
    pub available: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureBackendProbeReport {
    pub probes: Vec<CaptureBackendProbe>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgcFramePathReport {
    pub item_width: i32,
    pub item_height: i32,
    pub frame_pool_created: bool,
    pub session_started: bool,
    pub first_frame_size: Option<(i32, i32)>,
    pub poll_attempts: u32,
    pub last_poll_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WgcFrameStreamReport {
    pub item_width: i32,
    pub item_height: i32,
    pub frame_pool_created: bool,
    pub session_started: bool,
    pub frames_observed: u32,
    pub surfaces_observed: u32,
    pub last_frame_size: Option<(i32, i32)>,
    pub last_surface_size: Option<(i32, i32)>,
    pub poll_attempts: u32,
    pub last_poll_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureError {
    InvalidCaptureItemSize { width: i32, height: i32 },
    Api(String),
    NotImplemented(&'static str),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCaptureItemSize { width, height } => {
                write!(f, "invalid capture item size {width}x{height}")
            }
            Self::Api(detail) => f.write_str(detail),
            Self::NotImplemented(detail) => f.write_str(detail),
        }
    }
}

impl Error for CaptureError {}

#[cfg(windows)]
mod wgc {
    use super::{CaptureError, WgcFramePathReport, WgcFrameStreamReport};
    use std::{
        ptr::null_mut,
        thread,
        time::{Duration, Instant},
    };
    use windows::{
        core::Interface,
        Graphics::{
            Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem},
            DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
        },
        Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::{
                    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
                    D3D_FEATURE_LEVEL_11_1,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
                },
                Dxgi::{IDXGIAdapter, IDXGIDevice},
            },
            System::WinRT::Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
        },
    };

    fn create_winrt_d3d_device() -> Result<IDirect3DDevice, CaptureError> {
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
        .map_err(|error| CaptureError::Api(format!("D3D11CreateDevice failed: {error:?}")))?;

        let device = device.ok_or_else(|| {
            CaptureError::Api("D3D11CreateDevice returned no ID3D11Device".to_string())
        })?;
        let dxgi_device: IDXGIDevice = device
            .cast()
            .map_err(|error| CaptureError::Api(format!("IDXGIDevice cast failed: {error:?}")))?;
        let inspectable =
            unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }.map_err(|error| {
                CaptureError::Api(format!(
                    "CreateDirect3D11DeviceFromDXGIDevice failed: {error:?}"
                ))
            })?;
        inspectable
            .cast::<IDirect3DDevice>()
            .map_err(|error| CaptureError::Api(format!("IDirect3DDevice cast failed: {error:?}")))
    }

    pub fn probe_wgc_d3d11_frame_path(
        item: &GraphicsCaptureItem,
        timeout: Duration,
    ) -> Result<WgcFramePathReport, CaptureError> {
        let size = item.Size().map_err(|error| {
            CaptureError::Api(format!("GraphicsCaptureItem::Size failed: {error:?}"))
        })?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err(CaptureError::InvalidCaptureItemSize {
                width: size.Width,
                height: size.Height,
            });
        }

        let device = create_winrt_d3d_device()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(|error| {
            CaptureError::Api(format!(
                "Direct3D11CaptureFramePool::CreateFreeThreaded failed: {error:?}"
            ))
        })?;
        let session = pool.CreateCaptureSession(item).map_err(|error| {
            CaptureError::Api(format!("CreateCaptureSession failed: {error:?}"))
        })?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        session
            .StartCapture()
            .map_err(|error| CaptureError::Api(format!("StartCapture failed: {error:?}")))?;

        let deadline = Instant::now() + timeout;
        let mut attempts = 0u32;
        let mut first_frame_size = None;
        let mut last_poll_error = None;

        while Instant::now() < deadline {
            attempts += 1;
            match pool.TryGetNextFrame() {
                Ok(frame) => {
                    let content_size = frame.ContentSize().map_err(|error| {
                        CaptureError::Api(format!(
                            "Direct3D11CaptureFrame::ContentSize failed: {error:?}"
                        ))
                    })?;
                    first_frame_size = Some((content_size.Width, content_size.Height));
                    last_poll_error = None;
                    break;
                }
                Err(error) => {
                    last_poll_error = Some(format!("{error:?}"));
                    thread::sleep(Duration::from_millis(16));
                }
            }
        }

        let _ = session.Close();
        let _ = pool.Close();

        Ok(WgcFramePathReport {
            item_width: size.Width,
            item_height: size.Height,
            frame_pool_created: true,
            session_started: true,
            first_frame_size,
            poll_attempts: attempts,
            last_poll_error,
        })
    }

    pub fn probe_wgc_frame_stream(
        item: &GraphicsCaptureItem,
        timeout: Duration,
        target_frames: u32,
    ) -> Result<WgcFrameStreamReport, CaptureError> {
        let size = item.Size().map_err(|error| {
            CaptureError::Api(format!("GraphicsCaptureItem::Size failed: {error:?}"))
        })?;
        if size.Width <= 0 || size.Height <= 0 {
            return Err(CaptureError::InvalidCaptureItemSize {
                width: size.Width,
                height: size.Height,
            });
        }

        let device = create_winrt_d3d_device()?;
        let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
            &device,
            DirectXPixelFormat::B8G8R8A8UIntNormalized,
            2,
            size,
        )
        .map_err(|error| {
            CaptureError::Api(format!(
                "Direct3D11CaptureFramePool::CreateFreeThreaded failed: {error:?}"
            ))
        })?;
        let session = pool.CreateCaptureSession(item).map_err(|error| {
            CaptureError::Api(format!("CreateCaptureSession failed: {error:?}"))
        })?;
        let _ = session.SetIsCursorCaptureEnabled(false);
        let _ = session.SetIsBorderRequired(false);
        session
            .StartCapture()
            .map_err(|error| CaptureError::Api(format!("StartCapture failed: {error:?}")))?;

        let deadline = Instant::now() + timeout;
        let mut attempts = 0u32;
        let mut frames_observed = 0u32;
        let mut surfaces_observed = 0u32;
        let mut last_frame_size = None;
        let mut last_surface_size = None;
        let mut last_poll_error = None;

        while Instant::now() < deadline && frames_observed < target_frames.max(1) {
            attempts += 1;
            match pool.TryGetNextFrame() {
                Ok(frame) => {
                    let content_size = frame.ContentSize().map_err(|error| {
                        CaptureError::Api(format!(
                            "Direct3D11CaptureFrame::ContentSize failed: {error:?}"
                        ))
                    })?;
                    frames_observed += 1;
                    last_frame_size = Some((content_size.Width, content_size.Height));
                    let surface = frame.Surface().map_err(|error| {
                        CaptureError::Api(format!(
                            "Direct3D11CaptureFrame::Surface failed: {error:?}"
                        ))
                    })?;
                    let surface_description = surface.Description().map_err(|error| {
                        CaptureError::Api(format!(
                            "IDirect3DSurface::Description failed: {error:?}"
                        ))
                    })?;
                    surfaces_observed += 1;
                    last_surface_size =
                        Some((surface_description.Width, surface_description.Height));
                    last_poll_error = None;
                }
                Err(error) => {
                    last_poll_error = Some(format!("{error:?}"));
                    thread::sleep(Duration::from_millis(16));
                }
            }
        }

        let _ = session.Close();
        let _ = pool.Close();

        Ok(WgcFrameStreamReport {
            item_width: size.Width,
            item_height: size.Height,
            frame_pool_created: true,
            session_started: true,
            frames_observed,
            surfaces_observed,
            last_frame_size,
            last_surface_size,
            poll_attempts: attempts,
            last_poll_error,
        })
    }
}

#[cfg(not(windows))]
mod wgc {
    use super::{CaptureError, WgcFramePathReport, WgcFrameStreamReport};
    use std::time::Duration;

    pub fn probe_wgc_d3d11_frame_path(
        _item: &(),
        _timeout: Duration,
    ) -> Result<WgcFramePathReport, CaptureError> {
        Err(CaptureError::NotImplemented("Windows-only"))
    }

    pub fn probe_wgc_frame_stream(
        _item: &(),
        _timeout: Duration,
        _target_frames: u32,
    ) -> Result<WgcFrameStreamReport, CaptureError> {
        Err(CaptureError::NotImplemented("Windows-only"))
    }
}

#[cfg(windows)]
mod backend_probes {
    use super::{CaptureBackendKind, CaptureBackendProbe, CaptureBackendProbeReport};
    use std::{ffi::c_void, mem::size_of, ptr::null_mut};
    use windows::{
        core::Interface,
        Win32::{
            Foundation::{HMODULE, HWND, RECT},
            Graphics::{
                Direct3D::{
                    D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0,
                    D3D_FEATURE_LEVEL_11_1,
                },
                Direct3D11::{
                    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION,
                },
                Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS},
                Dxgi::{IDXGIAdapter, IDXGIDevice, IDXGIOutput1},
                Gdi::{
                    BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
                    GetDC, ReleaseDC, SelectObject, HBITMAP, HDC, HGDIOBJ, SRCCOPY,
                },
            },
        },
    };

    fn hwnd_from_raw(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }

    fn create_d3d11_device() -> Result<ID3D11Device, String> {
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
        .map_err(|error| format!("D3D11CreateDevice failed: {error:?}"))?;
        device.ok_or_else(|| "D3D11CreateDevice returned no device".to_string())
    }

    fn probe_desktop_duplication() -> CaptureBackendProbe {
        let result = (|| -> Result<String, String> {
            let device = create_d3d11_device()?;
            let dxgi_device: IDXGIDevice = device
                .cast()
                .map_err(|error| format!("IDXGIDevice cast failed: {error:?}"))?;
            let adapter = unsafe { dxgi_device.GetAdapter() }
                .map_err(|error| format!("IDXGIDevice::GetAdapter failed: {error:?}"))?;
            let output = unsafe { adapter.EnumOutputs(0) }
                .map_err(|error| format!("IDXGIAdapter::EnumOutputs(0) failed: {error:?}"))?;
            let output1: IDXGIOutput1 = output
                .cast()
                .map_err(|error| format!("IDXGIOutput1 cast failed: {error:?}"))?;
            let duplication = unsafe { output1.DuplicateOutput(&device) }
                .map_err(|error| format!("IDXGIOutput1::DuplicateOutput failed: {error:?}"))?;
            drop(duplication);
            Ok("DuplicateOutput created for adapter output 0".to_string())
        })();

        match result {
            Ok(detail) => CaptureBackendProbe {
                kind: CaptureBackendKind::DesktopDuplication,
                available: true,
                detail,
            },
            Err(detail) => CaptureBackendProbe {
                kind: CaptureBackendKind::DesktopDuplication,
                available: false,
                detail,
            },
        }
    }

    struct GdiObjects {
        window_dc: HDC,
        memory_dc: HDC,
        bitmap: HBITMAP,
        old_object: HGDIOBJ,
        hwnd: HWND,
    }

    impl Drop for GdiObjects {
        fn drop(&mut self) {
            unsafe {
                let _ = SelectObject(self.memory_dc, self.old_object);
                let _ = DeleteObject(self.bitmap.into());
                let _ = DeleteDC(self.memory_dc);
                let _ = ReleaseDC(Some(self.hwnd), self.window_dc);
            }
        }
    }

    fn probe_gdi(hwnd: isize) -> CaptureBackendProbe {
        let result = (|| -> Result<String, String> {
            let hwnd = hwnd_from_raw(hwnd);
            let window_dc = unsafe { GetDC(Some(hwnd)) };
            if window_dc.is_invalid() {
                return Err("GetDC returned invalid HDC".to_string());
            }
            let memory_dc = unsafe { CreateCompatibleDC(Some(window_dc)) };
            if memory_dc.is_invalid() {
                unsafe {
                    let _ = ReleaseDC(Some(hwnd), window_dc);
                }
                return Err("CreateCompatibleDC returned invalid HDC".to_string());
            }
            let bitmap = unsafe { CreateCompatibleBitmap(window_dc, 16, 16) };
            if bitmap.is_invalid() {
                unsafe {
                    let _ = DeleteDC(memory_dc);
                    let _ = ReleaseDC(Some(hwnd), window_dc);
                }
                return Err("CreateCompatibleBitmap returned invalid HBITMAP".to_string());
            }
            let old_object = unsafe { SelectObject(memory_dc, bitmap.into()) };
            let objects = GdiObjects {
                window_dc,
                memory_dc,
                bitmap,
                old_object,
                hwnd,
            };
            unsafe {
                BitBlt(
                    objects.memory_dc,
                    0,
                    0,
                    16,
                    16,
                    Some(objects.window_dc),
                    0,
                    0,
                    SRCCOPY,
                )
            }
            .map_err(|error| format!("BitBlt failed: {error:?}"))?;
            Ok("16x16 BitBlt succeeded".to_string())
        })();

        match result {
            Ok(detail) => CaptureBackendProbe {
                kind: CaptureBackendKind::Gdi,
                available: true,
                detail,
            },
            Err(detail) => CaptureBackendProbe {
                kind: CaptureBackendKind::Gdi,
                available: false,
                detail,
            },
        }
    }

    fn probe_dwm(hwnd: isize) -> CaptureBackendProbe {
        let result = (|| -> Result<String, String> {
            let hwnd = hwnd_from_raw(hwnd);
            let mut rect = RECT::default();
            unsafe {
                DwmGetWindowAttribute(
                    hwnd,
                    DWMWA_EXTENDED_FRAME_BOUNDS,
                    &mut rect as *mut _ as *mut c_void,
                    size_of::<RECT>() as u32,
                )
            }
            .map_err(|error| {
                format!("DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS) failed: {error:?}")
            })?;
            Ok(format!(
                "extended_frame_bounds={},{},{},{}",
                rect.left, rect.top, rect.right, rect.bottom
            ))
        })();

        match result {
            Ok(detail) => CaptureBackendProbe {
                kind: CaptureBackendKind::DwmSharedSurface,
                available: true,
                detail,
            },
            Err(detail) => CaptureBackendProbe {
                kind: CaptureBackendKind::DwmSharedSurface,
                available: false,
                detail,
            },
        }
    }

    pub fn probe_additional_backends(hwnd: isize) -> CaptureBackendProbeReport {
        CaptureBackendProbeReport {
            probes: vec![
                probe_desktop_duplication(),
                probe_gdi(hwnd),
                probe_dwm(hwnd),
            ],
        }
    }
}

#[cfg(not(windows))]
mod backend_probes {
    use super::{CaptureBackendProbeReport, CaptureError};

    pub fn probe_additional_backends(
        _hwnd: isize,
    ) -> Result<CaptureBackendProbeReport, CaptureError> {
        Err(CaptureError::NotImplemented("Windows-only"))
    }
}

pub use backend_probes::*;
pub use wgc::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_backends_distinguish_runtime_from_probe_capability() {
        let backends = planned_backends();
        let wgc = backends
            .iter()
            .find(|backend| backend.kind == CaptureBackendKind::WindowsGraphicsCapture)
            .expect("WGC backend should be planned");
        assert!(wgc.frame_producing_runtime);
        assert!(wgc.availability_probe);

        for backend in backends
            .iter()
            .filter(|backend| backend.kind != CaptureBackendKind::WindowsGraphicsCapture)
        {
            assert!(!backend.frame_producing_runtime, "{backend:?}");
            assert!(backend.availability_probe, "{backend:?}");
            assert!(backend.limitation.is_some(), "{backend:?}");
        }
    }

    #[test]
    fn title_bar_capture_region_respects_include_or_client_only_mode() {
        let window_rect = PhysicalRect {
            left: 10,
            top: 10,
            right: 210,
            bottom: 160,
        };
        let client_rect = PhysicalRect {
            left: 10,
            top: 40,
            right: 210,
            bottom: 160,
        };

        assert_eq!(
            resolve_title_bar_capture_region(
                window_rect,
                client_rect,
                TitleBarCaptureMode::IncludeTitleBar
            )
            .unwrap(),
            window_rect
        );
        assert_eq!(
            resolve_title_bar_capture_region(
                window_rect,
                client_rect,
                TitleBarCaptureMode::ClientOnly
            )
            .unwrap(),
            client_rect
        );
    }
}
