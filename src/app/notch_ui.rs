use super::*;
use chrono::{Datelike, Timelike};

fn date_time(now: &chrono::DateTime<impl chrono::TimeZone>) -> (String, String) {
    let days = ["Sen", "Sel", "Rab", "Kam", "Jum", "Sab", "Min"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
    ];
    (
        format!("{:02}:{:02}", now.hour(), now.minute()),
        format!(
            "{}, {} {}",
            days[now.weekday().num_days_from_monday() as usize],
            now.day(),
            months[now.month0() as usize]
        ),
    )
}

impl Cikung {
    pub(super) fn header(&mut self, ui: &mut egui::Ui) {
        let openness = self.motion.openness(self.started.elapsed().as_secs_f64());
        let show_actions = openness > 0.65 && ui.available_width() > 410.0;
        let width = ui.available_width();
        let now = chrono::Local::now();
        let (clock, date) = date_time(&now);
        ui.ctx().request_repaint_after(Duration::from_millis(
            60_000 - u64::from(now.second()) * 1000 - u64::from(now.nanosecond()) / 1_000_000,
        ));
        ui.horizontal_centered(|ui| {
            ui.spacing_mut().item_spacing.x = 9.0;
            if !show_actions && self.minimal_idle() {
                // Cat (22) + spacing (9) + time (~34), centred in the minimal pill.
                ui.add_space(((width - 65.0) / 2.0).max(0.0));
                self.draw_cat(ui, 22.0);
                ui.label(RichText::new(&clock).size(13.0).strong().color(ink()));
                return;
            }
            self.draw_cat(ui, 31.0);
            // Volume HUD: the bar eases from the last shown level to the new one.
            let hud = (!show_actions).then(|| self.volume_hud_level()).flatten();
            let hud_fill = hud.map(|(level, _)| {
                ui.ctx()
                    .animate_value_with_time(egui::Id::new("volume-hud-fill"), level, 0.18)
            });
            let hud_alpha = ui.ctx().animate_bool_with_time(
                egui::Id::new("volume-hud-on"),
                hud.is_some(),
                0.15,
            );
            if hud.is_some() {
                // Wake again in time to hand the slot back to the title.
                ui.ctx().request_repaint_after(Duration::from_millis(100));
            }
            // The HUD takes the whole row beside the cat; the title, clock and date step aside.
            if let (Some((level, muted)), Some(fill)) = (hud, hud_fill) {
                let size = Vec2::new(ui.available_width(), 32.0);
                let response = ui
                    .allocate_ui_with_layout(
                        size,
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.set_min_size(size);
                            volume_hud(ui, fill, level, muted, hud_alpha)
                        },
                    )
                    .inner;
                if response.clicked() {
                    self.expanded = !self.expanded;
                }
                return;
            }
            let notice = self.header_notice();
            let tooltip = notice
                .map(|n| n.text.clone())
                .unwrap_or_else(|| "Click to open or close · Escape to close".into());
            let title = if self.file_hover.is_some() {
                "Release to keep files".to_owned()
            } else if let Some(notice) = notice {
                if notice.title == "WhatsApp test" {
                    format!("{} · {}", notice.title, notice.text)
                } else {
                    notice.text.clone()
                }
            } else if self.recorder.is_recording() {
                "Recording audio".to_owned()
            } else if self.tools.focus.running {
                format!("Focus · {}", self.tools.focus.formatted_remaining())
            } else if show_actions {
                String::new()
            } else if self.demo {
                "Cikung · preview".into()
            } else {
                self.session
                    .as_ref()
                    .map(|s| s.title.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Cikung".into())
            };
            // Collapsed and playing with synced lyrics: show the live line like Nusa does.
            let lyric = (!show_actions
                && notice.is_none()
                && self.file_hover.is_none()
                && !self.recorder.is_recording()
                && !self.tools.focus.running
                && !self.demo)
                .then(|| self.current_lyric().map(str::to_owned))
                .flatten();
            // A WhatsApp message rides the lyric ticker, so the pill widening to fit it reveals
            // the line instead of re-cutting it every frame.
            let lyric = notice
                .filter(|n| n.title == "WhatsApp" && !show_actions && self.file_hover.is_none())
                .map(|n| n.text.clone())
                .or(lyric);
            let reduced = self.prefs.reduced_motion;
            // Open with nothing more urgent to say: Nusa-style two-tone page title.
            let heading = title
                .is_empty()
                .then(|| page_title((!self.more_open).then_some(self.page)));
            // Cat, clock, gear and × with their spacing.
            let reserved = if show_actions { 212.0 } else { 162.0 };
            let title_width = (width - reserved).max(20.0);
            // A lyric is laid out at the width the pill is growing to and clipped by its
            // moving edge, so a longer line is revealed rather than re-cut every frame.
            let lyric_cut = (self.motion.target().x - 28.0 - reserved).max(title_width);
            let response = ui
                .allocate_ui_with_layout(
                    Vec2::new(title_width, 32.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_size(Vec2::new(title_width, 32.0));
                        if let Some(line) = &lyric {
                            lyric_ticker(ui, line, reduced, lyric_cut)
                        } else if let Some(heading) = heading {
                            ui.add(
                                egui::Label::new(title_job(heading))
                                    .truncate()
                                    .sense(Sense::click()),
                            )
                        } else {
                            ui.add(
                                egui::Label::new(RichText::new(title).size(12.0))
                                    .truncate()
                                    .sense(Sense::click()),
                            )
                        }
                    },
                )
                .inner
                .on_hover_text(tooltip);
            if response.clicked() {
                self.expanded = !self.expanded;
            }
            let (clock_rect, clock_response) =
                ui.allocate_exact_size(Vec2::new(75.0, 32.0), Sense::click());
            ui.painter().text(
                clock_rect.center_top() + Vec2::new(0.0, 1.0),
                egui::Align2::CENTER_TOP,
                clock,
                egui::FontId::proportional(12.0),
                ink(),
            );
            ui.painter().text(
                clock_rect.center_bottom() - Vec2::new(0.0, 2.0),
                egui::Align2::CENTER_BOTTOM,
                date,
                egui::FontId::proportional(9.0),
                muted(),
            );
            if clock_response
                .on_hover_text(format!(
                    "{} · waktu lokal PC",
                    now.format("%d/%m/%Y %H:%M %:z")
                ))
                .clicked()
            {
                self.expanded = !self.expanded;
            }
            if show_actions {
                ui.scope(|ui| {
                    ui.multiply_opacity(((openness - 0.65) / 0.35).clamp(0.0, 1.0));
                    if !self.expanded {
                        ui.disable();
                    }
                    if gear_button(ui).clicked() {
                        self.navigate(Page::Settings);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new("×").size(18.0))
                                .frame(false)
                                .min_size(Vec2::new(28.0, 30.0)),
                        )
                        .on_hover_text("Close panel · Escape")
                        .clicked()
                    {
                        self.expanded = false;
                    }
                });
            } else {
                self.eq_bars(ui);
            }
        });
    }

    /// The latest notice the collapsed header shows in place of the title.
    pub(super) fn header_notice(&self) -> Option<&Notice> {
        self.notices.back().filter(|n| {
            matches!(
                n.title.as_str(),
                "Bluetooth"
                    | "Battery"
                    | "Network"
                    | "Caps Lock"
                    | "AI finished"
                    | "WhatsApp"
                    | "WhatsApp test"
                    | "Task reminder"
                    | "Water break"
                    | "Focus complete"
                    | "Break complete"
            ) && n.until > Instant::now()
        })
    }

    /// The volume HUD's level and mute while it is due in the collapsed notch.
    pub(super) fn volume_hud_level(&self) -> Option<(f32, bool)> {
        self.volume_hud
            .filter(|(_, _, until)| *until > Instant::now())
            .map(|(level, muted, _)| (level, muted))
    }

    /// Nusa's minimal notch: just the cat and the time while nothing plays or needs saying.
    pub(super) fn minimal_idle(&self) -> bool {
        self.prefs.minimal_notch
            && !self.demo
            && self.header_notice().is_none()
            && self.volume_hud_level().is_none()
            && self.file_hover.is_none()
            && !self.recorder.is_recording()
            && !self.tools.focus.running
            && self
                .session
                .as_ref()
                .is_none_or(|s| s.state != PlaybackState::Playing)
    }

    /// Three small bars that bounce while music plays.
    fn eq_bars(&self, ui: &mut egui::Ui) {
        let active = self
            .session
            .as_ref()
            .is_some_and(|s| s.state == PlaybackState::Playing);
        let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
        for i in 0..3 {
            let h = if active && !self.prefs.reduced_motion {
                5.0 + ((self.started.elapsed().as_secs_f32() * 4.0 + i as f32).sin() + 1.0) * 4.0
            } else {
                3.0
            };
            ui.painter().rect_filled(
                Rect::from_center_size(
                    Pos2::new(r.left() + 3.0 + i as f32 * 6.0, r.center().y),
                    Vec2::new(2.0, h),
                ),
                1,
                if active { accent() } else { muted() },
            );
        }
    }

    pub(super) fn content(&mut self, ui: &mut egui::Ui) {
        if self.page == Page::Music && !self.more_open {
            self.music(ui);
            return;
        }
        let page = self.page;
        let more_open = self.more_open;
        let footer_rect = Rect::from_min_size(
            Pos2::new(ui.max_rect().left(), ui.max_rect().bottom() - 16.0),
            Vec2::new(ui.available_width(), 16.0),
        );
        let body_height = (footer_rect.top() - ui.cursor().top() - 6.0).max(80.0);
        egui::ScrollArea::vertical()
            .id_salt(if self.more_open {
                "more-tools"
            } else {
                self.page.label()
            })
            .auto_shrink([false, false])
            .max_height(body_height)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if more_open {
                    self.more_tools(ui);
                } else {
                    match page {
                        Page::Overview => self.overview(ui),
                        Page::Music => self.music(ui),
                        Page::Lyrics => self.lyrics(ui),
                        Page::Focus => self.focus(ui),
                        Page::Files => self.files(ui),
                        Page::Tasks => self.tasks(ui),
                        Page::Water => self.water(ui),
                        Page::Notes => self.notes(ui),
                        Page::Agents => self.agents(ui),
                        Page::Chat => self.chat_panel(ui),
                        Page::Game => self.game_panel(ui),
                        Page::Services => self.services(ui),
                        Page::Phone => self.phone(ui),
                        Page::Style => self.style_panel(ui),
                        Page::Settings => self.settings(ui),
                    }
                }
            });
        let mut footer = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("notch-footer")
                .max_rect(footer_rect),
        );
        footer.set_clip_rect(footer_rect.intersect(ui.clip_rect()));
        let ui = &mut footer;
        if let Some(notice) = self.notices.back() {
            ui.add(
                egui::Label::new(
                    RichText::new(format!("{} · {}", notice.title, notice.text))
                        .size(10.0)
                        .color(accent()),
                )
                .truncate(),
            )
            .on_hover_text(&notice.text);
        } else if self.demo {
            ui.label(RichText::new("Preview").size(10.0).color(muted()));
        }
    }

    /// Carousel position: each `Page::CAROUSEL` page, then the all-tools grid last.
    fn carousel_index(&self) -> Option<usize> {
        if self.more_open {
            return Some(Page::CAROUSEL.len());
        }
        Page::CAROUSEL.iter().position(|page| *page == self.page)
    }

    fn show_carousel(&mut self, index: usize) {
        if let Some(page) = Page::CAROUSEL.get(index) {
            self.navigate(*page);
        } else {
            self.more_open = true;
            self.hover.interact();
        }
    }

    /// Nusa-style page indicator centred on the panel's bottom edge. The current page's pill
    /// glides between the dots, each dot widening as it arrives; the mouse wheel over the
    /// strip flips pages.
    fn page_indicator(&mut self, ui: &mut egui::Ui, center: Pos2, opacity: f32, interactive: bool) {
        let count = Page::CAROUSEL.len() + 1;
        let spacing = 12.0;
        let left = center.x - spacing * (count as f32 - 1.0) / 2.0;
        let glide = if self.prefs.reduced_motion { 0.0 } else { 0.32 };
        let pill = self
            .carousel_index()
            .map(|index| ease_toward(ui.ctx(), ui.id().with("page-pill"), index as f32, glide));
        for index in 0..count {
            let dot = Pos2::new(left + spacing * index as f32, center.y);
            let hit = Rect::from_center_size(dot, Vec2::new(spacing, 14.0));
            let hovered = interactive && ui.rect_contains_pointer(hit);
            // 1 under the pill, 0 a dot or more away, smoothstepped.
            let near = pill.map_or(0.0, |p| (1.0 - (index as f32 - p).abs()).clamp(0.0, 1.0));
            let near = near * near * (3.0 - 2.0 * near);
            let dim = Color32::from_gray(if hovered { 170 } else { 85 });
            ui.painter().rect_filled(
                Rect::from_center_size(dot, Vec2::new(5.0 + 9.0 * near, 5.0)),
                2.5,
                dim.lerp_to_gamma(ink(), near).gamma_multiply(opacity),
            );
            if !interactive {
                continue;
            }
            let (main, accent, _) = page_title(Page::CAROUSEL.get(index).copied());
            if ui
                .interact(hit, ui.id().with(("page-dot", index)), Sense::click())
                .on_hover_text(format!("{main} {accent}"))
                .clicked()
            {
                self.show_carousel(index);
            }
        }
        let strip = Rect::from_center_size(center, Vec2::new(spacing * count as f32 + 16.0, 22.0));
        if interactive && ui.rect_contains_pointer(strip) {
            // Wheel down or swipe left: next page. One notch (40 pt) is one page.
            let delta = ui.input(|i| i.raw_scroll_delta);
            let id = ui.id().with("page-wheel");
            let total = ui.data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0) - delta.x - delta.y;
            let step = if total.abs() >= 30.0 {
                total.signum() as isize
            } else {
                0
            };
            ui.data_mut(|d| d.insert_temp(id, if step == 0 { total } else { 0.0 }));
            if step != 0 {
                self.step_page(step);
            }
        }
    }

    /// Next/previous carousel page, wrapping; pages outside it enter at either end.
    pub(super) fn step_page(&mut self, delta: isize) {
        let len = Page::CAROUSEL.len() as isize + 1;
        let next = match self.carousel_index() {
            Some(index) => (index as isize + delta).rem_euclid(len),
            None if delta > 0 => 0,
            None => len - 1,
        };
        self.show_carousel(next as usize);
    }

    /// Nusa-style ‹ › arrows on the panel edges; ←/→ also flip pages when nothing has focus.
    pub(super) fn carousel_arrows(&mut self, ui: &mut egui::Ui, surface: Rect, opacity: f32) {
        let interactive = self.expanded && opacity > 0.98;
        self.page_indicator(
            ui,
            Pos2::new(surface.center().x, surface.bottom() - 9.0),
            opacity,
            interactive,
        );
        let y = surface.top() + 50.0 + (surface.height() - 62.0) / 2.0;
        for (delta, x) in [(-1, surface.left() + 13.0), (1, surface.right() - 13.0)] {
            let rect = Rect::from_center_size(Pos2::new(x, y), Vec2::splat(22.0));
            let hovered = interactive && ui.rect_contains_pointer(rect);
            ui.painter().circle_filled(
                rect.center(),
                11.0,
                veil(if hovered { 40 } else { 16 }).gamma_multiply(opacity),
            );
            let (c, s) = (rect.center(), delta as f32);
            ui.painter().add(egui::Shape::line(
                vec![
                    c + Vec2::new(-2.0 * s, -4.0),
                    c + Vec2::new(2.0 * s, 0.0),
                    c + Vec2::new(-2.0 * s, 4.0),
                ],
                Stroke::new(
                    1.8_f32,
                    ink().gamma_multiply(opacity * if hovered { 1.0 } else { 0.65 }),
                ),
            ));
            if interactive
                && ui
                    .interact(rect, ui.id().with(("carousel", delta)), Sense::click())
                    .on_hover_text(if delta < 0 { "Previous" } else { "Next" })
                    .clicked()
            {
                self.step_page(delta);
            }
        }
        if interactive
            && !ui.ctx().wants_keyboard_input()
            && ui.ctx().memory(|memory| memory.focused().is_none())
        {
            let (left, right) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                )
            });
            if left {
                self.step_page(-1);
            } else if right {
                self.step_page(1);
            }
        }
    }

    fn more_tools(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Every tool, one tap away.")
                .size(14.0)
                .color(muted()),
        );
        ui.add_space(12.0);
        let tools = [
            (Page::Music, "What's playing"),
            (Page::Focus, "Pomodoro sessions"),
            (Page::Water, "Time for a sip"),
            (Page::Files, "Keep files nearby"),
            (Page::Tasks, "One thing at a time"),
            (Page::Notes, "Capture and remember"),
            (Page::Agents, "Claude and Codex"),
            (Page::Lyrics, "Follow the words"),
            (Page::Chat, "Ask Cikung"),
            (Page::Game, "A hint for your adventure"),
            (Page::Services, "Connected accounts"),
            (Page::Phone, "Pair your phone"),
            (Page::Overview, "Your little team"),
            (Page::Style, "Meet your cats"),
        ];
        let width = (ui.available_width() - 10.0) / 2.0;
        for row in tools.chunks(2) {
            ui.horizontal(|ui| {
                for (page, caption) in row {
                    let text =
                        RichText::new(format!("{}  ·  {}", page.label(), caption)).size(11.0);
                    if ui
                        .add_sized([width, 42.0], egui::Button::new(text).corner_radius(10))
                        .clicked()
                    {
                        self.navigate(*page);
                        self.more_open = false;
                        self.hover.interact();
                    }
                }
            });
        }
    }
}

/// Eases out (cubic) over `seconds` from the value shown when `target` last changed.
fn ease_toward(ctx: &egui::Context, id: egui::Id, target: f32, seconds: f64) -> f32 {
    let now = ctx.input(|i| i.time);
    let (from, to, started) =
        ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(id, || (target, target, now)));
    let t = if seconds > 0.0 {
        ((now - started) / seconds).clamp(0.0, 1.0) as f32
    } else {
        1.0
    };
    let shown = from + (to - from) * (1.0 - (1.0 - t).powi(3));
    if to != target {
        ctx.data_mut(|d| d.insert_temp(id, (shown, target, now)));
    }
    if to != target || t < 1.0 {
        ctx.request_repaint();
    }
    shown
}

/// Nusa-style volume HUD beside the cat: a speaker, a bar easing to the new level, and the
/// percentage (or "Muted").
fn volume_hud(
    ui: &mut egui::Ui,
    fill: f32,
    level: f32,
    is_muted: bool,
    alpha: f32,
) -> egui::Response {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click());
    let painter = ui.painter_at(rect);
    let color = ink().gamma_multiply(alpha);
    let waves = if is_muted || level <= 0.0 {
        0
    } else if level > 0.5 {
        2
    } else {
        1
    };
    super::music::paint_speaker(
        &painter,
        Pos2::new(rect.left() + 8.0, rect.center().y),
        waves,
        is_muted,
        color,
    );
    let bar = Rect::from_min_max(
        Pos2::new(rect.left() + 26.0, rect.center().y - 2.0),
        Pos2::new(
            (rect.right() - 44.0).max(rect.left() + 40.0),
            rect.center().y + 2.0,
        ),
    );
    painter.rect_filled(bar, 2, veil(40).gamma_multiply(alpha));
    painter.rect_filled(
        bar.with_max_x(bar.left() + bar.width() * fill.clamp(0.0, 1.0)),
        2,
        if is_muted {
            muted().gamma_multiply(alpha)
        } else {
            color
        },
    );
    painter.text(
        Pos2::new(rect.right(), rect.center().y),
        egui::Align2::RIGHT_CENTER,
        if is_muted {
            "Muted".to_owned()
        } else {
            format!("{:.0}%", level * 100.0)
        },
        egui::FontId::proportional(12.0),
        color,
    );
    response
}

/// Collapsed-notch lyric line: the old line lifts away as the new one rises in.
/// `width` is where a line is cut; the visible rect clips anything past it.
pub(super) fn lyric_ticker(
    ui: &mut egui::Ui,
    line: &str,
    reduced: bool,
    width: f32,
) -> egui::Response {
    let rect = ui.max_rect();
    let response = ui.allocate_rect(rect, Sense::click());
    let now = ui.input(|i| i.time);
    let id = ui.id().with("lyric-ticker");
    let (previous, changed) = ui.data_mut(|data| {
        let state =
            data.get_temp_mut_or_insert_with(id, || (String::new(), line.to_owned(), f64::MIN));
        if state.1 != line {
            *state = (state.1.clone(), line.to_owned(), now);
        }
        (state.0.clone(), state.2)
    });
    let progress = if reduced {
        1.0
    } else {
        egui::emath::easing::cubic_out(((now - changed) / 0.32).clamp(0.0, 1.0) as f32)
    };
    if progress < 1.0 {
        ui.ctx().request_repaint_after(Duration::from_millis(8));
    }
    let painter = ui.painter_at(rect);
    let draw = |text: &str, dy: f32, alpha: f32| {
        let mut job = egui::text::LayoutJob::simple_singleline(
            text.to_owned(),
            egui::FontId::proportional(12.5),
            ink(),
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(rect.width()));
        let galley = painter.layout_job(job);
        let pos = Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0 + dy);
        painter.galley_with_override_text_color(pos, galley, ink().gamma_multiply(alpha));
    };
    if progress < 1.0 && !previous.is_empty() {
        draw(&previous, -9.0 * progress, 1.0 - progress);
    }
    draw(line, 9.0 * (1.0 - progress), progress);
    response
}

/// Frameless gear icon for Settings, drawn so it never depends on an icon font.
fn gear_button(ui: &mut egui::Ui) -> egui::Response {
    let response = ui.add(
        egui::Button::new(" ")
            .frame(false)
            .min_size(Vec2::new(28.0, 30.0)),
    );
    let (c, painter) = (response.rect.center(), ui.painter());
    let color = if response.hovered() {
        ink()
    } else {
        muted().lerp_to_gamma(ink(), 0.4)
    };
    for tooth in 0..8 {
        let direction = Vec2::angled(tooth as f32 * std::f32::consts::TAU / 8.0);
        painter.line_segment(
            [c + direction * 5.0, c + direction * 7.6],
            Stroke::new(2.6_f32, color),
        );
    }
    painter.circle_stroke(c, 4.6, Stroke::new(2.2_f32, color));
    response.on_hover_text("Settings")
}

/// Nusa-style two-tone title, such as "Todo List" or "Water Reminder". `None` is the Tools grid.
fn page_title(page: Option<Page>) -> (&'static str, &'static str, Color32) {
    let teal = Color32::from_rgb(80, 210, 200);
    match page {
        None => ("All", "Tools", accent()),
        Some(Page::Overview) => ("Cikung", "Home", accent()),
        Some(Page::Music) => ("Now", "Playing", Color32::from_rgb(30, 215, 96)),
        Some(Page::Lyrics) => ("Synced", "Lyrics", Color32::from_rgb(110, 165, 255)),
        Some(Page::Focus) => ("Focus", "Timer", Color32::from_rgb(255, 105, 97)),
        Some(Page::Water) => ("Water", "Reminder", Color32::from_rgb(80, 170, 255)),
        Some(Page::Files) => ("File", "Shelf", Color32::from_gray(170)),
        Some(Page::Tasks) => ("Todo", "List", Color32::from_rgb(70, 205, 120)),
        Some(Page::Notes) => ("Quick", "Notes", Color32::from_rgb(255, 204, 77)),
        Some(Page::Agents) => ("AI", "Agents", Color32::from_rgb(175, 135, 255)),
        Some(Page::Chat) => ("Ask", "Cikung", accent()),
        Some(Page::Game) => ("Game", "Hints", Color32::from_rgb(255, 120, 190)),
        Some(Page::Services) => ("Connected", "Accounts", teal),
        Some(Page::Phone) => ("Phone", "Link", teal),
        Some(Page::Style) => ("Cat", "Wardrobe", accent()),
        Some(Page::Settings) => ("Cikung", "Settings", muted()),
    }
}

fn title_job((main, accent, color): (&str, &str, Color32)) -> egui::text::LayoutJob {
    let font = egui::FontId::proportional(13.0);
    let mut job = egui::text::LayoutJob::default();
    job.append(main, 0.0, egui::TextFormat::simple(font.clone(), ink()));
    job.append(accent, 4.0, egui::TextFormat::simple(font, color));
    job
}

#[cfg(test)]
mod date_tests {
    use super::*;
    use chrono::TimeZone;
    #[test]
    fn date_and_clock_follow_the_supplied_local_day_across_midnight() {
        let zone = chrono::FixedOffset::east_opt(7 * 3600).unwrap();
        let before = zone.with_ymd_and_hms(2026, 10, 22, 23, 59, 59).unwrap();
        assert_eq!(date_time(&before), ("23:59".into(), "Kam, 22 Okt".into()));
        let after = before + chrono::Duration::seconds(1);
        assert_eq!(date_time(&after), ("00:00".into(), "Jum, 23 Okt".into()));
        let year = zone.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(date_time(&year), ("00:00".into(), "Jum, 1 Jan".into()));
    }
}
