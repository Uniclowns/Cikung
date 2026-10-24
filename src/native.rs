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
        // The NSView, used only by the macOS pocket drag-out.
        raw_window_handle::RawWindowHandle::AppKit(handle) => {
            Some(handle.ns_view.as_ptr() as usize)
        }
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
    hide_from_taskbar(hwnd);
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
/// Pins the notch window to the top centre of its Mac display, above the menu bar and on
/// every Space. With a camera notch it hangs right below the camera, as the Windows notch
/// hangs from the top edge; nothing is drawn beside the camera.
#[cfg(target_os = "macos")]
pub fn place_on_top(handle: Option<usize>, width: f32, height: f32) {
    use objc2::{runtime::NSObjectProtocol, sel};
    use objc2_app_kit::{NSMainMenuWindowLevel, NSScreen, NSView, NSWindowCollectionBehavior};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};
    let (Some(handle), Some(mtm)) = (handle, MainThreadMarker::new()) else {
        return;
    };
    let view = unsafe { &*(handle as *const NSView) };
    let Some(window) = view.window() else {
        return;
    };
    let Some(screen) = window.screen().or_else(|| NSScreen::mainScreen(mtm)) else {
        return;
    };
    let frame = screen.frame();
    // safeAreaInsets arrived in macOS 12 (the bundle still starts on 11); 0 without a notch.
    let camera = if screen.respondsToSelector(sel!(safeAreaInsets)) {
        unsafe { screen.safeAreaInsets().top }
    } else {
        0.0
    };
    let (w, h) = (f64::from(width), f64::from(height));
    window.setLevel(NSMainMenuWindowLevel + 3);
    unsafe {
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
    }
    window.setHasShadow(false);
    window.setFrame_display(
        NSRect::new(
            NSPoint::new(
                frame.origin.x + (frame.size.width - w) / 2.0,
                frame.origin.y + frame.size.height - camera - h,
            ),
            NSSize::new(w, h),
        ),
        true,
    );
}

/// Keeps the notch out of the taskbar and Alt+Tab so Cikung runs in the background
/// (the tray icon is its way in). winit's skip-taskbar is lost when eframe first shows
/// the window; a tool window never gets a taskbar button, however often it is shown.
#[cfg(target_os = "windows")]
fn hide_from_taskbar(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GetWindowLongPtrW, IsWindowVisible, SW_HIDE, SW_SHOWNOACTIVATE,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        let wanted = (style | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0;
        if style == wanted {
            return;
        }
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, wanted as isize);
        // Explorer drops an existing button only when the window is shown again.
        if IsWindowVisible(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }
        let _ = SetWindowPos(
            hwnd,
            None,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );
    }
}

/// What the Mac menu-bar icon asked for: 1 open the notch, 2 Settings, 3 quit.
#[cfg(target_os = "macos")]
static MENU_REQUEST: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
#[cfg(target_os = "macos")]
static MENU_CTX: std::sync::OnceLock<egui::Context> = std::sync::OnceLock::new();
#[cfg(target_os = "macos")]
fn menu_request(code: u8) {
    MENU_REQUEST.store(code, std::sync::atomic::Ordering::Relaxed);
    if let Some(ctx) = MENU_CTX.get() {
        ctx.request_repaint();
    }
}

#[cfg(target_os = "macos")]
mod mac_menu {
    use objc2::{
        ClassType, DeclaredClass, declare_class, msg_send_id, mutability,
        rc::Retained,
        runtime::{AnyObject, NSObjectProtocol},
    };
    use objc2_foundation::{MainThreadMarker, NSObject};

    declare_class!(
        pub struct MenuTarget;

        unsafe impl ClassType for MenuTarget {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "CikungMenuTarget";
        }

        impl DeclaredClass for MenuTarget {}

        unsafe impl MenuTarget {
            #[method(openCikung:)]
            fn open_cikung(&self, _sender: Option<&AnyObject>) {
                super::menu_request(1);
            }

            #[method(openSettings:)]
            fn open_settings(&self, _sender: Option<&AnyObject>) {
                super::menu_request(2);
            }

            #[method(quitCikung:)]
            fn quit_cikung(&self, _sender: Option<&AnyObject>) {
                super::menu_request(3);
            }
        }

        unsafe impl NSObjectProtocol for MenuTarget {}
    );

    impl MenuTarget {
        pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(());
            unsafe { msg_send_id![super(this), init] }
        }
    }
}

/// The cat in the Mac menu bar: Open Cikung, Settings… and Quit Cikung. Cikung has no
/// Dock icon, so this is how it is found and closed.
#[cfg(target_os = "macos")]
pub fn install_menu_bar_item(ctx: &egui::Context) {
    use objc2::{runtime::AnyObject, sel};
    use objc2_app_kit::{NSImage, NSMenu, NSMenuItem, NSStatusBar, NSVariableStatusItemLength};
    use objc2_foundation::{MainThreadMarker, NSData, NSSize, NSString};
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let _ = MENU_CTX.set(ctx.clone());
    unsafe {
        let item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        if let Some(button) = item.button(mtm) {
            let data = NSData::with_bytes(include_bytes!("../assets/embed/cikung-face.png"));
            if let Some(image) = NSImage::initWithData(mtm.alloc(), &data) {
                image.setSize(NSSize::new(18.0, 18.0));
                button.setImage(Some(&image));
            }
        }
        let target = mac_menu::MenuTarget::new(mtm);
        let target_object: &AnyObject = &target;
        let menu = NSMenu::new(mtm);
        let entries = [
            ("Open Cikung", sel!(openCikung:), ""),
            ("Settings…", sel!(openSettings:), ","),
            ("Quit Cikung", sel!(quitCikung:), "q"),
        ];
        for (index, (title, action, key)) in entries.into_iter().enumerate() {
            if index == 2 {
                menu.addItem(&NSMenuItem::separatorItem(mtm));
            }
            let entry = NSMenuItem::initWithTitle_action_keyEquivalent(
                mtm.alloc(),
                &NSString::from_str(title),
                Some(action),
                &NSString::from_str(key),
            );
            entry.setTarget(Some(target_object));
            menu.addItem(&entry);
        }
        item.setMenu(Some(&menu));
        // ponytail: the status item and its target live for the whole run.
        std::mem::forget(item);
        std::mem::forget(target);
    }
}
#[cfg(not(target_os = "macos"))]
pub fn install_menu_bar_item(_ctx: &egui::Context) {}

/// Takes the pending menu-bar request (0 when there is none).
#[cfg(target_os = "macos")]
pub fn take_menu_request() -> u8 {
    MENU_REQUEST.swap(0, std::sync::atomic::Ordering::Relaxed)
}
#[cfg(not(target_os = "macos"))]
pub fn take_menu_request() -> u8 {
    0
}

/// Like Nusa: hand idle pages back to Windows while the notch rests. Task Manager's
/// memory drops; pages fault back in only when touched again.
#[cfg(target_os = "windows")]
pub fn trim_memory() {
    use windows::Win32::System::Threading::{GetCurrentProcess, SetProcessWorkingSetSize};
    unsafe {
        let _ = SetProcessWorkingSetSize(GetCurrentProcess(), usize::MAX, usize::MAX);
    }
}
#[cfg(not(target_os = "windows"))]
pub fn trim_memory() {}

/// Same 8/9/10 = mute/down/up codes as Windows, through AppleScript's `set volume`.
#[cfg(target_os = "macos")]
pub fn volume(command: i32) {
    let script = match command {
        8 => "set volume output muted not (output muted of (get volume settings))",
        9 => "set volume output volume ((output volume of (get volume settings)) - 6)",
        10 => "set volume output volume ((output volume of (get volume settings)) + 6)",
        _ => return,
    };
    if let Ok(mut child) = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .spawn()
    {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
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

/// File Explorer's own icon, or thumbnail for images and videos, as unmultiplied RGBA.
#[cfg(target_os = "windows")]
pub fn shell_icon(path: &str, size: i32) -> Option<([usize; 2], Vec<u8>)> {
    use windows::{
        Win32::{
            Foundation::SIZE,
            Graphics::Gdi::{
                BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS,
                DeleteDC, DeleteObject, GetDIBits, GetObjectW, HGDIOBJ,
            },
            System::Com::IBindCtx,
            UI::Shell::{IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_BIGGERSIZEOK},
        },
        core::HSTRING,
    };
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(path), None::<&IBindCtx>).ok()?;
        let bitmap = factory
            .GetImage(SIZE { cx: size, cy: size }, SIIGBF_BIGGERSIZEOK)
            .ok()?;
        let object = HGDIOBJ(bitmap.0);
        let mut info = BITMAP::default();
        GetObjectW(
            object,
            std::mem::size_of::<BITMAP>() as i32,
            Some((&mut info as *mut BITMAP).cast()),
        );
        let (width, height) = (info.bmWidth, info.bmHeight.abs());
        let mut header = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut pixels = vec![0_u8; (width.max(0) * height * 4) as usize];
        let dc = CreateCompatibleDC(None);
        let rows = GetDIBits(
            dc,
            bitmap,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut header,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        let _ = DeleteObject(object);
        if rows == 0 || width <= 0 || height <= 0 {
            return None;
        }
        // BGRA to RGBA. Photo thumbnails often carry no alpha at all: show those opaque.
        let opaque = pixels.as_chunks::<4>().0.iter().all(|p| p[3] == 0);
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
            if opaque {
                pixel[3] = 255;
            }
        }
        Some(([width as usize, height as usize], pixels))
    }
}
/// Finder-style pocket image: photos get a real thumbnail, everything else Finder's icon.
#[cfg(target_os = "macos")]
pub fn shell_icon(path: &str, size: i32) -> Option<([usize; 2], Vec<u8>)> {
    let lower = path.to_lowercase();
    let photo = [".png", ".jpg", ".jpeg", ".webp"]
        .iter()
        .any(|ext| lower.ends_with(ext))
        && std::fs::metadata(path).is_ok_and(|meta| meta.len() < 40 << 20);
    let image = match photo.then(|| image::open(path).ok()).flatten() {
        Some(image) => image,
        None => image::load_from_memory(&finder_icon_png(path, size)?).ok()?,
    };
    let rgba = image.thumbnail(size as u32, size as u32).into_rgba8();
    Some((
        [rgba.width() as usize, rgba.height() as usize],
        rgba.into_raw(),
    ))
}
/// Renders `iconForFile:` into a fixed-size bitmap so the sharpest representation is used.
#[cfg(target_os = "macos")]
fn finder_icon_png(path: &str, size: i32) -> Option<Vec<u8>> {
    use objc2::ClassType;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSCompositingOperation, NSDeviceRGBColorSpace,
        NSGraphicsContext, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};
    unsafe {
        let icon = NSWorkspace::sharedWorkspace().iconForFile(&NSString::from_str(path));
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            size as isize,
            size as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        let side = f64::from(size);
        icon.drawInRect_fromRect_operation_fraction(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(side, side)),
            NSRect::ZERO,
            NSCompositingOperation::SourceOver,
            1.0,
        );
        NSGraphicsContext::restoreGraphicsState_class();
        let png = rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())?;
        Some(png.bytes().to_vec())
    }
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn shell_icon(_path: &str, _size: i32) -> Option<([usize; 2], Vec<u8>)> {
    None
}

/// Drags a pocket file out to Explorer, chats or mail exactly like File Explorer does.
/// Blocks in OLE's drag loop until the drop or cancel.
#[cfg(target_os = "windows")]
pub fn drag_file_out(handle: Option<usize>, path: &str) -> Result<(), String> {
    use windows::{
        Win32::{
            Foundation::{HWND, LPARAM, WPARAM},
            System::{
                Com::{IBindCtx, IDataObject},
                Ole::{DROPEFFECT_COPY, DROPEFFECT_LINK, IDropSource},
            },
            UI::{
                Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop},
                WindowsAndMessaging::{PostMessageW, WM_LBUTTONUP},
            },
        },
        core::HSTRING,
    };
    let hwnd = handle.map(|h| HWND(h as *mut std::ffi::c_void));
    let result = unsafe {
        SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(path), None::<&IBindCtx>)
            .and_then(|item| {
                item.BindToHandler::<_, IDataObject>(None::<&IBindCtx>, &BHID_DataObject)
            })
            // No custom drop source: the shell supplies its own, plus the drag image.
            .and_then(|data| {
                SHDoDragDrop(
                    hwnd,
                    &data,
                    None::<&IDropSource>,
                    DROPEFFECT_COPY | DROPEFFECT_LINK,
                )
            })
            .map(|_| ())
            .map_err(|e| format!("Couldn't drag this file: {e}"))
    };
    // OLE's drag loop swallowed the button release, so egui would think it is still held.
    if let Some(hwnd) = hwnd {
        unsafe {
            let _ = PostMessageW(Some(hwnd), WM_LBUTTONUP, WPARAM(0), LPARAM(0));
        }
    }
    result
}
/// Window number of the notch while a macOS pocket drag runs, so its end can release egui.
#[cfg(target_os = "macos")]
static DRAG_WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

#[cfg(target_os = "macos")]
mod mac_drag {
    use objc2::{
        ClassType, DeclaredClass, declare_class, msg_send_id, mutability, rc::Retained,
        runtime::NSObjectProtocol,
    };
    use objc2_app_kit::{
        NSApplication, NSDragOperation, NSDraggingContext, NSDraggingSession, NSDraggingSource,
        NSEvent, NSEventModifierFlags, NSEventType,
    };
    use objc2_foundation::{MainThreadMarker, NSObject, NSPoint};

    declare_class!(
        pub struct DragSource;

        unsafe impl ClassType for DragSource {
            type Super = NSObject;
            type Mutability = mutability::MainThreadOnly;
            const NAME: &'static str = "CikungPocketDragSource";
        }

        impl DeclaredClass for DragSource {}

        unsafe impl NSObjectProtocol for DragSource {}

        unsafe impl NSDraggingSource for DragSource {
            #[method(draggingSession:sourceOperationMaskForDraggingContext:)]
            fn operation_mask(
                &self,
                _session: &NSDraggingSession,
                _context: NSDraggingContext,
            ) -> NSDragOperation {
                NSDragOperation::Copy | NSDragOperation::Link
            }

            #[method(draggingSession:endedAtPoint:operation:)]
            fn ended(
                &self,
                _session: &NSDraggingSession,
                _point: NSPoint,
                _operation: NSDragOperation,
            ) {
                release_pointer();
            }
        }
    );

    impl DragSource {
        pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
            let this = mtm.alloc::<Self>().set_ivars(());
            unsafe { msg_send_id![super(this), init] }
        }
    }

    /// AppKit's drag session swallows the mouse-up; hand winit one so egui lets go too.
    fn release_pointer() {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let window = super::DRAG_WINDOW.load(std::sync::atomic::Ordering::Relaxed);
        let event = unsafe {
            NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                NSPoint::new(0.0, 0.0),
                NSEventModifierFlags::empty(),
                0.0,
                window,
                None,
                0,
                1,
                0.0,
            )
        };
        if let Some(event) = event {
            NSApplication::sharedApplication(mtm).postEvent_atStart(&event, false);
        }
    }
}

/// Drags a pocket file out to Finder, chats or mail exactly like Finder does.
#[cfg(target_os = "macos")]
pub fn drag_file_out(handle: Option<usize>, path: &str) -> Result<(), String> {
    use objc2::{
        ClassType,
        runtime::{AnyObject, ProtocolObject},
    };
    use objc2_app_kit::{
        NSApplication, NSDraggingItem, NSEvent, NSEventModifierFlags, NSEventType, NSView,
        NSWorkspace,
    };
    use objc2_foundation::{MainThreadMarker, NSArray, NSPoint, NSRect, NSSize, NSString, NSURL};
    let mtm = MainThreadMarker::new().ok_or("Dragging must start on the main thread.")?;
    let view = handle.ok_or("The notch window is not ready yet.")? as *const NSView;
    unsafe {
        let view = &*view;
        let window = view.window().ok_or("The notch window is not ready yet.")?;
        let number = window.windowNumber();
        // Use the live mouse event when there is one; otherwise describe the drag ourselves.
        let event = NSApplication::sharedApplication(mtm)
            .currentEvent()
            .filter(|event| {
                matches!(
                    event.r#type(),
                    NSEventType::LeftMouseDown | NSEventType::LeftMouseDragged
                )
            })
            .or_else(|| {
                NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                    NSEventType::LeftMouseDragged,
                    window.mouseLocationOutsideOfEventStream(),
                    NSEventModifierFlags::empty(),
                    0.0,
                    number,
                    None,
                    0,
                    1,
                    1.0,
                )
            })
            .ok_or("macOS could not start the drag.")?;
        let file = NSString::from_str(path);
        let url = NSURL::fileURLWithPath(&file);
        let item = NSDraggingItem::initWithPasteboardWriter(
            NSDraggingItem::alloc(),
            ProtocolObject::from_ref(&*url),
        );
        let icon = NSWorkspace::sharedWorkspace().iconForFile(&file);
        let at = view.convertPoint_fromView(event.locationInWindow(), None);
        let contents: &AnyObject = icon.as_ref();
        item.setDraggingFrame_contents(
            NSRect::new(
                NSPoint::new(at.x - 24.0, at.y - 24.0),
                NSSize::new(48.0, 48.0),
            ),
            Some(contents),
        );
        DRAG_WINDOW.store(number, std::sync::atomic::Ordering::Relaxed);
        let source = mac_drag::DragSource::new(mtm);
        view.beginDraggingSessionWithItems_event_source(
            &NSArray::from_vec(vec![item]),
            &event,
            ProtocolObject::from_ref(&*source),
        );
        // ponytail: one tiny source object per drag outlives the session (never freed).
        std::mem::forget(source);
    }
    Ok(())
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn drag_file_out(_handle: Option<usize>, _path: &str) -> Result<(), String> {
    Err("Dragging files out of the pocket currently requires Windows or macOS.".into())
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
