//! Original Cikung motion and hover policy. Time is injected for deterministic tests.
use eframe::egui::{Id, Painter, Rect, TextureHandle, Vec2, epaint::ClippedShape, pos2, vec2};

pub const COLLAPSED: Vec2 = vec2(252.0, 40.0);
pub const HOST: Vec2 = vec2(580.0, 480.0);

pub struct Motion {
    from: Vec2,
    velocity: Vec2,
    target: Vec2,
    started: f64,
}
impl Motion {
    pub fn new(size: Vec2) -> Self {
        Self {
            from: size,
            velocity: Vec2::ZERO,
            target: size,
            started: 0.0,
        }
    }
    fn state(&self, now: f64) -> (Vec2, Vec2) {
        // Analytic critical damping keeps velocity through reversals, independent of frame rate.
        let t = (now - self.started).max(0.0) as f32;
        let frequency = 22.0;
        let offset = self.from - self.target;
        let coefficient = self.velocity + frequency * offset;
        let decay = (-frequency * t).exp();
        let position = self.target + (offset + coefficient * t) * decay;
        let mut velocity = (self.velocity - frequency * coefficient * t) * decay;
        let bounded = position.clamp(COLLAPSED, HOST);
        if bounded.x != position.x {
            velocity.x = 0.0;
        }
        if bounded.y != position.y {
            velocity.y = 0.0;
        }
        if (bounded - self.target).length() < 0.02 && velocity.length() < 1.0 {
            (self.target, Vec2::ZERO)
        } else {
            (bounded, velocity)
        }
    }
    pub fn sample(&self, now: f64) -> Vec2 {
        self.state(now).0
    }
    pub fn retarget(&mut self, target: Vec2, now: f64, reduced_motion: bool) -> Vec2 {
        if reduced_motion {
            self.from = target;
            self.velocity = Vec2::ZERO;
            self.target = target;
            self.started = now;
        } else if target != self.target {
            (self.from, self.velocity) = self.state(now);
            self.target = target;
            self.started = now;
        }
        self.sample(now)
    }
    pub fn moving(&self, now: f64) -> bool {
        self.sample(now) != self.target || self.state(now).1 != Vec2::ZERO
    }
    pub fn openness(&self, now: f64) -> f32 {
        ((self.sample(now).x - COLLAPSED.x) / (HOST.x - COLLAPSED.x)).clamp(0.0, 1.0)
    }
}

pub struct ContentFrame {
    pub opacity: f32,
    pub offset: f32,
    outgoing_opacity: f32,
    outgoing_offset: f32,
    pub moving: bool,
}

#[derive(Default)]
pub struct ContentTransition {
    key: Option<Id>,
    started: f64,
    previous: Vec<ClippedShape>,
    last: Vec<ClippedShape>,
    previous_art: Vec<TextureHandle>,
    last_art: Vec<TextureHandle>,
}
impl ContentTransition {
    pub fn begin(&mut self, key: Id, now: f64, reduced: bool) -> ContentFrame {
        if self.key != Some(key) {
            self.started = if self.key.is_some() { now } else { now - 1.0 };
            self.previous = std::mem::take(&mut self.last);
            self.previous_art = std::mem::take(&mut self.last_art);
            self.key = Some(key);
        }
        let elapsed = if reduced {
            1.0
        } else {
            (now - self.started).max(0.0) as f32
        };
        let enter = (elapsed / 0.20).clamp(0.0, 1.0);
        let fade = (elapsed / 0.18).clamp(0.0, 1.0);
        let exit = (elapsed / 0.12).clamp(0.0, 1.0);
        let outgoing_opacity = (1.0 - elapsed / 0.10).clamp(0.0, 1.0);
        if outgoing_opacity == 0.0 {
            self.previous.clear();
            self.previous_art.clear();
        }
        ContentFrame {
            opacity: fade * fade * (3.0 - 2.0 * fade),
            offset: 8.0 * (1.0 - enter).powi(3),
            outgoing_opacity,
            outgoing_offset: -8.0 * (1.0 - (1.0 - exit).powi(3)),
            moving: enter < 1.0,
        }
    }
    pub fn paint_previous(
        &self,
        painter: &Painter,
        clip: Rect,
        opacity: f32,
        frame: &ContentFrame,
    ) {
        if frame.outgoing_opacity == 0.0 {
            return;
        }
        for entry in &self.previous {
            let shift = vec2(frame.outgoing_offset, 0.0);
            let mut shape = entry.shape.clone();
            shape.translate(shift);
            let mut painter =
                painter.with_clip_rect(entry.clip_rect.translate(shift).intersect(clip));
            painter.multiply_opacity(opacity * frame.outgoing_opacity);
            painter.add(shape);
        }
    }
    pub fn remember(&mut self, painter: &Painter, start: usize, art: Option<TextureHandle>) {
        self.last.clear();
        painter.ctx().graphics(|g| {
            if let Some(list) = g.get(painter.layer_id()) {
                self.last.extend(list.all_entries().skip(start).cloned());
            }
        });
        self.last_art.clone_from(&self.previous_art);
        if let Some(art) = art
            && !self.last_art.iter().any(|t| t.id() == art.id())
        {
            self.last_art.push(art);
        }
    }
}

pub fn surface_rect(host_width: f32, size: Vec2) -> Rect {
    Rect::from_min_size(pos2((host_width - size.x) / 2.0, 0.0), size)
}

pub fn aligned_surface_rect(host_width: f32, size: Vec2, scale: f32) -> Rect {
    let rect = surface_rect(host_width, size);
    Rect::from_min_max(
        pos2((rect.left() * scale).round() / scale, 0.0),
        pos2(
            (rect.right() * scale).round() / scale,
            (rect.bottom() * scale).round() / scale,
        ),
    )
}

pub struct HoverPolicy {
    pub enabled: bool,
    pub auto_close: bool,
    pub pinned: bool,
    pub held: bool,
}
#[derive(Default)]
pub struct HoverIntent {
    entered: Option<f64>,
    left: Option<f64>,
    suppressed: bool,
    was_expanded: bool,
    interacted: bool,
}
impl HoverIntent {
    pub fn interact(&mut self) {
        self.interacted = true;
    }
    pub fn waiting(&self) -> bool {
        self.entered.is_some() && !self.suppressed
    }
    pub fn update(&mut self, now: f64, inside: bool, expanded: bool, policy: HoverPolicy) -> bool {
        if !self.was_expanded && expanded {
            self.left = None;
            self.suppressed = false;
            self.interacted = true;
        }
        if self.was_expanded && !expanded {
            self.suppressed = true;
            self.interacted = false;
        }
        let mut next = expanded;
        if inside {
            self.left = None;
            let entered = *self.entered.get_or_insert(now);
            if !expanded && policy.enabled && !self.suppressed && now - entered >= 0.12 {
                next = true;
            }
        } else {
            self.entered = None;
            self.suppressed = false;
            let left = *self.left.get_or_insert(now);
            let delay = if self.interacted { 3.0 } else { 0.60 };
            if expanded
                && policy.auto_close
                && !policy.pinned
                && !policy.held
                && now - left >= delay
            {
                next = false;
                self.interacted = false;
            }
        }
        // A held popup/drag should get a full grace period after it ends.
        if policy.held || policy.pinned {
            self.left = None;
        }
        self.was_expanded = next;
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn policy() -> HoverPolicy {
        HoverPolicy {
            enabled: true,
            auto_close: true,
            pinned: false,
            held: false,
        }
    }
    #[test]
    fn explicit_open_gets_a_fresh_grace_even_when_hover_is_disabled() {
        let mut hover = HoverIntent::default();
        assert!(!hover.update(1.0, false, false, policy()));
        let mut disabled = policy();
        disabled.enabled = false;
        assert!(hover.update(100.0, false, true, disabled));
        let mut disabled = policy();
        disabled.enabled = false;
        assert!(hover.update(102.5, false, true, disabled));
        let mut disabled = policy();
        disabled.enabled = false;
        assert!(!hover.update(103.1, false, true, disabled));
    }
    #[test]
    fn geometry_is_bounded_centered_and_frame_rate_independent() {
        let mut motion = Motion::new(COLLAPSED);
        let open = vec2(HOST.x, 340.0);
        assert_eq!(motion.retarget(open, 1.0, false), COLLAPSED);
        let mut previous = COLLAPSED;
        for frame in 0..=30 {
            let now = 1.0 + frame as f64 / 100.0;
            let size = motion.sample(now);
            assert!(size.x >= previous.x && size.y >= previous.y);
            assert!(size.x <= open.x && size.y <= open.y);
            assert!((surface_rect(HOST.x, size).center().x - HOST.x / 2.0).abs() < 0.01);
            previous = size;
        }
        assert_eq!(motion.sample(3.0), open);
        assert!(!motion.moving(3.0));
    }
    #[test]
    fn reversing_or_changing_page_keeps_the_current_geometry() {
        let mut motion = Motion::new(COLLAPSED);
        motion.retarget(vec2(HOST.x, 340.0), 0.0, false);
        let middle = motion.sample(0.1);
        assert_eq!(motion.retarget(COLLAPSED, 0.1, false), middle);
        assert_eq!(motion.sample(0.9), COLLAPSED);
        motion.retarget(vec2(HOST.x, 460.0), 0.5, true);
        assert_eq!(
            motion.retarget(vec2(HOST.x, 360.0), 0.6, false),
            vec2(HOST.x, 460.0)
        );
        assert_eq!(motion.sample(1.3), vec2(HOST.x, 360.0));
    }
    #[test]
    fn interrupted_motion_preserves_velocity_including_height_only_changes() {
        let mut motion = Motion::new(COLLAPSED);
        motion.retarget(vec2(HOST.x, 440.0), 0.0, false);
        assert!(motion.moving(0.0));
        let velocity = motion.state(0.10).1;
        motion.retarget(COLLAPSED, 0.10, false);
        assert!((motion.state(0.10).1 - velocity).length() < 0.01);
        for frame in 0..120 {
            let size = motion.sample(0.10 + frame as f64 / 120.0);
            assert!(size.x >= COLLAPSED.x && size.x <= HOST.x);
            assert!(size.y >= COLLAPSED.y && size.y <= HOST.y);
        }
        motion.retarget(vec2(HOST.x, 440.0), 2.0, true);
        motion.retarget(vec2(HOST.x, 270.0), 2.1, false);
        let velocity = motion.state(2.18).1;
        motion.retarget(vec2(HOST.x, 380.0), 2.18, false);
        assert!((motion.state(2.18).1 - velocity).length() < 0.01);
        assert!(motion.sample(2.25).y < 440.0);
        assert_eq!(motion.sample(3.0), vec2(HOST.x, 380.0));
        assert_eq!(motion.retarget(COLLAPSED, 3.1, true), COLLAPSED);
        assert!(!motion.moving(3.1));
    }
    #[test]
    fn content_exits_before_entry_finishes_and_reduced_motion_is_immediate() {
        use eframe::egui::epaint::Shape;
        let mut transition = ContentTransition::default();
        let first = transition.begin(Id::new("files"), 0.0, false);
        assert_eq!(first.opacity, 1.0);
        assert_eq!(first.offset, 0.0);
        assert!(!first.moving);
        transition.last.push(ClippedShape {
            clip_rect: Rect::EVERYTHING,
            shape: Shape::Noop,
        });
        let start = transition.begin(Id::new("tasks"), 1.0, false);
        assert_eq!(start.opacity, 0.0);
        assert_eq!(start.offset, 8.0);
        assert_eq!(transition.previous.len(), 1);
        let middle = transition.begin(Id::new("tasks"), 1.11, false);
        assert_eq!(middle.outgoing_opacity, 0.0);
        assert!(transition.previous.is_empty());
        assert!(middle.opacity > 0.0 && middle.opacity < 1.0 && middle.moving);
        let end = transition.begin(Id::new("tasks"), 1.21, false);
        assert_eq!(end.opacity, 1.0);
        assert_eq!(end.offset, 0.0);
        assert!(!end.moving);
        let reduced = transition.begin(Id::new("water"), 1.3, true);
        assert_eq!(reduced.opacity, 1.0);
        assert_eq!(reduced.offset, 0.0);
        assert!(!reduced.moving);
    }
    #[test]
    fn animated_surface_stays_on_device_pixels_at_fractional_scales() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for frame in 0..100 {
                let size = COLLAPSED + (HOST - COLLAPSED) * frame as f32 / 100.0;
                let rect = aligned_surface_rect(HOST.x, size, scale);
                for edge in [rect.left(), rect.right(), rect.bottom()] {
                    assert!((edge * scale - (edge * scale).round()).abs() < 0.001);
                }
                assert!((rect.center().x - HOST.x / 2.0).abs() <= 0.5 / scale + 0.001);
                assert_eq!(rect.top(), 0.0);
            }
        }
    }
    #[test]
    fn quick_pass_does_not_open_and_reentry_cancels_close() {
        let mut hover = HoverIntent::default();
        assert!(!hover.update(0.0, true, false, policy()));
        assert!(!hover.update(0.08, false, false, policy()));
        assert!(!hover.update(0.2, true, false, policy()));
        assert!(hover.update(0.33, true, false, policy()));
        assert!(hover.update(0.4, false, true, policy()));
        assert!(hover.update(0.8, true, true, policy()));
        assert!(hover.update(1.0, false, true, policy()));
        assert!(hover.update(1.5, false, true, policy()));
        assert!(!hover.update(1.61, false, true, policy()));
    }
    #[test]
    fn explicit_close_requires_exit_before_hover_reopens() {
        let mut hover = HoverIntent::default();
        hover.update(0.0, true, true, policy());
        assert!(!hover.update(0.1, true, false, policy()));
        assert!(!hover.update(2.0, true, false, policy()));
        hover.update(2.1, false, false, policy());
        hover.update(2.2, true, false, policy());
        assert!(hover.update(2.33, true, false, policy()));
    }
    #[test]
    fn popups_drags_pin_and_interaction_preserve_the_panel() {
        let mut hover = HoverIntent::default();
        let mut held = policy();
        held.held = true;
        assert!(hover.update(0.0, false, true, held));
        assert!(hover.update(10.0, false, true, policy()));
        assert!(hover.update(10.5, false, true, policy()));
        hover.interact();
        assert!(hover.update(12.0, false, true, policy()));
        assert!(!hover.update(13.1, false, true, policy()));
        let mut pinned = policy();
        pinned.pinned = true;
        assert!(hover.update(20.0, false, true, pinned));
    }
}
