use eframe::egui;

#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub id: String,
    pub name: String,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

pub fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<usize> {
    use raw_window_handle::HasWindowHandle;
    match cc.window_handle().ok()?.as_raw() {
        raw_window_handle::RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as usize),
        _ => None,
    }
}
#[cfg(target_os = "windows")]
pub fn window_rect(handle: Option<usize>) -> Option<[i32; 4]> {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::GetWindowRect,
    };
    let mut rect = RECT::default();
    unsafe { GetWindowRect(HWND(handle? as *mut std::ffi::c_void), &mut rect) }.ok()?;
    Some([rect.left, rect.top, rect.right, rect.bottom])
}
#[cfg(not(target_os = "windows"))]
pub fn window_rect(_handle: Option<usize>) -> Option<[i32; 4]> {
    None
}

/// Global cursor coordinates remain valid while a region changes under the pointer.
#[cfg(target_os = "windows")]
pub fn pointer_in_window(handle: Option<usize>, scale: f32) -> Option<egui::Pos2> {
    use windows::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let mut point = POINT::default();
    unsafe { GetCursorPos(&mut point) }.ok()?;
    let rect = window_rect(handle)?;
    Some(egui::pos2(
        (point.x - rect[0]) as f32 / scale,
        (point.y - rect[1]) as f32 / scale,
    ))
}
#[cfg(not(target_os = "windows"))]
pub fn pointer_in_window(_handle: Option<usize>, _scale: f32) -> Option<egui::Pos2> {
    None
}

/// Windows owns a successful region. Everything outside it remains click-through.
#[cfg(target_os = "windows")]
pub fn shape_window(handle: Option<usize>, rect: egui::Rect, scale: f32, radius: f32) -> bool {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, RGN_AND, SetWindowRgn,
        },
    };
    let Some(handle) = handle else {
        return false;
    };
    let r = (radius * scale).round() as i32;
    // Extending the rounded rectangle above the top edge leaves the top corners square.
    let region = unsafe {
        if !rect.is_positive() {
            CreateRectRgn(0, 0, 0, 0)
        } else {
            CreateRoundRectRgn(
                (rect.left() * scale).round() as i32,
                (rect.top() * scale).round() as i32 - r,
                (rect.right() * scale).round() as i32 + 1,
                (rect.bottom() * scale).round() as i32 + 1,
                r * 2,
                r * 2,
            )
        }
    };
    if region.is_invalid() {
        return false;
    }
    let clip = unsafe {
        CreateRectRgn(
            (rect.left() * scale).round() as i32,
            (rect.top() * scale).round() as i32,
            (rect.right() * scale).round() as i32 + 1,
            (rect.bottom() * scale).round() as i32 + 1,
        )
    };
    if clip.is_invalid() {
        unsafe {
            let _ = DeleteObject(region.into());
        }
        return false;
    }
    unsafe {
        CombineRgn(Some(region), Some(region), Some(clip), RGN_AND);
        let _ = DeleteObject(clip.into());
    }
    if unsafe { SetWindowRgn(HWND(handle as *mut std::ffi::c_void), Some(region), false) } == 0 {
        unsafe {
            let _ = DeleteObject(region.into());
        }
        return false;
    }
    true
}
#[cfg(not(target_os = "windows"))]
pub fn shape_window(_handle: Option<usize>, _rect: egui::Rect, _scale: f32, _radius: f32) -> bool {
    false
}

#[cfg(any(target_os = "windows", test))]
fn covers_display(rect: [i32; 4], display: &DisplayInfo) -> bool {
    (rect[0] - display.left).abs() <= 2
        && (rect[1] - display.top).abs() <= 2
        && (rect[2] - display.left - display.width).abs() <= 2
        && (rect[3] - display.top - display.height).abs() <= 2
}

#[cfg(target_os = "windows")]
pub fn fullscreen_on_display(display_id: &str) -> bool {
    use windows::Win32::Foundation::RECT;
    use windows::Win32::UI::WindowsAndMessaging::*;
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() || !IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() {
            return false;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(window, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return false;
        }
        let mut class = [0u16; 128];
        let length = GetClassNameW(window, &mut class).max(0) as usize;
        if matches!(
            String::from_utf16_lossy(&class[..length]).as_str(),
            "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
        ) {
            return false;
        }
        let style = GetWindowLongPtrW(window, GWL_STYLE) as u32;
        // shortcut: borderless fullscreen only; use presentation signals for legacy exclusive modes.
        if style & (WS_CAPTION.0 | WS_THICKFRAME.0) != 0 {
            return false;
        }
        let mut rect = RECT::default();
        if GetWindowRect(window, &mut rect).is_err() {
            return false;
        }
        let displays = displays();
        displays
            .iter()
            .find(|d| d.id == display_id)
            .or(displays.first())
            .is_some_and(|d| covers_display([rect.left, rect.top, rect.right, rect.bottom], d))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn fullscreen_on_display(_display_id: &str) -> bool {
    false
}

#[cfg(target_os = "windows")]
pub fn displays() -> Vec<DisplayInfo> {
    use windows::Win32::{
        Foundation::{LPARAM, RECT},
        Graphics::Gdi::{EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW},
    };
    use windows::core::BOOL;
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _dc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
        if unsafe { GetMonitorInfoW(monitor, &mut info.monitorInfo) }.as_bool() {
            let end = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            let id = String::from_utf16_lossy(&info.szDevice[..end]);
            let r = info.monitorInfo.rcMonitor;
            let primary = info.monitorInfo.dwFlags & 1 != 0;
            let list = unsafe { &mut *(data.0 as *mut Vec<DisplayInfo>) };
            list.push(DisplayInfo {
                name: format!(
                    "{}{} · {} × {}",
                    id.replace("\\\\.\\", ""),
                    if primary { " (primary)" } else { "" },
                    r.right - r.left,
                    r.bottom - r.top
                ),
                id,
                left: r.left,
                top: r.top,
                width: r.right - r.left,
                height: r.bottom - r.top,
                primary,
            });
        }
        BOOL(1)
    }
    let mut result: Vec<DisplayInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect),
            LPARAM(&mut result as *mut Vec<DisplayInfo> as isize),
        );
    }
    result.sort_by_key(|d| !d.primary);
    result
}

#[cfg(not(target_os = "windows"))]
pub fn displays() -> Vec<DisplayInfo> {
    Vec::new()
}

pub fn anchor_rect(
    display: &DisplayInfo,
    width: f32,
    height: f32,
    scale: f32,
) -> (i32, i32, i32, i32) {
    let w = (width * scale).round().max(1.0).min(display.width as f32) as i32;
    let h = (height * scale).round().max(1.0).min(display.height as f32) as i32;
    (display.left + (display.width - w) / 2, display.top, w, h)
}

#[cfg(target_os = "windows")]
pub fn anchor(
    handle: Option<usize>,
    display_id: &str,
    width: f32,
    height: f32,
    scale: f32,
    topmost: bool,
    horizontal_offset: f32,
) -> Result<(), String> {
    use windows::Win32::{
        Foundation::{HWND, RECT},
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, HWND_NOTOPMOST, HWND_TOPMOST,
            SWP_NOACTIVATE, SetWindowPos, WS_EX_TOPMOST,
        },
    };
    let Some(handle) = handle else { return Ok(()) };
    let list = displays();
    let Some(display) = list
        .iter()
        .find(|d| d.id == display_id)
        .or_else(|| list.first())
    else {
        return Err("No display is available.".into());
    };
    let (base_x, y, w, h) = anchor_rect(display, width, height, scale);
    let x = (base_x + horizontal_offset.round() as i32)
        .clamp(display.left, display.left + display.width - w);
    let hwnd = HWND(handle as *mut std::ffi::c_void);
    let mut old = RECT::default();
    // The transparent host stays fixed. Avoid redundant compositor resizes.
    unsafe {
        let _ = GetWindowRect(hwnd, &mut old);
        let was_topmost = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32 & WS_EX_TOPMOST.0 != 0;
        if [
            old.left,
            old.top,
            old.right - old.left,
            old.bottom - old.top,
        ] == [x, y, w, h]
            && was_topmost == topmost
        {
            return Ok(());
        }
        SetWindowPos(
            hwnd,
            Some(if topmost {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            }),
            x,
            y,
            w,
            h,
            SWP_NOACTIVATE,
        )
        .map_err(|e| format!("Could not anchor the notch: {e}"))?;
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn anchor(
    _handle: Option<usize>,
    _display_id: &str,
    _width: f32,
    _height: f32,
    _scale: f32,
    _topmost: bool,
    _horizontal_offset: f32,
) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn raise(handle: Option<usize>, topmost: bool) {
    use windows::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{
            HWND_TOP, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
        },
    };
    if let Some(h) = handle {
        unsafe {
            let _ = SetWindowPos(
                HWND(h as *mut std::ffi::c_void),
                Some(if topmost { HWND_TOPMOST } else { HWND_TOP }),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }
}
#[cfg(not(windows))]
pub fn raise(_handle: Option<usize>, _topmost: bool) {}

#[cfg(target_os = "windows")]
pub fn volume(command: i32) {
    use windows::Win32::{
        Foundation::{LPARAM, WPARAM},
        UI::WindowsAndMessaging::{GetShellWindow, SendMessageW, WM_APPCOMMAND},
    };
    // Windows APPCOMMAND_VOLUME_MUTE/DOWN/UP are 8, 9 and 10.
    if (8..=10).contains(&command) {
        unsafe {
            SendMessageW(
                GetShellWindow(),
                WM_APPCOMMAND,
                Some(WPARAM(0)),
                Some(LPARAM((command as isize) << 16)),
            );
        }
    }
}
#[cfg(not(target_os = "windows"))]
pub fn volume(_command: i32) {}

#[cfg(target_os = "windows")]
pub struct GlobalShortcut {
    receiver: std::sync::mpsc::Receiver<()>,
    thread_id: u32,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[cfg(target_os = "windows")]
impl GlobalShortcut {
    pub fn new(ctx: egui::Context, letter: u8) -> Result<Self, String> {
        use windows::Win32::{
            System::Threading::GetCurrentThreadId,
            UI::{
                Input::KeyboardAndMouse::{
                    MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, RegisterHotKey, UnregisterHotKey,
                },
                WindowsAndMessaging::{GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, WM_HOTKEY},
            },
        };
        let (tx, receiver) = std::sync::mpsc::channel();
        let (setup_tx, setup) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || unsafe {
            let mut message = MSG::default();
            let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
            let result = RegisterHotKey(
                None,
                0x4349,
                MOD_ALT | MOD_CONTROL | MOD_NOREPEAT,
                u32::from(letter),
            );
            if let Err(error) = result {
                let _ = setup_tx.send(Err(format!("Shortcut is already in use: {error}")));
                return;
            }
            let _ = setup_tx.send(Ok(GetCurrentThreadId()));
            while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                if message.message == WM_HOTKEY {
                    if tx.send(()).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
            let _ = UnregisterHotKey(None, 0x4349);
        });
        let thread_id = setup
            .recv_timeout(std::time::Duration::from_secs(2))
            .map_err(|_| "Shortcut registration timed out.".to_owned())??;
        Ok(Self {
            receiver,
            thread_id,
            worker: Some(worker),
        })
    }
    pub fn pressed(&self) -> bool {
        self.receiver.try_iter().count() > 0
    }
}
#[cfg(target_os = "windows")]
impl Drop for GlobalShortcut {
    fn drop(&mut self) {
        use windows::Win32::{
            Foundation::{LPARAM, WPARAM},
            UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT},
        };
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(not(target_os = "windows"))]
pub struct GlobalShortcut;
#[cfg(not(target_os = "windows"))]
impl GlobalShortcut {
    pub fn new(_ctx: egui::Context, _letter: u8) -> Result<Self, String> {
        Err("Global shortcuts are currently available on Windows.".into())
    }
    pub fn pressed(&self) -> bool {
        false
    }
}

/// Capture only a named, visible window owned by this process. This also works
/// for immediate viewports, whose screenshot requests egui-glow 0.33 ignores.
#[cfg(target_os = "windows")]
pub fn capture_own_window(title: &str, path: &str) -> Result<(), String> {
    use std::ffi::c_void;
    type Handle = *mut c_void;
    #[repr(C)]
    #[derive(Default)]
    struct Bounds {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct BitmapHeader {
        size: u32,
        width: i32,
        height: i32,
        planes: u16,
        bits: u16,
        compression: u32,
        image_size: u32,
        xppm: i32,
        yppm: i32,
        used: u32,
        important: u32,
    }
    #[repr(C)]
    #[derive(Default)]
    struct BitmapInfo {
        header: BitmapHeader,
        color: u32,
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn EnumWindows(
            callback: unsafe extern "system" fn(Handle, isize) -> i32,
            data: isize,
        ) -> i32;
        fn GetWindowThreadProcessId(window: Handle, pid: *mut u32) -> u32;
        fn GetWindowTextW(window: Handle, text: *mut u16, length: i32) -> i32;
        fn IsWindowVisible(window: Handle) -> i32;
        fn GetWindowRect(window: Handle, rect: *mut Bounds) -> i32;
        fn GetDC(window: Handle) -> Handle;
        fn ReleaseDC(window: Handle, dc: Handle) -> i32;
        fn PrintWindow(window: Handle, dc: Handle, flags: u32) -> i32;
    }
    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateCompatibleDC(dc: Handle) -> Handle;
        fn CreateCompatibleBitmap(dc: Handle, width: i32, height: i32) -> Handle;
        fn SelectObject(dc: Handle, object: Handle) -> Handle;
        fn DeleteObject(object: Handle) -> i32;
        fn DeleteDC(dc: Handle) -> i32;
        fn GetDIBits(
            dc: Handle,
            bitmap: Handle,
            start: u32,
            lines: u32,
            bytes: *mut u8,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
    }
    struct Search<'a> {
        title: &'a str,
        found: Handle,
    }
    unsafe extern "system" fn find(window: Handle, data: isize) -> i32 {
        // EnumWindows is synchronous; Search outlives every callback.
        let search = unsafe { &mut *(data as *mut Search<'_>) };
        let mut pid = 0;
        unsafe {
            GetWindowThreadProcessId(window, &mut pid);
        }
        if pid == std::process::id() && unsafe { IsWindowVisible(window) } != 0 {
            let mut text = [0_u16; 256];
            let length = unsafe { GetWindowTextW(window, text.as_mut_ptr(), text.len() as i32) };
            if String::from_utf16_lossy(&text[..length.max(0) as usize]) == search.title {
                search.found = window;
                return 0;
            }
        }
        1
    }
    let mut search = Search {
        title,
        found: std::ptr::null_mut(),
    };
    unsafe {
        EnumWindows(find, &mut search as *mut Search<'_> as isize);
    }
    let window = search.found;
    if window.is_null() {
        return Err("The requested Cikung window is unavailable.".into());
    }
    let mut bounds = Bounds::default();
    if unsafe { GetWindowRect(window, &mut bounds) } == 0 {
        return Err("Cannot read the Cikung window size.".into());
    }
    let (width, height) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
    if width <= 0 || height <= 0 || i64::from(width) * i64::from(height) > 16_000_000 {
        return Err("Invalid capture size.".into());
    }
    let source = unsafe { GetDC(window) };
    if source.is_null() {
        return Err("Cannot access the Cikung window.".into());
    }
    let dc = unsafe { CreateCompatibleDC(source) };
    let bitmap = unsafe { CreateCompatibleBitmap(source, width, height) };
    if dc.is_null() || bitmap.is_null() {
        unsafe {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            if !dc.is_null() {
                DeleteDC(dc);
            }
            ReleaseDC(window, source);
        }
        return Err("Cannot allocate a window capture.".into());
    }
    let previous = unsafe { SelectObject(dc, bitmap) };
    let printed = unsafe { PrintWindow(window, dc, 2) };
    unsafe {
        SelectObject(dc, previous);
    }
    let result = (|| {
        if printed == 0 {
            return Err("Windows could not capture the Cikung window.".into());
        }
        let mut bytes = vec![0_u8; width as usize * height as usize * 4];
        let mut info = BitmapInfo {
            header: BitmapHeader {
                size: std::mem::size_of::<BitmapHeader>() as u32,
                width,
                height: -height,
                planes: 1,
                bits: 32,
                ..Default::default()
            },
            ..Default::default()
        };
        if unsafe {
            GetDIBits(
                dc,
                bitmap,
                0,
                height as u32,
                bytes.as_mut_ptr(),
                &mut info,
                0,
            )
        } != height
        {
            return Err("Incomplete window capture.".into());
        }
        for pixel in bytes.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            pixel[3] = 255;
        }
        image::save_buffer(
            path,
            &bytes,
            width as u32,
            height as u32,
            image::ColorType::Rgba8,
        )
        .map_err(|e| format!("Cannot save window capture: {e}"))
    })();
    unsafe {
        DeleteObject(bitmap);
        DeleteDC(dc);
        ReleaseDC(window, source);
    }
    result
}

#[cfg(not(target_os = "windows"))]
pub fn capture_own_window(_title: &str, _path: &str) -> Result<(), String> {
    Err("Native popup capture is supported on Windows only.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchors_at_secondary_monitor_origin_with_dpi() {
        let d = DisplayInfo {
            id: String::new(),
            name: String::new(),
            left: -1920,
            top: 200,
            width: 1920,
            height: 1080,
            primary: false,
        };
        assert_eq!(anchor_rect(&d, 240.0, 40.0, 1.5), (-1140, 200, 360, 60));
    }
    #[test]
    fn bounds_large_notch_to_display() {
        let d = DisplayInfo {
            id: String::new(),
            name: String::new(),
            left: 0,
            top: 0,
            width: 800,
            height: 600,
            primary: true,
        };
        assert_eq!(anchor_rect(&d, 900.0, 700.0, 1.0), (0, 0, 800, 600));
    }
    #[test]
    fn fullscreen_bounds_do_not_match_work_area_or_other_monitor() {
        let d = DisplayInfo {
            id: String::new(),
            name: String::new(),
            left: -1920,
            top: 0,
            width: 1920,
            height: 1080,
            primary: false,
        };
        assert!(covers_display([-1920, 0, 0, 1080], &d));
        assert!(!covers_display([-1920, 0, 0, 1040], &d));
        assert!(!covers_display([0, 0, 1920, 1080], &d));
        assert!(!covers_display([-1928, -8, 8, 1088], &d));
    }
}
