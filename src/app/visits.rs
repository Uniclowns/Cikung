//! Cikung drops by every few minutes: a hop, a happy wiggle and a few hearts in the notch,
//! with a short line for the hour or a task due soon, so it feels like company.
use super::*;
use chrono::Timelike;
use std::f32::consts::PI;

/// How long a visit's line stays in the notch.
const VISIT: Duration = Duration::from_secs(6);
/// How long the hop, wiggle and hearts play.
const MOTION_SECS: f32 = 2.4;
/// Choices in Settings → General; 0 turns visits off.
pub(super) const VISIT_MINUTES: [(u32, &str); 6] = [
    (0, "Off"),
    (5, "Every 5 minutes"),
    (10, "Every 10 minutes"),
    (15, "Every 15 minutes"),
    (30, "Every 30 minutes"),
    (60, "Every hour"),
];

#[derive(Default)]
pub(super) struct Visits {
    next: Option<Instant>,
    started: Option<Instant>,
    count: usize,
    minutes: u32,
}

/// The cat during a visit: lifted and wiggled by `offset`, squashed or stretched by `scale`
/// about its feet, `t` seconds in.
pub(super) struct Pose {
    pub offset: Vec2,
    pub scale: Vec2,
    pub t: f32,
}

impl Cikung {
    /// Starts a visit when one is due and nothing more important is on screen.
    pub(super) fn cat_visit(&mut self) {
        let minutes = self.prefs.cat_visit_minutes;
        let now = Instant::now();
        if minutes == 0 || self.demo || self.capture.is_some() || self.audit.is_some() {
            self.visits.minutes = 0;
            return;
        }
        let every = Duration::from_secs(u64::from(minutes) * 60);
        // Turning visits on, or a new interval, starts the count from now.
        if self.visits.minutes != minutes {
            self.visits.minutes = minutes;
            self.visits.next = Some(now + every);
        }
        if self.visits.next.is_none_or(|at| at > now) {
            return;
        }
        // Not over the open panel, another message, a recording, a focus session or a
        // fullscreen app: try again in a minute.
        if self.expanded
            || self.header_notice().is_some()
            || self.volume_hud_level().is_some()
            || self.recorder.is_recording()
            || self.tools.focus.running
            || self.screen_covered
        {
            self.visits.next = Some(now + Duration::from_secs(60));
            return;
        }
        let line = self.visit_line();
        self.visits.count += 1;
        self.visits.started = Some(now);
        self.visits.next = Some(now + every);
        self.toast_for("Cikung visit", line, VISIT);
    }

    /// A task due within the hour first; otherwise a line for the hour or a kind word.
    fn visit_line(&self) -> String {
        let now = chrono::Local::now();
        let unix = u64::try_from(now.timestamp()).unwrap_or_default();
        let soon = self
            .tools
            .tasks
            .iter()
            .filter(|task| !task.done)
            .filter_map(|task| Some((task.due_unix?, task)))
            .filter(|(due, _)| (unix..=unix + 3600).contains(due))
            .min_by_key(|(due, _)| *due);
        if let Some((due, task)) = soon {
            let time = crate::panels::format_task_due(due);
            return format!(
                "Don't forget: {} at {}",
                task.text,
                time.get(11..).unwrap_or("")
            );
        }
        let hour = match now.hour() {
            5..=10 => "Good morning! Ready for today?",
            11..=14 => "Had lunch yet? Don't skip it.",
            15..=17 => "Hang in there, you're nearly done.",
            18..=21 => "Good evening. You did well today.",
            _ => "It's late. Rest soon, okay?",
        };
        let lines = [
            hour,
            "Mrrp! Just checking on you.",
            "Stretch your back for a moment.",
            "Have a sip of water?",
            "You're doing great.",
            "Rest your eyes for a few seconds.",
            "I'm right here with you.",
        ];
        lines[self.visits.count % lines.len()].into()
    }

    /// Two hops (stretching up, squashing on landing), then a happy wiggle.
    pub(super) fn visit_pose(&self) -> Option<Pose> {
        if self.prefs.reduced_motion {
            return None;
        }
        let t = self.visits.started?.elapsed().as_secs_f32();
        if t >= MOTION_SECS {
            return None;
        }
        let hop = |start: f32, length: f32, height: f32| {
            let p = ((t - start) / length).clamp(0.0, 1.0);
            -(p * PI).sin() * height
        };
        let landing = |at: f32| (1.0 - ((t - at) / 0.12).abs()).max(0.0);
        let squash = (landing(0.45) + 0.5 * landing(0.85)) * 0.12;
        let stretch = if t < 0.45 {
            (t / 0.45 * PI).sin() * 0.06
        } else {
            0.0
        };
        let wiggle = if (1.0..2.2).contains(&t) {
            ((t - 1.0) * PI * 5.0).sin() * 2.0 * (2.2 - t) / 1.2
        } else {
            0.0
        };
        Some(Pose {
            offset: Vec2::new(wiggle, hop(0.0, 0.45, 6.0) + hop(0.55, 0.3, 2.5)),
            scale: Vec2::new(1.0 + squash - stretch, 1.0 - squash + stretch),
            t,
        })
    }
}

/// Three small hearts rising beside the cat's ear and fading out.
pub(super) fn paint_hearts(painter: &egui::Painter, cat: Rect, t: f32) {
    for (index, delay) in [0.3_f32, 0.7, 1.1].into_iter().enumerate() {
        let p = (t - delay) / 1.2;
        if !(0.0..1.0).contains(&p) {
            continue;
        }
        let at = Pos2::new(
            cat.right() - 2.0 + index as f32 * 4.0 + (p * PI * 2.0).sin() * 2.0,
            cat.top() + 8.0 - p * 14.0,
        );
        painter.text(
            at,
            egui::Align2::CENTER_CENTER,
            "♥",
            egui::FontId::proportional(10.0 - 2.0 * p),
            Color32::from_rgb(255, 122, 150).gamma_multiply(1.0 - p),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visit_waits_its_turn_then_plays_once_per_interval() {
        let (_, mut app) = super::super::tests::app();
        // Previews never visit.
        app.cat_visit();
        assert!(app.visits.started.is_none());
        app.demo = false;
        app.expanded = false;
        app.prefs.cat_visit_minutes = 5;
        app.cat_visit();
        assert!(
            app.visits.started.is_none(),
            "the first visit waits one interval"
        );
        app.visits.next = Some(Instant::now());
        app.cat_visit();
        assert!(app.visits.started.is_some());
        assert_eq!(
            app.header_notice().map(|n| n.title.as_str()),
            Some("Cikung visit")
        );
        assert!(app.visit_pose().is_some());
        // The next one is an interval away, and a message on screen pushes it back.
        assert!(app.visits.next.unwrap() > Instant::now() + Duration::from_secs(290));
        app.visits.next = Some(Instant::now());
        app.cat_visit();
        assert!(app.visits.next.unwrap() < Instant::now() + Duration::from_secs(61));
        app.prefs.reduced_motion = true;
        assert!(app.visit_pose().is_none());
    }
}
