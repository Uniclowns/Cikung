//! Read-only Windows notifications. Message contents never leave this monitor and the notch.
use std::{
    collections::{HashMap, HashSet},
    sync::mpsc,
    time::Duration,
};

type Key = (u32, i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Access {
    Checking,
    Allowed,
    Denied,
    Unspecified,
    Unavailable,
}
impl Access {
    pub fn label(self) -> &'static str {
        match self {
            Self::Checking => "Checking Windows notification access…",
            Self::Allowed => "Connected to Windows notifications",
            Self::Denied => "Access is blocked. Allow notification access in Windows Settings.",
            Self::Unspecified => "Windows notification permission is needed.",
            Self::Unavailable => "Windows notification access is unavailable.",
        }
    }
}

#[derive(Clone)]
pub struct Message {
    key: Key,
    title: String,
    body: String,
}
impl Message {
    pub fn text(&self, preview: bool) -> String {
        if !preview {
            return "New notification".into();
        }
        match (self.title.is_empty(), self.body.is_empty()) {
            (false, false) => format!("{} · {}", self.title, self.body),
            (false, true) => self.title.clone(),
            (true, false) => self.body.clone(),
            (true, true) => "New notification".into(),
        }
    }
}

struct Snapshot {
    access: Access,
    items: Vec<Message>,
}
#[derive(Default)]
struct Changes {
    seen: Option<HashSet<Key>>,
}
impl Changes {
    fn update(&mut self, snapshot: Snapshot) -> Vec<Message> {
        if snapshot.access != Access::Allowed {
            self.seen = None;
            return vec![];
        }
        let next = snapshot.items.iter().map(|m| m.key).collect();
        let Some(previous) = self.seen.replace(next) else {
            return vec![];
        };
        snapshot
            .items
            .into_iter()
            .filter(|m| !previous.contains(&m.key))
            .collect()
    }
}

#[cfg(any(windows, test))]
fn is_whatsapp(app_id: &str, display_name: &str) -> bool {
    let id = app_id.to_ascii_lowercase();
    let family = id.split('!').next().unwrap_or_default();
    matches!(
        family,
        "5319275a.whatsappdesktop_cv1g1gvanyjgm"
            | "5319275a.whatsappbeta_cv1g1gvanyjgm"
            | "com.squirrel.whatsapp.whatsapp"
    ) || matches!(
        display_name.trim().to_ascii_lowercase().as_str(),
        "whatsapp"
            | "whatsapp beta"
            | "whatsapp desk"
            | "whatsappdesk"
            | "whatsapp desk 2"
            | "whatsappdesk_2"
    )
}

#[cfg(any(windows, test))]
fn clean(text: &str, limit: usize) -> String {
    text.chars()
        .filter_map(|c| {
            if c.is_whitespace() {
                Some(' ')
            } else if c.is_control() {
                None
            } else {
                Some(c)
            }
        })
        .take(limit)
        .collect::<String>()
        .trim()
        .to_owned()
}

struct Update {
    access: Access,
    messages: Vec<Message>,
    error: Option<String>,
}
pub struct Monitor {
    _stop: mpsc::Sender<()>,
    rx: mpsc::Receiver<Update>,
    pub access: Access,
    pub error: Option<String>,
    pub preview: bool,
}
impl Monitor {
    pub fn start(ctx: eframe::egui::Context, preview: bool) -> Self {
        let (stop, commands) = mpsc::channel();
        let (tx, rx) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("Cikung WhatsApp notifications".into())
            .spawn(move || {
                let apartment = platform::Apartment::new(false);
                let mut changes = Changes::default();
                // Which app posted each notification, so a poll reads app info only for new ones.
                let mut apps = HashMap::new();
                loop {
                    let result = apartment.as_ref().map_err(Clone::clone).and_then(|_| {
                        platform::snapshot(preview, changes.seen.as_ref(), &mut apps)
                    });
                    let update = match result {
                        Ok(snapshot) => Update {
                            access: snapshot.access,
                            messages: changes.update(snapshot),
                            error: None,
                        },
                        Err(error) => Update {
                            access: Access::Unavailable,
                            messages: vec![],
                            error: Some(error),
                        },
                    };
                    if tx.send(update).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                    if commands.recv_timeout(Duration::from_millis(500))
                        != Err(mpsc::RecvTimeoutError::Timeout)
                    {
                        break;
                    }
                }
            });
        Self {
            _stop: stop,
            rx,
            preview,
            access: Access::Checking,
            error: worker
                .err()
                .map(|_| "Could not start the notification reader.".into()),
        }
    }
    pub fn poll(&mut self) -> Vec<Message> {
        let mut messages = Vec::new();
        for update in self.rx.try_iter() {
            self.access = update.access;
            self.error = update.error;
            if self.access != Access::Allowed {
                messages.clear();
            }
            messages.extend(update.messages);
        }
        messages
    }
}

#[derive(serde::Serialize)]
pub struct Probe {
    access: Access,
    whatsapp_notifications: usize,
}
pub fn probe() -> Result<Probe, String> {
    let _apartment = platform::Apartment::new(false)?;
    let snapshot = platform::snapshot(false, None, &mut HashMap::new())?;
    Ok(Probe {
        access: snapshot.access,
        whatsapp_notifications: snapshot.items.len(),
    })
}
pub fn request_access(
    ctx: eframe::egui::Context,
) -> Result<mpsc::Receiver<Result<Access, String>>, String> {
    platform::request_access(ctx)
}
pub fn open_access_settings() -> Result<(), String> {
    platform::open_access_settings()
}

#[cfg(windows)]
mod platform {
    use super::*;
    use windows::{
        UI::Notifications::{
            KnownNotificationBindings,
            Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
            NotificationKinds,
        },
        Win32::System::WinRT::{
            RO_INIT_MULTITHREADED, RO_INIT_SINGLETHREADED, RoInitialize, RoUninitialize,
        },
    };
    fn error(e: windows::core::Error) -> String {
        format!(
            "Windows notification access is unavailable ({}). Check notification access in Windows Settings.",
            e.code()
        )
    }
    pub struct Apartment;
    impl Apartment {
        pub fn new(ui: bool) -> Result<Self, String> {
            unsafe {
                RoInitialize(if ui {
                    RO_INIT_SINGLETHREADED
                } else {
                    RO_INIT_MULTITHREADED
                })
            }
            .map_err(error)?;
            Ok(Self)
        }
    }
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    fn access(status: UserNotificationListenerAccessStatus) -> Access {
        match status {
            UserNotificationListenerAccessStatus::Allowed => Access::Allowed,
            UserNotificationListenerAccessStatus::Denied => Access::Denied,
            _ => Access::Unspecified,
        }
    }
    pub fn snapshot(
        preview: bool,
        seen: Option<&HashSet<Key>>,
        apps: &mut HashMap<Key, bool>,
    ) -> Result<Snapshot, String> {
        let listener = UserNotificationListener::Current().map_err(error)?;
        let status = access(listener.GetAccessStatus().map_err(error)?);
        if status != Access::Allowed {
            return Ok(Snapshot {
                access: status,
                items: vec![],
            });
        }
        let notifications = listener
            .GetNotificationsAsync(NotificationKinds::Toast)
            .map_err(error)?
            .join()
            .map_err(error)?;
        let mut items = Vec::new();
        let mut current = HashSet::new();
        for notification in notifications {
            // A dismissed or malformed notification must not prevent processing the others.
            let mut read = || -> windows::core::Result<Option<Message>> {
                let key = (
                    notification.Id()?,
                    notification.CreationTime()?.UniversalTime,
                );
                current.insert(key);
                let whatsapp = match apps.get(&key) {
                    Some(&known) => known,
                    None => {
                        let info = notification.AppInfo()?;
                        let app_id = info.AppUserModelId()?.to_string();
                        let name = info
                            .DisplayInfo()
                            .and_then(|d| d.DisplayName())
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        let known = is_whatsapp(&app_id, &name);
                        apps.insert(key, known);
                        known
                    }
                };
                if !whatsapp {
                    return Ok(None);
                }
                let mut message = Message {
                    key,
                    title: String::new(),
                    body: String::new(),
                };
                if preview
                    && seen.is_some_and(|seen| !seen.contains(&key))
                    && let Ok(text) = notification
                        .Notification()
                        .and_then(|n| n.Visual())
                        .and_then(|v| v.GetBinding(&KnownNotificationBindings::ToastGeneric()?))
                        .and_then(|b| b.GetTextElements())
                {
                    let mut lines = text
                        .into_iter()
                        .filter_map(|t| t.Text().ok())
                        .map(|s| clean(&s.to_string(), 240));
                    message.title = lines.next().unwrap_or_default();
                    message.body = clean(&lines.take(4).collect::<Vec<_>>().join(" · "), 360);
                }
                Ok(Some(message))
            };
            if let Ok(Some(message)) = read() {
                items.push(message);
            }
        }
        apps.retain(|key, _| current.contains(key));
        // A revoked permission can otherwise look like an empty notification list.
        let status = access(listener.GetAccessStatus().map_err(error)?);
        Ok(Snapshot {
            access: status,
            items,
        })
    }
    pub fn request_access(
        ctx: eframe::egui::Context,
    ) -> Result<mpsc::Receiver<Result<Access, String>>, String> {
        let _apartment = Apartment::new(true)?;
        let operation = UserNotificationListener::Current()
            .map_err(error)?
            .RequestAccessAsync()
            .map_err(error)?;
        let (tx, rx) = mpsc::channel();
        operation
            .when(move |result| {
                let _ = tx.send(result.map(access).map_err(error));
                ctx.request_repaint();
            })
            .map_err(error)?;
        Ok(rx)
    }
    pub fn open_access_settings() -> Result<(), String> {
        use windows::{
            Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
            core::w,
        };
        let result = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                w!("ms-settings:privacy-notifications"),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };
        if result.0 as isize > 32 {
            Ok(())
        } else {
            Err("Could not open Windows notification access settings.".into())
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub struct Apartment;
    impl Apartment {
        pub fn new(_: bool) -> Result<Self, String> {
            Err("WhatsApp notifications currently require Windows.".into())
        }
    }
    pub fn snapshot(
        _: bool,
        _: Option<&HashSet<Key>>,
        _: &mut HashMap<Key, bool>,
    ) -> Result<Snapshot, String> {
        Err("WhatsApp notifications currently require Windows.".into())
    }
    pub fn request_access(
        _: eframe::egui::Context,
    ) -> Result<mpsc::Receiver<Result<Access, String>>, String> {
        Err("WhatsApp notifications currently require Windows.".into())
    }
    pub fn open_access_settings() -> Result<(), String> {
        Err("WhatsApp notifications currently require Windows.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot(keys: &[(u32, i64)]) -> Snapshot {
        Snapshot {
            access: Access::Allowed,
            items: keys
                .iter()
                .map(|&key| Message {
                    key,
                    title: "A sender".into(),
                    body: "A message".into(),
                })
                .collect(),
        }
    }
    #[test]
    fn startup_is_silent_new_messages_are_once_and_reused_ids_are_detected() {
        let mut changes = Changes::default();
        assert!(changes.update(snapshot(&[(1, 100)])).is_empty());
        assert_eq!(changes.update(snapshot(&[(1, 100), (2, 101)])).len(), 1);
        assert!(changes.update(snapshot(&[(1, 100), (2, 101)])).is_empty());
        assert_eq!(changes.update(snapshot(&[(1, 102), (2, 101)])).len(), 1);
        assert!(changes.update(snapshot(&[])).is_empty());
    }
    #[test]
    fn revoked_access_discards_messages_and_regrant_is_a_quiet_baseline() {
        let mut changes = Changes::default();
        changes.update(snapshot(&[]));
        let mut blocked = snapshot(&[(3, 100)]);
        blocked.access = Access::Denied;
        assert!(changes.update(blocked).is_empty());
        assert!(changes.update(snapshot(&[(3, 100)])).is_empty());
        assert_eq!(changes.update(snapshot(&[(3, 100), (4, 101)])).len(), 1);
    }
    #[test]
    fn filtering_uses_app_identity_instead_of_message_content() {
        assert!(is_whatsapp(
            "5319275A.WhatsAppDesktop_cv1g1gvanyjgm!App",
            ""
        ));
        assert!(is_whatsapp("portable.app", "WhatsApp Desk"));
        assert!(is_whatsapp("portable.app", "WhatsAppDesk_2"));
        assert!(!is_whatsapp("com.cikung.desktop", "Cikung"));
        assert!(!is_whatsapp("chrome", "Google Chrome"));
        assert!(!is_whatsapp("not-whatsapp", "WhatsApp backup"));
    }
    #[test]
    fn hidden_previews_never_reveal_sender_or_body_and_text_is_bounded() {
        let message = Message {
            key: (1, 1),
            title: "Sender".into(),
            body: "Private body".into(),
        };
        assert_eq!(message.text(false), "New notification");
        assert_eq!(message.text(true), "Sender · Private body");
        assert_eq!(clean("  hello\nworld\0  ", 100), "hello world");
        assert_eq!(clean(&"猫".repeat(1000), 240).chars().count(), 240);
    }
}
