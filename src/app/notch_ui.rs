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
            self.draw_cat(ui, 31.0);
            let camera = self.camera_notch.map(|(gap, _)| gap);
            if camera.is_some() && !show_actions {
                // Collapsed under a Mac camera notch: cat left, music bars right, and
                // nothing drawn behind the camera itself.
                ui.add_space((width - 31.0 - 20.0 - 18.0).max(0.0));
                self.eq_bars(ui);
                return;
            }
            let notice = self.notices.back().filter(|n| {
                matches!(
                    n.title.as_str(),
                    "Bluetooth"
                        | "AI finished"
                        | "WhatsApp"
                        | "WhatsApp test"
                        | "Task reminder"
                        | "Water break"
                        | "Focus complete"
                        | "Break complete"
                ) && n.until > Instant::now()
            });
            let tooltip = notice
                .map(|n| n.text.clone())
                .unwrap_or_else(|| "Click to open or close · Escape to close".into());
            let title = if self.file_hover.is_some() {
                "Release to keep files".to_owned()
            } else if let Some(notice) = notice {
                if notice.title.starts_with("WhatsApp") {
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
            let reduced = self.prefs.reduced_motion;
            // Open with nothing more urgent to say: Nusa-style two-tone page title.
            let heading = title
                .is_empty()
                .then(|| page_title((!self.more_open).then_some(self.page)));
            // Cat, clock, gear and × with their spacing.
            let reserved = if show_actions { 212.0 } else { 162.0 };
            let title_width = match camera {
                // The title stays left of the camera; clock and buttons go right of it.
                Some(gap) => ((width - gap) / 2.0 - 31.0 - 18.0).max(20.0),
                None => (width - reserved).max(20.0),
            };
            let response = ui
                .allocate_ui_with_layout(
                    Vec2::new(title_width, 32.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_size(Vec2::new(title_width, 32.0));
                        if let Some(line) = &lyric {
                            lyric_ticker(ui, line, reduced)
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
            if let Some(gap) = camera {
                ui.add_space(gap);
            }
            let (clock_rect, clock_response) =
                ui.allocate_exact_size(Vec2::new(75.0, 32.0), Sense::click());
            ui.painter().text(
                clock_rect.center_top() + Vec2::new(0.0, 1.0),
                egui::Align2::CENTER_TOP,
                clock,
                egui::FontId::proportional(12.0),
                WHITE,
            );
            ui.painter().text(
                clock_rect.center_bottom() - Vec2::new(0.0, 2.0),
                egui::Align2::CENTER_BOTTOM,
                date,
                egui::FontId::proportional(9.0),
                GRAY,
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
                if active { ORANGE } else { GRAY },
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
                        .color(ORANGE),
                )
                .truncate(),
            )
            .on_hover_text(&notice.text);
        } else if self.demo {
            ui.label(RichText::new("Preview").size(10.0).color(GRAY));
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

    /// Nusa-style page indicator centred on the panel's bottom edge: a pill for the
    /// current page, dots for the rest.
    fn page_indicator(&mut self, ui: &mut egui::Ui, center: Pos2, opacity: f32, interactive: bool) {
        let count = Page::CAROUSEL.len() + 1;
        let spacing = 12.0;
        let left = center.x - spacing * (count as f32 - 1.0) / 2.0;
        let current = self.carousel_index();
        for index in 0..count {
            let dot = Pos2::new(left + spacing * index as f32, center.y);
            let hit = Rect::from_center_size(dot, Vec2::new(spacing, 14.0));
            let hovered = interactive && ui.rect_contains_pointer(hit);
            if current == Some(index) {
                ui.painter().rect_filled(
                    Rect::from_center_size(dot, Vec2::new(14.0, 5.0)),
                    3,
                    WHITE.gamma_multiply(opacity),
                );
            } else {
                let gray = if hovered { 170 } else { 85 };
                ui.painter().circle_filled(
                    dot,
                    2.5,
                    Color32::from_gray(gray).gamma_multiply(opacity),
                );
            }
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
                Color32::from_white_alpha(if hovered { 40 } else { 16 }).gamma_multiply(opacity),
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
                    WHITE.gamma_multiply(opacity * if hovered { 1.0 } else { 0.65 }),
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
                .color(GRAY),
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

/// Collapsed-notch lyric line: the old line lifts away as the new one rises in.
fn lyric_ticker(ui: &mut egui::Ui, line: &str, reduced: bool) -> egui::Response {
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
            WHITE,
        );
        job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width());
        let galley = painter.layout_job(job);
        let pos = Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0 + dy);
        painter.galley_with_override_text_color(pos, galley, WHITE.gamma_multiply(alpha));
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
        WHITE
    } else {
        GRAY.lerp_to_gamma(WHITE, 0.4)
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
        None => ("All", "Tools", ORANGE),
        Some(Page::Overview) => ("Cikung", "Home", ORANGE),
        Some(Page::Music) => ("Now", "Playing", Color32::from_rgb(30, 215, 96)),
        Some(Page::Lyrics) => ("Synced", "Lyrics", Color32::from_rgb(110, 165, 255)),
        Some(Page::Focus) => ("Focus", "Timer", Color32::from_rgb(255, 105, 97)),
        Some(Page::Water) => ("Water", "Reminder", Color32::from_rgb(80, 170, 255)),
        Some(Page::Files) => ("File", "Shelf", Color32::from_gray(170)),
        Some(Page::Tasks) => ("Todo", "List", Color32::from_rgb(70, 205, 120)),
        Some(Page::Notes) => ("Quick", "Notes", Color32::from_rgb(255, 204, 77)),
        Some(Page::Agents) => ("AI", "Agents", Color32::from_rgb(175, 135, 255)),
        Some(Page::Chat) => ("Ask", "Cikung", ORANGE),
        Some(Page::Game) => ("Game", "Hints", Color32::from_rgb(255, 120, 190)),
        Some(Page::Services) => ("Connected", "Accounts", teal),
        Some(Page::Phone) => ("Phone", "Link", teal),
        Some(Page::Style) => ("Cat", "Wardrobe", ORANGE),
        Some(Page::Settings) => ("Cikung", "Settings", GRAY),
    }
}

fn title_job((main, accent, color): (&str, &str, Color32)) -> egui::text::LayoutJob {
    let font = egui::FontId::proportional(13.0);
    let mut job = egui::text::LayoutJob::default();
    job.append(main, 0.0, egui::TextFormat::simple(font.clone(), WHITE));
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
