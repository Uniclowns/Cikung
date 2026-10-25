use super::*;
use crate::cloud::{self, ChatProvider};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(super) enum SettingsPage {
    Dictation,
    Notes,
    Focus,
    Tasks,
    Water,
    Game,
    #[default]
    General,
    Cats,
    Music,
    Agents,
    Chat,
    Integrations,
    Phone,
    Shortcuts,
    Notifications,
}

impl SettingsPage {
    pub const FLOW: [Self; 6] = [
        Self::Dictation,
        Self::Notes,
        Self::Focus,
        Self::Tasks,
        Self::Water,
        Self::Game,
    ];
    pub const NOTCH: [Self; 9] = [
        Self::General,
        Self::Notifications,
        Self::Cats,
        Self::Music,
        Self::Agents,
        Self::Chat,
        Self::Integrations,
        Self::Phone,
        Self::Shortcuts,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Dictation => "Dictation",
            Self::Notes => "Notes & recording",
            Self::Focus => "Focus",
            Self::Tasks => "Tasks",
            Self::Water => "Water",
            Self::Game => "Game assistant",
            Self::General => "General",
            Self::Cats => "Cats & appearance",
            Self::Music => "Music & lyrics",
            Self::Agents => "Agents",
            Self::Chat => "Chat",
            Self::Integrations => "Integrations",
            Self::Phone => "Phone connection",
            Self::Shortcuts => "Shortcuts",
            Self::Notifications => "Notifications",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Dictation => "dictation",
            Self::Notes => "notes",
            Self::Focus => "focus",
            Self::Tasks => "tasks",
            Self::Water => "water",
            Self::Game => "game",
            Self::General => "general",
            Self::Cats => "cats",
            Self::Music => "music",
            Self::Agents => "agents",
            Self::Chat => "chat",
            Self::Integrations => "integrations",
            Self::Phone => "phone",
            Self::Shortcuts => "shortcuts",
            Self::Notifications => "notifications",
        }
    }

    pub fn from_request(request: &str) -> Option<Self> {
        if request.eq_ignore_ascii_case("settings") {
            return Some(Self::General);
        }
        if request.eq_ignore_ascii_case("cats") || request.eq_ignore_ascii_case("style") {
            return Some(Self::Cats);
        }
        let slug = request.strip_prefix("settings/")?;
        Self::FLOW
            .into_iter()
            .chain(Self::NOTCH)
            .find(|p| p.slug() == slug)
    }
}

pub(super) fn card(ui: &mut egui::Ui, content: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(Color32::from_rgb(24, 25, 28))
        .stroke(Stroke::new(1.0_f32, Color32::from_gray(40)))
        .corner_radius(14)
        .inner_margin(20)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            content(ui);
        });
    ui.add_space(16.0);
}

impl Cikung {
    pub(crate) fn settings(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            heading(
                ui,
                "Appearance",
                "The notch's colour. Album colour follows the playing song's cover.",
            );
            ui.horizontal_wrapped(|ui| {
                for theme in Theme::ALL {
                    ui.radio_value(&mut self.prefs.theme, theme, theme.label());
                }
            });
        });
        card(ui, |ui| {
            heading(
                ui,
                "Features in your notch",
                "Turn off what you don't use: it leaves the page dots and the all-tools grid.",
            );
            for (page, caption) in super::notch_ui::TOOLS {
                let mut shown = !self.prefs.hidden_pages.contains(&page);
                if ui
                    .checkbox(&mut shown, format!("{} · {caption}", page.label()))
                    .changed()
                {
                    if shown {
                        self.prefs.hidden_pages.retain(|hidden| *hidden != page);
                    } else {
                        self.prefs.hidden_pages.push(page);
                    }
                }
            }
        });
        card(ui, |ui| {
            heading(
                ui,
                "At home in your notch",
                "A small companion at the top of your screen.",
            );
            ui.checkbox(&mut self.prefs.hover_open, "Open when I hover the notch");
            ui.checkbox(
                &mut self.prefs.auto_collapse,
                "Close when I leave the notch",
            );
            ui.checkbox(
                &mut self.prefs.always_on_top,
                "Keep the notch above other windows",
            );
            ui.add_enabled_ui(cfg!(target_os = "windows"), |ui| {
                ui.checkbox(&mut self.prefs.tuck_in_fullscreen, "Tuck away during fullscreen apps")
                    .on_hover_text("Off: the notch stays in front of fullscreen apps such as a YouTube video. On: it hides the collapsed notch during borderless fullscreen on its screen; Ctrl+Alt+C still opens it.");
            });
            ui.checkbox(
                &mut self.prefs.minimal_notch,
                "Minimal notch: just the cat and the time while nothing plays",
            );
            ui.horizontal(|ui| {
                ui.label("Cikung visits");
                let minutes = self.prefs.cat_visit_minutes;
                egui::ComboBox::from_id_salt("cat-visits")
                    .selected_text(
                        super::visits::VISIT_MINUTES
                            .iter()
                            .find(|(m, _)| *m == minutes)
                            .map_or_else(|| format!("Every {minutes} minutes"), |(_, n)| (*n).into()),
                    )
                    .show_ui(ui, |ui| {
                        for (m, name) in super::visits::VISIT_MINUTES {
                            ui.selectable_value(&mut self.prefs.cat_visit_minutes, m, name);
                        }
                    });
            })
            .response
            .on_hover_text("Every so often Cikung hops, wiggles and says hello in the notch. It waits while the panel is open, another message shows, you focus or record, or a fullscreen app is in front.");
            if ui
                .checkbox(&mut self.autostart, "Start Cikung when I sign in")
                .changed()
            {
                let result = crate::shell::set_autostart(self.autostart);
                if result.is_err() {
                    self.autostart = crate::shell::autostart_enabled();
                }
                self.report(result, "Startup preference updated.");
            }
        });
        card(ui, |ui| {
            heading(
                ui,
                "Display",
                "The notch stays centered at the top of this screen.",
            );
            let displays = native::displays();
            egui::ComboBox::from_id_salt("display")
                .selected_text(
                    displays
                        .iter()
                        .find(|d| d.id == self.prefs.display)
                        .or(displays.first())
                        .map(|d| d.name.as_str())
                        .unwrap_or("Primary display"),
                )
                .show_ui(ui, |ui| {
                    for display in &displays {
                        ui.selectable_value(
                            &mut self.prefs.display,
                            display.id.clone(),
                            &display.name,
                        );
                    }
                });
        });
        card(ui, |ui| {
            heading(
                ui,
                "Bluetooth notifications",
                "A small heads-up when your devices connect or disconnect.",
            );
            ui.add_enabled_ui(cfg!(target_os = "windows"), |ui| {
                ui.checkbox(
                    &mut self.prefs.bluetooth_notifications,
                    "Notify me about Bluetooth connections",
                );
            });
            if let Some(monitor) = &self.bluetooth {
                if let Some(error) = &monitor.error {
                    ui.label(RichText::new(error).color(accent()));
                } else if !monitor.ready() {
                    ui.label(RichText::new("Checking connected devices…").color(muted()));
                } else if monitor.devices().next().is_none() {
                    ui.label(
                        RichText::new("No connected Bluetooth devices detected.").color(muted()),
                    );
                } else {
                    for device in monitor.devices() {
                        ui.label(format!("● {}", device.name));
                    }
                }
            }
            ui.label(RichText::new("Windows Bluetooth and Bluetooth LE · checks every 3 seconds. Existing connections are quiet at startup.").size(11.0).color(muted()));
        });
        card(ui, |ui| {
            heading(
                ui,
                "Battery, internet, Caps Lock and volume",
                "A heads-up in the notch when your charger, battery, connection, Caps Lock or volume changes.",
            );
            ui.add_enabled_ui(cfg!(target_os = "windows"), |ui| {
                ui.checkbox(
                    &mut self.prefs.system_notices,
                    "Show battery, internet, Caps Lock and volume notices",
                );
            });
            ui.label(RichText::new("Charger connected or unplugged, low battery at 20 %, critical at 10 %, fully charged; internet connected, no internet or disconnected; Caps Lock on or off; the volume level with an animated bar when it changes. Windows only. The state at startup is quiet.").size(11.0).color(muted()));
        });
        card(ui, |ui| {
            heading(
                ui,
                "Your data & connections",
                "You choose when to record, connect, or send a message.",
            );
            ui.label(RichText::new("Recording and dictation start from their buttons. Keys are kept in your operating system credential store. Phone access starts off; coding hooks are opt-in.").color(muted()));
            ui.add_space(12.0);
            if ui.button("Quit Cikung").clicked() {
                ui.ctx()
                    .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
        });
    }

    pub(super) fn open_settings_page(&mut self, page: SettingsPage) {
        self.settings_tab = page;
        self.settings_open = true;
        self.settings_focus = true;
    }

    pub(super) fn open_request(&mut self, request: Option<&str>) {
        if let Some(page) = request.and_then(SettingsPage::from_request) {
            self.open_settings_page(page);
        } else {
            if let Some(request) = request {
                self.navigate(Page::parse(request));
            }
            self.expanded = true;
        }
    }

    pub(super) fn settings_hero(&self, ui: &mut egui::Ui) {
        let page = self.settings_tab;
        let (title, subtitle) = match page {
            SettingsPage::General => (
                "A little friend. Your kind of day.",
                "Settle Cikung into your desktop, just the way you like.",
            ),
            SettingsPage::Dictation => (
                "Let your words flow.",
                "Speak a thought, keep a note. Start whenever you're ready.",
            ),
            SettingsPage::Agents => (
                "A companion for your AI.",
                "Claude App · Codex / ChatGPT App · Oh My Pi",
            ),
            _ => (page.label(), "Your Cikung, all in one place."),
        };
        egui::Frame::new()
            .fill(Color32::from_rgb(18, 14, 10))
            .stroke(Stroke::new(1.0_f32, Color32::from_rgb(62, 42, 26)))
            .corner_radius(18)
            .inner_margin(24)
            .show(ui, |ui| {
                let width = ui.available_width();
                let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 104.0), Sense::hover());
                let mut glow = egui::Mesh::default();
                glow.colored_vertex(rect.left_top(), Color32::TRANSPARENT);
                glow.colored_vertex(rect.right_top(), Color32::from_rgb(72, 41, 17));
                glow.colored_vertex(rect.right_bottom(), Color32::from_rgb(32, 22, 13));
                glow.colored_vertex(rect.left_bottom(), Color32::TRANSPARENT);
                glow.add_triangle(0, 1, 2);
                glow.add_triangle(0, 2, 3);
                ui.painter().add(egui::Shape::mesh(glow));
                let text_rect = Rect::from_min_max(rect.min, rect.max - Vec2::new(82.0, 0.0));
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text_rect));
                child.add_space(7.0);
                child.add(egui::Label::new(RichText::new(title).size(26.0).strong()).wrap());
                child.add_space(5.0);
                child.add(
                    egui::Label::new(RichText::new(subtitle).size(12.0).color(muted())).wrap(),
                );
                let image = Rect::from_center_size(
                    rect.right_bottom() - Vec2::new(35.0, 35.0),
                    Vec2::splat(64.0),
                );
                let feature = match page {
                    SettingsPage::Agents => Page::Agents,
                    SettingsPage::Game => Page::Game,
                    SettingsPage::Water => Page::Water,
                    SettingsPage::Dictation | SettingsPage::Notes => Page::Notes,
                    _ => Page::Overview,
                };
                ui.painter().image(
                    self.cat_textures[self.breed_for(feature).index()].id(),
                    image,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    ink(),
                );
            });
        ui.add_space(22.0);
    }

    pub(super) fn hub_content(&mut self, ui: &mut egui::Ui) {
        use SettingsPage::*;
        match self.settings_tab {
            General => self.settings(ui),
            Notifications => self.notification_settings(ui),
            Cats => card(ui, |ui| self.style_panel(ui)),
            Music => {
                card(ui, |ui| {
                    heading(
                        ui,
                        "Music on hover",
                        "Spotify, YouTube and VLC follow the active media session.",
                    );
                    ui.checkbox(
                        &mut self.prefs.music_on_hover,
                        "Show the player on hover while music is playing",
                    );
                    ui.checkbox(
                        &mut self.prefs.auto_lyrics,
                        "Find lyrics automatically when a song plays",
                    )
                    .on_hover_text("Looks up the title, artist and duration using LRCLIB.");
                    ui.add_space(12.0);
                    if ui.button("Open player in the notch").clicked() {
                        self.navigate(Page::Music);
                        self.expanded = true;
                    }
                });
                card(ui, |ui| {
                    heading(
                        ui,
                        "Playback options",
                        "Choose the player and its shuffle, repeat and volume.",
                    );
                    self.playback_options(ui);
                });
                card(ui, |ui| self.vlc_settings(ui));
            }
            Agents => {
                card(ui, |ui| self.agents(ui));
                ui.label(RichText::new("Desktop connections use the active app window. Oh My Pi uses a connected coding session.").size(11.0).color(muted()));
            }
            Chat => {
                card(ui, |ui| {
                    heading(
                        ui,
                        "Chat provider",
                        "For Cikung chat and note summaries. Desktop app chats are under Agents.",
                    );
                    self.provider_settings(ui);
                });
                card(ui, |ui| self.chat_panel(ui));
            }
            Integrations => card(ui, |ui| self.services(ui)),
            Phone => card(ui, |ui| self.phone(ui)),
            Notes => card(ui, |ui| self.notes(ui)),
            Focus => card(ui, |ui| self.focus(ui)),
            Tasks => {
                card(ui, |ui| self.tasks(ui));
                card(ui, |ui| self.google_calendar_settings(ui));
            }
            Water => card(ui, |ui| self.water(ui)),
            Game => card(ui, |ui| self.game_panel(ui)),
            Shortcuts => card(ui, |ui| self.shortcut_settings(ui)),
            Dictation => {
                card(ui, |ui| {
                    heading(
                        ui,
                        "Speak into a note",
                        "Uses Windows speech recognition. Your transcript appears in Notes & recording.",
                    );
                    let busy = self.dictation.busy();
                    if ui
                        .add_enabled(
                            !busy && !self.demo,
                            egui::Button::new(
                                RichText::new("Start dictation").color(Color32::BLACK),
                            )
                            .fill(accent()),
                        )
                        .clicked()
                    {
                        self.dictation_to_notes = true;
                        let result = self.dictation.start();
                        self.report(result, "Windows speech recognition is listening.");
                    }
                    if busy {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Listening or transcribing…");
                            if ui.button("Cancel").clicked() {
                                self.dictation.cancel();
                            }
                        });
                    }
                    ui.add_space(8.0);
                    if ui.button("Open notes & recording").clicked() {
                        self.settings_tab = Notes;
                    }
                });
                card(ui, |ui| {
                    heading(
                        ui,
                        "Keep audio on your computer",
                        "Offline file transcription is available with your own whisper.cpp engine and model.",
                    );
                    ui.label(RichText::new("Open Notes & recording → Audio files and transcription to choose the local engine, model and audio file. Cloud transcription is available separately with a saved provider key.").color(muted()));
                });
            }
        }
    }
}

impl Cikung {
    pub(crate) fn open_chat_settings(&mut self) {
        self.open_settings_page(SettingsPage::Chat);
    }
    fn vlc_settings(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "VLC connection",
            "For VLC versions that do not expose a system media session.",
        );
        {
            ui.label(RichText::new("If VLC does not expose a system media session, enable its Web interface on local port 8080 and enter that interface password here. Setup: docs/vlc.md.").size(11.0).color(muted()));
            ui.add(
                egui::TextEdit::singleline(&mut self.vlc_password)
                    .password(true)
                    .hint_text("Local VLC Web password"),
            );
            ui.horizontal(|ui| {
                if ui.button("Save VLC connection").clicked() {
                    let result = cloud::save_secret("service-vlc", &self.vlc_password);
                    if result.is_ok() {
                        self.vlc_password.clear();
                    }
                    self.report(result, "VLC connection saved. Detection refreshes shortly.");
                }
                if ui.button("Remove VLC connection").clicked() {
                    let result = cloud::remove_secret("service-vlc");
                    self.report(result, "VLC password removed.");
                }
            });
        }
    }
    /// Claude Code's Stop hook, so finished turns chime even while the screen is off.
    fn claude_hook_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Claude Code, even with the screen off").strong());
        ui.label(RichText::new("Claude App pauses its window while the screen is off, so its finished turns reach Cikung late. Claude Code's Stop hook tells Cikung at once, for the Code tab and the CLI. Connect adds one hook to ~/.claude/settings.json after saving a backup beside it; Disconnect removes only that hook.").size(11.0).color(muted()));
        if self.demo || self.capture.is_some() || self.audit.is_some() {
            ui.label(RichText::new("Preview · Claude's settings are not touched.").color(muted()));
            return;
        }
        let connected = *self
            .claude_hook
            .get_or_insert_with(crate::claude_hook::connected);
        ui.horizontal_wrapped(|ui| {
            if connected {
                ui.label(RichText::new("Connected").color(accent()));
            }
            let connect = if connected {
                "Reconnect"
            } else {
                "Connect Claude Code"
            };
            if ui
                .button(connect)
                .on_hover_text("Points the hook at this copy of Cikung.")
                .clicked()
            {
                let result = crate::claude_hook::connect();
                self.claude_hook = Some(crate::claude_hook::connected());
                self.report_hook(result, "Claude Code connected");
            }
            if connected && ui.button("Disconnect").clicked() {
                let result = crate::claude_hook::disconnect();
                self.claude_hook = Some(crate::claude_hook::connected());
                self.report_hook(result, "Claude Code disconnected");
            }
        });
    }

    fn report_hook(&mut self, result: Result<Option<std::path::PathBuf>, String>, done: &str) {
        match result {
            Ok(Some(backup)) => self.toast(
                "Claude Code",
                format!("{done}. Backup: {}", backup.display()),
            ),
            Ok(None) => self.toast("Claude Code", format!("{done}. Nothing needed changing.")),
            Err(error) => self.toast("Claude Code", error),
        }
    }

    fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        heading(ui, "Keyboard shortcuts", "Keep Cikung one keystroke away.");
        ui.horizontal(|ui| {
            ui.label("Global shortcut: Ctrl + Alt +");
            ui.add(
                egui::TextEdit::singleline(&mut self.prefs.shortcut)
                    .char_limit(1)
                    .desired_width(25.0),
            );
            if ui.button("Apply").clicked() {
                // Unregister the old letter first so the same one can be applied again.
                self.shortcut = None;
                match register_shortcut(ui.ctx(), &mut self.prefs.shortcut, b'C') {
                    Ok(shortcut) => {
                        self.shortcut = Some(shortcut);
                        self.toast("Shortcut", "Global shortcut updated.");
                    }
                    Err(e) => self.toast("Shortcut", e),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Hide or show Cikung: Ctrl + Alt +");
            ui.add(
                egui::TextEdit::singleline(&mut self.prefs.hide_shortcut)
                    .char_limit(1)
                    .desired_width(25.0),
            );
            if ui.button("Apply").clicked() {
                self.hide_shortcut = None;
                match register_shortcut(ui.ctx(), &mut self.prefs.hide_shortcut, b'H') {
                    Ok(shortcut) => {
                        self.hide_shortcut = Some(shortcut);
                        self.toast("Shortcut", "Hide shortcut updated.");
                    }
                    Err(e) => self.toast("Shortcut", e),
                }
            }
        });
        ui.label(RichText::new("For games and full-screen work: the notch tucks away completely until you press it again. The global shortcut above still opens the panel while it is hidden.").size(11.0).color(muted()));

        ui.add_space(18.0);
        ui.label("Escape · close the current panel");
        ui.label("Ctrl + Q · quit Cikung when its notch has focus");
    }
    fn provider_settings(&mut self, ui: &mut egui::Ui) {
        let previous = self.prefs.chat.provider;
        egui::ComboBox::from_id_salt("chat-provider")
            .selected_text(previous.name())
            .show_ui(ui, |ui| {
                for provider in ChatProvider::ALL {
                    ui.selectable_value(&mut self.prefs.chat.provider, provider, provider.name());
                }
            });
        if previous != self.prefs.chat.provider {
            self.prefs.chat = ChatConfig::for_provider(self.prefs.chat.provider);
            self.secret_input.clear();
            self.reset_chat();
        }
        ui.label("Model ID");
        let model_changed = ui
            .add(
                egui::TextEdit::singleline(&mut self.prefs.chat.model).desired_width(f32::INFINITY),
            )
            .changed();
        ui.label("Provider address");
        let address_changed = ui
            .add(
                egui::TextEdit::singleline(&mut self.prefs.chat.base_url)
                    .desired_width(f32::INFINITY),
            )
            .changed();
        if model_changed || address_changed {
            self.reset_chat();
        }
        ui.add(
            egui::TextEdit::singleline(&mut self.secret_input)
                .password(true)
                .hint_text("API key (optional for a local model)")
                .desired_width(f32::INFINITY),
        );
        ui.horizontal(|ui| {
            if ui.button("Save key securely").clicked() {
                let result =
                    cloud::save_secret(self.prefs.chat.provider.secret_name(), &self.secret_input);
                if result.is_ok() {
                    self.secret_input.clear();
                    self.reset_chat();
                }
                self.report(
                    result,
                    "API key saved in your operating system credential store.",
                );
            }
            if ui.button("Remove key").clicked() {
                let result = cloud::remove_secret(self.prefs.chat.provider.secret_name());
                if result.is_ok() {
                    self.reset_chat();
                }
                self.report(result, "Saved key removed.");
            }
        });
        ui.label(RichText::new("Ollama and LM Studio can run entirely on this PC. Cloud messages and summaries go only to the provider you select.").size(11.0).color(muted()));
    }
}

impl Cikung {
    fn notification_settings(&mut self, ui: &mut egui::Ui) {
        card(ui, |ui| {
            heading(
                ui,
                "A little sound when AI finishes",
                "Claude App · Claude Code · Codex / ChatGPT App",
            );
            ui.add_enabled_ui(cfg!(any(windows, target_os = "macos")), |ui| {
                ui.checkbox(
                    &mut self.prefs.agent_completion_sound,
                    "Play a sound when AI finishes",
                );
                ui.add(
                    egui::Slider::new(&mut self.prefs.agent_completion_volume, 0..=100)
                        .text("Sound volume")
                        .suffix("%"),
                );
                ui.label(
                    RichText::new("Only Cikung's AI chime. 0% is silent.")
                        .size(11.0)
                        .color(muted()),
                );
                if ui.button("Test completion sound").clicked() {
                    let result = self
                        .completion_player
                        .play(Instant::now(), self.prefs.agent_completion_volume);
                    let message = if self.prefs.agent_completion_volume == 0 {
                        "Completion sound is silent at 0%."
                    } else {
                        "Completion sound played."
                    };
                    self.report(result, message);
                }
            });
            if !cfg!(any(windows, target_os = "macos")) {
                ui.label(
                    RichText::new(
                        "Completion sounds are currently available on Windows and macOS.",
                    )
                    .color(muted()),
                );
            }
            ui.label(RichText::new("Link desktop apps in Agents. Claude Code needs its Cikung hook connection. The sound plays once when a turn finishes; starting Cikung or opening an idle app stays quiet.").size(11.0).color(muted()));
            ui.add_space(12.0);
            self.claude_hook_settings(ui);
        });
        card(ui, |ui| {
            heading(
                ui,
                "WhatsApp, close by",
                "New alerts from WhatsApp and WhatsApp Desk, in your notch.",
            );
            ui.add_enabled_ui(cfg!(windows), |ui| {
                ui.checkbox(
                    &mut self.prefs.whatsapp_notifications,
                    "Show WhatsApp notifications in Cikung",
                );
                ui.add_enabled_ui(self.prefs.whatsapp_notifications, |ui| {
                    ui.checkbox(
                        &mut self.prefs.whatsapp_preview,
                        "Show sender and message preview",
                    );
                    // The pill grows with the message up to this width; longer ones are cut.
                    ui.horizontal(|ui| {
                        ui.label("Widest message");
                        let width = self.prefs.whatsapp_width;
                        egui::ComboBox::from_id_salt("whatsapp-width")
                            .selected_text(
                                notch::MESSAGE_WIDTHS
                                    .iter()
                                    .find(|(w, _)| *w == width)
                                    .map_or_else(|| format!("{width:.0}"), |(_, n)| (*n).into()),
                            )
                            .show_ui(ui, |ui| {
                                for (w, name) in notch::MESSAGE_WIDTHS {
                                    ui.selectable_value(&mut self.prefs.whatsapp_width, w, name);
                                }
                            });
                    });
                });
            });
            ui.add_space(10.0);
            let access = self
                .whatsapp
                .as_ref()
                .map_or(crate::whatsapp::Access::Checking, |m| m.access);
            let status = if !cfg!(windows) {
                "Available on Windows only."
            } else if self.demo || self.capture.is_some() {
                "Preview · live notifications are not read."
            } else if !self.prefs.whatsapp_notifications {
                "WhatsApp notifications are off."
            } else {
                access.label()
            };
            ui.label(RichText::new(status).color(muted()));
            if let Some(error) = self.whatsapp.as_ref().and_then(|m| m.error.as_ref()) {
                ui.label(RichText::new(error).color(accent()));
            }
            ui.add_enabled_ui(
                cfg!(windows) && !self.demo && self.capture.is_none() && self.audit.is_none(),
                |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                self.prefs.whatsapp_notifications
                                    && access != crate::whatsapp::Access::Allowed
                                    && self.notification_access.is_none(),
                                egui::Button::new("Allow Windows access"),
                            )
                            .clicked()
                        {
                            match crate::whatsapp::request_access(ui.ctx().clone()) {
                                Ok(request) => self.notification_access = Some(request),
                                Err(error) => self.toast("Notification access", error),
                            }
                        }
                        if ui.button("Windows notification access").clicked() {
                            let result = crate::whatsapp::open_access_settings();
                            if let Err(error) = result {
                                self.toast("Notification access", error);
                            }
                        }
                    });
                },
            );
            ui.add_space(10.0);
            ui.label(RichText::new("Enable Windows notifications in WhatsApp / WhatsApp Desk. Cikung follows new Windows alerts, including those sent to Notification Center while Do not disturb is on. Old notifications stay quiet at startup.").size(11.0).color(muted()));
        });
        card(ui, |ui| {
            heading(ui, "Only on your screen", "Message previews start hidden.");
            ui.label(RichText::new("WhatsApp alerts are held briefly in memory, never saved to Cikung history or sent to AI, cloud services or your phone. Cikung does not reply, dismiss, or mark messages as read.").color(muted()));
            if ui.button("Test notch notification").clicked() {
                self.toast("WhatsApp test", "Test notification from Cikung");
            }
        });
    }
}
