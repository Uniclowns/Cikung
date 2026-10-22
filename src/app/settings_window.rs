use super::settings_pages::SettingsPage;
use super::*;

pub(super) fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("cikung-settings")
}

fn sidebar_row(ui: &mut egui::Ui, page: SettingsPage, selected: bool) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 32.0), Sense::click());
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, 9, Color32::from_gray(if selected { 25 } else { 14 }));
    }
    let color = if selected {
        WHITE
    } else {
        Color32::from_rgb(184, 187, 194)
    };
    let c = rect.left_center() + Vec2::new(17.0, 0.0);
    let stroke = Stroke::new(1.3_f32, color);
    // Small line icons stay still while their row changes color on hover.
    match page {
        SettingsPage::Dictation => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c - Vec2::new(0.0, 2.0), Vec2::new(5.0, 10.0)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            ui.painter()
                .line_segment([c + Vec2::new(-5.0, 0.0), c + Vec2::new(-5.0, 3.0)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(-5.0, 3.0), c + Vec2::new(0.0, 6.0)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(0.0, 6.0), c + Vec2::new(5.0, 3.0)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(5.0, 3.0), c], stroke);
        }
        SettingsPage::Music => {
            ui.painter()
                .line_segment([c + Vec2::new(3.0, -6.0), c + Vec2::new(3.0, 4.0)], stroke);
            ui.painter()
                .line_segment([c + Vec2::new(3.0, -6.0), c + Vec2::new(7.0, -4.0)], stroke);
            ui.painter()
                .circle_stroke(c + Vec2::new(0.0, 4.0), 3.0, stroke);
        }
        SettingsPage::General | SettingsPage::Focus => {
            ui.painter().circle_stroke(c, 5.0, stroke);
            if page == SettingsPage::Focus {
                ui.painter()
                    .line_segment([c, c - Vec2::new(0.0, 3.0)], stroke);
                ui.painter()
                    .line_segment([c, c + Vec2::new(3.0, 1.0)], stroke);
            } else {
                for delta in [Vec2::X, -Vec2::X, Vec2::Y, -Vec2::Y] {
                    ui.painter()
                        .line_segment([c + delta * 7.0, c + delta * 9.0], stroke);
                }
            }
        }
        _ => {
            ui.painter().rect_stroke(
                Rect::from_center_size(c, Vec2::new(14.0, 12.0)),
                3,
                stroke,
                egui::StrokeKind::Inside,
            );
            let mark = match page {
                SettingsPage::Agents => ">_",
                SettingsPage::Chat => "··",
                SettingsPage::Cats => "ω",
                SettingsPage::Phone => "·",
                SettingsPage::Shortcuts => "⌘",
                SettingsPage::Integrations => "+",
                SettingsPage::Tasks => "✓",
                _ => "=",
            };
            ui.painter().text(
                c,
                egui::Align2::CENTER_CENTER,
                mark,
                egui::FontId::proportional(9.0),
                color,
            );
        }
    }
    ui.painter().text(
        rect.left_center() + Vec2::new(35.0, 0.0),
        egui::Align2::LEFT_CENTER,
        page.label(),
        egui::FontId::proportional(12.0),
        color,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

impl Cikung {
    pub(crate) fn navigate(&mut self, page: Page) {
        if matches!(page, Page::Settings | Page::Style) {
            self.open_settings_page(if page == Page::Style {
                SettingsPage::Cats
            } else {
                SettingsPage::General
            });
        } else {
            self.page = page;
            self.more_open = false;
            self.hover.interact();
        }
    }

    pub(super) fn follow_music_on_open(&mut self, previously_expanded: bool) {
        if !previously_expanded
            && self.expanded
            && self.prefs.music_on_hover
            && self
                .session
                .as_ref()
                .is_some_and(|s| s.state == PlaybackState::Playing)
        {
            self.page = Page::Music;
            self.more_open = false;
        }
    }

    pub(super) fn settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let focus = std::mem::take(&mut self.settings_focus);
        let scale = ctx.pixels_per_point();
        let display = native::displays().into_iter().find(|d| d.primary);
        let size = display
            .as_ref()
            .map(|d| {
                Vec2::new(
                    (d.width as f32 / scale - 64.0).clamp(640.0, 1040.0),
                    (d.height as f32 / scale - 90.0).clamp(420.0, 700.0),
                )
            })
            .unwrap_or(Vec2::new(1040.0, 700.0));
        let mut builder = egui::ViewportBuilder::default()
            .with_title("Cikung Settings")
            .with_inner_size(size)
            .with_min_inner_size([size.x.min(740.0), size.y.min(500.0)])
            .with_decorations(true)
            .with_transparent(false)
            .with_resizable(true);
        if focus && let Some(d) = display {
            builder = builder.with_position([
                d.left as f32 / scale + (d.width as f32 / scale - size.x) / 2.0,
                d.top as f32 / scale + (d.height as f32 / scale - size.y) / 2.0,
            ]);
        }
        ctx.show_viewport_immediate(viewport_id(), builder, |ctx, class| {
            if focus {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
            }
            if ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape)) {
                self.settings_open = false;
                self.settings_save = true;
            }
            if class == egui::ViewportClass::Embedded {
                egui::Window::new("Cikung Settings").show(ctx, |ui| self.settings_content(ui));
            } else {
                egui::SidePanel::left("settings-sidebar")
                    .resizable(false)
                    .exact_width(228.0)
                    .frame(
                        egui::Frame::new()
                            .fill(Color32::BLACK)
                            .inner_margin(egui::Margin::symmetric(12, 18)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            self.draw_cat(ui, 28.0);
                            ui.label(RichText::new("Cikung").size(20.0).strong());
                        });
                        ui.add_space(20.0);
                        egui::ScrollArea::vertical()
                            .id_salt("hub-navigation")
                            .max_height((ui.available_height() - 50.0).max(80.0))
                            .show(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 3.0;
                                for (label, pages) in [
                                    ("FLOW", SettingsPage::FLOW.as_slice()),
                                    ("NOTCH", SettingsPage::NOTCH.as_slice()),
                                ] {
                                    ui.add_space(12.0);
                                    ui.label(
                                        RichText::new(label)
                                            .size(10.0)
                                            .strong()
                                            .color(Color32::from_gray(108)),
                                    );
                                    ui.add_space(5.0);
                                    for &page in pages {
                                        if sidebar_row(ui, page, self.settings_tab == page)
                                            .clicked()
                                        {
                                            self.settings_tab = page;
                                        }
                                    }
                                }
                            });
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                            ui.label(
                                RichText::new(format!("Cikung {}", env!("CARGO_PKG_VERSION")))
                                    .size(10.0)
                                    .color(GRAY),
                            );
                            ui.label(
                                RichText::new(if self.demo {
                                    "Preview"
                                } else {
                                    "Changes apply immediately"
                                })
                                .size(10.0)
                                .color(GRAY),
                            );
                        });
                    });
                egui::TopBottomPanel::bottom("settings-footer")
                    .frame(
                        egui::Frame::new()
                            .fill(Color32::BLACK)
                            .inner_margin(egui::Margin::symmetric(20, 8)),
                    )
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            let notice = self.notices.back();
                            ui.add(
                                egui::Label::new(
                                    RichText::new(
                                        notice
                                            .map(|n| format!("{} · {}", n.title, n.text))
                                            .unwrap_or_else(|| {
                                                "One little companion, all your settings.".into()
                                            }),
                                    )
                                    .size(11.0)
                                    .color(if notice.is_some() { ORANGE } else { GRAY }),
                                )
                                .truncate(),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("Done").clicked() {
                                        self.settings_open = false;
                                        self.settings_save = true;
                                    }
                                },
                            );
                        });
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::new().fill(Color32::BLACK).inner_margin(12))
                    .show(ctx, |ui| {
                        egui::Frame::new()
                            .fill(Color32::from_rgb(14, 15, 17))
                            .stroke(Stroke::new(1.0_f32, Color32::from_gray(34)))
                            .corner_radius(22)
                            .inner_margin(24)
                            .show(ui, |ui| {
                                ui.set_min_size(ui.available_size());
                                ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
                                ui.spacing_mut().button_padding = Vec2::new(12.0, 8.0);
                                ui.style_mut().visuals.widgets.inactive.bg_fill =
                                    Color32::from_rgb(31, 32, 36);
                                ui.style_mut().visuals.widgets.inactive.weak_bg_fill =
                                    Color32::from_rgb(31, 32, 36);
                                self.settings_content(ui);
                            });
                    });
            }
        });
        if self.capture_viewport == viewport_id()
            && self.started.elapsed() > Duration::from_secs(2)
            && let Some(path) = &self.capture
        {
            if let Err(error) = native::capture_own_window("Cikung Settings", path) {
                eprintln!("{error}");
            }
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
        }
    }

    fn settings_content(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical()
            .id_salt(("hub-content", self.settings_tab))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.settings_hero(ui);
                self.hub_content(ui);
            });
    }

    pub(super) fn capture_window(&mut self, ctx: &egui::Context, crop: Option<Rect>) {
        let Some(path) = &self.capture else {
            return;
        };
        if ctx.viewport_id() != self.capture_viewport {
            return;
        }
        let screenshot = ctx.input(|i| {
            i.events.iter().find_map(|e| {
                if let egui::Event::Screenshot {
                    viewport_id, image, ..
                } = e
                    && *viewport_id == self.capture_viewport
                {
                    Some(image.clone())
                } else {
                    None
                }
            })
        });
        if let Some(shot) = screenshot {
            let scale = ctx.pixels_per_point();
            let crop = crop.unwrap_or_else(|| {
                Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(shot.width() as f32 / scale, shot.height() as f32 / scale),
                )
            });
            let left = ((crop.left() * scale).round().max(0.0) as usize).min(shot.width());
            let top = ((crop.top() * scale).round().max(0.0) as usize).min(shot.height());
            let right =
                ((crop.right() * scale).round().max(left as f32) as usize).min(shot.width());
            let bottom =
                ((crop.bottom() * scale).round().max(top as f32) as usize).min(shot.height());
            let mut bytes = Vec::with_capacity((right - left) * (bottom - top) * 4);
            for y in top..bottom {
                bytes.extend(
                    shot.pixels[y * shot.width() + left..y * shot.width() + right]
                        .iter()
                        .flat_map(|p| p.to_array()),
                );
            }
            image::save_buffer(
                path,
                &bytes,
                (right - left) as u32,
                (bottom - top) as u32,
                image::ColorType::Rgba8,
            )
            .expect("save window capture");
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
        } else if !self.capture_requested && self.started.elapsed() > Duration::from_secs(2) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            self.capture_requested = true;
        }
    }
}
