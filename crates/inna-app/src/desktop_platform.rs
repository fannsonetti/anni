//! Changes only Anni's own windows and its per-user startup entry.
use egui::{pos2, vec2, Rect};

pub const WORK_TITLE: &str = "anni · Desktop homework";
pub const CLOCK_TITLE: &str = "anni · Desktop clock";
pub const SCHEDULE_TITLE: &str = "anni · Desktop schedule";

#[cfg(windows)]
mod windows {
    use super::*;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::{Dwm::*, Gdi::*},
        System::{Registry::*, Threading::*},
        UI::{Accessibility::*, Controls::MARGINS, HiDpi::*, WindowsAndMessaging::*},
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub fn work_area() -> Rect {
        unsafe {
            let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut info: MONITORINFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if GetMonitorInfoW(monitor, &mut info) == 0 {
                return Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1040.0));
            }
            let (mut x_dpi, mut y_dpi) = (96, 96);
            GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut x_dpi, &mut y_dpi);
            let scale = x_dpi.max(96) as f32 / 96.0;
            let r = info.rcWork;
            Rect::from_min_max(
                pos2(r.left as f32 / scale, r.top as f32 / scale),
                pos2(r.right as f32 / scale, r.bottom as f32 / scale),
            )
        }
    }

    unsafe extern "system" fn arrange_widget(hwnd: HWND, foreground_changed: LPARAM) -> BOOL {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid != GetCurrentProcessId() {
            return 1;
        }
        let mut title = [0u16; 128];
        let length = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32);
        let title = String::from_utf16_lossy(&title[..length.max(0) as usize]);
        if title != WORK_TITLE && title != CLOCK_TITLE && title != SCHEDULE_TITLE {
            return 1;
        }
        let marker = wide("anni-widget-configured");
        let configured = !GetPropW(hwnd, marker.as_ptr()).is_null();
        if !configured {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            let borderless = (style | WS_POPUP as isize)
                & !((WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX | WS_THICKFRAME)
                    as isize);
            if style != borderless {
                SetWindowLongPtrW(hwnd, GWL_STYLE, borderless);
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                );
            }
            let ex_style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(
                hwnd,
                GWL_EXSTYLE,
                // Display-only widgets must not take focus when clicked.
                (ex_style | (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize)
                    & !((WS_EX_APPWINDOW
                        | WS_EX_WINDOWEDGE
                        | WS_EX_CLIENTEDGE
                        | WS_EX_STATICEDGE
                        | WS_EX_DLGMODALFRAME) as isize),
            );
            // Glass client rendering can retain a DWM caption (including a ghost X)
            // or shadow even after WS_CAPTION has been removed. These widgets have
            // no native non-client UI; the optional outline is painted in egui.
            let non_client_policy = DWMNCRP_DISABLED;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_NCRENDERING_POLICY as u32,
                &non_client_policy as *const _ as _,
                std::mem::size_of_val(&non_client_policy) as u32,
            );
            let corners = DWMWCP_DONOTROUND;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE as u32,
                &corners as *const _ as _,
                std::mem::size_of_val(&corners) as u32,
            );
            // WGL child viewports can lose their glass surface when their native
            // styles change. Extend DWM composition over the entire client area so
            // the framebuffer's per-pixel alpha reaches the wallpaper, rather than
            // being composited against an opaque black client background.
            let margins = MARGINS {
                cxLeftWidth: -1,
                cxRightWidth: -1,
                cyTopHeight: -1,
                cyBottomHeight: -1,
            };
            DwmExtendFrameIntoClientArea(hwnd, &margins);
            let region = CreateRectRgn(0, 0, -1, -1);
            if !region.is_null() {
                let blur = DWM_BLURBEHIND {
                    dwFlags: DWM_BB_ENABLE | DWM_BB_BLURREGION,
                    fEnable: 1,
                    hRgnBlur: region,
                    fTransitionOnMaximized: 0,
                };
                DwmEnableBlurBehindWindow(hwnd, &blur);
                DeleteObject(region);
            }
            // Suppress Windows 11's own hairline as well as the app's panel outlines.
            // DWMWA_COLOR_NONE; unsupported Windows versions simply ignore the attribute.
            let no_border: u32 = 0xFFFF_FFFE;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_BORDER_COLOR as u32,
                &no_border as *const _ as _,
                std::mem::size_of::<u32>() as u32,
            );
            let excluded: BOOL = 1;
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_EXCLUDED_FROM_PEEK as u32,
                &excluded as *const _ as _,
                std::mem::size_of::<BOOL>() as u32,
            );
            SetPropW(hwnd, marker.as_ptr(), 1_usize as HANDLE);
        }
        if configured && foreground_changed == 0 && IsIconic(hwnd) == 0 {
            return 1;
        }
        // Explorer raises the desktop during Show Desktop. Restore the overlays
        // above that surface; return them beneath apps when an app is used.
        // No parenting or undocumented WorkerW messages: Wallpaper Engine keeps ownership.
        let foreground = GetForegroundWindow();
        let mut class = [0u16; 64];
        let length = GetClassNameW(foreground, class.as_mut_ptr(), class.len() as i32);
        let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
        let desktop_active = class == "Progman" || class == "WorkerW";
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        SetWindowPos(
            hwnd,
            if desktop_active {
                HWND_TOP
            } else {
                HWND_BOTTOM
            },
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        );
        1
    }

    pub fn arrange_widgets() {
        use std::sync::atomic::{AtomicIsize, Ordering};
        static FOREGROUND: AtomicIsize = AtomicIsize::new(0);
        unsafe {
            let foreground = GetForegroundWindow() as isize;
            let changed = FOREGROUND.swap(foreground, Ordering::Relaxed) != foreground;
            EnumWindows(Some(arrange_widget), changed as LPARAM);
        }
    }

    // React to switching between Explorer's desktop and application windows
    // without waking every widget just to poll the foreground window.
    pub struct ForegroundWatcher(isize);

    impl ForegroundWatcher {
        pub fn new() -> Self {
            unsafe extern "system" fn changed(
                _: HWINEVENTHOOK,
                _: u32,
                _: HWND,
                _: i32,
                _: i32,
                _: u32,
                _: u32,
            ) {
                arrange_widgets();
            }
            Self(unsafe {
                SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    null_mut(),
                    Some(changed),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                ) as isize
            })
        }
    }

    impl Drop for ForegroundWatcher {
        fn drop(&mut self) {
            if self.0 != 0 {
                unsafe {
                    UnhookWinEvent(self.0 as HWINEVENTHOOK);
                }
            }
        }
    }

    const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
    const RUN_VALUE: &str = "anni";

    pub fn startup_enabled() -> Result<bool, String> {
        unsafe {
            let mut key = null_mut();
            let status = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                KEY_QUERY_VALUE,
                &mut key,
            );
            if status == ERROR_FILE_NOT_FOUND {
                return Ok(false);
            }
            if status != ERROR_SUCCESS {
                return Err("Could not read Windows startup settings.".into());
            }
            let status = RegQueryValueExW(
                key,
                wide(RUN_VALUE).as_ptr(),
                null(),
                null_mut(),
                null_mut(),
                null_mut(),
            );
            RegCloseKey(key);
            match status {
                ERROR_SUCCESS => Ok(true),
                ERROR_FILE_NOT_FOUND => Ok(false),
                _ => Err("Could not read Windows startup settings.".into()),
            }
        }
    }

    pub fn set_startup(enabled: bool) -> Result<(), String> {
        unsafe {
            let mut key = null_mut();
            let status = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(RUN_KEY).as_ptr(),
                0,
                null(),
                0,
                KEY_SET_VALUE,
                null(),
                &mut key,
                null_mut(),
            );
            if status != ERROR_SUCCESS {
                return Err("Could not change Windows startup settings.".into());
            }
            let status = if enabled {
                let exe = match std::env::current_exe() {
                    Ok(exe) => exe,
                    Err(_) => {
                        RegCloseKey(key);
                        return Err("Could not find Anni's executable.".into());
                    }
                };
                let command = wide(&format!("\"{}\" --desktop", exe.display()));
                RegSetValueExW(
                    key,
                    wide(RUN_VALUE).as_ptr(),
                    0,
                    REG_SZ,
                    command.as_ptr() as _,
                    (command.len() * 2) as u32,
                )
            } else {
                RegDeleteValueW(key, wide(RUN_VALUE).as_ptr())
            };
            RegCloseKey(key);
            if status == ERROR_SUCCESS || (!enabled && status == ERROR_FILE_NOT_FOUND) {
                Ok(())
            } else {
                Err("Could not change Windows startup settings. Please try again.".into())
            }
        }
    }

    pub struct Instance(HANDLE);
    impl Drop for Instance {
        fn drop(&mut self) {
            unsafe {
                if !self.0.is_null() {
                    CloseHandle(self.0);
                }
            }
        }
    }

    /// A second launch reopens the existing planner, even when its root is hidden.
    pub fn single_instance() -> Option<Instance> {
        unsafe {
            let handle = CreateMutexW(null(), 0, wide("Local\\anni-planner").as_ptr());
            if handle.is_null() {
                return Some(Instance(handle));
            }
            if GetLastError() == ERROR_ALREADY_EXISTS {
                CloseHandle(handle);
                let root = FindWindowW(null(), wide("anni").as_ptr());
                if !root.is_null() {
                    ShowWindow(root, SW_RESTORE);
                    SetForegroundWindow(root);
                }
                return None;
            }
            Some(Instance(handle))
        }
    }
}

#[cfg(windows)]
pub use windows::*;

#[cfg(not(windows))]
pub fn work_area() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(1920.0, 1040.0))
}
#[cfg(not(windows))]
pub fn arrange_widgets() {}
#[cfg(not(windows))]
pub struct ForegroundWatcher;
#[cfg(not(windows))]
impl ForegroundWatcher {
    pub fn new() -> Self {
        Self
    }
}
#[cfg(not(windows))]
pub fn startup_enabled() -> Result<bool, String> {
    Ok(false)
}
#[cfg(not(windows))]
pub fn set_startup(_: bool) -> Result<(), String> {
    Err("Automatic startup is supported on Windows. See the README for Linux setup.".into())
}
