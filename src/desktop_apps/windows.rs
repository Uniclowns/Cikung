use super::*;
use ::windows::{
    Win32::{
        Foundation::{CloseHandle, HWND, LPARAM},
        System::{
            Com::{
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
                CoUninitialize,
            },
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
        UI::{
            Accessibility::*,
            Shell::ShellExecuteW,
            WindowsAndMessaging::{
                EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible, SW_RESTORE,
                SW_SHOWNORMAL, SetForegroundWindow, ShowWindow,
            },
        },
    },
    core::{BOOL, Interface, PCWSTR, PWSTR},
};
use std::{
    ffi::c_void,
    time::{Duration, Instant},
};

fn error(_: impl std::fmt::Display) -> String {
    "The app's accessibility interface is not ready. Open the app and try again.".into()
}
fn executable(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = vec![0_u16; 32768];
        let mut length = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        Some(String::from_utf16_lossy(&buffer[..length as usize]))
    }
}
fn matches_app(target: Target, path: &str) -> bool {
    let path = path.replace('\\', "/").to_lowercase();
    match target {
        Target::Claude => {
            path.ends_with("/claude.exe")
                && (path.contains("/windowsapps/claude_") || path.contains("/anthropicclaude/"))
        }
        Target::Codex => {
            path.ends_with("/chatgpt.exe")
                && (path.contains("/windowsapps/openai.codex_")
                    || path.contains("/windowsapps/openai.chatgpt_")
                    || path.contains("/openai/codex/"))
        }
    }
}
struct Find {
    target: Target,
    found: Option<(usize, u32, String)>,
}
unsafe extern "system" fn find_window(window: HWND, data: LPARAM) -> BOOL {
    let search = unsafe { &mut *(data.0 as *mut Find) };
    if !unsafe { IsWindowVisible(window) }.as_bool() {
        return BOOL(1);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, Some(&mut pid));
    }
    let mut title = [0_u16; 512];
    let length = unsafe { GetWindowTextW(window, &mut title) };
    if length > 0 && executable(pid).is_some_and(|p| matches_app(search.target, &p)) {
        search.found = Some((
            window.0 as usize,
            pid,
            String::from_utf16_lossy(&title[..length as usize]),
        ));
        return BOOL(0);
    }
    BOOL(1)
}
fn window(target: Target) -> Option<(usize, u32, String)> {
    let mut search = Find {
        target,
        found: None,
    };
    unsafe {
        let _ = EnumWindows(Some(find_window), LPARAM(&mut search as *mut Find as isize));
    }
    search.found
}
fn elements(
    root: &IUIAutomationElement,
    automation: &IUIAutomation,
) -> Result<Vec<IUIAutomationElement>, String> {
    unsafe {
        let all = root
            .FindAll(
                TreeScope_Descendants,
                &automation.CreateTrueCondition().map_err(error)?,
            )
            .map_err(error)?;
        let count = all.Length().map_err(error)?.min(4000);
        Ok((0..count).filter_map(|i| all.GetElement(i).ok()).collect())
    }
}
fn name(element: &IUIAutomationElement) -> String {
    unsafe {
        element
            .CurrentName()
            .map(|s| s.to_string())
            .unwrap_or_default()
    }
}
fn class(element: &IUIAutomationElement) -> String {
    unsafe {
        element
            .CurrentClassName()
            .map(|s| s.to_string())
            .unwrap_or_default()
    }
}
fn visible(element: &IUIAutomationElement) -> bool {
    unsafe {
        element.CurrentIsOffscreen().is_ok_and(|v| !v.as_bool())
            && element.CurrentIsEnabled().is_ok_and(|v| v.as_bool())
    }
}
fn control(element: &IUIAutomationElement) -> UIA_CONTROLTYPE_ID {
    unsafe {
        element
            .CurrentControlType()
            .unwrap_or(UIA_CONTROLTYPE_ID(0))
    }
}
fn composer(element: &IUIAutomationElement) -> bool {
    if control(element) != UIA_EditControlTypeId || !visible(element) {
        return false;
    }
    if unsafe { element.CurrentIsPassword().is_ok_and(|v| v.as_bool()) } {
        return false;
    }
    let class = class(element);
    let label = name(element).to_lowercase();
    let id = unsafe {
        element
            .CurrentAutomationId()
            .map(|s| s.to_string())
            .unwrap_or_default()
    };
    (class.split_whitespace().any(|s| s == "ProseMirror") && label == "prompt")
        || id == "prompt-textarea"
        || id == "chat-input"
        || matches!(
            label.as_str(),
            "work with chatgpt"
                | "message chatgpt"
                | "reply to claude"
                | "message claude"
                | "ask claude"
        )
}
fn value(element: &IUIAutomationElement) -> Result<IUIAutomationValuePattern, String> {
    unsafe {
        element
            .GetCurrentPatternAs(UIA_ValuePatternId)
            .map_err(error)
    }
}
struct Capture {
    app: AppSnapshot,
    root: Option<IUIAutomationElement>,
    composer: Option<IUIAutomationElement>,
}
fn capture(target: Target, automation: &IUIAutomation) -> Result<Capture, String> {
    capture_window(target, window(target), automation)
}
fn capture_window(
    target: Target,
    found: Option<(usize, u32, String)>,
    automation: &IUIAutomation,
) -> Result<Capture, String> {
    let Some((hwnd, pid, title)) = found else {
        return Ok(Capture {
            app: AppSnapshot::empty(target, State::Closed, "Open the desktop app to connect."),
            root: None,
            composer: None,
        });
    };
    let root = unsafe {
        automation
            .ElementFromHandle(HWND(hwnd as *mut c_void))
            .map_err(error)?
    };
    let nodes = elements(&root, automation)?;
    let composers: Vec<_> = nodes.iter().filter(|n| composer(n)).cloned().collect();
    let composer = (composers.len() == 1).then(|| composers[0].clone());
    let busy = nodes
        .iter()
        .any(|n| control(n) == UIA_ButtonControlTypeId && visible(n) && is_stop(&name(n)));
    let attention = nodes
        .iter()
        .any(|n| control(n) == UIA_ButtonControlTypeId && visible(n) && is_attention(&name(n)));
    let preferred = nodes
        .iter()
        .find(|n| {
            visible(n)
                && (class(n).contains("thread-scroll-container")
                    || name(n) == "Conversation"
                    || name(n) == "Chat messages")
        })
        .cloned();
    // Claude hosts more than one web document. Follow the actual composer to
    // its document rather than picking an unrelated shell/hidden web area.
    let walker = unsafe { automation.ControlViewWalker().map_err(error)? };
    let mut document = None;
    if let Some(composer) = &composer {
        let mut parent = composer.clone();
        for _ in 0..40 {
            let Ok(next) = (unsafe { walker.GetParentElement(&parent) }) else {
                break;
            };
            if control(&next) == UIA_DocumentControlTypeId {
                document = Some(next);
                break;
            }
            parent = next;
        }
    }
    let document = document.or_else(|| {
        nodes
            .iter()
            .rev()
            .find(|n| control(n) == UIA_DocumentControlTypeId && visible(n))
            .cloned()
    });
    let body = preferred.or(document);
    let mut conversation = String::new();
    if let Some(body) = &body {
        for node in elements(body, automation)? {
            if control(&node) != UIA_TextControlTypeId || !visible(&node) {
                continue;
            }
            let mut parent = node.clone();
            let mut excluded = false;
            for _ in 0..30 {
                let Ok(next) = (unsafe { walker.GetParentElement(&parent) }) else {
                    break;
                };
                if control(&next) == UIA_EditControlTypeId
                    || control(&next) == UIA_ButtonControlTypeId
                {
                    excluded = true;
                    break;
                }
                if control(&next) == UIA_DocumentControlTypeId
                    || control(&next) == UIA_WindowControlTypeId
                {
                    break;
                }
                parent = next;
            }
            if !excluded {
                append_bounded(&mut conversation, &name(&node));
            }
        }
    }
    let draft = composer
        .as_ref()
        .and_then(|e| value(e).ok())
        .and_then(|v| unsafe { v.CurrentValue().ok() })
        .map(|s| s.to_string())
        .unwrap_or_default();
    let submit_enabled = nodes.iter().any(|n| {
        control(n) == UIA_ButtonControlTypeId
            && visible(n)
            && (is_send(&name(n))
                || matches!(
                    name(n).to_lowercase().as_str(),
                    "send now" | "queue" | "queue message" | "add to queue"
                ))
    });
    let draft_present = composer.as_ref().is_some_and(|c| {
        has_draft(
            &draft,
            &name(c),
            class(c).split_whitespace().any(|s| s == "ProseMirror"),
            submit_enabled,
        )
    });
    let writable = |e: &IUIAutomationElement| {
        value(e)
            .ok()
            .is_some_and(|v| unsafe { v.CurrentIsReadOnly().is_ok_and(|v| !v.as_bool()) })
    };
    let editable = composer.as_ref().is_some_and(writable);
    // Claude's Code tab and split views expose several prompts. Any writable one
    // means the app is idle, so finished turns still notify; Send keeps needing
    // exactly one composer (`can_send`) so it can never type into the wrong chat.
    let idle = composers.iter().any(writable);
    let state = if busy {
        State::Working
    } else if attention {
        State::Attention
    } else if idle {
        State::Ready
    } else {
        State::Loading
    };
    let detail=match state {
        State::Working=>"The app is working. Its visible conversation updates here.",
        State::Attention=>"Continue or sign in inside the app.",
        State::Ready if !editable=>"Several chats are open. Finished replies still notify; Send needs one visible chat.",
        State::Ready if draft_present=>"The app has an existing draft. It will be preserved.",
        State::Ready=>"Linked to the active app conversation.",
        _=>"The app is loading or has not exposed its chat yet. Open it and finish loading/sign-in.",
    }.into();
    Ok(Capture {
        app: AppSnapshot {
            target,
            state,
            binding: Some(Binding {
                hwnd,
                pid,
                context: context_key(&conversation),
            }),
            title,
            conversation,
            detail,
            can_send: state == State::Ready && editable,
            draft_present,
        },
        root: Some(root),
        composer,
    })
}
fn open(target: Target) -> Result<(), String> {
    if let Some((hwnd, _, _)) = window(target) {
        unsafe {
            let _ = ShowWindow(HWND(hwnd as *mut c_void), SW_RESTORE);
            let _ = SetForegroundWindow(HWND(hwnd as *mut c_void));
        }
        return Ok(());
    }
    let uri = match target {
        Target::Claude => "claude://",
        Target::Codex => "codex://",
    };
    let uri: Vec<u16> = uri.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(uri.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    if result.0 as isize > 32 {
        Ok(())
    } else {
        Err("The desktop app is not installed or its app link is unavailable.".into())
    }
}
fn send(
    target: Target,
    expected: Binding,
    prompt: &str,
    automation: &IUIAutomation,
) -> Result<(), String> {
    submit(
        capture(target, automation)?,
        expected,
        prompt,
        automation,
        || window(target),
    )
}
fn submit(
    capture: Capture,
    expected: Binding,
    prompt: &str,
    automation: &IUIAutomation,
    current_window: impl Fn() -> Option<(usize, u32, String)>,
) -> Result<(), String> {
    validate_send(&capture.app, prompt)?;
    if capture.app.binding != Some(expected) {
        return Err(
            "The active conversation changed. Check it in Cikung before sending again.".into(),
        );
    }
    let composer = capture
        .composer
        .ok_or("The app's message box is unavailable.")?;
    let root = capture.root.ok_or("The app window is unavailable.")?;
    let value = value(&composer)?;
    unsafe {
        value
            .SetValue(&::windows::core::BSTR::from(prompt))
            .map_err(error)?;
    }
    let result = (|| {
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(150));
            let buttons: Vec<_> = elements(&root, automation)?
                .into_iter()
                .filter(|n| {
                    control(n) == UIA_ButtonControlTypeId && visible(n) && is_send(&name(n))
                })
                .collect();
            if buttons.len() == 1 {
                let current = capture_window(capture.app.target, current_window(), automation)?;
                if current.app.binding != Some(expected)
                    || current.app.state != State::Ready
                    || !unsafe { value.CurrentValue().is_ok_and(|v| v == prompt) }
                {
                    return Err("The app or conversation changed before sending. Your message is kept in Cikung.".into());
                }
                // Send only a named submit button. Never invoke an approval action.
                let invoke: IUIAutomationInvokePattern = unsafe {
                    buttons[0]
                        .GetCurrentPatternAs(UIA_InvokePatternId)
                        .map_err(error)?
                };
                unsafe {
                    invoke.Invoke().map_err(error)?;
                }
                return Ok(());
            }
            if buttons.len() > 1 {
                return Err(
                    "More than one send button is available. Send from the app instead.".into(),
                );
            }
        }
        Err("The app did not expose a send button. Your message remains in Cikung.".into())
    })();
    if result.is_err() && unsafe { value.CurrentValue().is_ok_and(|s| s == prompt) } {
        unsafe {
            let _ = value.SetValue(&::windows::core::BSTR::from(""));
        }
    }
    result
}
pub(super) fn run(
    ctx: eframe::egui::Context,
    commands: Receiver<Command>,
    events: Sender<Event>,
    mut enabled: [bool; 2],
) {
    let initialized = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
    if !initialized {
        return;
    }
    let automation: Result<IUIAutomation, _> =
        unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) };
    let Ok(automation) = automation else {
        let _ = events.send(Event::Snapshot(Box::new(Target::ALL.map(|t| {
            AppSnapshot::empty(
                t,
                State::Unsupported,
                "Windows accessibility is unavailable.",
            )
        }))));
        ctx.request_repaint();
        unsafe {
            CoUninitialize();
        }
        return;
    };
    if let Ok(timeout) = automation.cast::<IUIAutomation2>() {
        unsafe {
            let _ = timeout.SetConnectionTimeout(1200);
            let _ = timeout.SetTransactionTimeout(1200);
        }
    }
    let mut last = Instant::now() - Duration::from_secs(10);
    let mut interval = Duration::from_secs(2);
    loop {
        match commands.recv_timeout(Duration::from_millis(250)) {
            Ok(Command::Quit) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Command::Configure(value)) => {
                enabled = value;
                last = Instant::now() - Duration::from_secs(10);
            }
            Ok(Command::Pace(resting)) => {
                interval = Duration::from_secs(if resting { 5 } else { 2 });
            }
            Ok(Command::Open(target)) => {
                let _ = events.send(Event::Action(false, open(target)));
                ctx.request_repaint();
                last = Instant::now() - Duration::from_secs(10);
            }
            Ok(Command::Send(target, binding, prompt)) => {
                let result = if enabled[target.index()] {
                    send(target, binding, &prompt, &automation)
                } else {
                    Err("This app link is paused.".into())
                };
                let _ = events.send(Event::Action(true, result));
                ctx.request_repaint();
                last = Instant::now() - Duration::from_secs(10);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if last.elapsed() >= interval {
            let apps = Target::ALL.map(|target| {
                if enabled[target.index()] {
                    capture(target, &automation)
                        .map(|c| c.app)
                        .unwrap_or_else(|e| AppSnapshot::empty(target, State::Loading, &e))
                } else {
                    AppSnapshot::empty(
                        target,
                        State::Paused,
                        "Enable the link to read this app's active conversation.",
                    )
                }
            });
            if events.send(Event::Snapshot(Box::new(apps))).is_err() {
                break;
            }
            ctx.request_repaint();
            last = Instant::now();
        }
    }
    drop(automation);
    unsafe {
        CoUninitialize();
    }
}

pub(super) fn probe() -> Result<[AppSnapshot; 2], String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(error)?;
    }
    let result = (|| {
        let automation: IUIAutomation = unsafe {
            CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).map_err(error)?
        };
        if let Ok(timeout) = automation.cast::<IUIAutomation2>() {
            unsafe {
                let _ = timeout.SetConnectionTimeout(1200);
                let _ = timeout.SetTransactionTimeout(1200);
            }
        }
        Ok(Target::ALL.map(|t| {
            capture(t, &automation)
                .map(|c| c.app)
                .unwrap_or_else(|e| AppSnapshot::empty(t, State::Loading, &e))
        }))
    })();
    unsafe {
        CoUninitialize();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_fixture_can_send_and_read_a_reply_without_touching_real_ai_apps() {
        use std::{
            fs,
            os::windows::process::CommandExt,
            path::PathBuf,
            process::{Child, Command},
        };
        struct Fixture {
            child: Child,
            folder: PathBuf,
            hwnd: Option<usize>,
        }
        impl Drop for Fixture {
            fn drop(&mut self) {
                if let Some(hwnd) = self.hwnd {
                    unsafe {
                        let _ = ::windows::Win32::UI::WindowsAndMessaging::PostMessageW(
                            Some(HWND(hwnd as *mut c_void)),
                            ::windows::Win32::UI::WindowsAndMessaging::WM_CLOSE,
                            ::windows::Win32::Foundation::WPARAM(0),
                            LPARAM(0),
                        );
                    }
                }
                for _ in 0..20 {
                    if self.child.try_wait().ok().flatten().is_some() {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                if self.child.try_wait().ok().flatten().is_none() {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                }
                let _ = fs::remove_file(self.folder.join("window.txt"));
                let _ = fs::remove_file(self.folder.join("window.complete"));
                let _ = fs::remove_dir(&self.folder);
            }
        }
        let folder = std::env::temp_dir().join(format!(
            "cikung-desktop-fixture-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&folder).unwrap();
        let ready = folder.join("window.txt");
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/desktop-app-fixture.ps1");
        let shell = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| "C:/Windows".into())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let child = Command::new(shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-STA",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(script)
            .arg("-ReadyFile")
            .arg(&ready)
            .creation_flags(0x0800_0000)
            .spawn()
            .unwrap();
        let mut fixture = Fixture {
            child,
            folder,
            hwnd: None,
        };
        let deadline = Instant::now() + Duration::from_secs(20);
        let hwnd = loop {
            if let Ok(text) = fs::read_to_string(&ready)
                && let Ok(hwnd) = text.trim().parse::<usize>()
            {
                break hwnd;
            }
            assert!(Instant::now() < deadline, "Native UI fixture did not start");
            std::thread::sleep(Duration::from_millis(100));
        };
        fixture.hwnd = Some(hwnd);
        unsafe {
            CoInitializeEx(None, COINIT_MULTITHREADED).ok().unwrap();
        }
        let automation: IUIAutomation =
            unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).unwrap() };
        let found = || Some((hwnd, fixture.child.id(), "Cikung Desktop Fixture".into()));
        let before = capture_window(Target::Codex, found(), &automation).unwrap();
        assert_eq!(before.app.state, State::Ready);
        let expected = before.app.binding.unwrap();
        let mut stale = expected;
        stale.context = stale.context.wrapping_add(1);
        assert!(
            submit(
                capture_window(Target::Codex, found(), &automation).unwrap(),
                stale,
                "Must not send",
                &automation,
                found
            )
            .is_err()
        );
        let mid_change = capture_window(Target::Codex, found(), &automation).unwrap();
        assert!(
            submit(
                mid_change,
                expected,
                "Must not submit after a switch",
                &automation,
                || None
            )
            .is_err()
        );
        let rolled_back = capture_window(Target::Codex, found(), &automation).unwrap();
        assert!(!rolled_back.app.draft_present);
        assert!(
            rolled_back
                .app
                .conversation
                .contains("Fixture conversation ready")
        );
        submit(before, expected, "Cikung test message", &automation, found).unwrap();
        std::thread::sleep(Duration::from_millis(200));
        let working = capture_window(Target::Codex, found(), &automation).unwrap();
        assert_eq!(working.app.state, State::Working);
        let mut completion = crate::desktop_apps::Completion::default();
        assert!(!completion.observe(&working.app, true));
        fs::write(ready.with_extension("complete"), "complete").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let after = loop {
            let capture = capture_window(Target::Codex, found(), &automation).unwrap();
            if capture.app.state == State::Ready {
                break capture;
            }
            assert!(Instant::now() < deadline, "Fixture did not finish");
            std::thread::sleep(Duration::from_millis(50));
        };
        assert!(!completion.observe(&after.app, true));
        let stable = capture_window(Target::Codex, found(), &automation).unwrap();
        assert!(completion.observe(&stable.app, true));
        assert!(!completion.observe(&stable.app, true));
        assert!(
            after
                .app
                .conversation
                .contains("Fixture reply: Cikung test message"),
            "The reply must be read from the same fixture window"
        );
        assert!(!after.app.draft_present);
        let composer = after.composer.as_ref().unwrap();
        unsafe {
            value(composer)
                .unwrap()
                .SetValue(&::windows::core::BSTR::from("Existing user draft"))
                .unwrap();
        }
        let drafted = capture_window(Target::Codex, found(), &automation).unwrap();
        assert!(
            submit(
                drafted,
                expected,
                "Do not replace the user draft",
                &automation,
                found
            )
            .is_err()
        );
        assert_eq!(
            unsafe { value(composer).unwrap().CurrentValue().unwrap().to_string() },
            "Existing user draft"
        );
        drop(automation);
        unsafe {
            CoUninitialize();
        }
    }
    #[test]
    fn cli_executables_and_similar_names_are_not_desktop_apps() {
        assert!(matches_app(
            Target::Codex,
            "C:/Program Files/WindowsApps/OpenAI.Codex_26_x64__id/app/ChatGPT.exe"
        ));
        assert!(matches_app(
            Target::Claude,
            "C:/Program Files/WindowsApps/Claude_2_x64__id/app/Claude.exe"
        ));
        assert!(!matches_app(
            Target::Codex,
            "C:/Users/pet/.codex/bin/codex.exe"
        ));
        assert!(!matches_app(
            Target::Claude,
            "C:/Users/pet/.local/bin/claude.exe"
        ));
        assert!(!matches_app(Target::Codex, "C:/Other/ChatGPT.exe"));
        assert!(!matches_app(Target::Claude, "C:/Other/claude.exe"));
    }
}
