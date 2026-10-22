use crate::{
    app::{Cikung, GRAY, ORANGE, WHITE, clock, heading},
    cloud,
    media::{MediaCommand, PlaybackState},
    tools::FocusMode,
};
use eframe::egui::{self, Color32, RichText, Vec2};
use std::time::Instant;

impl Cikung {
    pub(crate) fn lyrics(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Words to your music",
            "Lyrics are found automatically as songs change. Synced lyrics follow playback.",
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.lyrics_busy && self.session.is_some(),
                    egui::Button::new(if self.lyrics_busy {
                        "Looking up…"
                    } else if self.prefs.auto_lyrics {
                        "Retry lyrics"
                    } else {
                        "Find lyrics"
                    }),
                )
                .on_hover_text("Sends this track's title, artist and duration to LRCLIB.")
                .clicked()
                && let Some(s) = self.session.clone()
            {
                let result = self.request_lyrics(s);
                self.report(result, "Looking for this track's lyrics…");
            }
            if ui.button("Import LRC / text").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter("Lyrics", &["lrc", "txt"])
                    .pick_file()
            {
                let result = self.import_lyrics_file(&path);
                self.report(result, "Lyrics loaded.");
            }
            if ui.button("Save").clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_file_name("lyrics.lrc")
                    .save_file()
            {
                let result = self.tools.lyrics.save(&path);
                self.report(result, "Lyrics saved.");
            }
        });
        if self.tools.lyrics.lines.is_empty() {
            ui.add_space(18.0);
            ui.label(RichText::new(self.lyrics_status()).color(GRAY));
            return;
        }
        let identity = self
            .session
            .as_ref()
            .map(|s| format!("{}\n{}", s.title, s.artist))
            .unwrap_or_default();
        if !self.lyrics_for.is_empty() && identity != self.lyrics_for {
            ui.label(RichText::new(self.lyrics_status()).color(GRAY));
            return;
        }
        ui.add_space(14.0);
        let active = self.tools.lyrics.active_index(self.position());
        let scroll_id = ui.make_persistent_id(("lyrics-follow", &self.lyrics_for));
        let follow = ui.data(|d| d.get_temp::<usize>(scroll_id)) != active;
        if let Some(index) = active {
            ui.data_mut(|d| d.insert_temp(scroll_id, index));
        }
        let mut seek_to = None;
        for (index, line) in self.tools.lyrics.lines.iter().enumerate() {
            let highlighted = active == Some(index);
            let target = self.lyric_seek_target(line.time_secs);
            let response = ui.add(
                egui::Label::new(
                    RichText::new(&line.text)
                        .size(if highlighted { 22.0 } else { 16.0 })
                        .color(if highlighted { WHITE } else { GRAY })
                        .strong(),
                )
                .sense(if target.is_some() {
                    egui::Sense::click()
                } else {
                    egui::Sense::hover()
                }),
            );
            if target.is_some()
                && response
                    .clone()
                    .on_hover_text("Click to jump to this lyric")
                    .clicked()
            {
                seek_to = target;
            }
            if highlighted
                && follow
                && self
                    .session
                    .as_ref()
                    .is_some_and(|s| s.state == PlaybackState::Playing)
            {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }
        if let Some(seconds) = seek_to {
            self.control(MediaCommand::Seek(seconds));
        }
    }
    pub(crate) fn focus(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "A little room to focus",
            "One thing at a time. Cikung will keep the clock.",
        );
        ui.horizontal(|ui| {
            let is_focus = self.tools.focus.mode == FocusMode::Focus;
            if ui.selectable_label(is_focus, "Focus").clicked() && !is_focus {
                self.tools.focus.switch_mode();
            }
            if ui.selectable_label(!is_focus, "Break").clicked() && is_focus {
                self.tools.focus.switch_mode();
            }
        });
        ui.label(
            RichText::new(self.tools.focus.formatted_remaining())
                .size(58.0)
                .strong(),
        );
        ui.horizontal(|ui| {
            if ui
                .button(if self.tools.focus.running {
                    "Pause"
                } else {
                    "Start"
                })
                .clicked()
            {
                if self.tools.focus.running {
                    self.tools.focus.pause();
                } else {
                    self.tools.focus.start();
                }
            }
            if ui.button("Reset").clicked() {
                self.tools.focus.reset();
            }
            for minutes in [5, 15, 25, 50] {
                if ui.small_button(format!("{minutes} min")).clicked() {
                    self.tools.focus.set_duration(minutes);
                }
            }
        });
        let mut minutes = if self.tools.focus.mode == FocusMode::Focus {
            self.tools.focus.focus_minutes
        } else {
            self.tools.focus.break_minutes
        };
        ui.horizontal(|ui| {
            ui.label("Custom minutes");
            if ui
                .add(egui::DragValue::new(&mut minutes).range(1..=240))
                .changed()
            {
                self.tools.focus.set_duration(minutes);
            }
        });
        ui.add_space(12.0);
        ui.label(
            RichText::new(format!(
                "{} focus sessions completed · {} minutes focused",
                self.tools.focus.completed_sessions,
                self.tools.focus.total_focus_secs / 60
            ))
            .color(GRAY)
            .size(12.0),
        );
    }
    pub(crate) fn files(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "A pocket for your files",
            "Drop files on the notch. Originals stay exactly where they are.",
        );
        if ui.button("Add files").clicked()
            && let Some(paths) = rfd::FileDialog::new().pick_files()
        {
            for path in paths {
                if let Err(e) = self.tools.add_file(path) {
                    self.toast("Files", e);
                }
            }
        }
        ui.add_space(8.0);
        let drag = if self.prefs.reduced_motion {
            if self.file_hover.is_some() { 1.0 } else { 0.0 }
        } else {
            ui.ctx().animate_bool_with_time(
                egui::Id::new("pocket-drop-hover"),
                self.file_hover.is_some(),
                0.18,
            )
        };
        egui::Frame::new()
            .fill(Color32::from_rgb(
                24 + (drag * 18.0) as u8,
                23 + (drag * 9.0) as u8,
                22,
            ))
            .stroke(egui::Stroke::new(1.0_f32, GRAY.lerp_to_gamma(ORANGE, drag)))
            .corner_radius(12)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.set_width((ui.available_width() - 2.0).max(1.0));
                ui.label(
                    RichText::new(if self.file_hover.is_some() {
                        "Release to keep files here"
                    } else {
                        "Drop files here to keep them close"
                    })
                    .color(WHITE),
                );
                ui.label(
                    RichText::new("A temporary pocket. Your original files stay in place.")
                        .size(11.0)
                        .color(GRAY),
                );
            });
        let mut remove = None;
        for (index, item) in self.tools.shelf.clone().iter().enumerate() {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.add_sized([260.0, 24.0], egui::Label::new(&item.name).truncate())
                    .on_hover_text(&item.path);
                if ui.small_button("Open").clicked() {
                    let result = self.tools.open_file(index);
                    self.report(result, "Opened file.");
                }
                if ui.small_button("Show").clicked() {
                    let result = self.tools.reveal_file(index);
                    self.report(result, "Opened its folder.");
                }
                if ui.small_button("Remove").clicked() {
                    remove = Some(index);
                }
            });
        }
        if let Some(index) = remove {
            self.tools.remove_file(index);
        }
    }
    pub(crate) fn tasks(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Little things, done",
            "Edit a task or give it a time. Cikung will remind you while it is running.",
        );
        let editing = self.task_edit.is_some();
        let mut save = false;
        ui.horizontal(|ui| {
            let width = (ui.available_width() - 86.0).max(80.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.task_input)
                    .desired_width(width)
                    .char_limit(2000)
                    .hint_text("What needs doing?"),
            );
            save = ui.button(if editing { "Save" } else { "Add" }).clicked()
                || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.task_due_input)
                .desired_width(f32::INFINITY)
                .char_limit(16)
                .hint_text("Optional reminder · YYYY-MM-DD HH:MM"),
        );
        ui.label(
            RichText::new("Local time · leave empty for no reminder")
                .size(10.0)
                .color(GRAY),
        );
        if editing && ui.small_button("Cancel edit").clicked() {
            self.task_edit = None;
            self.task_input.clear();
            self.task_due_input.clear();
        }
        if save {
            let result = parse_task_due(&self.task_due_input).and_then(|due| {
                let id = if let Some(id) = self.task_edit {
                    id
                } else {
                    self.tools.add_task(&self.task_input)?
                };
                self.tools.edit_task(id, &self.task_input, due)
            });
            match result {
                Ok(()) => {
                    self.task_input.clear();
                    self.task_due_input.clear();
                    self.task_edit = None;
                }
                Err(error) => self.toast("Tasks", error),
            }
        }
        ui.add_space(8.0);
        let mut remove = None;
        for task in self.tools.tasks.clone() {
            ui.horizontal(|ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                let mut done = task.done;
                let width = (ui.available_width() - 78.0).max(40.0);
                if ui
                    .add_sized(
                        [width, 24.0],
                        egui::Checkbox::new(
                            &mut done,
                            RichText::new(&task.text).color(if task.done { GRAY } else { WHITE }),
                        ),
                    )
                    .on_hover_text(&task.text)
                    .changed()
                {
                    self.tools.toggle_task(task.id);
                    if done {
                        self.stats().tasks += 1;
                    }
                }
                if ui.small_button("Edit").clicked() {
                    self.task_edit = Some(task.id);
                    self.task_input = task.text.clone();
                    self.task_due_input = task.due_unix.map(format_task_due).unwrap_or_default();
                }
                if ui.small_button("×").on_hover_text("Remove task").clicked() {
                    remove = Some(task.id);
                }
            });
            if let Some(due) = task.due_unix {
                ui.label(
                    RichText::new(format!(
                        "{}{}",
                        format_task_due(due),
                        if task.due_notified {
                            " · reminded"
                        } else {
                            " · reminder"
                        }
                    ))
                    .size(10.0)
                    .color(GRAY),
                );
            }
        }
        if let Some(id) = remove {
            self.tools.remove_task(id);
            if self.task_edit == Some(id) {
                self.task_edit = None;
                self.task_input.clear();
                self.task_due_input.clear();
            }
        }
        if self.tools.tasks.is_empty() {
            ui.label(RichText::new("Nothing to chase. Enjoy the quiet.").color(GRAY));
        }
    }
    pub(crate) fn water(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "A sip and a stretch",
            "Your bottle, your daily goal, your hours.",
        );
        let goal = self.tools.water.goal_ml.clamp(250, 10000);
        let total = self.tools.water.ml_today;
        ui.label(
            RichText::new(format!(
                "{:.2} / {:.2} L",
                total as f32 / 1000.0,
                goal as f32 / 1000.0
            ))
            .size(30.0)
            .strong(),
        );
        ui.add(
            egui::ProgressBar::new((total as f32 / goal as f32).min(1.0))
                .fill(ORANGE)
                .text(format!(
                    "{} drinks today{}",
                    self.tools.water.drank_today,
                    if total >= goal {
                        " · goal reached"
                    } else {
                        ""
                    }
                )),
        );
        if total == 0 && self.tools.water.drank_today > 0 {
            ui.label(
                RichText::new("Your earlier drink count is kept. Volume starts with new drinks.")
                    .size(10.0)
                    .color(GRAY),
            );
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(format!(
                    "Drink {} ml",
                    self.tools.water.bottle_ml.clamp(50, 2000)
                ))
                .clicked()
            {
                self.tools.water.drink();
                self.stats().water += 1;
                if total < goal && self.tools.water.ml_today >= goal {
                    self.toast(
                        "Goal reached",
                        "You've logged your daily water goal. Reminders will rest for today.",
                    );
                } else {
                    self.toast("Cheers!", "Your drink has been added.");
                }
            }
            if ui
                .add_enabled(self.tools.water.enabled, egui::Button::new("Snooze 10 min"))
                .clicked()
            {
                self.tools.water.snooze(10);
            }
        });
        ui.collapsing("Bottle, goal & reminder hours", |ui| {
            let mut changed = false;
            ui.horizontal_wrapped(|ui| {
                ui.label("Bottle"); changed |= ui.add(egui::DragValue::new(&mut self.tools.water.bottle_ml).range(50..=2000).suffix(" ml")).changed();
                ui.label("Daily goal"); changed |= ui.add(egui::DragValue::new(&mut self.tools.water.goal_ml).range(250..=10000).suffix(" ml")).changed();
            });
            changed |= ui.checkbox(&mut self.tools.water.enabled, "Remind me to drink water").changed();
            ui.horizontal_wrapped(|ui| {
                ui.label("Every"); changed |= ui.add(egui::DragValue::new(&mut self.tools.water.interval_minutes).range(1..=1440).suffix(" min")).changed();
                ui.label("From"); changed |= ui.add(egui::DragValue::new(&mut self.tools.water.active_start_hour).range(0..=23).suffix(":00")).changed();
                ui.label("to"); changed |= ui.add(egui::DragValue::new(&mut self.tools.water.active_end_hour).range(0..=23).suffix(":00")).changed();
            });
            ui.label(RichText::new("Local hours · matching start/end means all day. Reminders rest after your goal.").size(10.0).color(GRAY));
            if changed { self.tools.water.reschedule(); }
        });
        if let Some(seconds) = self.tools.water.seconds_until_reminder() {
            ui.label(
                RichText::new(format!("Next reminder in {}", clock(seconds as f64)))
                    .size(11.0)
                    .color(GRAY),
            );
        }
        ui.collapsing("Recent daily totals", |ui| {
            if self.tools.water.history.is_empty() {
                ui.label(RichText::new("Log a drink to start your volume history.").color(GRAY));
            }
            for day in self.tools.water.history.iter().rev().take(7) {
                ui.label(RichText::new(&day.day).size(10.0).color(GRAY));
                ui.add(
                    egui::ProgressBar::new((day.ml as f32 / goal as f32).min(1.0))
                        .fill(ORANGE)
                        .text(format!("{} ml", day.ml)),
                );
            }
        });
    }
    pub(crate) fn notes(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Notes, without the scramble",
            "Record locally, dictate, or bring a transcript. Upload only when you choose.",
        );
        ui.horizontal(|ui|{
            if self.recorder.is_recording(){ui.label(RichText::new(format!("RECORDING · {}",clock(self.record_started.map(|t|t.elapsed().as_secs_f64()).unwrap_or(0.0)))).color(Color32::from_rgb(255,105,105)));if ui.button("Stop recording").clicked(){let result=self.recorder.stop();self.report(result,"Finishing audio files…");}}
            else if ui.button("Record mic + PC audio").on_hover_text("Captures microphone and system audio to separate WAV files in a folder you choose. Let people in the call know.").clicked()&&let Some(folder)=rfd::FileDialog::new().set_title("Save meeting audio to this folder").pick_folder(){match self.recorder.start(&folder){Ok(())=>{self.record_started=Some(Instant::now());self.toast("Recording","Microphone and PC audio are being saved on this device.");},Err(e)=>self.toast("Recording",e)}}
            if ui.add_enabled(!self.dictation.busy(),egui::Button::new("Dictate")).clicked(){self.dictation_to_notes=true;let result=self.dictation.start();self.report(result,"Windows speech recognition is listening.");}
            if ui.button("Import text").clicked()&&let Some(path)=rfd::FileDialog::new().add_filter("Transcript",&["txt","md","vtt","srt"]).pick_file(){match self.tools.import_transcript(&path){Ok(id)=>{self.note_selected=Some(id);self.stats().notes+=1;},Err(e)=>self.toast("Notes",e)}}
        });
        ui.collapsing("Audio files and transcription",|ui|{
            ui.label(RichText::new("Transcribe uploads a selected audio track to OpenAI using your saved OpenAI key. Local Whisper can keep audio on this PC.").size(11.0).color(GRAY));
            if ui.button("Add audio file").clicked()&&let Some(path)=rfd::FileDialog::new().add_filter("Audio",&["wav","mp3","m4a","flac","ogg","webm"]).pick_file(){self.recording_files.push(path);}
            for path in self.recording_files.clone(){ui.horizontal(|ui|{ui.add_sized([320.0,22.0],egui::Label::new(path.file_name().unwrap_or_default().to_string_lossy()).truncate()).on_hover_text(path.display().to_string());if ui.add_enabled(!self.transcription_busy(&path),egui::Button::new("Transcribe")).clicked(){let result=self.transcribe_file(path.clone());self.report(result,"Transcription requested. This audio is being uploaded.");}});}
            if ui.add_enabled(!self.dictation.busy(),egui::Button::new("Transcribe offline with whisper.cpp")).clicked()&&let Some(engine)=rfd::FileDialog::new().set_title("Choose your whisper.cpp executable").pick_file()&&let Some(model)=rfd::FileDialog::new().set_title("Choose your Whisper model").pick_file()&&let Some(audio)=rfd::FileDialog::new().set_title("Choose local audio").pick_file(){self.dictation_to_notes=true;let result=self.dictation.transcribe_audio_with_model(&engine,&audio,Some(&model));self.report(result,"Offline transcription started.");}
            if self.dictation.busy(){ui.label("Speech recognition or transcription is running…");if ui.button("Cancel transcription").clicked(){self.dictation.cancel();}}
        });
        ui.add_space(6.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.note_title)
                .hint_text("Note title")
                .desired_width(f32::INFINITY),
        );
        ui.add(
            egui::TextEdit::multiline(&mut self.note_text)
                .hint_text("Write a note, or dictate into this space…")
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        if ui.button("Save note").clicked() {
            match self.tools.add_note(&self.note_title, &self.note_text) {
                Ok(id) => {
                    self.note_selected = Some(id);
                    self.note_title.clear();
                    self.note_text.clear();
                    self.stats().notes += 1;
                }
                Err(e) => self.toast("Notes", e),
            }
        }
        for note in self.tools.notes.clone().into_iter().rev() {
            let selected = self.note_selected == Some(note.id);
            if ui.selectable_label(selected, &note.title).clicked() {
                self.note_selected = Some(note.id);
            }
            if selected {
                ui.label(RichText::new(&note.text).size(12.0));
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !self.summary_busy(note.id),
                            egui::Button::new("Summarize with AI"),
                        )
                        .on_hover_text("Sends this note to your configured chat provider.")
                        .clicked()
                    {
                        let result = self.summarize_note(note.id);
                        self.report(result, "Note sent to your selected AI provider.");
                    }
                    if ui.button("Extract tasks").clicked() {
                        match self.tools.tasks_from_note(note.id) {
                            Ok(n) => self.toast(
                                "Tasks",
                                format!(
                                    "Added {n} explicit action items. Open Tasks to review them."
                                ),
                            ),
                            Err(error) => self.toast("Tasks", error),
                        }
                    }
                    if ui.button("Export").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .set_file_name("meeting-note.md")
                            .save_file()
                    {
                        let result = self.tools.export_note(note.id, &path);
                        self.report(result, "Note exported.");
                    }
                    if ui.small_button("Delete").clicked() {
                        self.tools.remove_note(note.id);
                        self.note_selected = None;
                    }
                });
            }
        }
    }
    pub(crate) fn agents(&mut self, ui: &mut egui::Ui) {
        if self.prefs.agents.selected == crate::agent_connect::Agent::Omp {
            heading(ui, "Oh My Pi", "Your connected coding agent");
        }
        self.agent_connections(ui);
        if self.prefs.agents.selected != crate::agent_connect::Agent::Omp {
            return;
        }
        if ui
            .selectable_label(self.show_agent_setup, "Connection setup")
            .clicked()
        {
            self.show_agent_setup = !self.show_agent_setup;
        }
        if self.show_agent_setup {
            if let Some(bridge) = &self.bridge {
                ui.label(RichText::new("Connect tools with scripts/cikung-hook.ps1. No tool settings are changed automatically.").color(GRAY).size(11.0));
                ui.monospace(bridge.local_endpoint());
                if ui
                    .button(if self.show_agent_token {
                        "Hide local token"
                    } else {
                        "Reveal local token"
                    })
                    .clicked()
                {
                    self.show_agent_token = !self.show_agent_token;
                }
                if self.show_agent_token {
                    ui.add(
                        egui::Label::new(
                            RichText::new(bridge.agent_token()).monospace().size(10.0),
                        )
                        .wrap(),
                    );
                }
                if ui.button("Copy hook connection command").clicked() {
                    ui.ctx().copy_text(format!(
                        "$env:CIKUNG_ENDPOINT='{}'; $env:CIKUNG_TOKEN='{}'",
                        bridge.local_endpoint(),
                        bridge.agent_token()
                    ));
                }
                ui.label(RichText::new("The token changes when Cikung restarts. Hook installation steps are in docs/agent-phone-link.md.").size(10.0).color(GRAY));
            } else {
                ui.label("Bridge is unavailable in this preview, or could not start.");
            }
        }
        let sessions = self
            .bridge_view
            .as_ref()
            .map(|s| s.sessions.clone())
            .unwrap_or_default();
        if sessions.is_empty() {
            ui.add_space(15.0);
            ui.label(RichText::new("No agent sessions connected yet.").color(GRAY));
            ui.label(RichText::new("Choose a project and start a connected terminal above. Available means the CLI was found; live activity appears only after receiving an event.").size(11.0).color(GRAY));
        }
        for session in sessions {
            ui.add_space(7.0);
            ui.horizontal(|ui| {
                let selected = self.selected_agent.as_deref() == Some(&session.session_id);
                if ui
                    .selectable_label(selected, format!("{} · {}", session.agent, session.state))
                    .clicked()
                {
                    self.selected_agent = Some(session.session_id.clone());
                }
                ui.label(RichText::new(&session.project).size(11.0).color(GRAY));
            });
            ui.label(&session.detail);
            if let Some(id) = &session.request_id {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Needs your permission").color(ORANGE));
                    if ui.button("Allow once").clicked()
                        && let Some(bridge) = &self.bridge
                    {
                        let result = bridge.decide(id, true);
                        self.report(result, "Allowed this request.");
                    }
                    if ui.button("Deny").clicked()
                        && let Some(bridge) = &self.bridge
                    {
                        let result = bridge.decide(id, false);
                        self.report(result, "Denied this request.");
                    }
                });
            }
            if self.selected_agent.as_deref() == Some(&session.session_id) {
                for step in session.history.iter().rev().take(12) {
                    ui.label(
                        RichText::new(format!("{} · {}", step.state, step.detail))
                            .size(11.0)
                            .color(GRAY),
                    );
                }
                if let Some(diff) = session.file_diff {
                    ui.collapsing("File changes", |ui| {
                        ui.monospace(diff);
                    });
                }
            }
        }
    }
    pub(crate) fn chat_panel(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "A thought, a question, a chat",
            self.prefs.chat.provider.name(),
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(&self.prefs.chat.model).color(GRAY).size(11.0));
            if ui.small_button("Configure").clicked() {
                self.open_chat_settings();
            }
            if ui.small_button("New chat").clicked() {
                self.reset_chat();
                self.draft.clear();
            }
        });
        for message in &self.chat {
            ui.add_space(5.0);
            ui.label(
                RichText::new(if message.role == "user" {
                    "You"
                } else {
                    "Cikung"
                })
                .size(11.0)
                .color(ORANGE),
            );
            ui.label(&message.content);
        }
        if self.chat_busy {
            ui.spinner();
            ui.label("Thinking…");
        }
        ui.add(
            egui::TextEdit::multiline(&mut self.draft)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("Ask something…"),
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.chat_busy && !self.draft.trim().is_empty(),
                    egui::Button::new("Send"),
                )
                .clicked()
            {
                self.submit_chat();
            }
            if ui
                .add_enabled(!self.dictation.busy(), egui::Button::new("Dictate"))
                .clicked()
            {
                self.dictation_to_notes = false;
                let result = self.dictation.start();
                self.report(result, "Windows speech recognition is listening.");
            }
            ui.label(
                RichText::new(if self.prefs.chat.provider.is_local() {
                    "Configured server · no cloud key required"
                } else {
                    "Messages go to your selected provider"
                })
                .size(10.0)
                .color(GRAY),
            );
        });
    }
    pub(crate) fn services(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Your services, close by",
            "Read-only status from the accounts you choose to connect.",
        );
        if self.prefs.services.is_empty() {
            return;
        }
        self.service_index = self.service_index.min(self.prefs.services.len() - 1);
        egui::ComboBox::from_id_salt("service-picker")
            .selected_text(self.prefs.services[self.service_index].service.name())
            .show_ui(ui, |ui| {
                for (index, config) in self.prefs.services.iter().enumerate() {
                    if ui
                        .selectable_value(&mut self.service_index, index, config.service.name())
                        .clicked()
                    {
                        self.service_secret.clear();
                    }
                }
            });
        let config = self.prefs.services[self.service_index].clone();
        let name = config.service.name();
        if ui
            .add_enabled(
                !self.service_busy(config.service),
                egui::Button::new("Refresh account status"),
            )
            .clicked()
        {
            let result = self.refresh_current_service();
            self.report(result, "Fetching account status…");
        }
        if let Some(cards) = self.service_cards.get(name) {
            for card in cards {
                ui.add_space(7.0);
                ui.label(RichText::new(&card.title).strong());
                ui.label(RichText::new(&card.detail).color(GRAY));
                if !card.url.is_empty() {
                    ui.hyperlink_to("Open in browser", &card.url);
                }
            }
        } else {
            ui.label(
                RichText::new(
                    "Connect this service below, then refresh. Account data is never fabricated.",
                )
                .size(12.0)
                .color(GRAY),
            );
        }
        ui.add_space(8.0);
        ui.collapsing("Connection settings", |ui| {
            let service_config = &mut self.prefs.services[self.service_index];
            if config.service == crate::cloud::Service::GitHub {
                ui.label("Repository (optional)");
                ui.add(
                    egui::TextEdit::singleline(&mut service_config.repository)
                        .hint_text("owner/repo")
                        .desired_width(f32::INFINITY),
                );
            }
            if config.service == crate::cloud::Service::N8n {
                ui.label("Server address");
                ui.add(
                    egui::TextEdit::singleline(&mut service_config.base_url)
                        .hint_text("https://your-n8n.example")
                        .desired_width(f32::INFINITY),
                );
            }
            ui.label(config.service.scope_hint());
            ui.add(
                egui::TextEdit::singleline(&mut service_config.scope).desired_width(f32::INFINITY),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.service_secret)
                    .password(true)
                    .hint_text("API key / access token")
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                if ui.button("Save key securely").clicked() {
                    let result =
                        cloud::save_secret(config.service.secret_name(), &self.service_secret);
                    if result.is_ok() {
                        self.service_secret.clear();
                        self.clear_service_result(config.service);
                    }
                    self.report(result, "Saved in your operating system credential store.");
                }
                if ui.button("Remove saved key").clicked() {
                    let result = cloud::remove_secret(config.service.secret_name());
                    if result.is_ok() {
                        self.clear_service_result(config.service);
                    }
                    self.report(result, "Saved key removed.");
                }
            });
        });
    }
    pub(crate) fn phone(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Cikung, in your pocket",
            "Your agents and music, on a phone connected to the same Wi-Fi.",
        );
        let Some(bridge) = &mut self.bridge else {
            ui.label("Phone pairing is available in a normal launch.");
            return;
        };
        let mut enabled = bridge.snapshot().phone_enabled;
        if ui
            .checkbox(&mut enabled, "Enable phone connection on my local network")
            .changed()
        {
            let result = bridge.enable_phone(enabled);
            self.qr = None;
            self.report(
                result,
                if enabled {
                    "Phone pairing is ready."
                } else {
                    "Phone sessions revoked."
                },
            );
        }
        let Some(bridge) = &self.bridge else {
            return;
        };
        let snapshot = bridge.snapshot();
        if let Some(url) = bridge.pairing_url() {
            if self.qr.as_ref().is_none_or(|(old, _)| old != &url)
                && let Ok(code) = qrcode::QrCode::new(url.as_bytes())
            {
                let size = code.width();
                let border = 4usize;
                let width = size + border * 2;
                let mut pixels = vec![Color32::WHITE; width * width];
                for y in 0..size {
                    for x in 0..size {
                        if code[(x, y)] == qrcode::Color::Dark {
                            pixels[(y + border) * width + x + border] = Color32::BLACK;
                        }
                    }
                }
                let texture = ui.ctx().load_texture(
                    "Phone pairing QR",
                    egui::ColorImage::new([width, width], pixels),
                    egui::TextureOptions::NEAREST,
                );
                self.qr = Some((url.clone(), texture));
            }
            ui.horizontal(|ui|{if let Some((_,texture))=&self.qr{ui.image((texture.id(),Vec2::splat(170.0)));}ui.vertical(|ui|{ui.label("Scan to pair");ui.label(RichText::new(format!("{} paired phone sessions",snapshot.paired_phones)).color(GRAY));if ui.button("Copy pairing link").clicked(){ui.ctx().copy_text(url.clone());}ui.label(RichText::new("The link grants access. Share it only with your phone. Disabling the connection revokes every session.").size(11.0).color(GRAY));});});
            ui.label(RichText::new("The companion runs in your phone browser. Add it to your Home Screen. Native iOS Live Activities and Android widgets are separate future clients.").size(11.0).color(GRAY));
            ui.label(RichText::new("Local HTTP · use a trusted Wi-Fi network. Allow the private-network firewall prompt if Windows asks.").size(10.0).color(GRAY));
        } else {
            ui.add_space(20.0);
            ui.label(
                RichText::new("Phone access is off. The desktop app stays local.").color(GRAY),
            );
        }
        if let Some(error) = snapshot.last_error {
            ui.label(RichText::new(error).color(ORANGE));
        }
    }
    pub(crate) fn style_panel(&mut self, ui: &mut egui::Ui) {
        self.cat_gallery(ui);
        ui.add_space(10.0);
        ui.checkbox(&mut self.prefs.reduced_motion, "Reduce motion");
        ui.label(RichText::new("Cikung breathes while idle, bops while music plays, and reacts when you poke. Right-click the cat for quick options.").size(12.0).color(GRAY));
        ui.collapsing("Your last seven days", |ui| {
            for (day, stats) in self.prefs.days.iter().rev().take(7) {
                ui.label(format!(
                    "{day} · {} focus sessions · {} tasks · {} water breaks · {} notes",
                    stats.focus, stats.tasks, stats.water, stats.notes
                ));
            }
            if self.prefs.days.is_empty() {
                ui.label(
                    RichText::new(
                        "Your recap will grow as you use the tools. Everything is stored locally.",
                    )
                    .color(GRAY),
                );
            }
        });
    }
}

fn parse_task_due(value: &str) -> Result<Option<u64>, String> {
    use chrono::TimeZone;
    if value.trim().is_empty() {
        return Ok(None);
    }
    let local = chrono::NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%d %H:%M")
        .map_err(|_| "Use YYYY-MM-DD HH:MM in your local time for the reminder.".to_owned())?;
    let date = chrono::Local
        .from_local_datetime(&local)
        .single()
        .ok_or("That local time is ambiguous or does not exist. Choose another time.")?;
    u64::try_from(date.timestamp())
        .map(Some)
        .map_err(|_| "Choose a reminder date after 1970.".into())
}
fn format_task_due(unix: u64) -> String {
    i64::try_from(unix)
        .ok()
        .and_then(|timestamp| chrono::DateTime::from_timestamp(timestamp, 0))
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| "Reminder date unavailable".into())
}
#[cfg(test)]
mod reminder_tests {
    use super::*;
    #[test]
    fn reminder_dates_validate_and_round_trip_local_minutes() {
        assert_eq!(parse_task_due(" ").unwrap(), None);
        assert!(parse_task_due("tomorrow").is_err());
        assert!(parse_task_due("2026-02-30 18:30").is_err());
        assert!(parse_task_due("1960-01-01 12:00").is_err());
        let due = parse_task_due("2026-10-20 18:30").unwrap().unwrap();
        assert_eq!(format_task_due(due), "2026-10-20 18:30");
    }
}
