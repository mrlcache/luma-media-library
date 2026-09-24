#[cfg(windows)]
use std::sync::{Arc, Mutex};

/// Tracks whether the Home surface currently needs the system Acrylic material.
/// The material is supplied by DWM on the existing app HWND; no helper window is created.
#[cfg(windows)]
#[derive(Clone, Default)]
pub struct BackdropState(Arc<Mutex<bool>>);

#[cfg(not(windows))]
#[derive(Clone, Default)]
pub struct BackdropState;

#[cfg(windows)]
mod windows {
    use super::BackdropState;
    use std::mem;
    use windows_sys::Win32::{
        Foundation::HWND,
        Graphics::Dwm::{
            DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_SYSTEMBACKDROP_TYPE,
            DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        },
    };

    const DWMSBT_NONE: u32 = 1;
    const DWMSBT_TRANSIENTWINDOW: u32 = 3;
    // COLORREF is 0x00BBGGRR. DWM does not accept alpha for frame colors, so this
    // low-contrast cool gray approximates the requested translucent outline.
    const FRAME_BORDER_COLOR: u32 = 0x003c3631;

    /// Ask DWM to own the restored-window corners and border. Unlike a WebView
    /// overlay, this frame tracks the actual HWND bounds throughout native resizing.
    pub fn set_native_window_frame(main_hwnd: usize) -> Result<(), String> {
        let hwnd = main_hwnd as HWND;
        if hwnd.is_null() {
            return Err("The main app window is unavailable".to_owned());
        }

        unsafe {
            let corner_preference = DWMWCP_ROUND;
            let corner_result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &corner_preference as *const _ as _,
                mem::size_of_val(&corner_preference) as u32,
            );
            let border_result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &FRAME_BORDER_COLOR as *const _ as _,
                mem::size_of_val(&FRAME_BORDER_COLOR) as u32,
            );

            if corner_result < 0 || border_result < 0 {
                return Err(format!(
                    "Windows could not apply its native window frame (corner HRESULT 0x{:08X}, border HRESULT 0x{:08X})",
                    corner_result as u32, border_result as u32
                ));
            }
        }

        Ok(())
    }

    /// Enables Desktop Acrylic on the app's one existing window while the HSS is visible.
    /// The caller enables it only while HSS is visible: DWM's supported Win32 API applies Acrylic
    /// to the whole HWND. Transparent webview pixels reveal it; opaque UI (such as the sidebar)
    /// masks it. This avoids the unsynchronized second HWND used by the previous implementation.
    pub fn set_enabled(
        state: &BackdropState,
        main_hwnd: usize,
        should_enable: bool,
    ) -> Result<(), String> {
        let mut enabled = state
            .0
            .lock()
            .map_err(|_| "The HSS backdrop state is unavailable".to_owned())?;

        if *enabled == should_enable {
            return Ok(());
        }

        let hwnd = main_hwnd as HWND;
        if hwnd.is_null() {
            return Err("The main app window is unavailable".to_owned());
        }

        unsafe {
            let dark_mode = 1i32;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                &dark_mode as *const _ as _,
                mem::size_of_val(&dark_mode) as u32,
            );

            let backdrop_type = if should_enable {
                DWMSBT_TRANSIENTWINDOW
            } else {
                DWMSBT_NONE
            };
            let result = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE as u32,
                &backdrop_type as *const _ as _,
                mem::size_of_val(&backdrop_type) as u32,
            );

            if result < 0 {
                return Err(format!(
                    "Windows Desktop Acrylic is unavailable for this window (HRESULT 0x{:08X})",
                    result as u32
                ));
            }
        }

        *enabled = should_enable;
        Ok(())
    }
}

#[cfg(windows)]
pub use windows::set_enabled;
#[cfg(windows)]
pub use windows::set_native_window_frame;

#[cfg(not(windows))]
pub fn set_enabled(
    _state: &BackdropState,
    _main_hwnd: usize,
    _enabled: bool,
) -> Result<(), String> {
    Err("The native HSS Acrylic backdrop is only available on Windows".to_owned())
}

#[cfg(not(windows))]
pub fn set_native_window_frame(_main_hwnd: usize) -> Result<(), String> {
    Err("The native Windows window frame is only available on Windows".to_owned())
}
