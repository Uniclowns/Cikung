use super::*;

impl Cikung {
    pub(crate) fn music(&mut self, ui: &mut egui::Ui) {
        ui.spacing_mut().item_spacing.y = 4.0;
        let Some(session) = self.session.clone() else {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                self.draw_cat(ui, 66.0);
                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new("I'm all ears.").size(19.0));
                    ui.label(
                        RichText::new("Start a song, video, or podcast on your PC.")
                            .size(12.0)
                            .color(GRAY),
                    );
                    ui.label(
                        RichText::new("Spotify · YouTube · VLC")
                            .size(10.0)
                            .color(GRAY),
                    );
                });
            });
            ui.add_space(18.0);
            ui.label(
                RichText::new(
                    self.snapshot
                        .error
                        .as_deref()
                        .unwrap_or(if self.snapshot.ready {
                            "Waiting for your media player…"
                        } else {
                            "Connecting to your media players…"
                        }),
                )
                .size(11.0)
                .color(GRAY),
            );
            ui.add_space(8.0);
            ui.label(RichText::new("Your player needs to share playback with the system. VLC setup is in Settings.").size(10.0).color(GRAY));
            return;
        };
        let artwork_key = format!("{}:{}:{}", session.id, session.title, session.album);
        if self
            .artwork
            .as_ref()
            .is_none_or(|(key, _)| key != &artwork_key)
        {
            self.artwork = None;
            if let Some(bytes) = &session.artwork
                && let Ok(image) = image::load_from_memory(bytes)
            {
                let rgba = image.thumbnail(256, 256).into_rgba8();
                self.artwork = Some((
                    artwork_key,
                    ui.ctx().load_texture(
                        "Album cover",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [rgba.width() as usize, rgba.height() as usize],
                            rgba.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    ),
                ));
            }
        }
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(274.0, 166.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.horizontal(|ui| {
                        if let Some((_, cover)) = &self.artwork {
                            ui.add(
                                egui::Image::new((cover.id(), Vec2::splat(56.0))).corner_radius(10),
                            );
                        } else {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(56.0), Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 10, Color32::from_rgb(42, 31, 24));
                            for (index, height) in
                                [13.0, 25.0, 19.0, 31.0, 15.0].into_iter().enumerate()
                            {
                                ui.painter().rect_filled(
                                    Rect::from_center_size(
                                        rect.center() + Vec2::new((index as f32 - 2.0) * 7.0, 0.0),
                                        Vec2::new(3.0, height),
                                    ),
                                    2,
                                    ORANGE,
                                );
                            }
                        }
                        ui.add_space(5.0);
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width());
                            ui.add(
                                egui::Label::new(RichText::new(&session.title).size(15.0).strong())
                                    .truncate(),
                            )
                            .on_hover_text(&session.title);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(if session.artist.is_empty() {
                                        &session.source
                                    } else {
                                        &session.artist
                                    })
                                    .size(12.0)
                                    .color(GRAY),
                                )
                                .truncate(),
                            );
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(match session.state {
                                        PlaybackState::Playing => "Playing",
                                        PlaybackState::Paused => "Paused",
                                        PlaybackState::Stopped => "Stopped",
                                    })
                                    .size(10.0)
                                    .color(ORANGE),
                                );
                                ui.label(RichText::new(&session.source).size(10.0).color(GRAY));
                            });
                        });
                    });
                    ui.add_space(8.0);
                    let position = self.position();
                    if session.can_seek && session.duration_secs > 0.0 && !self.demo {
                        let seek_id =
                            ui.make_persistent_id(("music-seek", &session.id, &session.title));
                        let mut seek = ui
                            .data_mut(|data| data.get_temp::<f64>(seek_id))
                            .unwrap_or(position);
                        let response = ui
                            .scope(|ui| {
                                ui.spacing_mut().slider_width = ui.available_width();
                                ui.add(
                                    egui::Slider::new(&mut seek, 0.0..=session.duration_secs)
                                        .show_value(false),
                                )
                            })
                            .inner;
                        if response.dragged() {
                            ui.data_mut(|data| data.insert_temp(seek_id, seek));
                        } else {
                            ui.data_mut(|data| data.remove::<f64>(seek_id));
                            if response.drag_stopped() || response.changed() {
                                self.control(MediaCommand::Seek(seek));
                            }
                        }
                        response.on_hover_text("Drag to seek");
                    } else {
                        ui.add(
                            egui::ProgressBar::new(if session.duration_secs > 0.0 {
                                (position / session.duration_secs).clamp(0.0, 1.0) as f32
                            } else {
                                0.0
                            })
                            .fill(WHITE)
                            .desired_height(3.0),
                        );
                    }
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(clock(position)).size(10.0).color(GRAY));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                RichText::new(if session.duration_secs > 0.0 {
                                    format!("−{}", clock(session.duration_secs - position))
                                } else {
                                    "–:––".into()
                                })
                                .size(10.0)
                                .color(GRAY),
                            );
                        });
                    });
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        let width = ui.available_width();
                        ui.add_space(((width - 144.0) / 2.0).max(0.0));
                        if transport(ui, Transport::Previous, !self.demo, false).clicked() {
                            self.control(MediaCommand::Previous);
                        }
                        if transport(
                            ui,
                            if session.state == PlaybackState::Playing {
                                Transport::Pause
                            } else {
                                Transport::Play
                            },
                            !self.demo,
                            true,
                        )
                        .clicked()
                        {
                            self.control(MediaCommand::TogglePlayPause);
                        }
                        if transport(ui, Transport::Next, !self.demo, false).clicked() {
                            self.control(MediaCommand::Next);
                        }
                    });
                },
            );
            ui.add_space(12.0);
            let (line, _) = ui.allocate_exact_size(Vec2::new(1.0, 166.0), Sense::hover());
            ui.painter().vline(
                line.center().x,
                line.y_range(),
                Stroke::new(1.0_f32, Color32::from_gray(30)),
            );
            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), 166.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    self.music_lyrics(ui, &session);
                },
            );
        });
        ui.add_space(6.0);
        let options = ui.collapsing(
            RichText::new("Playback options").size(10.0).color(GRAY),
            |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("players")
                        .selected_text(format!(
                            "{} · {}",
                            if self.selected.is_some() {
                                "Selected"
                            } else {
                                "Auto"
                            },
                            session.source
                        ))
                        .width(215.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.selected,
                                None,
                                "Auto · follow playing media",
                            );
                            for s in &self.snapshot.sessions {
                                ui.selectable_value(
                                    &mut self.selected,
                                    Some(s.id.clone()),
                                    format!("{} · {}", s.source, s.title),
                                );
                            }
                        });
                    if let Some(shuffle) = session.shuffle
                        && ui
                            .add_enabled(
                                !self.demo,
                                egui::Button::new(if shuffle { "Shuffle on" } else { "Shuffle" }),
                            )
                            .clicked()
                    {
                        self.control(MediaCommand::Shuffle(!shuffle));
                    }
                    if let Some(repeat) = session.repeat
                        && ui
                            .add_enabled(
                                !self.demo,
                                egui::Button::new(if repeat { "Repeat on" } else { "Repeat" }),
                            )
                            .clicked()
                    {
                        self.control(MediaCommand::Repeat(!repeat));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(RichText::new("System volume").size(11.0).color(GRAY));
                    ui.add_enabled_ui(cfg!(target_os = "windows"), |ui| {
                        if ui.small_button("−").clicked() {
                            native::volume(9);
                        }
                        if ui.small_button("+").clicked() {
                            native::volume(10);
                        }
                        if ui.small_button("Mute").clicked() {
                            native::volume(8);
                        }
                    });
                });
            },
        );
        self.music_options_open = options.body_response.is_some();
    }
    fn music_lyrics(&mut self, ui: &mut egui::Ui, session: &MediaSession) {
        let mut seek_to = None;
        ui.label(RichText::new("LYRICS").size(9.0).color(GRAY));
        ui.add_space(12.0);
        egui::ScrollArea::vertical()
            .id_salt("music-lyrics-preview")
            .max_height(108.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if self.lyrics_for == Self::track_identity(Some(session))
                    && !self.tools.lyrics.lines.is_empty()
                {
                    let active = self.tools.lyrics.active_index(self.position()).unwrap_or(0);
                    let lines = &self.tools.lyrics.lines;
                    for (index, line) in lines
                        .iter()
                        .enumerate()
                        .take(active + 2)
                        .skip(active.saturating_sub(1))
                    {
                        let target = self.lyric_seek_target(line.time_secs);
                        let text = RichText::new(&line.text)
                            .size(if index == active { 16.0 } else { 12.0 })
                            .color(if index == active {
                                WHITE
                            } else if index < active {
                                Color32::from_gray(90)
                            } else {
                                GRAY
                            });
                        let label =
                            egui::Label::new(if index == active { text.strong() } else { text })
                                .sense(if target.is_some() {
                                    Sense::click()
                                } else {
                                    Sense::hover()
                                });
                        let response = ui.add(if index == active {
                            label.wrap()
                        } else {
                            label.truncate()
                        });
                        if target.is_some()
                            && response
                                .on_hover_text("Click to jump to this lyric")
                                .clicked()
                        {
                            seek_to = target;
                        }
                    }
                } else {
                    ui.label(RichText::new(self.lyrics_status()).size(15.0).strong());
                    ui.label(
                        RichText::new(if self.prefs.auto_lyrics {
                            "Lyrics appear automatically when a match is available."
                        } else {
                            "Find lyrics or add an LRC file to follow along."
                        })
                        .size(11.0)
                        .color(GRAY),
                    );
                }
            });
        if let Some(seconds) = seek_to {
            self.control(MediaCommand::Seek(seconds));
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let missing = self.lyrics_for != Self::track_identity(Some(session))
                || self.tools.lyrics.lines.is_empty();
            if missing
                && (!self.prefs.auto_lyrics || self.auto_lyrics.error.is_some())
                && ui
                    .add_enabled(
                        !self.demo && !self.lyrics_busy,
                        egui::Button::new(if self.prefs.auto_lyrics {
                            "Retry"
                        } else {
                            "Find lyrics"
                        })
                        .small(),
                    )
                    .clicked()
            {
                let result = self.request_lyrics(session.clone());
                if let Err(error) = result {
                    self.toast("Lyrics", error);
                }
            }
            if ui.small_button("Open lyrics").clicked() {
                self.navigate(Page::Lyrics);
            }
        });
    }

    pub(crate) fn lyric_seek_target(&self, seconds: f64) -> Option<f64> {
        let session = self.session.as_ref()?;
        (!self.demo
            && self.capture.is_none()
            && self.audit.is_none()
            && self.tools.lyrics.synced
            && session.can_seek
            && self.lyrics_for == Self::track_identity(Some(session))
            && seconds.is_finite()
            && seconds >= 0.0
            && session.duration_secs.is_finite()
            && session.duration_secs > 0.0
            && seconds <= session.duration_secs)
            .then_some(seconds)
    }
}

enum Transport {
    Previous,
    Play,
    Pause,
    Next,
}
fn transport(ui: &mut egui::Ui, icon: Transport, enabled: bool, primary: bool) -> egui::Response {
    let label = match icon {
        Transport::Previous => "Previous",
        Transport::Play => "Play",
        Transport::Pause => "Pause",
        Transport::Next => "Next",
    };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(" ")
            .fill(if primary {
                WHITE
            } else {
                Color32::from_gray(18)
            })
            .corner_radius(24)
            .min_size(Vec2::splat(if primary { 44.0 } else { 38.0 })),
    );
    let color = if primary { Color32::BLACK } else { WHITE }.gamma_multiply(if enabled {
        1.0
    } else {
        0.35
    });
    let c = response.rect.center();
    let painter = ui.painter();
    match icon {
        Transport::Pause => {
            for x in [-3.0, 3.0] {
                painter.rect_filled(
                    Rect::from_center_size(c + Vec2::new(x, 0.0), Vec2::new(3.0, 13.0)),
                    1,
                    color,
                );
            }
        }
        Transport::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-4.0, -7.0),
                    c + Vec2::new(7.0, 0.0),
                    c + Vec2::new(-4.0, 7.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        Transport::Previous | Transport::Next => {
            let sign = if matches!(icon, Transport::Next) {
                1.0
            } else {
                -1.0
            };
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-5.0 * sign, -6.0),
                    c + Vec2::new(4.0 * sign, 0.0),
                    c + Vec2::new(-5.0 * sign, 6.0),
                ],
                color,
                Stroke::NONE,
            ));
            painter.rect_filled(
                Rect::from_center_size(c + Vec2::new(6.0 * sign, 0.0), Vec2::new(2.0, 12.0)),
                1,
                color,
            );
        }
    }
    response.on_hover_text(label)
}
