use super::*;
use crate::agent_connect::{self, Agent};
impl Cikung {
    pub(crate) fn agent_connections(&mut self, ui: &mut egui::Ui) {
        use crate::desktop_apps::{State, Target};
        ui.horizontal(|ui| {
            for (agent, label) in [
                (Agent::Claude, Target::Claude.label()),
                (Agent::Codex, Target::Codex.label()),
                (Agent::Omp, "Oh My Pi"),
            ] {
                ui.selectable_value(&mut self.prefs.agents.selected, agent, label);
            }
        });
        if self.prefs.agents.selected == Agent::Omp {
            self.agent_cli_connections(ui);
            return;
        }
        let target = if self.prefs.agents.selected == Agent::Claude {
            Target::Claude
        } else {
            Target::Codex
        };
        let app = self.desktop_manager.apps[target.index()].clone();
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            let color = if app.state == State::Ready {
                accent()
            } else {
                muted()
            };
            ui.label(
                RichText::new(format!("● {}", app.state.label()))
                    .size(12.0)
                    .color(color),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.demo && !self.desktop_manager.pending,
                        egui::Button::new("Open app").small(),
                    )
                    .clicked()
                    && let Err(error) = self.desktop_manager.open(target)
                {
                    self.toast("Desktop app", error);
                }
                ui.checkbox(
                    &mut self.prefs.desktop_apps.enabled[target.index()],
                    "Link active chat",
                );
            });
        });
        ui.label(RichText::new(&app.detail).size(11.0).color(muted()));
        if !app.title.is_empty() && !matches!(app.title.as_str(), "ChatGPT" | "Claude") {
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Window: {}", app.title))
                        .size(10.0)
                        .color(muted()),
                )
                .truncate(),
            );
        }
        ui.add_space(7.0);
        egui::Frame::new().fill(Color32::from_gray(14)).corner_radius(10).inner_margin(10).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Active conversation").size(10.0).color(muted()));
            let height = if ui.available_width() > 600.0 { 220.0 } else { 90.0 };
            egui::ScrollArea::vertical().id_salt(("desktop-conversation", target.index())).max_height(height).stick_to_bottom(true).show(ui, |ui| {
                ui.set_min_height(42.0);
                ui.add(egui::Label::new(RichText::new(if app.conversation.is_empty() { "Visible replies will appear here when the app exposes its conversation." } else { &app.conversation }).size(12.0)).wrap().selectable(true));
            });
        });
        ui.add_space(6.0);
        ui.add_enabled(
            !self.desktop_manager.pending,
            egui::TextEdit::multiline(&mut self.desktop_draft)
                .desired_width(f32::INFINITY)
                .desired_rows(2)
                .hint_text("Write a message for the active app chat…"),
        );
        ui.horizontal(|ui| {
            let ready = app.can_send
                && !app.draft_present
                && !self.desktop_manager.pending
                && self.prefs.desktop_apps.enabled[target.index()]
                && !self.demo
                && !self.desktop_draft.trim().is_empty();
            if ui
                .add_enabled(
                    ready,
                    egui::Button::new(if self.desktop_manager.pending {
                        "Sending…"
                    } else {
                        "Send to active chat"
                    }),
                )
                .clicked()
                && let Err(error) = self
                    .desktop_manager
                    .send(target, self.desktop_draft.clone())
            {
                self.toast("Desktop app", error);
            }
        });
        ui.label(RichText::new("Uses your desktop app and its current chat. Messages are sent only when you press Send here.").size(10.0).color(muted()));
    }
    fn agent_cli_connections(&mut self, ui: &mut egui::Ui) {
        let selected = self.prefs.agents.selected;
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("Choose project folder").clicked()
                && let Some(folder) = rfd::FileDialog::new().pick_folder()
            {
                self.prefs.agents.project = folder.to_string_lossy().into_owned();
            }
            ui.add(
                egui::Label::new(
                    RichText::new(if self.prefs.agents.project.is_empty() {
                        "No project chosen"
                    } else {
                        &self.prefs.agents.project
                    })
                    .size(11.0)
                    .color(muted()),
                )
                .truncate(),
            );
        });
        if selected == Agent::Omp {
            ui.checkbox(
                &mut self.prefs.agents.omp_gate,
                "Ask Cikung before every OMP tool (additional approval)",
            );
        }
        if selected == Agent::Codex {
            ui.label(
                RichText::new(agent_connect::codex::REVIEW_NOTICE)
                    .size(11.0)
                    .color(accent()),
            );
        }
        let available = Path::new(&self.prefs.agents.executables[selected.index()]).is_file()
            && (!agent_connect::needs_node(
                selected,
                Path::new(&self.prefs.agents.executables[selected.index()]),
            ) || Path::new(&self.prefs.agents.node).is_file())
            && !self.prefs.agents.project.is_empty()
            && self.bridge.is_some();
        if ui
            .add_enabled(
                available,
                egui::Button::new(format!("Start connected {} terminal", selected.label())),
            )
            .clicked()
            && let Some(bridge) = &self.bridge
        {
            let result = self.agent_manager.launch(
                &self.prefs.agents,
                &bridge.local_endpoint(),
                &bridge.agent_token(),
            );
            self.report(
                result,
                "Terminal opened. Activity will appear when the agent sends its hooks.",
            );
        }
        ui.label(RichText::new("Launches a new connected CLI session. Existing chats and global agent settings stay with their apps.").size(10.0).color(muted()));
        ui.collapsing("Connection settings", |ui| {
            ui.label("Agent executable");
            ui.add(
                egui::TextEdit::singleline(&mut self.prefs.agents.executables[selected.index()])
                    .desired_width(f32::INFINITY),
            );
            if ui.small_button("Browse agent executable").clicked()
                && let Some(file) = rfd::FileDialog::new().pick_file()
            {
                self.prefs.agents.executables[selected.index()] =
                    file.to_string_lossy().into_owned();
            }
            ui.label("Node.js executable (for Claude / Codex hooks)");
            ui.add(
                egui::TextEdit::singleline(&mut self.prefs.agents.node)
                    .desired_width(f32::INFINITY),
            );
            if ui.small_button("Browse Node.js").clicked()
                && let Some(file) = rfd::FileDialog::new().pick_file()
            {
                self.prefs.agents.node = file.to_string_lossy().into_owned();
            }
        });
    }
}
