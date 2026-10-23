use super::*;

#[derive(Default)]
pub(super) struct GameAssistant {
    game: String,
    situation: String,
    spoiler: usize,
    request: Option<u64>,
    answer: Option<Result<String, String>>,
}
const LEVELS: [&str; 3] = [
    "Gentle hint · no story spoilers",
    "Next step · small spoilers",
    "Full solution · spoilers allowed",
];
impl GameAssistant {
    fn prompt(&self) -> Result<String, String> {
        if self.game.trim().is_empty() || self.situation.trim().is_empty() {
            return Err("Enter the game and what you need help with first.".into());
        }
        if self.game.len() > 1024 || self.situation.len() > 24000 {
            return Err("Shorten the game name or situation before sending.".into());
        }
        Ok(format!(
            "Help me with this noncompetitive game.\nGame: {}\nHint level: {}\nMy situation / question: {}\n\nReply in the language of my question. Respect the hint level: a gentle hint must not reveal the full solution or later story. Use only the context I give you; you cannot see my screen or control the game. If the game/version/quest is unclear, ask rather than inventing an answer.",
            self.game.trim(),
            LEVELS[self.spoiler.min(2)],
            self.situation.trim()
        ))
    }
}

impl Cikung {
    pub(crate) fn game_panel(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "A little help on your adventure",
            "For puzzles, quests and exploring noncompetitive games.",
        );
        let busy = self.game.request.is_some();
        let mut changed = false;
        ui.add_enabled_ui(!busy, |ui| {
            ui.label("Game");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.game.game)
                        .desired_width(f32::INFINITY)
                        .char_limit(120)
                        .hint_text("Game name, version or DLC if relevant"),
                )
                .changed();
            ui.label("Where are you stuck?");
            changed |= ui
                .add(
                    egui::TextEdit::multiline(&mut self.game.situation)
                        .desired_width(f32::INFINITY)
                        .desired_rows(3)
                        .char_limit(6000)
                        .hint_text("Describe the quest, puzzle, location or what you have tried…"),
                )
                .changed();
            egui::ComboBox::from_id_salt("game-spoilers")
                .selected_text(LEVELS[self.game.spoiler.min(2)])
                .show_ui(ui, |ui| {
                    for (level, label) in LEVELS.iter().enumerate() {
                        changed |= ui
                            .selectable_value(&mut self.game.spoiler, level, *label)
                            .changed();
                    }
                });
        });
        if changed {
            self.game.answer = None;
        }
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy && !self.demo && self.game.prompt().is_ok(),
                    egui::Button::new("Ask for a hint"),
                )
                .clicked()
            {
                let result = self.request_game_hint();
                if let Err(error) = result {
                    self.game.answer = Some(Err(error));
                }
            }
            if busy
                && ui.button("Cancel").clicked()
                && let Some(id) = self.game.request.take()
            {
                self.pending.remove(&id);
            }
            if ui.small_button("AI settings").clicked() {
                self.open_chat_settings();
            }
        });
        ui.label(
            RichText::new(format!(
                "Uses {} · {}. Your question is sent only when you ask.",
                self.prefs.chat.provider.name(),
                self.prefs.chat.model
            ))
            .size(10.0)
            .color(muted()),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new("Or prepare the question in:")
                    .size(11.0)
                    .color(muted()),
            );
            for (target, label) in [
                (crate::desktop_apps::Target::Claude, "Claude App"),
                (crate::desktop_apps::Target::Codex, "Codex / ChatGPT App"),
            ] {
                if ui
                    .add_enabled(
                        !busy
                            && self.game.prompt().is_ok()
                            && self.desktop_draft.is_empty()
                            && !self.desktop_manager.pending,
                        egui::Button::new(label).small(),
                    )
                    .clicked()
                {
                    self.prepare_game_in_app(target);
                }
            }
        });
        ui.label(RichText::new("Desktop buttons prepare a draft in Agents; review it and press Send there. Existing drafts are kept.").size(10.0).color(muted()));
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Thinking about your next step…");
            });
        }
        if let Some(answer) = &self.game.answer {
            ui.add_space(9.0);
            match answer {
                Ok(text) => {
                    ui.add(egui::Label::new(text).wrap().selectable(true));
                }
                Err(error) => {
                    ui.label(RichText::new(error).color(accent()));
                }
            }
        }
    }

    fn request_game_hint(&mut self) -> Result<(), String> {
        if self.game.request.is_some() {
            return Err("A hint is already being requested.".into());
        }
        let prompt = self.game.prompt()?;
        let id = self.request(Pending::Game {
            prompt: prompt.clone(),
            config: serde_json::to_string(&self.prefs.chat).unwrap_or_default(),
        });
        let result = self.cloud.send_chat(id, self.prefs.chat.clone(), vec![
            ChatMessage { role: "system".into(), content: "You are Cikung, a concise game companion. Follow the player's spoiler preference. Do not claim live screen access or certainty about an unknown game state.".into() },
            ChatMessage { role: "user".into(), content: prompt },
        ]);
        if let Err(error) = result {
            self.pending.remove(&id);
            return Err(error);
        }
        self.game.request = Some(id);
        self.game.answer = None;
        Ok(())
    }

    pub(super) fn finish_game_hint(
        &mut self,
        id: u64,
        prompt: String,
        config: String,
        result: Result<String, String>,
    ) {
        if self.game.request != Some(id) {
            return;
        }
        self.game.request = None;
        if self.game.prompt().as_ref() != Ok(&prompt)
            || config != serde_json::to_string(&self.prefs.chat).unwrap_or_default()
        {
            self.game.answer = Some(Err(
                "The game question or AI settings changed. Ask again for the current context."
                    .into(),
            ));
            return;
        }
        self.game.answer = Some(result);
    }

    fn prepare_game_in_app(&mut self, target: crate::desktop_apps::Target) {
        if !self.desktop_draft.is_empty() || self.desktop_manager.pending {
            return;
        }
        if let Ok(prompt) = self.game.prompt() {
            self.desktop_draft = prompt;
            self.prefs.agents.selected = match target {
                crate::desktop_apps::Target::Claude => crate::agent_connect::Agent::Claude,
                crate::desktop_apps::Target::Codex => crate::agent_connect::Agent::Codex,
            };
            self.open_settings_page(super::settings_pages::SettingsPage::Agents);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hints_require_context_and_default_to_no_story_spoilers() {
        let mut game = GameAssistant::default();
        assert!(game.prompt().is_err());
        game.game = "A puzzle game".into();
        game.situation = "How do I open this door?".into();
        let prompt = game.prompt().unwrap();
        assert!(prompt.contains("Gentle hint · no story spoilers"));
        assert!(prompt.contains("cannot see my screen"));
    }
    #[test]
    fn app_handoff_preserves_existing_drafts_and_does_not_send() {
        let (_ctx, mut app) = super::super::tests::app();
        app.game.game = "Puzzle game".into();
        app.game.situation = "A locked door".into();
        app.desktop_draft = "My unsent message".into();
        app.prepare_game_in_app(crate::desktop_apps::Target::Claude);
        assert_eq!(app.desktop_draft, "My unsent message");
        app.desktop_draft.clear();
        app.prepare_game_in_app(crate::desktop_apps::Target::Codex);
        assert!(app.desktop_draft.contains("A locked door"));
        assert_eq!(
            app.settings_tab,
            super::super::settings_pages::SettingsPage::Agents
        );
        assert!(!app.desktop_manager.pending);
    }
    #[test]
    fn canceled_or_changed_questions_cannot_receive_stale_answers() {
        let (_ctx, mut app) = super::super::tests::app();
        app.game.game = "Puzzle game".into();
        app.game.situation = "A door".into();
        let prompt = app.game.prompt().unwrap();
        let config = serde_json::to_string(&app.prefs.chat).unwrap();
        app.finish_game_hint(
            4,
            prompt.clone(),
            config.clone(),
            Ok("Canceled reply".into()),
        );
        assert!(app.game.answer.is_none());
        app.game.request = Some(4);
        app.game.situation = "A different room".into();
        app.finish_game_hint(4, prompt, config, Ok("Wrong room solution".into()));
        assert!(matches!(app.game.answer, Some(Err(_))));
        assert!(app.game.request.is_none());
    }
    #[test]
    fn a_game_reply_does_not_replace_or_unlock_normal_chat() {
        let (ctx, mut app) = super::super::tests::app();
        app.game.game = "Puzzle game".into();
        app.game.situation = "A door".into();
        let prompt = app.game.prompt().unwrap();
        let config = serde_json::to_string(&app.prefs.chat).unwrap();
        let id = app.request(Pending::Game { prompt, config });
        app.game.request = Some(id);
        app.chat_busy = true;
        app.chat.push(ChatMessage {
            role: "user".into(),
            content: "An unrelated chat".into(),
        });
        app.cloud.inject_event(CloudEvent::Chat {
            request_id: id,
            result: Ok("Look for a symbol nearby.".into()),
        });
        app.tick(&ctx);
        assert_eq!(
            app.game.answer.as_ref().unwrap().as_ref().unwrap(),
            "Look for a symbol nearby."
        );
        assert!(app.game.request.is_none());
        assert!(app.chat_busy);
        assert_eq!(app.chat[0].content, "An unrelated chat");
    }
}
