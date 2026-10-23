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
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(274.0, 166.0),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                    ui.horizontal(|ui| {
                        if let Some((_, cover)) = &self.artwork {
                            ui.add(
                                egui::Image::new((cover.id(), Vec2::splat(64.0))).corner_radius(12),
                            );
                        } else {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(64.0), Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 12, Color32::from_rgb(42, 31, 24));
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
                        ui.add_space(8.0);
                        ui.vertical(|ui| {
                            ui.set_width(ui.available_width());
                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&session.title).size(16.0).color(WHITE),
                                )
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
                    ui.add_space(12.0);
                    let position = self.position();
                    // Elapsed · bar · remaining on one line, like Nusa.
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        ui.label(RichText::new(clock(position)).size(11.0).color(WHITE));
                        let bar = (ui.available_width() - 48.0).max(40.0);
                        if session.can_seek && session.duration_secs > 0.0 && !self.demo {
                            let seek_id =
                                ui.make_persistent_id(("music-seek", &session.id, &session.title));
                            let mut seek = ui
                                .data_mut(|data| data.get_temp::<f64>(seek_id))
                                .unwrap_or(position);
                            let response = ui
                                .scope(|ui| {
                                    ui.spacing_mut().slider_width = bar;
                                    ui.spacing_mut().slider_rail_height = 3.0;
                                    ui.visuals_mut().selection.bg_fill = WHITE;
                                    ui.add(
                                        egui::Slider::new(&mut seek, 0.0..=session.duration_secs)
                                            .show_value(false)
                                            .trailing_fill(true)
                                            .handle_shape(egui::style::HandleShape::Circle),
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
                                .desired_width(bar)
                                .desired_height(3.0),
                            );
                        }
                        ui.label(
                            RichText::new(if session.duration_secs > 0.0 {
                                format!("−{}", clock(session.duration_secs - position))
                            } else {
                                "–:––".into()
                            })
                            .size(11.0)
                            .color(GRAY),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        let width = ui.available_width();
                        ui.add_space(((width - 174.0) / 2.0).max(0.0));
                        if transport(ui, Transport::Lyrics, true, false).clicked() {
                            self.navigate(Page::Lyrics);
                        }
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
    }
    /// Player choice, shuffle/repeat and system volume; lives in Settings → Music.
    pub(crate) fn playback_options(&mut self, ui: &mut egui::Ui) {
        let source = self
            .session
            .as_ref()
            .map_or_else(|| "Nothing playing".to_owned(), |s| s.source.clone());
        ui.horizontal(|ui| {
            ui.label(RichText::new("Player").size(11.0).color(GRAY));
            egui::ComboBox::from_id_salt("players")
                .selected_text(format!(
                    "{} · {}",
                    if self.selected.is_some() {
                        "Selected"
                    } else {
                        "Auto"
                    },
                    source
                ))
                .width(240.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected, None, "Auto · follow playing media");
                    for s in &self.snapshot.sessions {
                        ui.selectable_value(
                            &mut self.selected,
                            Some(s.id.clone()),
                            format!("{} · {}", s.source, s.title),
                        );
                    }
                });
        });
        let (shuffle, repeat) = self
            .session
            .as_ref()
            .map_or((None, None), |s| (s.shuffle, s.repeat));
        ui.horizontal(|ui| {
            if let Some(shuffle) = shuffle
                && ui
                    .add_enabled(
                        !self.demo,
                        egui::Button::new(if shuffle { "Shuffle on" } else { "Shuffle" }),
                    )
                    .clicked()
            {
                self.control(MediaCommand::Shuffle(!shuffle));
            }
            if let Some(repeat) = repeat
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
            ui.add_enabled_ui(
                cfg!(any(target_os = "windows", target_os = "macos")),
                |ui| {
                    if ui.small_button("−").clicked() {
                        native::volume(9);
                    }
                    if ui.small_button("+").clicked() {
                        native::volume(10);
                    }
                    if ui.small_button("Mute").clicked() {
                        native::volume(8);
                    }
                },
            );
        });
    }
    fn music_lyrics(&mut self, ui: &mut egui::Ui, session: &MediaSession) {
        if self.lyrics_for == Self::track_identity(Some(session))
            && !self.tools.lyrics.lines.is_empty()
        {
            if let Some(seconds) = self.lyric_flow(ui, 166.0) {
                self.control(MediaCommand::Seek(seconds));
            }
            return;
        }
        ui.add_space(26.0);
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
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if (!self.prefs.auto_lyrics || self.auto_lyrics.error.is_some())
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

    /// Nusa-style follow-along: equal-size lines, the current one lit, easing upward on change.
    fn lyric_flow(&self, ui: &mut egui::Ui, height: f32) -> Option<f64> {
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());
        let lines = &self.tools.lyrics.lines;
        let active = self
            .tools
            .lyrics
            .active_index(self.position())
            .unwrap_or(0)
            .min(lines.len() - 1);
        let now = ui.input(|i| i.time);
        let id = ui.id().with("lyric-flow");
        let (from, changed) = ui.data_mut(|data| {
            let state = data.get_temp_mut_or_insert_with(id, || (active, active, f64::MIN));
            if state.1 != active {
                *state = (state.1, active, now);
            }
            (state.0.min(lines.len() - 1), state.2)
        });
        // Seeks and new songs jump; consecutive lines glide.
        let progress = if self.prefs.reduced_motion || from.abs_diff(active) > 2 {
            1.0
        } else {
            egui::emath::easing::cubic_out(((now - changed) / 0.42).clamp(0.0, 1.0) as f32)
        };
        if progress < 1.0 {
            ui.ctx().request_repaint_after(Duration::from_millis(8));
        }
        let first = active.min(from).saturating_sub(1);
        let last = (active.max(from) + 4).min(lines.len());
        let painter = ui.painter_at(rect);
        let mut tops = Vec::with_capacity(last - first);
        let mut y = 0.0;
        let galleys: Vec<_> = (first..last)
            .map(|index| {
                let galley = painter.layout(
                    lines[index].text.clone(),
                    egui::FontId::proportional(17.0),
                    WHITE,
                    rect.width(),
                );
                tops.push(y);
                y += galley.size().y + 9.0;
                galley
            })
            .collect();
        let scroll = egui::lerp(tops[from - first]..=tops[active - first], progress);
        // The current line sits one row down so the previous line still peeks in above it.
        let anchor = rect.top() + 30.0;
        let mut seek = None;
        for (offset, galley) in galleys.into_iter().enumerate() {
            let index = first + offset;
            let pos = Pos2::new(rect.left(), anchor + tops[offset] - scroll);
            let row = Rect::from_min_size(pos, galley.size()).intersect(rect);
            if row.height() <= 0.0 {
                continue;
            }
            let lit = if index == active {
                progress
            } else if index == from {
                1.0 - progress
            } else {
                0.0
            };
            let base = if index < active {
                Color32::from_gray(78)
            } else if index == active + 1 {
                Color32::from_gray(128)
            } else {
                Color32::from_gray(84)
            };
            painter.galley_with_override_text_color(pos, galley, base.lerp_to_gamma(WHITE, lit));
            if let Some(target) = self.lyric_seek_target(lines[index].time_secs) {
                let response = ui.interact(row, id.with(index), Sense::click());
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if response
                    .on_hover_text("Click to jump to this lyric")
                    .clicked()
                {
                    seek = Some(target);
                }
            }
        }
        seek
    }

    /// Current synced lyric for the collapsed notch ticker.
    pub(crate) fn current_lyric(&self) -> Option<&str> {
        let session = self.session.as_ref()?;
        if self.lyrics_for != Self::track_identity(Some(session)) {
            return None;
        }
        let index = self.tools.lyrics.active_index(self.position())?;
        let text = self.tools.lyrics.lines.get(index)?.text.trim();
        (!text.is_empty()).then_some(text)
    }

    /// Decode the current cover once per track for the Music page and the notch tint.
    pub(crate) fn sync_artwork(&mut self, ctx: &egui::Context) {
        let Some(session) = &self.session else {
            self.artwork = None;
            self.tint = Color32::BLACK;
            return;
        };
        let attempt = (
            format!("{}:{}:{}", session.id, session.title, session.album),
            session.artwork.as_ref().map_or(0, |bytes| bytes.len()),
        );
        let id = egui::Id::new("artwork-attempt");
        // ponytail: remembers the last attempt so a broken cover is not decoded every frame.
        if ctx
            .data(|data| data.get_temp::<(String, usize)>(id))
            .as_ref()
            == Some(&attempt)
        {
            return;
        }
        ctx.data_mut(|data| data.insert_temp(id, attempt.clone()));
        self.artwork = None;
        self.tint = Color32::BLACK;
        if let Some(bytes) = &session.artwork
            && let Ok(image) = image::load_from_memory(bytes)
        {
            let rgba = image.thumbnail(256, 256).into_rgba8();
            self.tint = album_tint(&image::imageops::thumbnail(&rgba, 24, 24));
            self.artwork = Some((
                attempt.0,
                ctx.load_texture(
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

/// Dark, colourful average of a cover for the notch gradient; grey covers stay neutral.
pub(crate) fn album_tint(image: &image::RgbaImage) -> Color32 {
    let mut sum = [0.0_f32; 3];
    let mut total = 0.0;
    for pixel in image.pixels() {
        let [r, g, b, a] = pixel.0.map(f32::from);
        let (max, min) = (r.max(g).max(b), r.min(g).min(b));
        // Prefer saturated mid-tones over black, white and grey pixels.
        let weight = a / 255.0 * ((max - min) / 255.0 + 0.05) * (1.0 - (max / 255.0 - 0.6).abs());
        for (channel, value) in sum.iter_mut().zip([r, g, b]) {
            *channel += value * weight;
        }
        total += weight;
    }
    let mean = sum.map(|channel| channel / total.max(f32::EPSILON));
    let peak = mean[0].max(mean[1]).max(mean[2]);
    if peak < 1.0 {
        return Color32::BLACK;
    }
    let saturation = (peak - mean[0].min(mean[1]).min(mean[2])) / peak;
    // Nusa's tint peaks near 115/255, dark enough to keep white text readable.
    let [r, g, b] = mean.map(|channel| (channel / peak * (72.0 + 46.0 * saturation)).round() as u8);
    Color32::from_rgb(r, g, b)
}

enum Transport {
    Lyrics,
    Previous,
    Play,
    Pause,
    Next,
}
/// Flat white icons with a soft hover disc, matching Nusa's lighter transport row.
fn transport(ui: &mut egui::Ui, icon: Transport, enabled: bool, primary: bool) -> egui::Response {
    let label = match icon {
        Transport::Lyrics => "Lyrics",
        Transport::Previous => "Previous",
        Transport::Play => "Play",
        Transport::Pause => "Pause",
        Transport::Next => "Next",
    };
    let size = if primary { 42.0 } else { 36.0 };
    let response = ui.add_enabled(
        enabled,
        egui::Button::new(" ")
            .frame(false)
            .min_size(Vec2::splat(size)),
    );
    let color = WHITE.gamma_multiply(if enabled { 1.0 } else { 0.35 });
    let c = response.rect.center();
    let painter = ui.painter();
    if response.hovered() {
        let alpha = if response.is_pointer_button_down_on() {
            40
        } else {
            22
        };
        painter.circle_filled(c, size / 2.0, Color32::from_white_alpha(alpha));
    }
    let s = if primary { 1.45 } else { 1.0 };
    match icon {
        Transport::Lyrics => {
            let bubble = Rect::from_center_size(c - Vec2::new(0.0, 1.0), Vec2::new(16.0, 12.0));
            painter.rect_filled(bubble, 4, color);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    bubble.left_bottom() + Vec2::new(3.0, -1.0),
                    bubble.left_bottom() + Vec2::new(7.0, -1.0),
                    bubble.left_bottom() + Vec2::new(2.0, 4.0),
                ],
                color,
                Stroke::NONE,
            ));
            for x in [-2.8, 2.8] {
                painter.circle_filled(bubble.center() + Vec2::new(x, 0.0), 1.7, Color32::BLACK);
            }
        }
        Transport::Pause => {
            for x in [-3.5, 3.5] {
                painter.rect_filled(
                    Rect::from_center_size(c + Vec2::new(x * s, 0.0), Vec2::new(3.0 * s, 13.0 * s)),
                    1,
                    color,
                );
            }
        }
        Transport::Play => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    c + Vec2::new(-4.0, -7.0) * s,
                    c + Vec2::new(7.0, 0.0) * s,
                    c + Vec2::new(-4.0, 7.0) * s,
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

#[cfg(test)]
mod tint_tests {
    use super::*;
    #[test]
    fn cover_tint_follows_colour_stays_dark_and_ignores_grey_and_black() {
        let crimson = image::RgbaImage::from_fn(24, 24, |x, _| {
            if x < 18 {
                image::Rgba([196, 18, 72, 255])
            } else {
                image::Rgba([10, 10, 10, 255])
            }
        });
        let tint = album_tint(&crimson);
        assert!(tint.r() > tint.b() && tint.b() > tint.g());
        assert!((100..=120).contains(&tint.r()));
        let grey = album_tint(&image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([128, 128, 128, 255]),
        ));
        assert!(grey.r() == grey.g() && grey.g() == grey.b() && grey.r() < 80);
        let black = album_tint(&image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([0, 0, 0, 255]),
        ));
        assert_eq!(black, Color32::BLACK);
    }
}
