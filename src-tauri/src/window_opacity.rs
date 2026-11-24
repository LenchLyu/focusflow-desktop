use tauri::WebviewWindow;
use windows_sys::Win32::{
    Foundation::{GetLastError, SetLastError, HWND},
    UI::WindowsAndMessaging::{
        GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, GWL_EXSTYLE, LWA_ALPHA,
        WS_EX_LAYERED,
    },
};

const MINI_OPACITY: u8 = 214;

pub(crate) fn apply(window: &WebviewWindow) {
    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd.0,
        Err(error) => {
            eprintln!("FocusFlow mini window handle: {error}");
            return;
        }
    };
    if let Err(error) = set_opacity(hwnd) {
        eprintln!("FocusFlow mini window opacity: {error}");
    }
}

fn set_opacity(hwnd: HWND) -> std::io::Result<()> {
    // Alpha blending changes the visual appearance while retaining normal pointer input.
    unsafe {
        let original = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetLastError(0);
        if SetWindowLongW(hwnd, GWL_EXSTYLE, original | WS_EX_LAYERED as i32) == 0
            && GetLastError() != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        if SetLayeredWindowAttributes(hwnd, 0, MINI_OPACITY, LWA_ALPHA) == 0 {
            let error = std::io::Error::last_os_error();
            SetWindowLongW(hwnd, GWL_EXSTYLE, original);
            return Err(error);
        }
    }
    Ok(())
}
