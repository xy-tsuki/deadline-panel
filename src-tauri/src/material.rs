//! Desktop blur belongs to the OS compositor; CSS only supplies the surface.
//! Windows 10 has no public system-backdrop attribute. Its compatibility API
//! is resolved dynamically and its BOOL is checked, unlike window-vibrancy 0.6.
use std::{ffi::c_void, mem::size_of};
use windows::{core::{s, w, BOOL}, Win32::{Foundation::HWND, Graphics::Dwm::{DwmIsCompositionEnabled, DwmSetWindowAttribute, DWMWINDOWATTRIBUTE}, System::LibraryLoader::{GetModuleHandleW, GetProcAddress}}};

#[repr(C)]
struct AccentPolicy { state: u32, flags: u32, color: u32, animation: u32 }
#[repr(C)]
struct CompositionAttribute { attribute: u32, data: *mut c_void, size: usize }
type SetComposition = unsafe extern "system" fn(HWND, *mut CompositionAttribute) -> BOOL;

fn abgr(red: u8, green: u8, blue: u8, alpha: u8) -> u32 {
    u32::from_le_bytes([red, green, blue, alpha])
}

pub fn apply(window: &tauri::WebviewWindow, dark: bool) -> Result<&'static str, String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    unsafe {
        if !DwmIsCompositionEnabled().map_err(|error| error.to_string())?.as_bool() {
            return Err("Desktop composition is disabled".into());
        }
        // Public system backdrop on Windows 11 22621+. Older systems reject
        // attribute 38, so no guessed build number or manifest version is used.
        let transient: u32 = 3;
        if DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(38), &transient as *const _ as _, size_of::<u32>() as u32).is_ok() {
            let round: u32 = 2;
            let _ = DwmSetWindowAttribute(hwnd, DWMWINDOWATTRIBUTE(33), &round as *const _ as _, size_of::<u32>() as u32);
            return Ok("acrylic-dwm");
        }
        let library = GetModuleHandleW(w!("user32.dll")).map_err(|error| error.to_string())?;
        let entry = GetProcAddress(library, s!("SetWindowCompositionAttribute"))
            .ok_or_else(|| "Windows 10 desktop material API is unavailable".to_string())?;
        let set_composition: SetComposition = std::mem::transmute(entry);
        #[cfg(debug_assertions)]
        if std::env::var_os("DEADLINE_PANEL_COMPOSITION_GLASS").is_some() {
            let mut disabled=AccentPolicy {state:0,flags:0,color:0,animation:0};
            let mut data=CompositionAttribute {attribute:19,data:&mut disabled as *mut _ as _,size:size_of::<AccentPolicy>()};
            if !set_composition(hwnd,&mut data).as_bool() { return Err("Failed to clear rectangular Acrylic".into()); }
            return crate::backdrop::apply(window);
        }
        let mut policy = AccentPolicy {
            state: 4, flags: 0,
            color: if dark { abgr(24, 24, 26, 64) } else { abgr(244, 244, 246, 86) },
            animation: 0,
        };
        let mut data = CompositionAttribute { attribute: 19, data: &mut policy as *mut _ as _, size: size_of::<AccentPolicy>() };
        if !set_composition(hwnd, &mut data).as_bool() {
            return Err("Windows rejected the desktop Acrylic material".into());
        }
        Ok("acrylic-win10")
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
        assert_eq!(size_of::<CompositionAttribute>(), if size_of::<usize>() == 8 { 24 } else { 12 });
    }
}
