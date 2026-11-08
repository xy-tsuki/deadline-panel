//! DWM owns the real desktop backdrop; no application capture or render loop.
//! Win10 Accent is an undocumented compatibility API also used by window-vibrancy.
use std::{
    ffi::c_void,
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};
use windows::{
    core::{s, w, BOOL},
    Win32::{
        Foundation::HWND,
        Graphics::Dwm::{DwmIsCompositionEnabled, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE},
        System::LibraryLoader::{GetModuleHandleW, GetProcAddress},
    },
};
#[repr(C)]
struct AccentPolicy {
    state: u32,
    flags: u32,
    color: u32,
    animation: u32,
}
#[repr(C)]
struct CompositionAttribute {
    attribute: u32,
    data: *mut c_void,
    size: usize,
}
type SetComposition = unsafe extern "system" fn(HWND, *mut CompositionAttribute) -> BOOL;
static REQUESTS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
fn abgr(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    u32::from_le_bytes([red, green, blue, alpha])
}
pub fn diagnostics() -> serde_json::Value {
    serde_json::json!({"renderer":"translucent-no-blur","applicationBackgroundSampling":false,"applicationRenderWorker":false,"blurRadius":0,"nativeRequests":REQUESTS.load(Ordering::Relaxed)})
}
unsafe fn accent(hwnd: HWND, policy: &mut AccentPolicy) -> Result<(), String> {
    let library = GetModuleHandleW(w!("user32.dll")).map_err(|e| e.to_string())?;
    let entry = GetProcAddress(library, s!("SetWindowCompositionAttribute"))
        .ok_or_else(|| "Window composition API is unavailable".to_string())?;
    let set_composition: SetComposition = std::mem::transmute(entry);
    let mut data = CompositionAttribute {
        attribute: 19,
        data: policy as *mut _ as _,
        size: size_of::<AccentPolicy>(),
    };
    if !set_composition(hwnd, &mut data).as_bool() {
        return Err("DWM rejected the window blur policy".into());
    }
    Ok(())
}
pub fn apply(
    window: &tauri::WebviewWindow,
    _dark: bool,
    _low_power: bool,
) -> Result<&'static str, String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?;
    unsafe {
        if !DwmIsCompositionEnabled()
            .map_err(|e| e.to_string())?
            .as_bool()
        {
            return Err("Desktop composition is disabled".into());
        }
        REQUESTS.fetch_add(1, Ordering::Relaxed);
        let none: u32 = 1;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(38),
            &none as *const _ as _,
            size_of::<u32>() as u32,
        );
        accent(
            hwnd,
            &mut AccentPolicy {
                state: 0,
                flags: 0,
                color: 0,
                animation: 0,
            },
        )?;
        Ok("compatibility")
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accent_color_uses_abgr_and_correct_abi() {
        assert_eq!(abgr(24, 24, 26, 64), 0x401a1818);
        assert_eq!(abgr(244, 244, 246, 86), 0x56f6f4f4);
        assert_eq!(size_of::<AccentPolicy>(), 16);
        assert_eq!(
            size_of::<CompositionAttribute>(),
            if size_of::<usize>() == 8 { 24 } else { 12 }
        );
    }
}
