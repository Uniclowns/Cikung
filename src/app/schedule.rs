//! The chat assistant's task cards and the Google Calendar sync for tasks with a due time.
use super::*;
use crate::{
    google_calendar,
    panels::{format_task_due, parse_task_due},
    tools::{CalendarEvent, Task, calendar_signature},
};
use serde_json::Value;

const BLOCK: &str = "```cikung-task";
const GUIDE: &str = "https://github.com/Uniclowns/Cikung/blob/main/docs/google-calendar.md";

/// A task the assistant proposed; the user adds or dismisses it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Proposal {
    pub text: String,
    pub due_unix: Option<u64>,
}
impl Proposal {
    fn from_json(value: &Value) -> Option<Self> {
        let text = value["text"].as_str()?.trim();
        if text.is_empty() {
            return None;
        }
        Some(Self {
            text: text.chars().take(2_000).collect(),
            due_unix: value["due"]
                .as_str()
                .and_then(|due| parse_task_due(due).ok().flatten()),
        })
    }
}

#[derive(Default)]
pub(crate) struct Schedule {
    pub proposals: Vec<Proposal>,
    /// Whether a Google sign-in is saved; read from the credential store once.
    connected: Option<bool>,
    /// The sign-in page while the browser is out, to open again if it did not.
    signing_in: Option<String>,
    /// One calendar request at a time, so chat keeps its request slots.
    busy: bool,
    /// After a failure, wait before trying again rather than every frame.
    retry_at: Option<Instant>,
    client_id: String,
    client_secret: String,
}

/// Sent ahead of every chat request; not kept in the conversation.
pub(super) fn assistant_prompt(now: chrono::DateTime<chrono::Local>) -> ChatMessage {
    ChatMessage {
        role: "system".into(),
        content: format!(
            "You are Cikung, a concise assistant in a desktop notch app. Reply in the user's language. \
The user's local time is {} ({}).\n\
When the user asks you to add a task, reminder or appointment, confirm it in one short sentence \
and end the reply with one block per task:\n\
{BLOCK}\n{{\"text\": \"short task title\", \"due\": \"YYYY-MM-DD HH:MM\"}}\n```\n\
Write \"due\" in the user's local time, working out words like tomorrow or next Monday from the \
time above, and leave it out when no time is given. Cikung turns each block into a card the user \
confirms. Never write the block otherwise.",
            now.format("%A, %Y-%m-%d %H:%M"),
            now.format("UTC%:z"),
        ),
    }
}

/// Splits the assistant's task blocks out of its reply: the text to show, and the tasks.
pub(super) fn take_proposals(reply: &str) -> (String, Vec<Proposal>) {
    let mut text = String::new();
    let mut proposals = Vec::new();
    let mut rest = reply;
    while let Some(start) = rest.find(BLOCK) {
        text.push_str(&rest[..start]);
        let after = &rest[start + BLOCK.len()..];
        let (block, tail) = after.split_once("```").unwrap_or((after, ""));
        // One object per task, an array, or several objects in a row all work.
        for value in serde_json::Deserializer::from_str(block)
            .into_iter::<Value>()
            .map_while(Result::ok)
        {
            match value {
                Value::Array(items) => {
                    proposals.extend(items.iter().filter_map(Proposal::from_json))
                }
                value => proposals.extend(Proposal::from_json(&value)),
            }
        }
        rest = tail;
    }
    text.push_str(rest);
    (text.trim().to_owned(), proposals)
}

fn needs_sync(task: &Task) -> bool {
    !task.done
        && task.due_unix.is_some()
        && task.google_event.as_ref().map(|event| event.synced) != Some(calendar_signature(task))
}

impl Cikung {
    /// Cards for the tasks the assistant proposed, under the conversation.
    pub(crate) fn task_proposals(&mut self, ui: &mut egui::Ui) {
        let mut answer = None;
        for (index, proposal) in self.schedule.proposals.iter().enumerate() {
            ui.add_space(6.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(&proposal.text).strong());
                let due = proposal.due_unix.map_or_else(
                    || "No due time".to_owned(),
                    |due| format!("Due {}", format_task_due(due)),
                );
                ui.label(RichText::new(due).size(11.0).color(muted()));
                ui.horizontal(|ui| {
                    if ui.button("Add task").clicked() {
                        answer = Some((index, true));
                    }
                    if ui.button("Dismiss").clicked() {
                        answer = Some((index, false));
                    }
                });
            });
        }
        let Some((index, add)) = answer else {
            return;
        };
        let proposal = self.schedule.proposals.remove(index);
        if add {
            let calendar = proposal.due_unix.is_some() && self.calendar_on();
            let result = self
                .tools
                .add_task(&proposal.text)
                .and_then(|id| self.tools.edit_task(id, &proposal.text, proposal.due_unix));
            self.report(
                result,
                if calendar {
                    "Task added · adding it to Google Calendar."
                } else {
                    "Task added."
                },
            );
        }
    }

    fn calendar_on(&self) -> bool {
        self.schedule.connected == Some(true) && self.prefs.calendar_sync
    }

    /// While signed in, saves or removes one event at a time: deleted tasks' events first, then
    /// tasks whose text or due time changed since their last save.
    pub(super) fn sync_calendar(&mut self, ctx: &egui::Context) {
        if self.demo || self.capture.is_some() || self.audit.is_some() {
            return;
        }
        if self.schedule.busy || self.schedule.signing_in.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
            return;
        }
        if self.schedule.connected.is_none() {
            self.schedule.connected = Some(google_calendar::connected());
        }
        if !self.calendar_on() || self.schedule.retry_at.is_some_and(|at| at > Instant::now()) {
            return;
        }
        let started = if let Some(event_id) = self.tools.calendar_orphans.first().cloned() {
            self.cloud.delete_calendar_event(event_id)
        } else if let Some(task) = self.tools.tasks.iter().find(|task| needs_sync(task)) {
            self.cloud.save_calendar_event(
                task.id,
                calendar_signature(task),
                task.google_event.as_ref().map(|event| event.id.clone()),
                task.text.clone(),
                task.due_unix.unwrap_or_default(),
            )
        } else {
            return;
        };
        // Every request slot taken: try again on the next tick.
        self.schedule.busy = started.is_ok();
    }

    pub(super) fn calendar_event(&mut self, event: CloudEvent) {
        match event {
            CloudEvent::GoogleSignIn(result) => {
                self.schedule.signing_in = None;
                match result {
                    Ok(()) => {
                        self.schedule.connected = Some(true);
                        self.schedule.retry_at = None;
                        self.schedule.client_secret.clear();
                        self.toast(
                            "Google Calendar",
                            "Connected. Tasks with a due time now appear in your calendar.",
                        );
                    }
                    Err(error) => self.toast("Google Calendar", error),
                }
            }
            CloudEvent::CalendarSaved {
                task_id,
                signature,
                result,
            } => {
                self.schedule.busy = false;
                match result {
                    Ok(id) => match self
                        .tools
                        .tasks
                        .iter_mut()
                        .find(|task| task.id == task_id && task.due_unix.is_some())
                    {
                        Some(task) => {
                            task.google_event = Some(CalendarEvent {
                                id,
                                synced: signature,
                            })
                        }
                        // Deleted, or its due time cleared, while saving: take it out again.
                        None => self.tools.calendar_orphans.push(id),
                    },
                    Err(error) => self.calendar_failed(error),
                }
            }
            CloudEvent::CalendarDeleted { event_id, result } => {
                self.schedule.busy = false;
                match result {
                    Ok(()) => self.tools.calendar_orphans.retain(|id| *id != event_id),
                    Err(error) => self.calendar_failed(error),
                }
            }
            _ => {}
        }
    }

    fn calendar_failed(&mut self, error: String) {
        // One message, then a two-minute pause, so an offline PC does not retry every frame.
        self.schedule.retry_at = Some(Instant::now() + Duration::from_secs(120));
        self.toast("Google Calendar", error);
    }

    pub(super) fn google_calendar_settings(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Google Calendar",
            "Tasks with a due time, in your calendar.",
        );
        if self.demo || self.capture.is_some() || self.audit.is_some() {
            ui.label(RichText::new("Preview · Google is not contacted.").color(muted()));
            return;
        }
        if self.schedule.connected.is_none() {
            self.schedule.connected = Some(google_calendar::connected());
            if let Ok(Some(id)) = crate::cloud::read_secret(google_calendar::CLIENT_ID) {
                self.schedule.client_id = id;
            }
        }
        if self.schedule.connected == Some(true) {
            ui.checkbox(
                &mut self.prefs.calendar_sync,
                "Add tasks with a due time to Google Calendar",
            );
            ui.label(RichText::new("Each becomes a 30-minute event in your primary calendar. Editing a task moves its event; deleting the task or its due time removes it. Finished tasks keep theirs.").size(11.0).color(muted()));
            if ui.button("Disconnect Google").clicked() {
                let result = google_calendar::disconnect();
                self.schedule.connected = Some(false);
                self.report(result, "Google Calendar disconnected.");
            }
            return;
        }
        ui.label(
            RichText::new(
                "Connect with your own Google Cloud OAuth client (type Desktop app). The setup guide walks through it once.",
            )
            .color(muted()),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.schedule.client_id)
                .hint_text("OAuth client ID")
                .desired_width(f32::INFINITY),
        );
        ui.add(
            egui::TextEdit::singleline(&mut self.schedule.client_secret)
                .password(true)
                .hint_text("OAuth client secret")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal_wrapped(|ui| {
            match self.schedule.signing_in.clone() {
                Some(url) => {
                    ui.spinner();
                    ui.label("Waiting for Google…");
                    ui.hyperlink_to("Open the sign-in page again", url);
                }
                None => {
                    if ui.button("Connect Google").clicked() {
                        self.connect_google(ui.ctx());
                    }
                }
            }
            ui.hyperlink_to("Setup guide", GUIDE);
        });
    }

    fn connect_google(&mut self, ctx: &egui::Context) {
        let started =
            google_calendar::begin_sign_in(&self.schedule.client_id, &self.schedule.client_secret)
                .and_then(|sign_in| {
                    let url = sign_in.url.clone();
                    self.cloud.google_sign_in(sign_in).map(|()| url)
                });
        match started {
            Ok(url) => {
                ctx.open_url(egui::OpenUrl::new_tab(&url));
                self.schedule.signing_in = Some(url);
            }
            Err(error) => self.toast("Google Calendar", error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_blocks_become_cards_and_leave_the_reply() {
        let reply = "Siap, saya catat.\n```cikung-task\n{\"text\": \"Meeting klien\", \"due\": \"2030-01-02 15:00\"}\n{\"text\": \"Beli kopi\"}\n```";
        let (text, proposals) = take_proposals(reply);
        assert_eq!(text, "Siap, saya catat.");
        assert_eq!(proposals.len(), 2);
        assert_eq!(proposals[0].text, "Meeting klien");
        assert_eq!(
            proposals[0].due_unix.map(format_task_due).as_deref(),
            Some("2030-01-02 15:00")
        );
        assert_eq!(proposals[1].due_unix, None);
        // An array works too; a broken block or empty title adds nothing.
        let (_, list) = take_proposals("```cikung-task\n[{\"text\":\"A\"},{\"text\":\" \"}]\n```");
        assert_eq!(list.len(), 1);
        let (text, none) = take_proposals("Just chatting.\n```cikung-task\n{oops\n```\nBye");
        assert!(none.is_empty());
        assert_eq!(text, "Just chatting.\n\nBye");
        let prompt = assistant_prompt(chrono::Local::now()).content;
        assert!(prompt.contains(BLOCK) && prompt.contains("UTC"));
    }

    #[test]
    fn only_open_tasks_with_a_due_time_and_changes_sync() {
        let mut task = Task {
            id: 1,
            text: "Meeting".into(),
            done: false,
            due_unix: Some(1_900_000_000),
            due_notified: false,
            google_event: None,
        };
        assert!(needs_sync(&task));
        task.google_event = Some(CalendarEvent {
            id: "e1".into(),
            synced: calendar_signature(&task),
        });
        assert!(!needs_sync(&task));
        task.text = "Meeting moved".into();
        assert!(needs_sync(&task));
        task.done = true;
        assert!(!needs_sync(&task));
        task.done = false;
        task.due_unix = None;
        assert!(!needs_sync(&task));
    }
}
