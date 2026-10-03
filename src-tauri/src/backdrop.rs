//! Development probe: clipped, lightly blurred, spatially warped live backdrop.
//! Dynamic DWM private exports are build-dependent; never treat creation as
//! visual acceptance. No capture exclusion or HWND region is used.
use std::{cell::RefCell, collections::HashMap, ffi::c_void};
use windows::{core::{Interface, HRESULT, PCSTR, w}, Win32::{Foundation::{HWND, RECT, SIZE}, Graphics::{DirectComposition::*, Direct3D::D3D_DRIVER_TYPE_HARDWARE, Direct3D11::*, Direct2D::Common::D2D1_BORDER_MODE_HARD, Dxgi::IDXGIDevice}, System::LibraryLoader::{GetModuleHandleW, GetProcAddress}, UI::WindowsAndMessaging::IsWindowVisible}};
use tauri::Manager;

type Create = unsafe extern "system" fn(HWND, *mut c_void, *mut *mut c_void, *mut isize) -> HRESULT;
type UpdateOld = unsafe extern "system" fn(isize, *const HWND, u32, *const HWND, u32, *mut RECT, *mut SIZE) -> HRESULT;
type UpdateNew = unsafe extern "system" fn(isize, *const HWND, u32, *const HWND, u32, *mut RECT, *mut SIZE, u32) -> HRESULT;
#[repr(C)]
struct Version { size:u32, major:u32, minor:u32, build:u32, platform:u32, text:[u16;128] }
struct Layer { visual: IDCompositionVisual2, clip:IDCompositionRectangleClip, thumbnail:isize, y:f32 }
impl Drop for Layer {
    fn drop(&mut self) { if self.thumbnail != 0 { let _=unsafe {windows::Win32::Graphics::Dwm::DwmUnregisterThumbnail(self.thumbnail)}; } }
}
struct Backdrop { device:IDCompositionDesktopDevice, _graphics:IDCompositionDevice3, _target:IDCompositionTarget, root:IDCompositionVisual2, clip:IDCompositionRectangleClip, layers:Vec<Layer>, update:usize, modern:bool, window:tauri::WebviewWindow, size:(u32,u32), position:(i32,i32) }
thread_local! { static BACKDROPS:RefCell<HashMap<String,Backdrop>> = RefCell::new(HashMap::new()); }

unsafe fn update_source(entry:usize, modern:bool, thumbnail:isize, exclude:&[HWND]) -> windows::core::Result<(RECT,SIZE)> {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics,SM_CXSCREEN,SM_CYSCREEN};
    let mut size=SIZE{cx:GetSystemMetrics(SM_CXSCREEN),cy:GetSystemMetrics(SM_CYSCREEN)};
    let mut rect=RECT{left:0,top:0,right:size.cx,bottom:size.cy};
    let _=exclude;
    let exclude=[HWND::default()];
    if modern { let call:UpdateNew=std::mem::transmute(entry); call(thumbnail,std::ptr::null(),0,exclude.as_ptr(),exclude.len() as _,&mut rect,&mut size,1).ok()?; }
    else { let call:UpdateOld=std::mem::transmute(entry); call(thumbnail,std::ptr::null(),0,exclude.as_ptr(),exclude.len() as _,&mut rect,&mut size).ok()?; }
    Ok((rect,size))
}

fn exclusions(window:&tauri::WebviewWindow)->Vec<HWND> {
    window.app_handle().webview_windows().values().filter_map(|item|item.hwnd().ok()).collect()
}

impl Backdrop {
    unsafe fn sync(&mut self, force:bool)->windows::core::Result<()> {
        let position=self.window.inner_position().map_err(|_|windows::core::Error::from_hresult(HRESULT(0x80004005u32 as i32)))?;
        let size=self.window.inner_size().map_err(|_|windows::core::Error::from_hresult(HRESULT(0x80004005u32 as i32)))?;
        if !force && self.position==(position.x,position.y) && self.size==(size.width,size.height) { return Ok(()); }
        let width=size.width as f32; let height=size.height as f32;
        let radius=14.0*self.window.scale_factor().unwrap_or(1.0) as f32;
        self.clip.SetLeft2(0.0)?;self.clip.SetTop2(0.0)?;self.clip.SetRight2(width)?;self.clip.SetBottom2(height)?;
        self.clip.SetTopLeftRadiusX2(radius)?;self.clip.SetTopLeftRadiusY2(radius)?;self.clip.SetTopRightRadiusX2(radius)?;self.clip.SetTopRightRadiusY2(radius)?;
        self.clip.SetBottomLeftRadiusX2(radius)?;self.clip.SetBottomLeftRadiusY2(radius)?;self.clip.SetBottomRightRadiusX2(radius)?;self.clip.SetBottomRightRadiusY2(radius)?;
        let exclude=exclusions(&self.window);
        let count=self.layers.len() as f32;
        for layer in &self.layers {
            let (source,_)=stage("update live backdrop",update_source(self.update,self.modern,layer.thumbnail,&exclude))?;
            let top=height*layer.y/count;let bottom=height*(layer.y+1.0)/count;
            layer.clip.SetLeft2(0.0)?;layer.clip.SetRight2(width)?;layer.clip.SetTop2(top)?;layer.clip.SetBottom2(bottom)?;
            let normalized=((top+bottom)*0.5/height)*2.0-1.0;
            // A shallow convex lens: horizontal magnification varies across
            // height; background vertical lines bend rather than merely blur.
            let scale=1.008+0.022*normalized.powi(4);
            let transform=self.device.CreateScaleTransform()?;
            transform.SetScaleX2(scale)?;transform.SetScaleY2(1.0)?;
            layer.visual.SetTransform(&transform)?;
            layer.visual.SetOffsetX2(width*0.5-scale*((position.x-source.left) as f32+width*0.5))?;
            layer.visual.SetOffsetY2((source.top-position.y) as f32)?;
        }
        self.device.Commit()?;
        self.position=(position.x,position.y);self.size=(size.width,size.height);
        Ok(())
    }
}

pub fn apply(window:&tauri::WebviewWindow)->Result<&'static str,String> {
    unsafe { create(window).map_err(|error|error.to_string()) }
}
fn stage<T>(name:&str,result:windows::core::Result<T>)->windows::core::Result<T> {
    result.map_err(|error|windows::core::Error::new(error.code(),format!("{name}: {error}")))
}
unsafe fn create(window:&tauri::WebviewWindow)->windows::core::Result<&'static str> {
    if BACKDROPS.with(|state|state.borrow().contains_key(window.label())) { BACKDROPS.with(|state|state.borrow_mut().get_mut(window.label()).unwrap().sync(true))?;return Ok("composition-glass"); }
    let dll=GetModuleHandleW(w!("dwmapi.dll"))?;
    let create:GetProc=GetProcAddress(dll,PCSTR(163usize as *const u8)).ok_or_else(windows::core::Error::from_win32)?;
    let update=GetProcAddress(dll,PCSTR(164usize as *const u8)).ok_or_else(windows::core::Error::from_win32)? as usize;
    let ntdll=GetModuleHandleW(w!("ntdll.dll"))?;
    let version=GetProcAddress(ntdll,windows::core::s!("RtlGetVersion")).ok_or_else(windows::core::Error::from_win32)?;
    let version:unsafe extern "system" fn(*mut Version)->i32=std::mem::transmute(version);
    let mut info=Version {size:std::mem::size_of::<Version>() as _,major:0,minor:0,build:0,platform:0,text:[0;128]};
    if version(&mut info)!=0 || info.build<19041 || info.build>=22000 { return Err(windows::core::Error::from_hresult(HRESULT(0x80004001u32 as i32))); }
    let mut d3d=None;
    D3D11CreateDevice(None,D3D_DRIVER_TYPE_HARDWARE,windows::Win32::Foundation::HMODULE::default(),D3D11_CREATE_DEVICE_BGRA_SUPPORT,None,D3D11_SDK_VERSION,Some(&mut d3d),None,None)?;
    let dxgi:IDXGIDevice=d3d.unwrap().cast()?;
    let device:IDCompositionDesktopDevice=stage("create composition device",DCompositionCreateDevice3(&dxgi))?;
    let graphics:IDCompositionDevice3=device.cast()?;
    let target=stage("bind target to WebView host",device.CreateTargetForHwnd(window.hwnd().map_err(|_|windows::core::Error::from_win32())?,false))?;
    let root:IDCompositionVisual2=graphics.CreateVisual()?.cast()?;
    let clip=device.CreateRectangleClip()?;root.SetClip(&clip)?;target.SetRoot(&root)?;
    let blur=graphics.CreateGaussianBlurEffect()?;blur.SetStandardDeviation2(2.0)?;blur.SetBorderMode(D2D1_BORDER_MODE_HARD)?;
    let make:Create=std::mem::transmute(create);
    let mut layers=Vec::new();
    for index in 0..20 {
        let mut raw=std::ptr::null_mut();let mut thumbnail=0;
        stage("create shared backdrop visual",make(window.hwnd().map_err(|_|windows::core::Error::from_win32())?,device.as_raw(),&mut raw,&mut thumbnail).ok())?;
        let visual:IDCompositionVisual2=IDCompositionVisual::from_raw(raw).cast()?;
        let wrapper:IDCompositionVisual2=graphics.CreateVisual()?.cast()?;
        let band=device.CreateRectangleClip()?;wrapper.SetClip(&band)?;wrapper.SetEffect(&blur)?;wrapper.AddVisual(&visual,false,None)?;root.AddVisual(&wrapper,true,None)?;
        layers.push(Layer{visual,clip:band,thumbnail,y:index as f32});
    }
    let mut backdrop=Backdrop{device,_graphics:graphics,_target:target,root,clip,layers,update,modern:true,window:window.clone(),size:(0,0),position:(0,0)};
    backdrop.sync(true)?;
    BACKDROPS.with(|state|state.borrow_mut().insert(window.label().into(),backdrop));
    Ok("composition-glass")
}
type GetProc=unsafe extern "system" fn()->isize;

pub fn refresh() {
    BACKDROPS.with(|state| for backdrop in state.borrow_mut().values_mut() {
        if let Ok(hwnd)=backdrop.window.hwnd() { if unsafe {IsWindowVisible(hwnd)}.as_bool() { let _=unsafe {backdrop.sync(false)}; } }
    });
}


