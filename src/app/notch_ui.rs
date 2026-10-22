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
                format!(
                    "Cikung  /  {}",
                    if self.more_open {
                        "More"
                    } else {
                        self.page.label()
                    }
                )
            } else if self.demo {
                "Cikung · preview".into()
            } else {
                self.session
                    .as_ref()
                    .map(|s| s.title.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Cikung".into())
            };
            let reserved = if show_actions {
                if self.page == Page::Music {
                    312.0
                } else {
                    174.0
                }
            } else {
                162.0
            };
            let response = ui
                .allocate_ui_with_layout(
                    Vec2::new((width - reserved).max(20.0), 32.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.set_min_size(Vec2::new((width - reserved).max(20.0), 32.0));
                        ui.add(
                            egui::Label::new(RichText::new(title).size(12.0))
                                .truncate()
                                .sense(Sense::click()),
                        )
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
                    if self.page == Page::Music {
                        if ui.small_button("Tools").clicked() {
                            self.navigate(Page::Overview);
                        }
                        if ui.small_button("Settings").clicked() {
                            self.navigate(Page::Settings);
                        }
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
                let active = self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.state == PlaybackState::Playing);
                let (r, _) = ui.allocate_exact_size(Vec2::new(20.0, 20.0), Sense::hover());
                for i in 0..3 {
                    let h = if active && !self.prefs.reduced_motion {
                        5.0 + ((self.started.elapsed().as_secs_f32() * 4.0 + i as f32).sin() + 1.0)
                            * 4.0
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
        });
    }

    pub(super) fn content(&mut self, ui: &mut egui::Ui) {
        if self.page == Page::Music && !self.more_open {
            self.music(ui);
            return;
        }
        let page = self.page;
        let more_open = self.more_open;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for page in Page::PRIMARY {
                let selected = self.page == page && !self.more_open;
                if ui.add(nav_button(page.label(), selected)).clicked() {
                    self.page = page;
                    self.more_open = false;
                    self.hover.interact();
                }
            }
            if ui
                .add(nav_button(
                    "More ···",
                    self.more_open
                        || (!Page::PRIMARY.contains(&self.page)
                            && !matches!(self.page, Page::Settings | Page::Style)),
                ))
                .clicked()
            {
                self.more_open = !self.more_open;
                self.hover.interact();
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(nav_button(
                        "Cats",
                        self.page == Page::Style && !self.more_open,
                    ))
                    .clicked()
                {
                    self.navigate(Page::Style);
                    self.more_open = false;
                    self.hover.interact();
                }
                if ui
                    .add(nav_button(
                        "Settings",
                        self.page == Page::Settings && !self.more_open,
                    ))
                    .clicked()
                {
                    self.navigate(Page::Settings);
                    self.more_open = false;
                    self.hover.interact();
                }
            });
        });
        ui.add_space(9.0);
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Stroke::new(1.0_f32, Color32::from_gray(26)),
        );
        ui.add_space(9.0);
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
        } else {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(if self.demo {
                        "Preview · playback controls disabled"
                    } else {
                        "Cikung"
                    })
                    .size(10.0)
                    .color(GRAY),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("Ctrl + Alt + {}", self.prefs.shortcut))
                            .size(10.0)
                            .color(Color32::from_gray(100)),
                    );
                });
            });
        }
    }

    fn more_tools(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("A few more things, close by.")
                .size(14.0)
                .color(GRAY),
        );
        ui.add_space(12.0);
        let tools = [
            (Page::Files, "Keep files nearby"),
            (Page::Tasks, "One thing at a time"),
            (Page::Notes, "Capture and remember"),
            (Page::Lyrics, "Follow the words"),
            (Page::Water, "Time for a sip"),
            (Page::Chat, "Ask Cikung"),
            (Page::Game, "A hint for your adventure"),
            (Page::Services, "Connected accounts"),
            (Page::Phone, "Pair your phone"),
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

fn nav_button(label: &str, selected: bool) -> egui::Button<'_> {
    egui::Button::new(
        RichText::new(label)
            .size(11.0)
            .color(if selected { WHITE } else { GRAY }),
    )
    .fill(if selected {
        Color32::from_gray(30)
    } else {
        Color32::BLACK
    })
    .corner_radius(8)
    .min_size(Vec2::new(48.0, 30.0))
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
