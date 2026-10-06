//! The programs with a window open, for picking the ones that switch a profile on.

/// Programs that are part of the system or Open Controller, never what someone plays.
const SYSTEM: [&str; 13] = [
    "explorer.exe",
    "open-controller.exe",
    "open-controller-ui.exe",
    "applicationframehost.exe",
    "textinputhost.exe",
    "systemsettings.exe",
    "searchhost.exe",
    "shellexperiencehost.exe",
    "open-controller",
    "open-controller-ui",
    "finder",
    "gnome-shell",
    "plasmashell",
];

pub fn open() -> Vec<String> {
    let mut names = sys::open();
    names.retain(|n| !SYSTEM.contains(&n.as_str()));
    names.sort();
    names.dedup();
    names
}

#[cfg(target_os = "linux")]
mod sys {
    pub use open_controller_core::linux::x11::open_programs as open;
}

#[cfg(target_os = "macos")]
mod sys {
    pub use open_controller_core::macos::workspace::open_programs as open;
}

#[cfg(windows)]
mod sys {
    use open_controller_core::profile::program_name;
    use windows_sys::Win32::Foundation::{CloseHandle, HWND, LPARAM};
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GWL_EXSTYLE, GetWindowLongW, GetWindowTextLengthW, GetWindowThreadProcessId, IsWindowVisible, WS_EX_TOOLWINDOW,
    };

    /// Executable names of the programs with a visible, titled window, sorted.
    pub fn open() -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        unsafe { EnumWindows(Some(each), &mut names as *mut Vec<String> as LPARAM) };
        names
    }

    unsafe extern "system" fn each(hwnd: HWND, data: LPARAM) -> i32 {
        let names = unsafe { &mut *(data as *mut Vec<String>) };
        let shown = unsafe { IsWindowVisible(hwnd) != 0 && GetWindowTextLengthW(hwnd) > 0 };
        let tool = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW != 0 };
        if shown
            && !tool
            && let Some(n) = program_of(hwnd)
        {
            names.push(n);
        }
        1
    }

    fn program_of(hwnd: HWND) -> Option<String> {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) };
        unsafe { CloseHandle(process) };
        (ok != 0).then(|| program_name(&String::from_utf16_lossy(&buf[..len as usize])))
    }
}
