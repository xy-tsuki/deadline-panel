//! GPU glass renderer. One locally composed background per window, reusable
//! shaders and textures, and continuous SDF edge refraction. No CPU frame
//! readback, capture-exclusion setting, desktop duplication, or HWND region.
//! DWM shared surfaces are private: failures retain the readable CSS fallback.
use std::{
    collections::{HashMap, HashSet},
    hash::{Hash, Hasher},
    sync::{mpsc, OnceLock},
    time::{Duration, Instant},
};
use tauri::Emitter;
use windows::{
    core::{s, w, Interface, BOOL, HRESULT, PCWSTR},
    Win32::{
        Foundation::{GENERIC_READ, HANDLE, HWND, LUID, RECT},
        Graphics::{
            Direct3D::Fxc::D3DCompile,
            Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST},
            Direct3D11::*,
            DirectComposition::*,
            Dwm::{DwmGetWindowAttribute, DWMWINDOWATTRIBUTE},
            Dxgi::{Common::*, *},
            Imaging::*,
        },
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            LibraryLoader::{GetModuleHandleW, GetProcAddress},
            Threading::GetCurrentProcessId,
        },
        UI::{
            HiDpi::GetDpiForWindow,
            Shell::{
                DesktopWallpaper, IDesktopWallpaper, DESKTOP_WALLPAPER_POSITION, DWPOS_CENTER,
                DWPOS_FIT, DWPOS_SPAN, DWPOS_STRETCH, DWPOS_TILE,
            },
            WindowsAndMessaging::{
                EnumWindows, GetClassNameW, GetWindowDisplayAffinity, GetWindowRect,
                GetWindowThreadProcessId, IsIconic, IsWindowVisible,
            },
        },
    },
};
type SharedSurface =
    unsafe extern "system" fn(HWND, *mut HANDLE, *mut LUID, *mut u32, *mut u32, *mut u64) -> BOOL;
type Reply = mpsc::Sender<Result<(), String>>;
enum Request {
    Apply {
        hwnd: isize,
        dark: bool,
        window: tauri::WebviewWindow,
        reply: Reply,
    },
    Geometry(isize, Vec<[f32; 5]>),
    Remove(isize),
}
static WORKER: OnceLock<mpsc::Sender<Request>> = OnceLock::new();

pub fn apply(window: &tauri::WebviewWindow, dark: bool) -> Result<&'static str, String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?.0 as isize;
    let (tx, rx) = mpsc::channel();
    WORKER
        .get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            std::thread::Builder::new()
                .name("deadline-glass-gpu".into())
                .spawn(move || unsafe { worker(rx) })
                .expect("start glass GPU worker");
            tx
        })
        .send(Request::Apply {
            hwnd,
            dark,
            window: window.clone(),
            reply: tx,
        })
        .map_err(|error| error.to_string())?;
    rx.recv_timeout(Duration::from_secs(3))
        .map_err(|error| error.to_string())??;
    Ok("gpu-glass")
}
pub fn remove(window: &tauri::WebviewWindow) {
    if let (Some(worker), Ok(hwnd)) = (WORKER.get(), window.hwnd()) {
        let _ = worker.send(Request::Remove(hwnd.0 as isize));
    }
}
pub fn geometry(window: &tauri::WebviewWindow, rects: Vec<[f32; 5]>) {
    if let (Some(worker), Ok(hwnd)) = (WORKER.get(), window.hwnd()) {
        let _ = worker.send(Request::Geometry(hwnd.0 as isize, rects));
    }
}
unsafe fn worker(requests: mpsc::Receiver<Request>) {
    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    let mut gpu: Option<Gpu> = None;
    let mut windows: HashMap<isize, GlassWindow> = HashMap::new();
    let mut handles: HashMap<isize, tauri::WebviewWindow> = HashMap::new();
    loop {
        let tick = Instant::now();
        match requests.recv_timeout(if windows.is_empty() {
            Duration::from_secs(60)
        } else {
            Duration::from_millis(16)
        }) {
            Ok(Request::Apply {
                hwnd,
                dark,
                window,
                reply,
            }) => {
                handles.insert(hwnd, window);
                let result = (|| -> Result<(), String> {
                    if gpu.is_none() {
                        gpu = Some(Gpu::new().map_err(|e| e.to_string())?);
                    }
                    if let Some(window) = windows.get_mut(&hwnd) {
                        window.dark = dark;
                        window.dirty = true;
                    } else {
                        windows.insert(
                            hwnd,
                            GlassWindow::new(gpu.as_ref().unwrap(), HWND(hwnd as _), dark)
                                .map_err(|e| e.to_string())?,
                        );
                    }
                    // A material request succeeds only after a shader frame is presented.
                    windows
                        .get_mut(&hwnd)
                        .unwrap()
                        .render(gpu.as_mut().unwrap())
                        .map_err(|e| e.to_string())?;
                    Ok(())
                })();
                if result.is_err() {
                    windows.remove(&hwnd);
                    handles.remove(&hwnd);
                }
                let _ = reply.send(result);
            }
            Ok(Request::Geometry(hwnd, rects)) => {
                if let Some(window) = windows.get_mut(&hwnd) {
                    window.rects = rects;
                    window.dirty = true;
                }
            }
            Ok(Request::Remove(hwnd)) => {
                windows.remove(&hwnd);
                handles.remove(&hwnd);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if let Some(gpu) = gpu.as_mut() {
            if gpu.wallpaper_check.elapsed() >= Duration::from_secs(5) && !windows.is_empty() {
                gpu.wallpaper_check = Instant::now();
                match wallpaper_fingerprint().and_then(|signature| {
                    if signature != gpu.wallpaper_signature {
                        gpu.wallpapers = gpu.load_wallpapers()?;
                        gpu.wallpaper_signature = signature;
                        for window in windows.values_mut() {
                            window.dirty = true;
                        }
                    }
                    Ok(())
                }) {
                    Ok(()) => {}
                    Err(error) => {
                        for (&id, _) in &windows {
                            if let Some(handle) = handles.get(&id) {
                                let _ = handle.emit("deadline-material-failed", error.to_string());
                            }
                        }
                        windows.clear();
                        handles.clear();
                    }
                }
            }
            let mut failed = Vec::new();
            for (&id, window) in &mut windows {
                if IsWindowVisible(window.hwnd).as_bool() && !IsIconic(window.hwnd).as_bool() {
                    if let Err(error) = window.render(gpu) {
                        eprintln!("GPU glass frame failed: {error}");
                        if let Some(handle) = handles.get(&id) {
                            let _ = handle.emit("deadline-material-failed", error.to_string());
                        }
                        failed.push(id);
                    }
                }
            }
            for id in failed {
                windows.remove(&id);
                handles.remove(&id);
            }
            let active: HashSet<isize> = windows
                .values()
                .flat_map(|window| window.sources.iter().copied())
                .collect();
            gpu.sources.retain(|id, _| active.contains(id));
        }
        if windows.is_empty() {
            gpu = None;
        }
        // One bounded worker drives all windows; no timers or COM work on UI thread.
        let budget = Duration::from_millis(16);
        if tick.elapsed() < budget && !windows.is_empty() {
            std::thread::sleep(budget - tick.elapsed());
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Parameters {
    surface: [f32; 4],
    origin: [f32; 4],
    layer: [f32; 4],
    clip: [f32; 4],
    axis: [f32; 4],
}
struct Texture {
    _texture: ID3D11Texture2D,
    view: ID3D11ShaderResourceView,
    target: Option<ID3D11RenderTargetView>,
    width: u32,
    height: u32,
}
struct Source {
    handle: isize,
    update: u64,
    shared: ID3D11Texture2D,
    image: Texture,
}
struct Wallpaper {
    image: Texture,
    rect: RECT,
    mode: DESKTOP_WALLPAPER_POSITION,
}
struct Gpu {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    composition: IDCompositionDesktopDevice,
    factory: IDXGIFactory2,
    vertex: ID3D11VertexShader,
    compose: ID3D11PixelShader,
    blur: ID3D11PixelShader,
    glass: ID3D11PixelShader,
    constants: ID3D11Buffer,
    sampler: ID3D11SamplerState,
    blend: ID3D11BlendState,
    shared_surface: SharedSurface,
    sources: HashMap<isize, Source>,
    wallpapers: Vec<Wallpaper>,
    wallpaper_signature: u64,
    wallpaper_check: Instant,
}
unsafe fn wallpaper_fingerprint() -> windows::core::Result<u64> {
    let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    desktop.GetPosition()?.0.hash(&mut hash);
    for index in 0..desktop.GetMonitorDevicePathCount()? {
        let monitor = desktop.GetMonitorDevicePathAt(index)?;
        let rect = desktop.GetMonitorRECT(PCWSTR(monitor.0))?;
        let path = desktop.GetWallpaper(PCWSTR(monitor.0))?;
        let text = path.to_string()?;
        CoTaskMemFree(Some(monitor.0 as _));
        CoTaskMemFree(Some(path.0 as _));
        text.hash(&mut hash);
        [rect.left, rect.top, rect.right, rect.bottom].hash(&mut hash);
        std::fs::metadata(text)
            .ok()
            .and_then(|m| m.modified().ok())
            .hash(&mut hash);
    }
    Ok(hash.finish())
}
fn failure(message: impl Into<String>) -> windows::core::Error {
    windows::core::Error::new(HRESULT(0x80004005u32 as i32), message.into())
}
unsafe fn shader(
    entry: windows::core::PCSTR,
    target: windows::core::PCSTR,
) -> windows::core::Result<Vec<u8>> {
    let source = include_str!("glass.hlsl");
    let mut code = None;
    let mut errors = None;
    if let Err(error) = D3DCompile(
        source.as_ptr() as _,
        source.len(),
        s!("glass.hlsl"),
        None,
        None,
        entry,
        target,
        1 << 15,
        0,
        &mut code,
        Some(&mut errors),
    ) {
        let detail = errors
            .map(|blob| {
                String::from_utf8_lossy(std::slice::from_raw_parts(
                    blob.GetBufferPointer() as *const u8,
                    blob.GetBufferSize(),
                ))
                .into_owned()
            })
            .unwrap_or_else(|| error.to_string());
        return Err(failure(detail));
    }
    let code = code.ok_or_else(|| failure("Shader compiler returned no bytecode"))?;
    Ok(
        std::slice::from_raw_parts(code.GetBufferPointer() as *const u8, code.GetBufferSize())
            .to_vec(),
    )
}
impl Gpu {
    unsafe fn new() -> windows::core::Result<Self> {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            windows::Win32::Foundation::HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )?;
        let device = device.unwrap();
        let context = context.unwrap();
        let dxgi: IDXGIDevice = device.cast()?;
        let composition: IDCompositionDesktopDevice = DCompositionCreateDevice3(&dxgi)?;
        let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
        let mut vertex = None;
        device.CreateVertexShader(
            &shader(s!("fullscreen"), s!("vs_4_0"))?,
            None,
            Some(&mut vertex),
        )?;
        let mut compose = None;
        device.CreatePixelShader(
            &shader(s!("compose"), s!("ps_4_0"))?,
            None,
            Some(&mut compose),
        )?;
        let mut blur = None;
        device.CreatePixelShader(&shader(s!("blur"), s!("ps_4_0"))?, None, Some(&mut blur))?;
        let mut glass = None;
        device.CreatePixelShader(&shader(s!("glass"), s!("ps_4_0"))?, None, Some(&mut glass))?;
        let mut constants = None;
        device.CreateBuffer(
            &D3D11_BUFFER_DESC {
                ByteWidth: std::mem::size_of::<Parameters>() as _,
                Usage: D3D11_USAGE_DEFAULT,
                BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as _,
                ..Default::default()
            },
            None,
            Some(&mut constants),
        )?;
        let mut sampler = None;
        device.CreateSamplerState(
            &D3D11_SAMPLER_DESC {
                Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                MaxLOD: f32::MAX,
                ..Default::default()
            },
            Some(&mut sampler),
        )?;
        let mut blend = None;
        let mut blend_desc = D3D11_BLEND_DESC::default();
        blend_desc.RenderTarget[0] = D3D11_RENDER_TARGET_BLEND_DESC {
            BlendEnable: BOOL(1),
            SrcBlend: D3D11_BLEND_ONE,
            DestBlend: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOp: D3D11_BLEND_OP_ADD,
            SrcBlendAlpha: D3D11_BLEND_ONE,
            DestBlendAlpha: D3D11_BLEND_INV_SRC_ALPHA,
            BlendOpAlpha: D3D11_BLEND_OP_ADD,
            RenderTargetWriteMask: D3D11_COLOR_WRITE_ENABLE_ALL.0 as _,
        };
        device.CreateBlendState(&blend_desc, Some(&mut blend))?;
        let module = GetModuleHandleW(w!("user32.dll"))?;
        let shared_surface = std::mem::transmute(
            GetProcAddress(module, s!("DwmGetDxSharedSurface"))
                .ok_or_else(|| failure("DWM shared surface API unavailable"))?,
        );
        let mut gpu = Self {
            device,
            context,
            composition,
            factory,
            vertex: vertex.unwrap(),
            compose: compose.unwrap(),
            blur: blur.unwrap(),
            glass: glass.unwrap(),
            constants: constants.unwrap(),
            sampler: sampler.unwrap(),
            blend: blend.unwrap(),
            shared_surface,
            sources: HashMap::new(),
            wallpapers: Vec::new(),
            wallpaper_signature: wallpaper_fingerprint()?,
            wallpaper_check: Instant::now(),
        };
        gpu.wallpapers = gpu.load_wallpapers()?;
        Ok(gpu)
    }
    unsafe fn texture(
        &self,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        target: bool,
        pixels: Option<&[u8]>,
    ) -> windows::core::Result<Texture> {
        let descriptor = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_SHADER_RESOURCE.0
                | if target {
                    D3D11_BIND_RENDER_TARGET.0
                } else {
                    0
                }) as _,
            ..Default::default()
        };
        let data = pixels.map(|pixels| D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr() as _,
            SysMemPitch: width * 4,
            ..Default::default()
        });
        let mut texture = None;
        self.device.CreateTexture2D(
            &descriptor,
            data.as_ref().map(|data| data as *const _),
            Some(&mut texture),
        )?;
        let texture = texture.unwrap();
        let mut view = None;
        self.device
            .CreateShaderResourceView(&texture, None, Some(&mut view))?;
        let mut rtv = None;
        if target {
            self.device
                .CreateRenderTargetView(&texture, None, Some(&mut rtv))?;
        }
        Ok(Texture {
            _texture: texture,
            view: view.unwrap(),
            target: rtv,
            width,
            height,
        })
    }
    unsafe fn load_wallpapers(&self) -> windows::core::Result<Vec<Wallpaper>> {
        let desktop: IDesktopWallpaper = CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL)?;
        let imaging: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
        let mode = desktop.GetPosition()?;
        let mut wallpapers = Vec::new();
        for index in 0..desktop.GetMonitorDevicePathCount()? {
            let monitor = desktop.GetMonitorDevicePathAt(index)?;
            let rect = desktop.GetMonitorRECT(PCWSTR(monitor.0))?;
            let path = desktop.GetWallpaper(PCWSTR(monitor.0))?;
            let path_text = path.to_string()?;
            CoTaskMemFree(Some(monitor.0 as _));
            CoTaskMemFree(Some(path.0 as _));
            if path_text.is_empty() {
                continue;
            }
            let filename: Vec<u16> = path_text.encode_utf16().chain(Some(0)).collect();
            let frame = imaging
                .CreateDecoderFromFilename(
                    PCWSTR(filename.as_ptr()),
                    None,
                    GENERIC_READ,
                    WICDecodeMetadataCacheOnLoad,
                )?
                .GetFrame(0)?;
            let converted = imaging.CreateFormatConverter()?;
            converted.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            let mut width = 0;
            let mut height = 0;
            converted.GetSize(&mut width, &mut height)?;
            if width == 0 || height == 0 || width > 16384 || height > 16384 {
                return Err(failure("Unsupported wallpaper dimensions"));
            }
            let mut pixels = vec![0u8; width as usize * height as usize * 4];
            converted.CopyPixels(std::ptr::null(), width * 4, &mut pixels)?;
            wallpapers.push(Wallpaper {
                image: self.texture(
                    width,
                    height,
                    DXGI_FORMAT_B8G8R8A8_UNORM,
                    false,
                    Some(&pixels),
                )?,
                rect,
                mode,
            });
        }
        if wallpapers.is_empty() {
            return Err(failure("No readable desktop wallpaper; use theme fallback"));
        }
        Ok(wallpapers)
    }
    unsafe fn draw(
        &self,
        target: &ID3D11RenderTargetView,
        width: u32,
        height: u32,
        input: &ID3D11ShaderResourceView,
        shader: &ID3D11PixelShader,
        parameters: &Parameters,
        blend: bool,
    ) {
        self.context.PSSetShaderResources(0, Some(&[None]));
        self.context
            .OMSetRenderTargets(Some(&[Some(target.clone())]), None);
        self.context
            .OMSetBlendState(if blend { Some(&self.blend) } else { None }, None, u32::MAX);
        self.context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: width as _,
            Height: height as _,
            MinDepth: 0.0,
            MaxDepth: 1.0,
            ..Default::default()
        }]));
        self.context
            .UpdateSubresource(&self.constants, 0, None, parameters as *const _ as _, 0, 0);
        self.context.VSSetShader(&self.vertex, None);
        self.context.PSSetShader(shader, None);
        self.context
            .IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
        self.context
            .PSSetConstantBuffers(0, Some(&[Some(self.constants.clone())]));
        self.context
            .PSSetSamplers(0, Some(&[Some(self.sampler.clone())]));
        self.context
            .PSSetShaderResources(0, Some(&[Some(input.clone())]));
        self.context.Draw(3, 0);
        self.context.PSSetShaderResources(0, Some(&[None]));
    }
    unsafe fn source(&mut self, hwnd: HWND) -> windows::core::Result<bool> {
        let mut affinity = 0;
        if GetWindowDisplayAffinity(hwnd, &mut affinity).is_ok() && affinity != 0 {
            return Err(failure("Protected background window omitted"));
        }
        let mut handle = HANDLE::default();
        let mut luid = LUID::default();
        let mut format = 0;
        let mut flags = 0;
        let mut update = 0;
        if !(self.shared_surface)(
            hwnd,
            &mut handle,
            &mut luid,
            &mut format,
            &mut flags,
            &mut update,
        )
        .as_bool()
            || handle.is_invalid()
        {
            return Err(failure("Background surface unavailable"));
        }
        let id = hwnd.0 as isize;
        if self
            .sources
            .get(&id)
            .is_some_and(|source| source.handle == handle.0 as isize)
        {
            let source = self.sources.get_mut(&id).unwrap();
            let changed = update == 0 || source.update != update;
            if changed {
                self.context
                    .CopyResource(&source.image._texture, &source.shared);
                source.update = update;
            }
            return Ok(changed);
        }
        let mut shared: Option<ID3D11Texture2D> = None;
        self.device.OpenSharedResource(handle, &mut shared)?;
        let shared = shared.ok_or_else(|| failure("DWM surface returned no texture"))?;
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        shared.GetDesc(&mut desc);
        if desc.Width == 0 || desc.Height == 0 {
            return Err(failure("Empty background surface"));
        }
        let image = self.texture(desc.Width, desc.Height, desc.Format, false, None)?;
        self.context.CopyResource(&image._texture, &shared);
        // Handle belongs to DWM. Never close it or reopen unchanged resources.
        self.sources.insert(
            id,
            Source {
                handle: handle.0 as isize,
                update,
                shared,
                image,
            },
        );
        Ok(true)
    }
}
struct GlassWindow {
    hwnd: HWND,
    dark: bool,
    dirty: bool,
    rect: RECT,
    dpi: f32,
    target: IDCompositionTarget,
    composition: IDCompositionDesktopDevice,
    _visual: IDCompositionVisual2,
    swap: IDXGISwapChain1,
    background: Texture,
    horizontal: Texture,
    vertical: Texture,
    output: ID3D11RenderTargetView,
    sources: Vec<isize>,
    layers: Vec<(isize, RECT)>,
    presented: u64,
    rects: Vec<[f32; 5]>,
}
impl Drop for GlassWindow {
    fn drop(&mut self) {
        unsafe {
            let _ = self.target.SetRoot(None);
            let _ = self.composition.Commit();
        }
    }
}
unsafe fn bounds(hwnd: HWND) -> windows::core::Result<RECT> {
    let mut rect = RECT::default();
    GetWindowRect(hwnd, &mut rect)?;
    Ok(rect)
}
fn intersects(a: RECT, b: RECT) -> bool {
    a.left < b.right && b.left < a.right && a.top < b.bottom && b.top < a.bottom
}
fn same_rect(a: RECT, b: RECT) -> bool {
    a.left == b.left && a.top == b.top && a.right == b.right && a.bottom == b.bottom
}
fn array(rect: RECT) -> [f32; 4] {
    [
        rect.left as _,
        rect.top as _,
        rect.right as _,
        rect.bottom as _,
    ]
}
unsafe extern "system" fn collect(hwnd: HWND, data: windows::Win32::Foundation::LPARAM) -> BOOL {
    (*(data.0 as *mut Vec<HWND>)).push(hwnd);
    BOOL(1)
}
unsafe fn behind(hwnd: HWND, rect: RECT) -> Vec<(isize, RECT)> {
    let mut ordered = Vec::new();
    let _ = EnumWindows(
        Some(collect),
        windows::Win32::Foundation::LPARAM(&mut ordered as *mut _ as _),
    );
    let mut below = false;
    let mut layers = Vec::new();
    let pid = GetCurrentProcessId();
    for source in ordered {
        if source == hwnd {
            below = true;
            continue;
        }
        if !below || !IsWindowVisible(source).as_bool() || IsIconic(source).as_bool() {
            continue;
        }
        let mut source_pid = 0;
        GetWindowThreadProcessId(source, Some(&mut source_pid));
        if source_pid == pid {
            continue;
        }
        let mut class = [0u16; 128];
        let count = GetClassNameW(source, &mut class) as usize;
        let class = String::from_utf16_lossy(&class[..count]);
        if class == "Progman" || class == "WorkerW" {
            continue;
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(
            source,
            DWMWINDOWATTRIBUTE(14),
            &mut cloaked as *mut _ as _,
            4,
        );
        if cloaked != 0 {
            continue;
        }
        if let Ok(source_rect) = bounds(source) {
            if intersects(rect, source_rect) {
                layers.push((source.0 as isize, source_rect));
            }
        }
    }
    layers.reverse();
    layers
}
impl GlassWindow {
    unsafe fn new(gpu: &Gpu, hwnd: HWND, dark: bool) -> windows::core::Result<Self> {
        let rect = bounds(hwnd)?;
        let width = (rect.right - rect.left).max(1) as u32;
        let height = (rect.bottom - rect.top).max(1) as u32;
        let dpi = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
        let margin = (16.0 * dpi).ceil() as u32;
        let swap = gpu.factory.CreateSwapChainForComposition(
            &gpu.device,
            &DXGI_SWAP_CHAIN_DESC1 {
                Width: width,
                Height: height,
                Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                SampleDesc: DXGI_SAMPLE_DESC {
                    Count: 1,
                    Quality: 0,
                },
                BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                BufferCount: 2,
                Scaling: DXGI_SCALING_STRETCH,
                SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                AlphaMode: DXGI_ALPHA_MODE_PREMULTIPLIED,
                ..Default::default()
            },
            None,
        )?;
        let target = gpu.composition.CreateTargetForHwnd(hwnd, false)?;
        let visual = gpu.composition.CreateVisual()?;
        visual.SetContent(&swap)?;
        target.SetRoot(&visual)?;
        gpu.composition.Commit()?;
        let buffer: ID3D11Texture2D = swap.GetBuffer(0)?;
        let mut output = None;
        gpu.device
            .CreateRenderTargetView(&buffer, None, Some(&mut output))?;
        let background = gpu.texture(
            width + margin * 2,
            height + margin * 2,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            true,
            None,
        )?;
        let horizontal = gpu.texture(
            width + margin * 2,
            height + margin * 2,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            true,
            None,
        )?;
        let vertical = gpu.texture(
            width + margin * 2,
            height + margin * 2,
            DXGI_FORMAT_B8G8R8A8_UNORM,
            true,
            None,
        )?;
        Ok(Self {
            hwnd,
            composition: gpu.composition.clone(),
            dark,
            dirty: true,
            rect,
            dpi,
            target,
            _visual: visual,
            swap,
            background,
            horizontal,
            vertical,
            output: output.unwrap(),
            sources: Vec::new(),
            layers: Vec::new(),
            presented: 0,
            rects: Vec::new(),
        })
    }
    unsafe fn render(&mut self, gpu: &mut Gpu) -> windows::core::Result<()> {
        let rect = bounds(self.hwnd)?;
        let width = (rect.right - rect.left).max(1) as u32;
        let height = (rect.bottom - rect.top).max(1) as u32;
        let dpi = GetDpiForWindow(self.hwnd).max(96) as f32 / 96.0;
        let margin = (16.0 * dpi).ceil() as u32;
        if width != self.background.width - margin * 2
            || height != self.background.height - margin * 2
            || dpi != self.dpi
        {
            // The app HWND is untouched; only GPU buffers follow its dimensions.
            gpu.context.ClearState();
            // Release the previous back-buffer view before ResizeBuffers.
            let temporary = gpu.texture(1, 1, DXGI_FORMAT_B8G8R8A8_UNORM, true, None)?;
            self.output = temporary.target.unwrap();
            self.swap.ResizeBuffers(
                2,
                width,
                height,
                DXGI_FORMAT_UNKNOWN,
                DXGI_SWAP_CHAIN_FLAG(0),
            )?;
            let buffer: ID3D11Texture2D = self.swap.GetBuffer(0)?;
            let mut output = None;
            gpu.device
                .CreateRenderTargetView(&buffer, None, Some(&mut output))?;
            self.output = output.unwrap();
            self.background = gpu.texture(
                width + margin * 2,
                height + margin * 2,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                true,
                None,
            )?;
            self.horizontal = gpu.texture(
                width + margin * 2,
                height + margin * 2,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                true,
                None,
            )?;
            self.vertical = gpu.texture(
                width + margin * 2,
                height + margin * 2,
                DXGI_FORMAT_B8G8R8A8_UNORM,
                true,
                None,
            )?;
            self.dirty = true;
        }
        let sample_rect = RECT {
            left: rect.left - margin as i32,
            top: rect.top - margin as i32,
            right: rect.right + margin as i32,
            bottom: rect.bottom + margin as i32,
        };
        let layers = behind(self.hwnd, sample_rect);
        let mut dirty = self.dirty
            || !same_rect(self.rect, rect)
            || self.layers.len() != layers.len()
            || self
                .layers
                .iter()
                .zip(&layers)
                .any(|(a, b)| a.0 != b.0 || !same_rect(a.1, b.1));
        let previous_sources = std::mem::take(&mut self.sources);
        for &(source, _) in &layers {
            if let Ok(changed) = gpu.source(HWND(source as _)) {
                dirty |= changed;
                self.sources.push(source);
            }
        }
        dirty |= previous_sources != self.sources;
        self.layers = layers;
        self.rect = rect;
        self.dpi = dpi;
        if !dirty {
            return Ok(());
        }
        let mut p = Parameters {
            surface: [width as _, height as _, margin as _, dpi],
            origin: [
                rect.left as _,
                rect.top as _,
                14.0 * dpi,
                if self.dark { 1.0 } else { 0.0 },
            ],
            ..Default::default()
        };
        let background = self.background.target.as_ref().unwrap();
        gpu.context
            .ClearRenderTargetView(background, &[0.08, 0.08, 0.09, 1.0]);
        let mut desktop_rect = gpu.wallpapers[0].rect;
        for wallpaper in &gpu.wallpapers {
            desktop_rect.left = desktop_rect.left.min(wallpaper.rect.left);
            desktop_rect.top = desktop_rect.top.min(wallpaper.rect.top);
            desktop_rect.right = desktop_rect.right.max(wallpaper.rect.right);
            desktop_rect.bottom = desktop_rect.bottom.max(wallpaper.rect.bottom);
        }
        for wallpaper in &gpu.wallpapers {
            if !intersects(sample_rect, wallpaper.rect) {
                continue;
            }
            p.layer = wallpaper_placement(
                if wallpaper.mode == DWPOS_SPAN {
                    desktop_rect
                } else {
                    wallpaper.rect
                },
                wallpaper.image.width,
                wallpaper.image.height,
                wallpaper.mode,
            );
            p.clip = array(wallpaper.rect);
            p.axis[2] = if wallpaper.mode == DWPOS_TILE {
                2.0
            } else {
                0.0
            };
            gpu.draw(
                background,
                self.background.width,
                self.background.height,
                &wallpaper.image.view,
                &gpu.compose,
                &p,
                true,
            );
        }
        p.axis = [0.0; 4];
        for &(id, source_rect) in &self.layers {
            if !self.sources.contains(&id) {
                // Never reveal wallpaper through an unavailable/protected window.
                p.clip = array(source_rect);
                p.axis = [0.0, 0.0, 1.0, if self.dark { 1.0 } else { 0.0 }];
                gpu.draw(
                    background,
                    self.background.width,
                    self.background.height,
                    &gpu.wallpapers[0].image.view,
                    &gpu.compose,
                    &p,
                    true,
                );
                p.axis = [0.0; 4];
                continue;
            }
            if let Some(source) = gpu.sources.get(&id) {
                p.layer = [
                    source_rect.left as _,
                    source_rect.top as _,
                    (source_rect.right - source_rect.left) as _,
                    (source_rect.bottom - source_rect.top) as _,
                ];
                p.clip = array(source_rect);
                gpu.draw(
                    background,
                    self.background.width,
                    self.background.height,
                    &source.image.view,
                    &gpu.compose,
                    &p,
                    true,
                );
            }
        }
        p.axis = [2.0 * dpi / self.background.width as f32, 0.0, 0.0, 0.0];
        gpu.draw(
            self.horizontal.target.as_ref().unwrap(),
            self.background.width,
            self.background.height,
            &self.background.view,
            &gpu.blur,
            &p,
            false,
        );
        p.axis = [0.0, 2.0 * dpi / self.background.height as f32, 0.0, 0.0];
        gpu.draw(
            self.vertical.target.as_ref().unwrap(),
            self.background.width,
            self.background.height,
            &self.horizontal.view,
            &gpu.blur,
            &p,
            false,
        );
        gpu.context
            .ClearRenderTargetView(&self.output, &[0.0, 0.0, 0.0, 0.0]);
        for rect in &self.rects {
            p.layer = [rect[0] * dpi, rect[1] * dpi, rect[2] * dpi, rect[3] * dpi];
            p.origin[2] = rect[4] * dpi;
            gpu.draw(
                &self.output,
                width,
                height,
                &self.vertical.view,
                &gpu.glass,
                &p,
                true,
            );
        }
        self.swap.Present(0, DXGI_PRESENT(0)).ok()?;
        self.dirty = false;
        self.presented += 1;
        if self.presented == 1 {
            eprintln!(
                "GPU glass ready: {}x{}, {} sampled windows, DPI {:.2}",
                width,
                height,
                self.sources.len(),
                dpi
            );
        }
        Ok(())
    }
}
fn wallpaper_placement(
    rect: RECT,
    width: u32,
    height: u32,
    mode: DESKTOP_WALLPAPER_POSITION,
) -> [f32; 4] {
    let w = (rect.right - rect.left) as f32;
    let h = (rect.bottom - rect.top) as f32;
    if mode == DWPOS_TILE {
        return [rect.left as _, rect.top as _, width as _, height as _];
    }
    if mode == DWPOS_STRETCH {
        return [rect.left as _, rect.top as _, w, h];
    }
    let scale = if mode == DWPOS_CENTER || mode == DWPOS_TILE {
        1.0
    } else if mode == DWPOS_FIT {
        (w / width as f32).min(h / height as f32)
    } else {
        (w / width as f32).max(h / height as f32)
    };
    let image_w = width as f32 * scale;
    let image_h = height as f32 * scale;
    [
        rect.left as f32 + (w - image_w) * 0.5,
        rect.top as f32 + (h - image_h) * 0.5,
        image_w,
        image_h,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe fn test_pixel(gpu: &Gpu, texture: &Texture, x: u32, y: u32) -> [u8; 4] {
        let mut desc = D3D11_TEXTURE2D_DESC::default();
        texture._texture.GetDesc(&mut desc);
        desc.Usage = D3D11_USAGE_STAGING;
        desc.BindFlags = 0;
        desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as _;
        let mut staging = None;
        gpu.device
            .CreateTexture2D(&desc, None, Some(&mut staging))
            .unwrap();
        let staging = staging.unwrap();
        gpu.context.CopyResource(&staging, &texture._texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        gpu.context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .unwrap();
        let mut result = [0; 4];
        result.copy_from_slice(std::slice::from_raw_parts(
            (mapped.pData as *const u8).add((y * mapped.RowPitch + x * 4) as usize),
            4,
        ));
        gpu.context.Unmap(&staging, 0);
        result
    }
    #[test]
    fn gpu_refraction_bends_background_only_near_card_edge() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let gpu = Gpu::new().unwrap();
            let mut pixels = vec![0u8; 232 * 132 * 4];
            for y in 0..132 {
                for x in 0..232 {
                    let value = if x < 20 { 255 } else { 0 };
                    pixels[(y * 232 + x) * 4..(y * 232 + x) * 4 + 4]
                        .copy_from_slice(&[value, value, value, 255]);
                }
            }
            let input = gpu
                .texture(232, 132, DXGI_FORMAT_B8G8R8A8_UNORM, false, Some(&pixels))
                .unwrap();
            let output = gpu
                .texture(200, 100, DXGI_FORMAT_B8G8R8A8_UNORM, true, None)
                .unwrap();
            let mut p = Parameters {
                surface: [200., 100., 16., 1.],
                origin: [0., 0., 14., 0.],
                layer: [0., 0., 200., 100.],
                ..Default::default()
            };
            gpu.context
                .ClearRenderTargetView(output.target.as_ref().unwrap(), &[0.; 4]);
            gpu.draw(
                output.target.as_ref().unwrap(),
                200,
                100,
                &input.view,
                &gpu.glass,
                &p,
                false,
            );
            let refracted = test_pixel(&gpu, &output, 2, 50);
            let center = test_pixel(&gpu, &output, 100, 50);
            p.axis[2] = -1.;
            gpu.draw(
                output.target.as_ref().unwrap(),
                200,
                100,
                &input.view,
                &gpu.glass,
                &p,
                false,
            );
            let flat = test_pixel(&gpu, &output, 2, 50);
            let flat_center = test_pixel(&gpu, &output, 100, 50);
            assert!(
                flat[1] as i32 - refracted[1] as i32 > 40,
                "Edge must displace the actual background sample"
            );
            assert_eq!(center, flat_center, "Text-area background must not wobble");
            p.axis[2] = 0.;
            p.layer = [20., 10., 100., 60.];
            gpu.context
                .ClearRenderTargetView(output.target.as_ref().unwrap(), &[0.; 4]);
            gpu.draw(
                output.target.as_ref().unwrap(),
                200,
                100,
                &input.view,
                &gpu.glass,
                &p,
                false,
            );
            assert_eq!(
                test_pixel(&gpu, &output, 5, 50)[3],
                0,
                "Outside the DOM card must stay transparent"
            );
            assert_eq!(test_pixel(&gpu, &output, 60, 40)[3], 255);
        }
    }
    #[test]
    fn wallpaper_fill_preserves_aspect_and_center() {
        let p = wallpaper_placement(
            RECT {
                left: 100,
                top: 50,
                right: 500,
                bottom: 350,
            },
            800,
            400,
            windows::Win32::UI::Shell::DWPOS_FILL,
        );
        assert_eq!(p, [0.0, 50.0, 600.0, 300.0]);
    }
    #[test]
    fn shader_compiles_on_real_d3d_compiler() {
        unsafe {
            for (entry, target) in [
                (s!("fullscreen"), s!("vs_4_0")),
                (s!("compose"), s!("ps_4_0")),
                (s!("blur"), s!("ps_4_0")),
                (s!("glass"), s!("ps_4_0")),
            ] {
                assert!(!shader(entry, target).unwrap().is_empty());
            }
        }
    }
    #[test]
    fn gpu_pixels_are_blurred_tinted_and_round() {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let gpu = Gpu::new().unwrap();
            let (w, h, margin) = (200u32, 100u32, 16u32);
            let (tw, th) = (w + 2 * margin, h + 2 * margin);
            let mut pixels = vec![0u8; (tw * th * 4) as usize];
            for y in 0..th {
                for x in 0..tw {
                    let index = ((y * tw + x) * 4) as usize;
                    let value = if (x / 2) % 2 == 0 { 255 } else { 0 };
                    pixels[index..index + 4].copy_from_slice(&[value, value, value, 255]);
                }
            }
            let input = gpu
                .texture(tw, th, DXGI_FORMAT_B8G8R8A8_UNORM, false, Some(&pixels))
                .unwrap();
            let horizontal = gpu
                .texture(tw, th, DXGI_FORMAT_B8G8R8A8_UNORM, true, None)
                .unwrap();
            let vertical = gpu
                .texture(tw, th, DXGI_FORMAT_B8G8R8A8_UNORM, true, None)
                .unwrap();
            let output = gpu
                .texture(w, h, DXGI_FORMAT_B8G8R8A8_UNORM, true, None)
                .unwrap();
            let mut p = Parameters {
                surface: [w as _, h as _, margin as _, 1.0],
                origin: [0.0, 0.0, 14.0, 1.0],
                layer: [0.0, 0.0, w as _, h as _],
                axis: [2.0 / tw as f32, 0.0, 0.0, 0.0],
                ..Default::default()
            };
            gpu.draw(
                horizontal.target.as_ref().unwrap(),
                tw,
                th,
                &input.view,
                &gpu.blur,
                &p,
                false,
            );
            p.axis = [0.0, 2.0 / th as f32, 0.0, 0.0];
            gpu.draw(
                vertical.target.as_ref().unwrap(),
                tw,
                th,
                &horizontal.view,
                &gpu.blur,
                &p,
                false,
            );
            gpu.context
                .ClearRenderTargetView(output.target.as_ref().unwrap(), &[0.0; 4]);
            gpu.draw(
                output.target.as_ref().unwrap(),
                w,
                h,
                &vertical.view,
                &gpu.glass,
                &p,
                false,
            );
            let mut descriptor = D3D11_TEXTURE2D_DESC::default();
            output._texture.GetDesc(&mut descriptor);
            descriptor.Usage = D3D11_USAGE_STAGING;
            descriptor.BindFlags = 0;
            descriptor.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as _;
            let mut staging = None;
            gpu.device
                .CreateTexture2D(&descriptor, None, Some(&mut staging))
                .unwrap();
            let staging = staging.unwrap();
            gpu.context.CopyResource(&staging, &output._texture);
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            gpu.context
                .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .unwrap();
            let pixel = |x: u32, y: u32| -> Vec<u8> {
                std::slice::from_raw_parts(
                    (mapped.pData as *const u8).add((y * mapped.RowPitch + x * 4) as usize),
                    4,
                )
                .to_vec()
            };
            let corner = pixel(0, 0);
            let center = pixel(100, 50);
            let neighbor = pixel(102, 50);
            gpu.context.Unmap(&staging, 0);
            assert_eq!(corner[3], 0, "Corner must actually be transparent");
            assert_eq!(center[3], 255, "Body must contain the rendered material");
            assert!(
                (center[0] as i32 - neighbor[0] as i32).abs() < 12,
                "Blur must suppress fine background stripes"
            );
            assert!(
                (20..140).contains(&center[0]),
                "Dark tint must retain color and readability"
            );
        }
    }
}
