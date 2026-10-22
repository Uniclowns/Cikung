use super::*;
use crate::cats::Breed;
pub(super) const FEATURES: [Page; 12] = [
    Page::Music,
    Page::Agents,
    Page::Focus,
    Page::Files,
    Page::Tasks,
    Page::Notes,
    Page::Lyrics,
    Page::Water,
    Page::Chat,
    Page::Game,
    Page::Services,
    Page::Phone,
];
impl Cikung {
    fn default_breed(page: Page) -> Breed {
        match page {
            Page::Overview | Page::Music => Breed::Orange,
            Page::Agents | Page::Tasks | Page::Services => Breed::Calico,
            Page::Focus | Page::Phone => Breed::Siamese,
            Page::Files => Breed::Tuxedo,
            Page::Water | Page::Chat | Page::Game => Breed::Silver,
            Page::Notes | Page::Lyrics => Breed::White,
            Page::Style | Page::Settings => Breed::Orange,
        }
    }
    pub(crate) fn breed_for(&self, page: Page) -> Breed {
        if !self.prefs.auto_cat {
            return self.prefs.cat_breed;
        }
        if page == Page::Agents {
            return self.prefs.agent_cats[self.prefs.agents.selected.index()];
        }
        self.prefs
            .feature_cats
            .get(&page)
            .copied()
            .unwrap_or_else(|| Self::default_breed(page))
    }
    pub(crate) fn active_breed(&self) -> Breed {
        self.breed_for(if matches!(self.page, Page::Style | Page::Settings) {
            self.last_feature
        } else {
            self.page
        })
    }
    pub(crate) fn active_outfit(&self) -> Outfit {
        self.outfit_for(if matches!(self.page, Page::Style | Page::Settings) {
            self.last_feature
        } else {
            self.page
        })
    }
    pub(super) fn outfit_for(&self, page: Page) -> Outfit {
        if self.prefs.outfit != Outfit::Auto {
            return self.prefs.outfit;
        }
        match page {
            Page::Music | Page::Lyrics => Outfit::Headphones,
            Page::Focus | Page::Water | Page::Services => Outfit::Bow,
            Page::Agents | Page::Chat | Page::Game => Outfit::Wizard,
            _ => Outfit::Plain,
        }
    }
    pub(crate) fn paint_outfit(ui: &egui::Ui, cat_rect: Rect, outfit: Outfit) {
        let size = cat_rect.width();
        match outfit {
            Outfit::Plain | Outfit::Auto => (),
            Outfit::Headphones => {
                let y = cat_rect.center().y;
                for x in [cat_rect.left() + 2.0, cat_rect.right() - 2.0] {
                    ui.painter().rect_filled(
                        Rect::from_center_size(Pos2::new(x, y), Vec2::new(size * 0.12, size * 0.3)),
                        3,
                        Color32::from_rgb(96, 90, 125),
                    );
                }
            }
            Outfit::Bow => {
                let c = cat_rect.min + Vec2::new(size * 0.7, size * 0.22);
                let r = size * 0.12;
                for x in [-r, r] {
                    ui.painter().circle_filled(
                        c + Vec2::new(x, 0.0),
                        r,
                        Color32::from_rgb(235, 125, 160),
                    );
                }
                ui.painter().circle_filled(c, 3.0, WHITE);
            }
            Outfit::Wizard => {
                let c = cat_rect.center_top();
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + Vec2::new(-size * 0.25, size * 0.2),
                        c + Vec2::new(size * 0.25, size * 0.2),
                        c + Vec2::new(0.0, -size * 0.08),
                    ],
                    Color32::from_rgb(110, 90, 185),
                    Stroke::NONE,
                ));
            }
        }
    }
    fn feature_status(&self, page: Page) -> String {
        match page {
            Page::Music => self
                .session
                .as_ref()
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "Nothing playing".into()),
            Page::Agents => {
                use crate::{agent_connect::Agent, desktop_apps::Target};
                if self.prefs.agents.selected == Agent::Omp {
                    format!(
                        "{} connected sessions",
                        self.bridge_view.as_ref().map_or(0, |s| s.sessions.len())
                    )
                } else {
                    let target = if self.prefs.agents.selected == Agent::Claude {
                        Target::Claude
                    } else {
                        Target::Codex
                    };
                    format!(
                        "{} · {}",
                        target.label(),
                        self.desktop_manager.apps[target.index()].state.label()
                    )
                }
            }
            Page::Focus => {
                if self.tools.focus.running {
                    self.tools.focus.formatted_remaining()
                } else {
                    "Ready when you are".into()
                }
            }
            Page::Tasks => format!(
                "{} to do",
                self.tools.tasks.iter().filter(|t| !t.done).count()
            ),
            Page::Files => format!("{} saved references", self.tools.shelf.len()),
            Page::Notes => format!("{} local notes", self.tools.notes.len()),
            Page::Water => format!(
                "{} / {} ml today",
                self.tools.water.ml_today,
                self.tools.water.goal_ml.clamp(250, 10000)
            ),
            Page::Lyrics => "Follow the words".into(),
            Page::Chat => "Ask your AI".into(),
            Page::Game => "Hints without the spoilers".into(),
            Page::Services => "Connected accounts".into(),
            Page::Phone => "Pair your phone".into(),
            _ => String::new(),
        }
    }
    pub(crate) fn overview(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Your little team",
            "Different cats, one Cikung. Pick a feature to spend time with its cat.",
        );
        let width = (ui.available_width() - 10.0) / 2.0;
        for row in FEATURES.chunks(2) {
            ui.horizontal(|ui| {
                for page in row {
                    let breed = self.breed_for(*page);
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, 57.0), Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        10,
                        Color32::from_gray(if response.hovered() { 29 } else { 17 }),
                    );
                    let image =
                        Rect::from_min_size(rect.min + Vec2::new(10.0, 8.0), Vec2::splat(40.0));
                    ui.painter().image(
                        self.cat_textures[breed.index()].id(),
                        image,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    Self::paint_outfit(ui, image, self.outfit_for(*page));
                    let text = Rect::from_min_max(
                        rect.min + Vec2::new(60.0, 8.0),
                        rect.max - Vec2::new(8.0, 4.0),
                    );
                    let mut label = ui.new_child(
                        egui::UiBuilder::new()
                            .max_rect(text)
                            .layout(egui::Layout::top_down(egui::Align::Min)),
                    );
                    label.set_clip_rect(text.intersect(ui.clip_rect()));
                    label.label(RichText::new(page.label()).size(12.0));
                    label.add(
                        egui::Label::new(
                            RichText::new(self.feature_status(*page))
                                .size(10.0)
                                .color(GRAY),
                        )
                        .truncate(),
                    );
                    if response.clicked() {
                        self.page = *page;
                        self.more_open = false;
                        self.hover.interact();
                    }
                }
            });
            ui.add_space(3.0);
        }
    }
    pub(crate) fn cat_gallery(&mut self, ui: &mut egui::Ui) {
        heading(
            ui,
            "Meet your Cikungs",
            "Six kinds of cat, all at home in one notch.",
        );
        ui.checkbox(
            &mut self.prefs.auto_cat,
            "Let each feature have its own cat",
        );
        let width = (ui.available_width() - 16.0) / 3.0;
        for row in Breed::ALL.chunks(3) {
            ui.horizontal(|ui| {
                for breed in row {
                    let selected = !self.prefs.auto_cat && self.prefs.cat_breed == *breed;
                    let (rect, response) =
                        ui.allocate_exact_size(Vec2::new(width, 83.0), Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        10,
                        if selected {
                            Color32::from_rgb(61, 39, 22)
                        } else {
                            Color32::from_gray(if response.hovered() { 29 } else { 17 })
                        },
                    );
                    let image = Rect::from_center_size(
                        rect.center_top() + Vec2::new(0.0, 30.0),
                        Vec2::splat(46.0),
                    );
                    ui.painter().image(
                        self.cat_textures[breed.index()].id(),
                        image,
                        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    ui.painter().text(
                        rect.center_bottom() - Vec2::new(0.0, 16.0),
                        egui::Align2::CENTER_CENTER,
                        breed.label(),
                        egui::FontId::proportional(11.0),
                        if selected { ORANGE } else { WHITE },
                    );
                    if response
                        .on_hover_text("Use this cat throughout Cikung")
                        .clicked()
                    {
                        self.prefs.cat_breed = *breed;
                        self.prefs.auto_cat = false;
                    }
                }
            });
            ui.add_space(3.0);
        }
        ui.horizontal(|ui| {
            ui.label("Accessory");
            egui::ComboBox::from_id_salt("cat-accessory")
                .selected_text(self.prefs.outfit.label())
                .show_ui(ui, |ui| {
                    for outfit in Outfit::ALL {
                        ui.selectable_value(&mut self.prefs.outfit, outfit, outfit.label());
                    }
                });
        });
        ui.collapsing("Choose a cat for each feature", |ui| {
            ui.label(
                RichText::new("These choices apply while feature cats are enabled.")
                    .size(10.0)
                    .color(GRAY),
            );
            for page in FEATURES {
                if page == Page::Agents {
                    continue;
                }
                let mut breed = self
                    .prefs
                    .feature_cats
                    .get(&page)
                    .copied()
                    .unwrap_or_else(|| Self::default_breed(page));
                ui.horizontal(|ui| {
                    ui.add_sized([92.0, 20.0], egui::Label::new(page.label()));
                    breed_picker(ui, format!("feature-cat-{page:?}"), &mut breed);
                });
                self.prefs.feature_cats.insert(page, breed);
            }
            for agent in crate::agent_connect::Agent::ALL {
                ui.horizontal(|ui| {
                    ui.add_sized([92.0, 20.0], egui::Label::new(agent.companion_label()));
                    breed_picker(
                        ui,
                        format!("agent-cat-{agent:?}"),
                        &mut self.prefs.agent_cats[agent.index()],
                    );
                });
            }
        });
    }
}
fn breed_picker(ui: &mut egui::Ui, id: String, breed: &mut Breed) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(breed.label())
        .show_ui(ui, |ui| {
            for option in Breed::ALL {
                ui.selectable_value(breed, option, option.label());
            }
        });
}
