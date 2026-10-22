//! Desktop connections use the apps' visible Windows accessibility interface.
//! No CLI sessions, account credentials, private databases, or browser debugging ports.
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver, Sender};
#[cfg(target_os = "windows")]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Target {
    Claude,
    Codex,
}
impl Target {
    pub const ALL: [Self; 2] = [Self::Claude, Self::Codex];
    pub fn index(self) -> usize {
        match self {
            Self::Claude => 0,
            Self::Codex => 1,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude App",
            Self::Codex => "Codex / ChatGPT App",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub enabled: [bool; 2],
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: [true, true],
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum State {
    Preview,
    Paused,
    Closed,
    Loading,
    Ready,
    Working,
    Attention,
    Unsupported,
}
impl State {
    pub fn label(self) -> &'static str {
        match self {
            Self::Preview => "Preview",
            Self::Paused => "Link paused",
            Self::Closed => "App closed",
            Self::Loading => "Waiting for app",
            Self::Ready => "Connected",
            Self::Working => "Working",
            Self::Attention => "Needs attention",
            Self::Unsupported => "Not supported on this platform",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Binding {
    pub hwnd: usize,
    pub pid: u32,
    pub context: u64,
}
#[derive(Clone, Debug, Serialize)]
pub struct AppSnapshot {
    pub target: Target,
    pub state: State,
    pub binding: Option<Binding>,
    pub title: String,
    pub conversation: String,
    pub detail: String,
    pub can_send: bool,
    pub draft_present: bool,
}
impl AppSnapshot {
    fn empty(target: Target, state: State, detail: &str) -> Self {
        Self {
            target,
            state,
            binding: None,
            title: String::new(),
            conversation: String::new(),
            detail: detail.into(),
            can_send: false,
            draft_present: false,
        }
    }
}
#[derive(Default)]
struct Completion {
    working: Option<(usize, u32, String)>,
    ready_context: Option<u64>,
}
impl Completion {
    fn observe(&mut self, app: &AppSnapshot, enabled: bool) -> bool {
        let Some(binding) = app.binding.filter(|_| enabled) else {
            *self = Self::default();
            return false;
        };
        match app.state {
            State::Working => {
                self.working = Some((binding.hwnd, binding.pid, app.title.clone()));
                self.ready_context = None;
            }
            State::Ready => {
                let same_chat = self.working.as_ref().is_some_and(|(hwnd, pid, title)| {
                    *hwnd == binding.hwnd && *pid == binding.pid && title == &app.title
                });
                if !same_chat {
                    *self = Self::default();
                } else if self.ready_context == Some(binding.context) {
                    *self = Self::default();
                    return true;
                } else {
                    // Require two stable ready snapshots, not a transient missing Stop button.
                    self.ready_context = Some(binding.context);
                }
            }
            _ => *self = Self::default(),
        }
        false
    }
}

#[cfg_attr(not(windows), allow(dead_code))]
pub enum Event {
    Snapshot(Box<[AppSnapshot; 2]>),
    Action(bool, Result<(), String>),
}
#[cfg_attr(not(windows), allow(dead_code))]
enum Command {
    Configure([bool; 2]),
    Open(Target),
    Send(Target, Binding, String),
    Quit,
}
pub struct Manager {
    tx: Option<Sender<Command>>,
    rx: Receiver<Event>,
    pub apps: [AppSnapshot; 2],
    pub pending: bool,
    enabled: [bool; 2],
    completion: [Completion; 2],
    completed: Vec<Target>,
}
impl Manager {
    pub fn new(ctx: eframe::egui::Context, active: bool, enabled: [bool; 2]) -> Self {
        let (event_tx, rx) = mpsc::channel();
        let apps = Target::ALL.map(|target| {
            AppSnapshot::empty(
                target,
                if active {
                    State::Loading
                } else {
                    State::Preview
                },
                "Open an AI app to connect its active conversation.",
            )
        });
        let tx = if active {
            let (tx, commands) = mpsc::channel();
            #[cfg(target_os = "windows")]
            let started = std::thread::Builder::new()
                .name("Cikung desktop apps".into())
                .spawn(move || windows::run(ctx, commands, event_tx, enabled))
                .is_ok();
            #[cfg(not(target_os = "windows"))]
            let started = {
                drop(commands);
                let _ = event_tx.send(Event::Snapshot(Box::new(Target::ALL.map(|t| {
                    AppSnapshot::empty(
                        t,
                        State::Unsupported,
                        "Desktop app links currently require Windows accessibility.",
                    )
                }))));
                ctx.request_repaint();
                false
            };
            started.then_some(tx)
        } else {
            drop(event_tx);
            None
        };
        Self {
            tx,
            rx,
            apps,
            pending: false,
            enabled,
            completion: Default::default(),
            completed: vec![],
        }
    }
    pub fn configure(&mut self, enabled: [bool; 2]) {
        if enabled != self.enabled {
            self.enabled = enabled;
            self.completed.retain(|target| enabled[target.index()]);
            for (index, enabled) in enabled.into_iter().enumerate() {
                if !enabled {
                    self.completion[index] = Completion::default();
                }
            }
            if let Some(tx) = &self.tx {
                let _ = tx.send(Command::Configure(enabled));
            }
        }
    }
    pub fn poll(&mut self) -> Vec<(bool, Result<(), String>)> {
        let mut results = Vec::new();
        for event in self.rx.try_iter() {
            match event {
                Event::Snapshot(apps) => {
                    for app in apps.iter() {
                        let index = app.target.index();
                        if self.completion[index].observe(app, self.enabled[index]) {
                            self.completed.push(app.target);
                        }
                    }
                    self.apps = *apps;
                }
                Event::Action(sent, result) => {
                    if sent {
                        self.pending = false;
                    }
                    results.push((sent, result));
                }
            }
        }
        results
    }
    pub fn take_completions(&mut self) -> Vec<Target> {
        std::mem::take(&mut self.completed)
    }
    pub fn open(&self, target: Target) -> Result<(), String> {
        self.tx
            .as_ref()
            .ok_or("Desktop links are unavailable in preview.")?
            .send(Command::Open(target))
            .map_err(|_| "Desktop link stopped.".into())
    }
    pub fn send(&mut self, target: Target, prompt: String) -> Result<(), String> {
        let app = &self.apps[target.index()];
        validate_send(app, &prompt)?;
        if self.pending {
            return Err("A message is already being sent.".into());
        }
        self.tx
            .as_ref()
            .ok_or("Desktop link is unavailable.")?
            .send(Command::Send(
                target,
                app.binding.ok_or("Open the app first.")?,
                prompt,
            ))
            .map_err(|_| "Desktop link stopped.")?;
        self.pending = true;
        Ok(())
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Quit);
        }
    }
}
fn validate_send(app: &AppSnapshot, prompt: &str) -> Result<(), String> {
    if prompt.trim().is_empty() {
        return Err("Write a message first.".into());
    }
    if prompt.len() > 32_000 {
        return Err("Keep the message under 32 KB.".into());
    }
    if app.state != State::Ready || !app.can_send || app.binding.is_none() {
        return Err("Wait until the app is connected and ready.".into());
    }
    if app.draft_present {
        return Err("The app already has a draft. Send or clear it in the app first.".into());
    }
    Ok(())
}
#[cfg(any(windows, test))]
fn is_send(name: &str) -> bool {
    matches!(
        name.trim().to_lowercase().as_str(),
        "send" | "send message" | "send prompt" | "submit" | "kirim" | "kirim pesan"
    )
}
#[cfg(any(windows, test))]
fn is_stop(name: &str) -> bool {
    matches!(
        name.trim().to_lowercase().as_str(),
        "stop"
            | "stop generating"
            | "stop response"
            | "stop streaming"
            | "stop generation"
            | "hentikan"
    )
}
#[cfg(any(windows, test))]
fn is_attention(name: &str) -> bool {
    matches!(
        name.trim().to_lowercase().as_str(),
        "allow"
            | "allow once"
            | "approve"
            | "approve once"
            | "sign in"
            | "log in"
            | "continue with google"
            | "continue with email"
    )
}
#[cfg(any(windows, test))]
fn has_draft(value: &str, label: &str, rich_editor: bool, submit_enabled: bool) -> bool {
    let text = value.trim();
    if text.is_empty() {
        return false;
    }
    // This Codex/ChatGPT build exposes its empty-editor hint as UIA Value text.
    // A real draft enables Send/Queue, even if its text happens to match the hint.
    !(rich_editor && label == "Work with ChatGPT" && text == label && !submit_enabled)
}

#[cfg(any(windows, test))]
fn context_key(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hash);
    hash.finish()
}
#[cfg(any(windows, test))]
fn append_bounded(out: &mut String, line: &str) {
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.extend(line.chars().take(16_000));
    if out.len() > 24_000 {
        let mut start = out.len() - 24_000;
        while !out.is_char_boundary(start) {
            start += 1;
        }
        out.drain(..start);
    }
}

pub fn probe() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        Ok(
            serde_json::json!({"apps": windows::probe()?.map(|app| serde_json::json!({"app":app.target.label(),"state":app.state,"can_send":app.can_send,"draft_present":app.draft_present,"visible_conversation_characters":app.conversation.chars().count(),"detail":app.detail}))}),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Desktop app links require Windows accessibility.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn completion_snapshot(state: State, context: u64) -> AppSnapshot {
        let mut app = AppSnapshot::empty(Target::Codex, state, "");
        app.binding = Some(Binding {
            hwnd: 12,
            pid: 44,
            context,
        });
        app.title = "Test conversation".into();
        app
    }
    #[test]
    fn completions_require_observed_work_and_two_stable_ready_snapshots() {
        let mut tracker = Completion::default();
        let ready = completion_snapshot(State::Ready, 10);
        assert!(!tracker.observe(&ready, true));
        assert!(!tracker.observe(&ready, true));
        assert!(!tracker.observe(&completion_snapshot(State::Working, 9), true));
        assert!(!tracker.observe(&ready, true));
        assert!(tracker.observe(&ready, true));
        assert!(!tracker.observe(&ready, true));
        assert!(!tracker.observe(&completion_snapshot(State::Working, 10), true));
        assert!(!tracker.observe(&completion_snapshot(State::Working, 11), true));
        assert!(!tracker.observe(&completion_snapshot(State::Ready, 12), true));
        assert!(!tracker.observe(&completion_snapshot(State::Ready, 13), true));
        assert!(tracker.observe(&completion_snapshot(State::Ready, 13), true));
    }
    #[test]
    fn attention_disconnect_and_pausing_are_not_completions() {
        for state in [
            State::Attention,
            State::Closed,
            State::Loading,
            State::Paused,
            State::Unsupported,
        ] {
            let mut tracker = Completion::default();
            tracker.observe(&completion_snapshot(State::Working, 1), true);
            assert!(!tracker.observe(&completion_snapshot(state, 2), true));
            assert!(!tracker.observe(&completion_snapshot(State::Ready, 2), true));
            assert!(!tracker.observe(&completion_snapshot(State::Ready, 2), true));
        }
        let mut tracker = Completion::default();
        tracker.observe(&completion_snapshot(State::Working, 1), true);
        assert!(!tracker.observe(&completion_snapshot(State::Ready, 2), false));
        assert!(!tracker.observe(&completion_snapshot(State::Ready, 2), true));
    }
    #[test]
    fn changing_window_process_or_chat_does_not_announce_completion() {
        for changed in 0..3 {
            let mut tracker = Completion::default();
            tracker.observe(&completion_snapshot(State::Working, 1), true);
            let mut ready = completion_snapshot(State::Ready, 2);
            match changed {
                0 => ready.binding.as_mut().unwrap().hwnd += 1,
                1 => ready.binding.as_mut().unwrap().pid += 1,
                _ => ready.title = "Different conversation".into(),
            }
            assert!(!tracker.observe(&ready, true));
            assert!(!tracker.observe(&ready, true));
        }
    }
    #[test]
    fn queued_snapshots_preserve_completions_and_are_drained_once() {
        let mut manager = Manager::new(eframe::egui::Context::default(), false, [true; 2]);
        let (tx, rx) = mpsc::channel();
        manager.rx = rx;
        for state in [State::Working, State::Ready, State::Ready] {
            tx.send(Event::Snapshot(Box::new([
                AppSnapshot::empty(Target::Claude, State::Closed, ""),
                completion_snapshot(state, 1),
            ])))
            .unwrap();
        }
        assert!(manager.poll().is_empty());
        assert_eq!(manager.take_completions(), vec![Target::Codex]);
        assert!(manager.take_completions().is_empty());
        manager.completed.push(Target::Claude);
        manager.configure([false, true]);
        assert!(manager.take_completions().is_empty());
    }
    #[test]
    fn only_ready_bound_empty_composers_can_send() {
        let mut app = AppSnapshot::empty(Target::Codex, State::Ready, "");
        app.binding = Some(Binding {
            hwnd: 12,
            pid: 44,
            context: 7,
        });
        app.can_send = true;
        assert!(validate_send(&app, "Hello").is_ok());
        for state in [
            State::Working,
            State::Closed,
            State::Loading,
            State::Attention,
            State::Paused,
            State::Unsupported,
        ] {
            app.state = state;
            assert!(validate_send(&app, "Hello").is_err());
        }
        app.state = State::Ready;
        app.draft_present = true;
        assert!(validate_send(&app, "Hello").is_err());
        app.draft_present = false;
        assert!(validate_send(&app, " ").is_err());
        assert!(validate_send(&app, &"x".repeat(32_001)).is_err());
        app.binding = None;
        assert!(validate_send(&app, "Hello").is_err());
    }
    #[test]
    fn codex_editor_hint_is_not_a_draft_but_user_text_is_preserved() {
        assert!(!has_draft(
            "Work with ChatGPT\n",
            "Work with ChatGPT",
            true,
            false
        ));
        assert!(has_draft(
            "Work with ChatGPT\n",
            "Work with ChatGPT",
            true,
            true
        ));
        assert!(has_draft(
            "An existing user message",
            "Work with ChatGPT",
            true,
            false
        ));
        assert!(has_draft("Work with ChatGPT", "Another box", true, false));
        assert!(has_draft(
            "Work with ChatGPT",
            "Work with ChatGPT",
            false,
            false
        ));
        assert!(!has_draft("\r\n", "Prompt", true, false));
    }
    #[test]
    fn submit_names_never_match_approvals_or_unrelated_actions() {
        assert!(is_send("Send message"));
        assert!(is_stop("Stop generating"));
        for name in [
            "Allow",
            "Approve once",
            "Delete",
            "Send to another task",
            "New chat",
            "Run",
        ] {
            assert!(!is_send(name));
        }
        assert!(is_attention("Approve once"));
    }
    #[test]
    fn conversation_is_bounded_unicode_and_context_changes_are_detected() {
        let mut text = String::new();
        for _ in 0..100 {
            append_bounded(&mut text, &"Kucing 😺 ".repeat(100));
        }
        assert!(text.len() <= 24_000);
        assert!(std::str::from_utf8(text.as_bytes()).is_ok());
        assert_ne!(context_key("Chat one"), context_key("Chat two"));
    }
}
