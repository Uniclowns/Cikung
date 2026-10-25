//! Local productivity state. Serialized by the app's normal eframe storage.
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

#[derive(Clone, Debug)]
pub struct ToolNotice {
    pub title: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub text: String,
    pub done: bool,
    #[serde(default)]
    pub due_unix: Option<u64>,
    #[serde(default)]
    pub due_notified: bool,
    /// The Google Calendar event holding this task, once synced.
    #[serde(default)]
    pub google_event: Option<CalendarEvent>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    /// `calendar_signature` of the text and due time the event was last saved with.
    pub synced: u64,
}
/// Changes when the task's text or due time does, so an edit moves its event.
pub fn calendar_signature(task: &Task) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    (&task.text, task.due_unix).hash(&mut hash);
    hash.finish()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ShelfItem {
    pub path: String,
    pub name: String,
    pub added_unix: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub text: String,
    pub created_unix: u64,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Toolkit {
    pub tasks: Vec<Task>,
    pub shelf: Vec<ShelfItem>,
    pub focus: FocusTimer,
    pub water: WaterReminder,
    pub notes: Vec<Note>,
    pub lyrics: Lyrics,
    /// Google Calendar events whose task was deleted or lost its due time.
    pub calendar_orphans: Vec<String>,
    next_id: u64,
}
impl Toolkit {
    pub fn tick(&mut self) -> Vec<ToolNotice> {
        self.tick_feature(true, true)
    }
    pub fn tick_feature(&mut self, focus: bool, water: bool) -> Vec<ToolNotice> {
        let mut notices = Vec::new();
        if focus && let Some(notice) = self.focus.tick_at(Instant::now()) {
            notices.push(notice);
        }
        if water && let Some(notice) = self.water.tick_at(Instant::now(), &today()) {
            notices.push(notice);
        }
        notices.extend(self.task_reminders_at(unix_now()));
        notices
    }
    fn id(&mut self) -> u64 {
        self.next_id = self
            .next_id
            .max(self.tasks.iter().map(|t| t.id).max().unwrap_or(0))
            .max(self.notes.iter().map(|t| t.id).max().unwrap_or(0))
            .saturating_add(1);
        self.next_id
    }
    pub fn add_task(&mut self, text: &str) -> Result<u64, String> {
        let text = Self::task_text(text)?;
        let id = self.id();
        self.tasks.push(Task {
            id,
            text: text.into(),
            done: false,
            due_unix: None,
            due_notified: false,
            google_event: None,
        });
        Ok(id)
    }
    fn task_text(text: &str) -> Result<&str, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err("Write a task first.".into());
        }
        if text.chars().count() > 2_000 {
            return Err("Keep task text under 2,000 characters.".into());
        }
        Ok(text)
    }
    pub fn edit_task(&mut self, id: u64, text: &str, due_unix: Option<u64>) -> Result<(), String> {
        let text = Self::task_text(text)?;
        let task = self
            .tasks
            .iter_mut()
            .find(|task| task.id == id)
            .ok_or("This task no longer exists.")?;
        task.text = text.into();
        if task.due_unix != due_unix {
            task.due_notified = false;
        }
        task.due_unix = due_unix;
        if due_unix.is_none()
            && let Some(event) = task.google_event.take()
        {
            self.calendar_orphans.push(event.id);
        }
        Ok(())
    }
    pub fn task_reminders_at(&mut self, unix: u64) -> Vec<ToolNotice> {
        let mut notices = Vec::new();
        for task in &mut self.tasks {
            if notices.len() == 5 {
                break;
            }
            if !task.done && !task.due_notified && task.due_unix.is_some_and(|due| due <= unix) {
                task.due_notified = true;
                notices.push(ToolNotice {
                    title: "Task reminder".into(),
                    message: task.text.chars().take(200).collect(),
                });
            }
        }
        notices
    }
    pub fn remove_task(&mut self, id: u64) {
        let events = self
            .tasks
            .iter_mut()
            .filter(|t| t.id == id)
            .filter_map(|t| t.google_event.take().map(|e| e.id));
        self.calendar_orphans.extend(events.collect::<Vec<_>>());
        self.tasks.retain(|t| t.id != id);
    }
    pub fn toggle_task(&mut self, id: u64) {
        if let Some(t) = self.tasks.iter_mut().find(|t| t.id == id) {
            t.done = !t.done;
        }
    }
    pub fn add_file(&mut self, path: PathBuf) -> Result<(), String> {
        let path =
            std::fs::canonicalize(path).map_err(|e| format!("Cannot access this file: {e}"))?;
        let path = clean_windows_path(&path);
        if self.shelf.iter().any(|f| Path::new(&f.path) == path) {
            return Ok(());
        }
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        self.shelf.push(ShelfItem {
            path: path.to_string_lossy().into_owned(),
            name,
            added_unix: unix_now(),
        });
        Ok(())
    }
    pub fn remove_file(&mut self, index: usize) {
        if index < self.shelf.len() {
            self.shelf.remove(index);
        }
    }
    fn shelf_path(&self, index: usize) -> Result<&Path, String> {
        let item = self
            .shelf
            .get(index)
            .ok_or("This shelf item no longer exists.")?;
        let path = Path::new(&item.path);
        if !path.exists() {
            return Err(format!(
                "{} has moved or is no longer available.",
                item.name
            ));
        }
        Ok(path)
    }
    pub fn open_file(&self, index: usize) -> Result<(), String> {
        spawn_file_command(self.shelf_path(index)?, false)
    }
    pub fn reveal_file(&self, index: usize) -> Result<(), String> {
        spawn_file_command(self.shelf_path(index)?, true)
    }
    pub fn add_note(&mut self, title: &str, text: &str) -> Result<u64, String> {
        if text.trim().is_empty() {
            return Err("Write or import a transcript first.".into());
        }
        if text.len() > 2_000_000 {
            return Err("Notes must be smaller than 2 MB.".into());
        }
        let id = self.id();
        self.notes.push(Note {
            id,
            title: if title.trim().is_empty() {
                "Untitled note".into()
            } else {
                title.trim().into()
            },
            text: text.into(),
            created_unix: unix_now(),
        });
        Ok(id)
    }
    pub fn remove_note(&mut self, id: u64) {
        self.notes.retain(|n| n.id != id);
    }
    pub fn export_note(&self, id: u64, path: &Path) -> Result<(), String> {
        let note = self
            .notes
            .iter()
            .find(|n| n.id == id)
            .ok_or("This note no longer exists.")?;
        std::fs::write(path, format!("# {}\n\n{}\n", note.title, note.text))
            .map_err(|e| format!("Could not export the note: {e}"))
    }
    pub fn import_transcript(&mut self, path: &Path) -> Result<u64, String> {
        let text = read_text(path, 2_000_000)?;
        self.add_note(
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Imported transcript"),
            &text,
        )
    }
    /// Extract only explicitly marked action items; this does not fabricate meeting decisions.
    pub fn tasks_from_note(&mut self, id: u64) -> Result<usize, String> {
        let tasks = self.explicit_tasks(id)?;
        let mut added = 0;
        for task in tasks {
            if !self.tasks.iter().any(|t| t.text == task) {
                self.add_task(&task)?;
                added += 1;
            }
        }
        Ok(added)
    }
    pub fn explicit_tasks(&self, id: u64) -> Result<Vec<String>, String> {
        let note = self
            .notes
            .iter()
            .find(|n| n.id == id)
            .ok_or("This note no longer exists.")?;
        let tasks: Vec<String> = note
            .text
            .lines()
            .filter_map(|line| {
                let l = line.trim();
                l.strip_prefix("- [ ] ")
                    .or_else(|| l.strip_prefix("TODO: "))
                    .or_else(|| l.strip_prefix("Action: "))
                    .map(str::to_owned)
            })
            .collect();
        Ok(tasks)
    }
    pub fn import_lyrics(&mut self, path: &Path) -> Result<(), String> {
        self.lyrics = Lyrics::parse(&read_text(path, 1_000_000)?)?;
        Ok(())
    }
}
fn read_text(path: &Path, limit: u64) -> Result<String, String> {
    if std::fs::metadata(path)
        .map_err(|e| format!("Cannot read this file: {e}"))?
        .len()
        > limit
    {
        return Err(format!("This text file exceeds {} MB.", limit / 1_000_000));
    }
    std::fs::read_to_string(path).map_err(|e| format!("Could not read UTF-8 text: {e}"))
}
fn clean_windows_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path.into()
}
fn file_command(path: &Path, reveal: bool) -> Command {
    #[cfg(windows)]
    {
        if reveal {
            let mut command = Command::new("explorer.exe");
            command.arg(format!("/select,{}", path.display()));
            command
        } else {
            let mut command = Command::new("rundll32.exe");
            command.args(["url.dll,FileProtocolHandler"]).arg(path);
            command
        }
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        if reveal {
            command.arg("-R");
        }
        command.arg("--").arg(path);
        command
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let mut command = Command::new("xdg-open");
        command.arg(if reveal && path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        });
        command
    }
}
fn spawn_file_command(path: &Path, reveal: bool) -> Result<(), String> {
    let mut command = file_command(path, reveal);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not open this file: {e}"))
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FocusMode {
    #[default]
    Focus,
    Break,
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct FocusTimer {
    pub mode: FocusMode,
    pub focus_minutes: u32,
    pub break_minutes: u32,
    pub remaining_secs: f64,
    #[serde(skip)]
    pub running: bool,
    pub completed_sessions: u64,
    pub total_focus_secs: u64,
    worked_secs: f64,
    #[serde(skip)]
    last_tick: Option<Instant>,
    #[serde(skip)]
    pending_notice: bool,
}
impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            mode: FocusMode::Focus,
            focus_minutes: 25,
            break_minutes: 5,
            remaining_secs: 1500.,
            running: false,
            completed_sessions: 0,
            total_focus_secs: 0,
            worked_secs: 0.,
            last_tick: None,
            pending_notice: false,
        }
    }
}
impl FocusTimer {
    pub fn start(&mut self) {
        self.start_at(Instant::now());
    }
    fn start_at(&mut self, now: Instant) {
        if self.running {
            return;
        }
        if self.remaining_secs <= 0. || !self.remaining_secs.is_finite() {
            self.reset();
        }
        self.running = true;
        self.last_tick = Some(now);
    }
    pub fn pause(&mut self) {
        self.pause_at(Instant::now());
    }
    fn pause_at(&mut self, now: Instant) {
        self.advance(now);
        self.running = false;
        self.last_tick = None;
    }
    pub fn reset(&mut self) {
        self.running = false;
        self.last_tick = None;
        self.pending_notice = false;
        self.remaining_secs = f64::from(match self.mode {
            FocusMode::Focus => self.focus_minutes.clamp(1, 240),
            FocusMode::Break => self.break_minutes.clamp(1, 240),
        }) * 60.;
    }
    pub fn switch_mode(&mut self) {
        self.pause();
        self.mode = match self.mode {
            FocusMode::Focus => FocusMode::Break,
            FocusMode::Break => FocusMode::Focus,
        };
        self.reset();
    }
    pub fn set_duration(&mut self, minutes: u32) {
        match self.mode {
            FocusMode::Focus => self.focus_minutes = minutes.clamp(1, 240),
            FocusMode::Break => self.break_minutes = minutes.clamp(1, 240),
        };
        self.reset();
    }
    pub fn formatted_remaining(&self) -> String {
        let seconds = self.remaining_secs.max(0.).ceil() as u64;
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
    fn advance(&mut self, now: Instant) {
        if !self.running {
            return;
        }
        let elapsed = self
            .last_tick
            .replace(now)
            .map(|t| now.saturating_duration_since(t).as_secs_f64())
            .unwrap_or(0.);
        let consumed = elapsed.min(self.remaining_secs.max(0.));
        self.remaining_secs = (self.remaining_secs - consumed).max(0.);
        if self.mode == FocusMode::Focus {
            self.worked_secs += consumed;
            self.total_focus_secs = self.worked_secs.floor() as u64;
        }
        if self.remaining_secs <= 0. {
            self.running = false;
            self.last_tick = None;
            if self.mode == FocusMode::Focus {
                self.completed_sessions += 1;
            }
            self.pending_notice = true;
        }
    }
    fn tick_at(&mut self, now: Instant) -> Option<ToolNotice> {
        self.advance(now);
        if !std::mem::take(&mut self.pending_notice) {
            return None;
        }
        Some(ToolNotice {
            title: match self.mode {
                FocusMode::Focus => "Focus complete",
                FocusMode::Break => "Break complete",
            }
            .into(),
            message: match self.mode {
                FocusMode::Focus => "Good work. Take a breath and start a break when you're ready.",
                FocusMode::Break => "Ready for another focus session?",
            }
            .into(),
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct WaterReminder {
    pub enabled: bool,
    pub interval_minutes: u32,
    pub drank_today: u32,
    pub bottle_ml: u32,
    pub goal_ml: u32,
    pub ml_today: u32,
    pub active_start_hour: u32,
    pub active_end_hour: u32,
    pub history: Vec<WaterDay>,
    /// Today's ml by local hour (index 0–23) for the Water chart; empty means none yet.
    pub hourly_ml: Vec<u32>,
    day: String,
    #[serde(skip)]
    next_due: Option<Instant>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WaterDay {
    pub day: String,
    pub ml: u32,
}
impl Default for WaterReminder {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: 60,
            drank_today: 0,
            bottle_ml: 250,
            goal_ml: 2000,
            ml_today: 0,
            active_start_hour: 8,
            active_end_hour: 22,
            history: Vec::new(),
            hourly_ml: Vec::new(),
            day: today(),
            next_due: None,
        }
    }
}
impl WaterReminder {
    fn reset_day(&mut self, day: &str) {
        if self.day != day {
            self.day = day.into();
            self.drank_today = 0;
            self.ml_today = 0;
            self.hourly_ml.clear();
            self.next_due = None;
        }
    }
    pub fn drink(&mut self) {
        let _ = self.drink_ml(self.bottle_ml.clamp(50, 2000));
    }
    pub fn drink_ml(&mut self, ml: u32) -> Result<(), String> {
        if !(1..=5000).contains(&ml) {
            return Err("A drink must contain between 1 and 5,000 ml.".into());
        }
        let hour = chrono::Timelike::hour(&chrono::Local::now());
        self.drink_at(ml, &today(), hour);
        Ok(())
    }
    fn drink_at(&mut self, ml: u32, day: &str, hour: u32) {
        self.reset_day(day);
        self.drank_today = self.drank_today.saturating_add(1);
        self.ml_today = self.ml_today.saturating_add(ml);
        self.hourly_ml.resize(24, 0);
        let slot = &mut self.hourly_ml[hour.min(23) as usize];
        *slot = slot.saturating_add(ml);
        if let Some(entry) = self.history.iter_mut().find(|entry| entry.day == day) {
            entry.ml = entry.ml.saturating_add(ml);
        } else {
            self.history.push(WaterDay {
                day: day.into(),
                ml,
            });
        }
        self.history.sort_by(|a, b| a.day.cmp(&b.day));
        if self.history.len() > 30 {
            self.history.drain(..self.history.len() - 30);
        }
        self.reschedule();
    }
    pub fn active_at_hour(&self, hour: u32) -> bool {
        let start = self.active_start_hour.min(23);
        let end = self.active_end_hour.min(23);
        let hour = hour.min(23);
        if start == end {
            true
        } else if start < end {
            (start..end).contains(&hour)
        } else {
            hour >= start || hour < end
        }
    }
    fn should_remind_at_hour(&self, hour: u32) -> bool {
        self.enabled && self.active_at_hour(hour) && self.ml_today < self.goal_ml.clamp(250, 10000)
    }
    pub fn snooze(&mut self, minutes: u32) {
        use chrono::Timelike;
        self.next_due = self
            .should_remind_at_hour(chrono::Local::now().hour())
            .then(|| Instant::now() + Duration::from_secs(u64::from(minutes.clamp(1, 1440)) * 60));
    }
    pub fn reschedule(&mut self) {
        use chrono::Timelike;
        self.next_due = self
            .should_remind_at_hour(chrono::Local::now().hour())
            .then(|| {
                Instant::now()
                    + Duration::from_secs(u64::from(self.interval_minutes.clamp(1, 1440)) * 60)
            });
    }
    pub fn seconds_until_reminder(&self) -> Option<u64> {
        self.next_due
            .map(|due| due.saturating_duration_since(Instant::now()).as_secs())
    }
    fn tick_at(&mut self, now: Instant, day: &str) -> Option<ToolNotice> {
        use chrono::Timelike;
        self.tick_at_hour(now, day, chrono::Local::now().hour())
    }
    pub fn tick_at_hour(&mut self, now: Instant, day: &str, hour: u32) -> Option<ToolNotice> {
        self.reset_day(day);
        if !self.should_remind_at_hour(hour) {
            self.next_due = None;
            return None;
        }
        let interval = Duration::from_secs(u64::from(self.interval_minutes.clamp(1, 1440)) * 60);
        let due = self.next_due.get_or_insert(now + interval);
        if now < *due {
            return None;
        }
        *due = now + interval;
        Some(ToolNotice {
            title: "Water break".into(),
            message: "A little sip for you. A little purr from Cikung.".into(),
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    pub synced: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LyricLine {
    pub time_secs: f64,
    pub text: String,
}
impl Lyrics {
    pub fn parse(input: &str) -> Result<Self, String> {
        if input.len() > 1_000_000 {
            return Err("Lyrics exceed 1 MB.".into());
        }
        let mut lines = Vec::new();
        let mut plain = Vec::new();
        let mut offset = 0.;
        for raw in input.trim_start_matches('\u{feff}').lines() {
            let raw = raw.trim();
            if raw.is_empty() {
                continue;
            }
            let mut remaining = raw;
            let mut timestamps = Vec::new();
            let mut metadata = false;
            while let Some(rest) = remaining.strip_prefix('[') {
                let Some(end) = rest.find(']') else {
                    break;
                };
                let tag = &rest[..end];
                if let Some(value) = tag.strip_prefix("offset:") {
                    offset = value
                        .trim()
                        .parse::<f64>()
                        .map_err(|_| "Invalid LRC offset.")?
                        / 1000.;
                    if !offset.is_finite() {
                        return Err("Invalid LRC offset.".into());
                    }
                    metadata = true;
                } else if tag.as_bytes().first().is_some_and(u8::is_ascii_digit)
                    && tag.contains(':')
                {
                    timestamps.push(parse_timestamp(tag)?);
                } else if ["ar:", "al:", "ti:", "au:", "by:", "re:", "ve:", "length:"]
                    .iter()
                    .any(|prefix| tag.starts_with(prefix))
                {
                    metadata = true;
                } else {
                    // A literal section label such as [Chorus] is lyric text.
                    break;
                }
                remaining = &rest[end + 1..];
            }
            if !timestamps.is_empty() {
                for time in timestamps {
                    lines.push(LyricLine {
                        time_secs: time,
                        text: remaining.trim().into(),
                    });
                }
            } else if !metadata {
                plain.push(LyricLine {
                    time_secs: 0.,
                    text: raw.into(),
                });
            }
        }
        if lines.is_empty() {
            if plain.is_empty() {
                return Err("No lyric text found.".into());
            }
            return Ok(Self {
                lines: plain,
                synced: false,
            });
        }
        for line in &mut lines {
            line.time_secs = (line.time_secs - offset).max(0.);
        }
        lines.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
        Ok(Self {
            lines,
            synced: true,
        })
    }
    pub fn active_index(&self, position: f64) -> Option<usize> {
        if !self.synced || !position.is_finite() {
            return None;
        }
        self.lines
            .partition_point(|line| line.time_secs <= position)
            .checked_sub(1)
    }
    pub fn to_lrc(&self) -> String {
        self.lines
            .iter()
            .map(|line| {
                if self.synced {
                    let ms = (line.time_secs.max(0.) * 1000.).round() as u64;
                    format!(
                        "[{:02}:{:02}.{:03}]{}",
                        ms / 60_000,
                        (ms / 1000) % 60,
                        ms % 1000,
                        line.text
                    )
                } else {
                    line.text.clone()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn save(&self, path: &Path) -> Result<(), String> {
        std::fs::write(path, self.to_lrc()).map_err(|e| format!("Could not save lyrics: {e}"))
    }
}
fn parse_timestamp(tag: &str) -> Result<f64, String> {
    let parts: Vec<&str> = tag.split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return Err(format!("Invalid lyric timestamp [{tag}]."));
    }
    let mut total = 0.;
    for (index, part) in parts.iter().enumerate() {
        let part = part
            .parse::<f64>()
            .map_err(|_| format!("Invalid lyric timestamp [{tag}]."))?;
        if !part.is_finite() || part < 0. || (index > 0 && part >= 60.) {
            return Err(format!("Invalid lyric timestamp [{tag}]."));
        }
        total = total * 60. + part;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_completion_is_once_and_pause_excludes_time() {
        let now = Instant::now();
        let mut timer = FocusTimer {
            remaining_secs: 5.,
            ..Default::default()
        };
        timer.start_at(now);
        assert!(timer.tick_at(now + Duration::from_secs(2)).is_none());
        timer.pause_at(now + Duration::from_secs(2));
        timer.start_at(now + Duration::from_secs(100));
        assert!(timer.tick_at(now + Duration::from_secs(103)).is_some());
        assert!(timer.tick_at(now + Duration::from_secs(200)).is_none());
        assert_eq!(timer.completed_sessions, 1);
        assert_eq!(timer.total_focus_secs, 5);
        timer.switch_mode();
        assert_eq!(timer.mode, FocusMode::Break);
        assert!(!timer.running);
    }
    #[test]
    fn restored_timer_is_paused() {
        let mut timer = FocusTimer::default();
        timer.start();
        let restored: FocusTimer =
            serde_json::from_str(&serde_json::to_string(&timer).unwrap()).unwrap();
        assert!(!restored.running);
        assert!(restored.last_tick.is_none());
    }
    #[test]
    fn water_is_opt_in_and_counts_reset_on_local_day() {
        let now = Instant::now();
        let mut water = WaterReminder {
            drank_today: 4,
            day: "2026-10-18".into(),
            ..Default::default()
        };
        assert!(water.tick_at_hour(now, "2026-10-18", 9).is_none());
        assert!(water.next_due.is_none());
        water.enabled = true;
        water.interval_minutes = 1;
        assert!(water.tick_at_hour(now, "2026-10-18", 9).is_none());
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(60), "2026-10-18", 9)
                .is_some()
        );
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(61), "2026-10-18", 9)
                .is_none()
        );
        water.tick_at_hour(now + Duration::from_secs(62), "2026-10-19", 9);
        assert_eq!(water.drank_today, 0);
    }
    #[test]
    fn old_tasks_and_water_restore_without_inventing_volume() {
        let task: Task = serde_json::from_str(r#"{"id":8,"text":"Tea","done":false}"#).unwrap();
        assert_eq!(task.due_unix, None);
        assert!(!task.due_notified);
        let task: Task = ron::from_str("(id:8,text:\"Tea\",done:false)").unwrap();
        assert_eq!(task.due_unix, None);
        let water: WaterReminder = serde_json::from_str(
            r#"{"enabled":true,"interval_minutes":45,"drank_today":4,"day":"2026-10-18"}"#,
        )
        .unwrap();
        assert_eq!(water.drank_today, 4);
        assert_eq!(
            (water.bottle_ml, water.goal_ml, water.ml_today),
            (250, 2000, 0)
        );
        assert_eq!((water.active_start_hour, water.active_end_hour), (8, 22));
        assert!(water.history.is_empty());
        let water: WaterReminder =
            ron::from_str("(enabled:true,interval_minutes:45,drank_today:4,day:\"2026-10-18\")")
                .unwrap();
        assert_eq!((water.drank_today, water.ml_today), (4, 0));
    }
    #[test]
    fn task_edits_preserve_identity_and_reminders_are_once_per_deadline() {
        let mut kit = Toolkit::default();
        let id = kit.add_task("Old title").unwrap();
        kit.edit_task(id, " New title ", Some(100)).unwrap();
        assert!(kit.task_reminders_at(99).is_empty());
        assert_eq!(kit.task_reminders_at(100)[0].message, "New title");
        assert!(kit.task_reminders_at(101).is_empty());
        kit.edit_task(id, "Another title", Some(100)).unwrap();
        assert!(kit.task_reminders_at(102).is_empty());
        kit.toggle_task(id);
        kit.edit_task(id, "Completed", Some(101)).unwrap();
        assert_eq!(kit.tasks[0].id, id);
        assert!(kit.tasks[0].done);
        assert!(kit.task_reminders_at(200).is_empty());
        assert!(kit.edit_task(id, " ", None).is_err());
        assert_eq!(kit.tasks[0].text, "Completed");
        kit.toggle_task(id);
        assert_eq!(kit.task_reminders_at(200).len(), 1);
        let mut restored: Toolkit =
            serde_json::from_str(&serde_json::to_string(&kit).unwrap()).unwrap();
        assert!(restored.task_reminders_at(300).is_empty());
        restored.edit_task(id, "Rescheduled", Some(299)).unwrap();
        assert_eq!(restored.task_reminders_at(300).len(), 1);
        assert!(restored.edit_task(id + 1, "Missing", None).is_err());
        assert!(restored.add_task(&"a".repeat(2001)).is_err());
    }
    #[test]
    fn calendar_events_follow_edits_and_deletions() {
        let mut kit = Toolkit::default();
        let id = kit.add_task("Meeting").unwrap();
        kit.edit_task(id, "Meeting", Some(1_800_000_000)).unwrap();
        let saved = calendar_signature(&kit.tasks[0]);
        kit.tasks[0].google_event = Some(CalendarEvent {
            id: "e1".into(),
            synced: saved,
        });
        kit.edit_task(id, "Meeting with Ana", Some(1_800_000_000))
            .unwrap();
        assert_ne!(calendar_signature(&kit.tasks[0]), saved);
        // Dropping the due time, then deleting the task, each hand back the event to remove.
        kit.edit_task(id, "Meeting with Ana", None).unwrap();
        assert_eq!(kit.calendar_orphans, ["e1"]);
        assert!(kit.tasks[0].google_event.is_none());
        kit.tasks[0].google_event = Some(CalendarEvent {
            id: "e2".into(),
            synced: 0,
        });
        kit.remove_task(id);
        assert_eq!(kit.calendar_orphans, ["e1", "e2"]);
        let restored: Toolkit =
            serde_json::from_str(&serde_json::to_string(&kit).unwrap()).unwrap();
        assert_eq!(restored.calendar_orphans, ["e1", "e2"]);
    }
    #[test]
    fn overdue_task_batches_are_bounded_without_losing_reminders() {
        let mut kit = Toolkit::default();
        for _ in 0..7 {
            let id = kit.add_task("Startup overdue").unwrap();
            kit.edit_task(id, "Startup overdue", Some(1)).unwrap();
        }
        assert_eq!(kit.task_reminders_at(100).len(), 5);
        assert_eq!(kit.task_reminders_at(100).len(), 2);
        assert!(kit.task_reminders_at(100).is_empty());
    }
    #[test]
    fn water_logs_actual_volume_and_rolls_over_with_bounded_history() {
        let now = Instant::now();
        let mut water = WaterReminder::default();
        water.drink_at(250, "2026-10-18", 9);
        water.drink_at(400, "2026-10-18", 9);
        assert_eq!((water.drank_today, water.ml_today), (2, 650));
        assert_eq!(water.history[0].ml, 650);
        assert_eq!((water.hourly_ml[9], water.hourly_ml[10]), (650, 0));
        water.tick_at_hour(now, "2026-10-19", 9);
        assert_eq!((water.drank_today, water.ml_today), (0, 0));
        assert!(water.hourly_ml.is_empty());
        assert_eq!(water.history[0].ml, 650);
        for day in 1..=31 {
            water.drink_at(100, &format!("2026-12-{day:02}"), 12);
        }
        assert_eq!(water.history.len(), 30);
        assert_eq!(water.history.first().unwrap().day, "2026-12-02");
        assert_eq!(water.history.last().unwrap().ml, 100);
        assert!(water.drink_ml(0).is_err());
        assert!(water.drink_ml(5001).is_err());
    }
    #[test]
    fn water_hours_goal_and_next_day_resume_without_stale_reminders() {
        let now = Instant::now();
        let mut water = WaterReminder {
            enabled: true,
            interval_minutes: 1,
            goal_ml: 500,
            ..Default::default()
        };
        assert!(!water.active_at_hour(7));
        assert!(water.active_at_hour(8));
        assert!(!water.active_at_hour(22));
        assert!(water.tick_at_hour(now, "2026-10-18", 21).is_none());
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(60), "2026-10-18", 22)
                .is_none()
        );
        assert!(water.next_due.is_none());
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(120), "2026-10-19", 8)
                .is_none()
        );
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(180), "2026-10-19", 8)
                .is_some()
        );
        water.drink_at(500, "2026-10-19", 8);
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(300), "2026-10-19", 9)
                .is_none()
        );
        assert!(water.next_due.is_none());
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(360), "2026-10-20", 9)
                .is_none()
        );
        assert!(
            water
                .tick_at_hour(now + Duration::from_secs(420), "2026-10-20", 9)
                .is_some()
        );
        water.active_start_hour = 22;
        water.active_end_hour = 6;
        assert!(water.active_at_hour(22));
        assert!(water.active_at_hour(5));
        assert!(!water.active_at_hour(6));
        assert!(!water.active_at_hour(12));
        water.active_end_hour = 22;
        assert!(water.active_at_hour(12));
    }
    #[test]
    fn lyrics_handle_unicode_repeated_timestamps_offset_and_before_first() {
        let lyrics =
            Lyrics::parse("[ar:猫]\n[offset:500]\n[00:02.00][00:04.25]แมว 🐈\n[00:05.00]Purr")
                .unwrap();
        assert_eq!(lyrics.lines.len(), 3);
        assert_eq!(lyrics.lines[0].time_secs, 1.5);
        assert_eq!(lyrics.active_index(1.), None);
        assert_eq!(lyrics.active_index(4.), Some(1));
        assert_eq!(lyrics.lines[1].text, "แมว 🐈");
        let restored = Lyrics::parse(&lyrics.to_lrc()).unwrap();
        assert_eq!(restored.lines[1].time_secs, 3.75);
        assert!(!Lyrics::parse("Hello\nแมว").unwrap().synced);
        assert_eq!(
            Lyrics::parse("[Chorus]\nHello").unwrap().lines[0].text,
            "[Chorus]"
        );
        assert_eq!(
            Lyrics::parse("[00:02][Chorus] Hello").unwrap().lines[0].text,
            "[Chorus] Hello"
        );
        assert!(Lyrics::parse("[00:99]bad").is_err());
    }
    #[test]
    fn tasks_and_notes_preserve_identifiers_and_extract_only_marked_actions() {
        let mut kit = Toolkit::default();
        let id = kit.add_task("buy tea").unwrap();
        kit.toggle_task(id);
        assert!(kit.tasks[0].done);
        let note = kit
            .add_note(
                "Standup",
                "We discussed deployment.\n- [ ] Ship fix\nTODO: Call Lee\nAction: Write docs",
            )
            .unwrap();
        assert_eq!(kit.tasks_from_note(note).unwrap(), 3);
        assert_eq!(kit.tasks_from_note(note).unwrap(), 0);
        let json = serde_json::to_string(&kit).unwrap();
        let mut restored: Toolkit = serde_json::from_str(&json).unwrap();
        assert!(restored.add_task("next").unwrap() > note);
    }
    #[test]
    fn opening_paths_does_not_construct_shell_code() {
        let path = Path::new(r"C:\My files\cat & $(hello).txt");
        let command = file_command(path, false);
        let args: Vec<_> = command
            .get_args()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        assert!(args.iter().any(|s| s == &path.to_string_lossy()));
        assert!(!command.get_program().to_string_lossy().contains("cmd"));
    }
}
