//! Native desktop shell integration. Startup changes are made only by set_autostart.
use eframe::egui;
#[cfg(any(target_os = "windows", test))]
use std::{ffi::OsString, path::Path};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum ShellAction {
    Toggle,
    Settings,
    Quit,
}

pub struct Shell {
    #[cfg(target_os = "windows")]
    inner: platform::WindowsShell,
}

impl Shell {
    pub fn start(ctx: egui::Context) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
            Ok(Self {
                inner: platform::WindowsShell::start(ctx)?,
            })
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = ctx;
            Err("The native tray is currently available on Windows only.".into())
        }
    }
    pub fn poll(&self) -> Vec<ShellAction> {
        #[cfg(target_os = "windows")]
        {
            self.inner.poll()
        }
        #[cfg(not(target_os = "windows"))]
        {
            Vec::new()
        }
    }
    pub fn notify(&self, title: &str, message: &str) {
        #[cfg(target_os = "windows")]
        {
            self.inner.notify(title, message);
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (title, message);
        }
    }
}

#[cfg(any(target_os = "windows", test))]
fn startup_command(executable: &Path) -> Result<OsString, String> {
    let path = executable
        .to_str()
        .ok_or("Startup requires a valid Unicode executable path.")?;
    if path.is_empty() || path.chars().any(|c| c == '"' || c.is_control()) {
        return Err("The executable path cannot be safely registered for startup.".into());
    }
    // This is the Run value's command line, never a shell command. The executable is
    // always quoted as one argument, including names with spaces or metacharacters.
    Ok(OsString::from(format!("\"{path}\"")))
}

pub fn autostart_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        registry_command(&["query", RUN_KEY, "/v", "Cikung"])
            .is_ok_and(|result| result.status.success())
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

pub fn set_autostart(enabled: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        let output = if enabled {
            let executable =
                std::env::current_exe().map_err(|e| format!("Could not locate Cikung: {e}"))?;
            let value = startup_command(&executable)?;
            use std::os::windows::process::CommandExt;
            std::process::Command::new(registry_executable())
                .args(["add", RUN_KEY, "/v", "Cikung", "/t", "REG_SZ", "/d"])
                .arg(value)
                .arg("/f")
                .creation_flags(0x08000000)
                .output()
                .map_err(|e| format!("Could not update your Cikung startup entry: {e}"))?
        } else {
            // Removing an already absent value is a successful disable.
            if !autostart_enabled() {
                return Ok(());
            }
            registry_command(&["delete", RUN_KEY, "/v", "Cikung", "/f"])?
        };
        if output.status.success() {
            Ok(())
        } else {
            Err("Windows could not update your Cikung startup setting. No administrator access is needed; check your account's startup policy.".into())
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = enabled;
        Err("Launch at login is currently supported on Windows only.".into())
    }
}

#[cfg(target_os = "windows")]
const RUN_KEY: &str = "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run";
#[cfg(target_os = "windows")]
fn registry_executable() -> std::path::PathBuf {
    std::env::var_os("SystemRoot")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("C:\\Windows"))
        .join("System32")
        .join("reg.exe")
}
#[cfg(target_os = "windows")]
fn registry_command(args: &[&str]) -> Result<std::process::Output, String> {
    use std::os::windows::process::CommandExt;
    std::process::Command::new(registry_executable())
        .args(args)
        .creation_flags(0x08000000)
        .output()
        .map_err(|e| format!("Could not read or update your Cikung startup entry: {e}"))
}

/// Copy complete Unicode characters into a nul-terminated Windows text buffer.
#[cfg(any(target_os = "windows", test))]
fn copy_wide<const N: usize>(target: &mut [u16; N], text: &str) {
    target.fill(0);
    let mut cursor = 0;
    for ch in text.chars() {
        if ch == '\0' {
            break;
        }
        let size = ch.len_utf16();
        if cursor + size >= N {
            break;
        }
        let mut units = [0; 2];
        target[cursor..cursor + size].copy_from_slice(ch.encode_utf16(&mut units));
        cursor += size;
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::{ShellAction, copy_wide};
    use eframe::egui;
    use std::{
        sync::mpsc::{self, Receiver, SyncSender},
        thread::{self, JoinHandle},
        time::Duration,
    };
    use windows::{
        Win32::{
            Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
            System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
            UI::{
                Shell::{
                    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_TIP, NIIF_INFO, NIIF_NOSOUND,
                    NIIF_RESPECT_QUIET_TIME, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIN_BALLOONUSERCLICK,
                    NOTIFYICONDATAW, Shell_NotifyIconW,
                },
                WindowsAndMessaging::*,
            },
        },
        core::{PCWSTR, w},
    };

    const TRAY_CALLBACK: u32 = WM_APP + 71;
    const NOTIFICATION_READY: u32 = WM_APP + 72;
    const TRAY_ID: u32 = 1;
    struct Notification {
        title: String,
        message: String,
    }
    pub(super) struct WindowsShell {
        hwnd: usize,
        thread_id: u32,
        actions: Receiver<ShellAction>,
        notifications: SyncSender<Notification>,
        worker: Option<JoinHandle<()>>,
    }
    impl WindowsShell {
        pub(super) fn start(ctx: egui::Context) -> Result<Self, String> {
            let (action_tx, action_rx) = mpsc::sync_channel(32);
            let (notification_tx, notification_rx) = mpsc::sync_channel(16);
            let (ready_tx, ready_rx) = mpsc::sync_channel(1);
            let worker = thread::Builder::new()
                .name("cikung-tray".into())
                .spawn(move || {
                    if let Err(error) = tray_thread(ctx, action_tx, notification_rx, &ready_tx) {
                        let _ = ready_tx.send(Err(error));
                    }
                })
                .map_err(|e| format!("Could not start the tray: {e}"))?;
            match ready_rx.recv_timeout(Duration::from_secs(10)) {
                Ok(Ok((hwnd, thread_id))) => Ok(Self {
                    hwnd,
                    thread_id,
                    actions: action_rx,
                    notifications: notification_tx,
                    worker: Some(worker),
                }),
                Ok(Err(error)) => {
                    let _ = worker.join();
                    Err(error)
                }
                Err(_) => Err("The Windows tray did not start in time.".into()),
            }
        }
        pub(super) fn poll(&self) -> Vec<ShellAction> {
            self.actions.try_iter().collect()
        }
        pub(super) fn notify(&self, title: &str, message: &str) {
            // Bound the queue as well as the Windows notification fields.
            let notice = Notification {
                title: title.chars().take(63).collect(),
                message: message.chars().take(255).collect(),
            };
            if self.notifications.try_send(notice).is_ok() {
                unsafe {
                    let _ = PostMessageW(
                        Some(HWND(self.hwnd as *mut _)),
                        NOTIFICATION_READY,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
        }
    }
    impl Drop for WindowsShell {
        fn drop(&mut self) {
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(self.hwnd as *mut _)),
                    WM_CANCELMODE,
                    WPARAM(0),
                    LPARAM(0),
                );
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
            }
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
        }
    }
    struct WindowState {
        ctx: egui::Context,
        actions: SyncSender<ShellAction>,
        notices: Receiver<Notification>,
        icon: HICON,
        taskbar_created: u32,
    }
    impl WindowState {
        fn action(&self, action: ShellAction) {
            let _ = self.actions.try_send(action);
            self.ctx.request_repaint();
        }
    }
    struct Icon(HICON);
    impl Drop for Icon {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyIcon(self.0);
            }
        }
    }
    struct Class {
        name: Vec<u16>,
        instance: HINSTANCE,
    }
    impl Drop for Class {
        fn drop(&mut self) {
            unsafe {
                let _ = UnregisterClassW(PCWSTR(self.name.as_ptr()), Some(self.instance));
            }
        }
    }
    struct Window(HWND);
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyWindow(self.0);
            }
        }
    }
    struct Tray(HWND);
    impl Drop for Tray {
        fn drop(&mut self) {
            let data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.0,
                uID: TRAY_ID,
                ..Default::default()
            };
            unsafe {
                let _ = Shell_NotifyIconW(NIM_DELETE, &data);
            }
        }
    }
    struct Menu(HMENU);
    impl Drop for Menu {
        fn drop(&mut self) {
            unsafe {
                let _ = DestroyMenu(self.0);
            }
        }
    }

    fn icon() -> Result<Icon, String> {
        let image = image::load_from_memory(include_bytes!("../assets/cikung-face.png"))
            .map_err(|e| format!("Could not load the tray cat: {e}"))?
            .resize_exact(32, 32, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        // An in-memory icon DIB: bottom-up BGRA pixels followed by a DWORD-aligned
        // transparent mask. This does not alter or write the generated artwork.
        let mut dib = Vec::with_capacity(40 + 32 * 32 * 4 + 128);
        dib.extend(40u32.to_le_bytes());
        dib.extend(32i32.to_le_bytes());
        dib.extend(64i32.to_le_bytes());
        dib.extend(1u16.to_le_bytes());
        dib.extend(32u16.to_le_bytes());
        dib.extend(0u32.to_le_bytes());
        dib.extend((32u32 * 32 * 4).to_le_bytes());
        dib.extend([0u8; 16]);
        for y in (0..32).rev() {
            for x in 0..32 {
                let [r, g, b, a] = image.get_pixel(x, y).0;
                dib.extend([b, g, r, a]);
            }
        }
        dib.extend([0u8; 128]);
        unsafe { CreateIconFromResourceEx(&dib, true, 0x00030000, 32, 32, LR_DEFAULTCOLOR) }
            .map(Icon)
            .map_err(|e| format!("Could not create the tray cat icon: {e}"))
    }
    fn tray_data(hwnd: HWND, icon: HICON) -> NOTIFYICONDATAW {
        let mut data = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: TRAY_CALLBACK,
            hIcon: icon,
            ..Default::default()
        };
        copy_wide(&mut data.szTip, "Cikung · click to show or hide");
        data
    }
    fn tray_thread(
        ctx: egui::Context,
        actions: SyncSender<ShellAction>,
        notices: Receiver<Notification>,
        ready: &SyncSender<Result<(usize, u32), String>>,
    ) -> Result<(), String> {
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).map_err(|e| e.to_string())?.0);
            let thread_id = GetCurrentThreadId();
            let name: Vec<u16> = format!("Cikung.Tray.{}.{}", std::process::id(), thread_id)
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let cat = icon()?;
            let class = Class { name, instance };
            let atom = RegisterClassW(&WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: PCWSTR(class.name.as_ptr()),
                ..Default::default()
            });
            if atom == 0 {
                return Err(format!(
                    "Could not register Cikung's tray window: {}",
                    windows::core::Error::from_thread()
                ));
            }
            let mut state = Box::new(WindowState {
                ctx,
                actions,
                notices,
                icon: cat.0,
                taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
            });
            // A hidden top-level tool window receives Explorer's TaskbarCreated
            // broadcast. It is never shown and never creates a taskbar button.
            let window = Window(
                CreateWindowExW(
                    WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    PCWSTR(class.name.as_ptr()),
                    w!("Cikung tray"),
                    WS_OVERLAPPED,
                    0,
                    0,
                    0,
                    0,
                    None,
                    None,
                    Some(instance),
                    Some((&mut *state as *mut WindowState).cast()),
                )
                .map_err(|e| e.to_string())?,
            );
            if !Shell_NotifyIconW(NIM_ADD, &tray_data(window.0, cat.0)).as_bool() {
                return Err("Windows did not accept Cikung's tray icon.".into());
            }
            let _tray = Tray(window.0);
            if ready.send(Ok((window.0.0 as usize, thread_id))).is_err() {
                return Ok(());
            }
            let mut message = MSG::default();
            loop {
                let status = GetMessageW(&mut message, None, 0, 0).0;
                if status <= 0 {
                    break;
                }
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            // _tray, window, state, class, and icon drop in the necessary order.
            Ok(())
        }
    }
    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        unsafe {
            if message == WM_NCCREATE {
                let creation = &*(lparam.0 as *const CREATESTRUCTW);
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, creation.lpCreateParams as isize);
            }
            let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if state_ptr.is_null() {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            let state = &*state_ptr;
            if state.taskbar_created != 0 && message == state.taskbar_created {
                let _ = Shell_NotifyIconW(NIM_ADD, &tray_data(hwnd, state.icon));
                return LRESULT(0);
            }
            match message {
                TRAY_CALLBACK => {
                    match lparam.0 as u32 {
                        WM_LBUTTONUP | NIN_BALLOONUSERCLICK => state.action(ShellAction::Toggle),
                        WM_RBUTTONUP => {
                            let _ = popup(hwnd, state);
                        }
                        _ => {}
                    }
                    LRESULT(0)
                }
                NOTIFICATION_READY => {
                    for notice in state.notices.try_iter() {
                        let mut data = tray_data(hwnd, state.icon);
                        data.uFlags = NIF_INFO;
                        copy_wide(&mut data.szInfoTitle, &notice.title);
                        copy_wide(&mut data.szInfo, &notice.message);
                        data.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND | NIIF_RESPECT_QUIET_TIME;
                        let _ = Shell_NotifyIconW(NIM_MODIFY, &data);
                    }
                    LRESULT(0)
                }
                WM_CLOSE => {
                    PostQuitMessage(0);
                    LRESULT(0)
                }
                WM_NCDESTROY => {
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                    DefWindowProcW(hwnd, message, wparam, lparam)
                }
                _ => DefWindowProcW(hwnd, message, wparam, lparam),
            }
        }
    }
    unsafe fn popup(hwnd: HWND, state: &WindowState) -> windows::core::Result<()> {
        unsafe {
            let menu = Menu(CreatePopupMenu()?);
            AppendMenuW(menu.0, MF_STRING, 1, w!("Show / Hide Cikung"))?;
            AppendMenuW(menu.0, MF_STRING, 2, w!("Settings"))?;
            AppendMenuW(menu.0, MF_SEPARATOR, 0, PCWSTR::null())?;
            AppendMenuW(menu.0, MF_STRING, 3, w!("Quit Cikung"))?;
            let mut point = POINT::default();
            GetCursorPos(&mut point)?;
            let _ = SetForegroundWindow(hwnd);
            let selection = TrackPopupMenu(
                menu.0,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                None,
                hwnd,
                None,
            )
            .0;
            match selection {
                1 => state.action(ShellAction::Toggle),
                2 => state.action(ShellAction::Settings),
                3 => state.action(ShellAction::Quit),
                _ => {}
            }
            let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_executable_is_one_quoted_argument() {
        let value = startup_command(Path::new("C:\\A folder\\cat & dog\\Cikung.exe")).unwrap();
        assert_eq!(
            value.to_str().unwrap(),
            "\"C:\\A folder\\cat & dog\\Cikung.exe\""
        );
        assert!(startup_command(Path::new("C:\\bad\"path.exe")).is_err());
        assert!(startup_command(Path::new("C:\\bad\npath.exe")).is_err());
    }
    #[test]
    fn tray_text_does_not_split_surrogate_pairs() {
        let mut text = [0u16; 4];
        copy_wide(&mut text, "A🐈B");
        assert_eq!(String::from_utf16(&text[..3]).unwrap(), "A🐈");
        assert_eq!(text[3], 0);
        let mut small = [0u16; 2];
        copy_wide(&mut small, "🐈");
        assert_eq!(small, [0, 0]);
    }
}
