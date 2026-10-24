use crate::{
    bridge::{Bridge, BridgeSnapshot},
    cloud::{
        ChatConfig, ChatMessage, CloudClient, CloudEvent, Service, ServiceCard, ServiceConfig,
    },
    dictation::DictationWorker,
    media::{MediaCommand, MediaSession, PlaybackState},
    monitor::{Monitor, Snapshot, choose_session},
    native,
    notch::{self, HoverIntent, HoverPolicy, Motion},
    recording::RecordingWorker,
    tools::{Lyrics, Toolkit},
};
use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, Stroke, Vec2};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
    time::{Duration, Instant},
};
mod agents;
mod auto_lyrics;
mod cat_gallery;
mod game;
mod music;
mod notch_ui;
mod settings_pages;
mod settings_window;

pub(crate) const WHITE: Color32 = Color32::from_rgb(245, 245, 247);
pub(crate) const GRAY: Color32 = Color32::from_rgb(145, 145, 152);
pub(crate) const ORANGE: Color32 = Color32::from_rgb(255, 169, 76);

/// Notch colour theme, chosen in Settings → General.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Theme {
    /// Black that warms into the album cover's colour (the original look).
    #[default]
    Album,
    Black,
    White,
    Orange,
}
impl Theme {
    pub const ALL: [Self; 4] = [Self::Album, Self::Black, Self::White, Self::Orange];
    pub fn label(self) -> &'static str {
        match self {
            Self::Album => "Album colour",
            Self::Black => "Pure black",
            Self::White => "White",
            Self::Orange => "Orange",
        }
    }
    /// Surface, main text, secondary text and accent.
    fn colors(self) -> [Color32; 4] {
        match self {
            Self::Album | Self::Black => [Color32::BLACK, WHITE, GRAY, ORANGE],
            Self::White => [
                Color32::from_rgb(246, 246, 248),
                Color32::from_rgb(22, 22, 26),
                Color32::from_rgb(108, 108, 116),
                Color32::from_rgb(214, 106, 0),
            ],
            Self::Orange => [
                Color32::from_rgb(255, 142, 38),
                Color32::from_rgb(30, 16, 4),
                Color32::from_rgb(112, 58, 14),
                Color32::from_rgb(255, 250, 240),
            ],
        }
    }
}

/// Theme in effect while drawing: the notch uses the chosen one, Settings stays dark.
static PAINTING: AtomicU8 = AtomicU8::new(0);
pub(crate) fn set_painting_theme(theme: Theme) {
    PAINTING.store(theme as u8, Ordering::Relaxed);
}
fn painting() -> Theme {
    Theme::ALL[usize::from(PAINTING.load(Ordering::Relaxed)) % Theme::ALL.len()]
}
pub(crate) fn paper() -> Color32 {
    painting().colors()[0]
}
pub(crate) fn ink() -> Color32 {
    painting().colors()[1]
}
pub(crate) fn muted() -> Color32 {
    painting().colors()[2]
}
pub(crate) fn accent() -> Color32 {
    painting().colors()[3]
}
/// Hover and card wash: the text colour at low alpha, readable on light and dark themes.
pub(crate) fn veil(alpha: u8) -> Color32 {
    let c = ink();
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha)
}
/// Light themes swap the dark widget fills for washes of their text colour.
fn theme_visuals(ui: &mut egui::Ui, theme: Theme) {
    if matches!(theme, Theme::Album | Theme::Black) {
        return;
    }
    let [paper, ink, _, _] = theme.colors();
    let wash = |a: u8| Color32::from_rgba_unmultiplied(ink.r(), ink.g(), ink.b(), a);
    let v = ui.visuals_mut();
    v.override_text_color = Some(ink);
    v.panel_fill = paper;
    v.window_fill = paper;
    v.extreme_bg_color = wash(14);
    v.faint_bg_color = wash(8);
    v.selection.bg_fill = wash(60);
    v.widgets.noninteractive.bg_stroke.color = wash(40);
    for (widget, alpha) in [
        (&mut v.widgets.noninteractive, 0),
        (&mut v.widgets.inactive, 22),
        (&mut v.widgets.hovered, 38),
        (&mut v.widgets.active, 56),
        (&mut v.widgets.open, 38),
    ] {
        widget.fg_stroke.color = ink;
        if alpha > 0 {
            widget.bg_fill = wash(alpha);
            widget.weak_bg_fill = wash(alpha);
        }
    }
}

pub struct Launch {
    pub demo: bool,
    pub capture: Option<String>,
    pub expanded: bool,
    pub page: Option<String>,
    pub audit: Option<String>,
    pub startup_warning: Option<String>,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Page {
    #[default]
    Overview,
    Music,
    Lyrics,
    Focus,
    Files,
    Tasks,
    Water,
    Notes,
    Agents,
    Chat,
    Game,
    Services,
    Phone,
    Style,
    Settings,
}
impl Page {
    /// Pages the ‹ › arrows and dots cycle through, like Nusa's one-tool-per-page notch.
    pub const CAROUSEL: [Self; 8] = [
        Self::Overview,
        Self::Music,
        Self::Focus,
        Self::Water,
        Self::Files,
        Self::Tasks,
        Self::Notes,
        Self::Agents,
    ];
    pub fn height(self) -> f32 {
        match self {
            Self::Music => 240.0,
            Self::Focus | Self::Files | Self::Tasks | Self::Water => 380.0,
            Self::Style => 460.0,
            Self::Overview => 460.0,
            Self::Lyrics
            | Self::Agents
            | Self::Chat
            | Self::Game
            | Self::Services
            | Self::Phone
            | Self::Notes => 440.0,
            Self::Settings => 460.0,
        }
    }
    pub const ALL: [Self; 15] = [
        Self::Overview,
        Self::Music,
        Self::Lyrics,
        Self::Focus,
        Self::Files,
        Self::Tasks,
        Self::Water,
        Self::Notes,
        Self::Agents,
        Self::Chat,
        Self::Game,
        Self::Services,
        Self::Phone,
        Self::Style,
        Self::Settings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Music => "Music",
            Self::Lyrics => "Lyrics",
            Self::Focus => "Focus",
            Self::Files => "Files",
            Self::Tasks => "Tasks",
            Self::Water => "Water",
            Self::Notes => "Notes",
            Self::Agents => "Agents",
            Self::Chat => "Chat",
            Self::Game => "Game",
            Self::Services => "Services",
            Self::Phone => "Phone",
            Self::Style => "Cats",
            Self::Settings => "Settings",
        }
    }
    pub fn parse(text: &str) -> Self {
        if text.eq_ignore_ascii_case("style") {
            return Self::Style;
        }
        Self::ALL
            .into_iter()
            .find(|p| p.label().eq_ignore_ascii_case(text))
            .unwrap_or_default()
    }
}
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Outfit {
    #[default]
    Auto,
    Plain,
    Headphones,
    Bow,
    Wizard,
}
impl Outfit {
    pub const ALL: [Self; 5] = [
        Self::Auto,
        Self::Plain,
        Self::Headphones,
        Self::Bow,
        Self::Wizard,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Match the feature",
            Self::Plain => "Classic",
            Self::Headphones => "Headphones",
            Self::Bow => "Little bow",
            Self::Wizard => "Wizard",
        }
    }
}
#[derive(Default, Serialize, Deserialize)]
pub(crate) struct DailyStats {
    pub focus: u32,
    pub tasks: u32,
    pub water: u32,
    pub notes: u32,
    pub agent_steps: u32,
}
#[derive(Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Preferences {
    pub auto_cat: bool,
    pub cat_breed: crate::cats::Breed,
    pub feature_cats: BTreeMap<Page, crate::cats::Breed>,
    pub agent_cats: [crate::cats::Breed; 3],
    pub agents: crate::agent_connect::Settings,
    pub desktop_apps: crate::desktop_apps::Settings,
    pub reduced_motion: bool,
    pub theme: Theme,
    pub always_on_top: bool,
    pub hover_open: bool,
    pub music_on_hover: bool,
    pub auto_lyrics: bool,
    pub bluetooth_notifications: bool,
    /// Battery, internet and Caps Lock heads-ups in the notch.
    pub system_notices: bool,
    /// Nusa's minimal notch: only the cat and the time while nothing plays.
    pub minimal_notch: bool,
    pub whatsapp_notifications: bool,
    pub whatsapp_preview: bool,
    pub agent_completion_sound: bool,
    pub agent_completion_volume: u8,
    pub auto_collapse: bool,
    /// Off: the notch stays in front of fullscreen apps (a YouTube video). On: it tucks away.
    /// Renamed from `hide_in_fullscreen`, so every profile starts with the notch in front.
    pub tuck_in_fullscreen: bool,
    pub display: String,
    pub shortcut: String,
    pub companion_shortcut_initialized: bool,
    /// Sign-in startup has been turned on once; switching it off afterwards is kept.
    pub startup_initialized: bool,
    pub outfit: Outfit,
    pub chat: ChatConfig,
    pub services: Vec<ServiceConfig>,
    pub days: BTreeMap<String, DailyStats>,
    pub last_focus_count: u64,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            auto_cat: true,
            cat_breed: crate::cats::Breed::Orange,
            feature_cats: BTreeMap::new(),
            agent_cats: [
                crate::cats::Breed::Calico,
                crate::cats::Breed::Silver,
                crate::cats::Breed::Tuxedo,
            ],
            agents: crate::agent_connect::Settings::default(),
            desktop_apps: crate::desktop_apps::Settings::default(),
            reduced_motion: false,
            theme: Theme::default(),
            always_on_top: true,
            hover_open: true,
            music_on_hover: true,
            auto_lyrics: true,
            bluetooth_notifications: true,
            system_notices: true,
            minimal_notch: true,
            whatsapp_notifications: true,
            whatsapp_preview: false,
            agent_completion_sound: true,
            agent_completion_volume: 100,
            auto_collapse: true,
            tuck_in_fullscreen: false,
            display: String::new(),
            shortcut: "C".into(),
            companion_shortcut_initialized: false,
            startup_initialized: false,
            outfit: Outfit::Auto,
            chat: ChatConfig::default(),
            services: Service::ALL
                .into_iter()
                .map(|service| ServiceConfig {
                    service,
                    ..Default::default()
                })
                .collect(),
            days: BTreeMap::new(),
            last_focus_count: 0,
        }
    }
}
pub(crate) enum Pending {
    Game { prompt: String, config: String },
    Chat { generation: u64, config: String },
    Summary(u64),
    Transcription { source: String, path: PathBuf },
    Service { service: Service, config: String },
    Lyrics { generation: u64, identity: String },
}
pub(crate) struct Notice {
    pub title: String,
    pub text: String,
    pub until: Instant,
}

pub struct Cikung {
    requests: crate::instance::Requests,
    last_feature: Page,
    was_open: bool,
    settings_open: bool,
    settings_tab: settings_pages::SettingsPage,
    settings_focus: bool,
    settings_save: bool,
    capture_viewport: egui::ViewportId,
    agent_manager: crate::agent_connect::Manager,
    desktop_manager: crate::desktop_apps::Manager,
    completion_player: crate::completion_sound::Player,
    desktop_draft: String,
    cat_textures: [egui::TextureHandle; 6],
    pub(crate) prefs: Preferences,
    pub(crate) tools: Toolkit,
    monitor: Option<Monitor>,
    bluetooth: Option<crate::bluetooth::Monitor>,
    system: Option<crate::system_notices::Monitor>,
    /// System output level and mute for the Music page's slider, and when they were read.
    volume: Option<(f32, bool)>,
    volume_checked: Option<Instant>,
    whatsapp: Option<crate::whatsapp::Monitor>,
    notification_access: Option<std::sync::mpsc::Receiver<Result<crate::whatsapp::Access, String>>>,
    game: game::GameAssistant,
    pub(crate) snapshot: Snapshot,
    pub(crate) selected: Option<String>,
    previous: Option<String>,
    pub(crate) session: Option<MediaSession>,
    pub(crate) cloud: CloudClient,
    pub(crate) bridge: Option<Bridge>,
    pub(crate) bridge_view: Option<BridgeSnapshot>,
    pub(crate) dictation: DictationWorker,
    pub(crate) recorder: RecordingWorker,
    pub(crate) shell: Option<crate::shell::Shell>,
    pub(crate) autostart: bool,
    pub(crate) vlc_password: String,
    pub(crate) artwork: Option<(String, egui::TextureHandle)>,
    /// Cover colour the notch fades into while music is shown; black without a cover.
    pub(crate) tint: Color32,
    pub(crate) page: Page,
    pub(crate) expanded: bool,
    pub(crate) demo: bool,
    started: Instant,
    motion: Motion,
    hover: HoverIntent,
    content_motion: notch::ContentTransition,
    pub(crate) file_hover: Option<(Page, bool, bool)>,
    more_open: bool,
    last_shape: Option<[i32; 5]>,
    /// When idle memory was last handed back during the current collapsed rest.
    last_trim: Option<f64>,
    last_anchor: Instant,
    fullscreen: bool,
    pub(crate) handle: Option<usize>,
    pub(crate) shortcut: Option<native::GlobalShortcut>,
    pub(crate) notices: VecDeque<Notice>,
    pub(crate) chat: Vec<ChatMessage>,
    pub(crate) draft: String,
    pub(crate) chat_busy: bool,
    pub(crate) pending: HashMap<u64, Pending>,
    next_request: u64,
    chat_generation: u64,
    active_chat: Option<u64>,
    lyrics_generation: u64,
    active_lyrics: Option<u64>,
    lyrics_identity: String,
    auto_lyrics: auto_lyrics::AutomaticLyrics,
    service_requests: HashMap<String, u64>,
    service_fingerprints: HashMap<String, String>,
    pub(crate) service_index: usize,
    pub(crate) service_cards: HashMap<String, Vec<ServiceCard>>,
    pub(crate) secret_input: String,
    pub(crate) service_secret: String,
    pub(crate) task_input: String,
    pub(crate) task_due_input: String,
    pub(crate) task_edit: Option<u64>,
    pub(crate) note_title: String,
    pub(crate) note_text: String,
    pub(crate) note_selected: Option<u64>,
    pub(crate) recording_files: Vec<PathBuf>,
    pub(crate) record_started: Option<Instant>,
    pub(crate) dictation_to_notes: bool,
    pub(crate) lyrics_text: String,
    pub(crate) lyrics_for: String,
    pub(crate) lyrics_busy: bool,
    pub(crate) show_agent_setup: bool,
    pub(crate) show_agent_token: bool,
    pub(crate) selected_agent: Option<String>,
    pub(crate) qr: Option<(String, egui::TextureHandle)>,
    pub(crate) poke_until: Instant,
    capture: Option<String>,
    audit: Option<String>,
    capture_requested: bool,
}

impl Cikung {
    pub fn new(cc: &eframe::CreationContext<'_>, launch: Launch) -> Self {
        let cat_textures = crate::cats::Breed::ALL.map(|breed| {
            let rgba = image::load_from_memory(breed.bytes())
                .expect("Cikung breed image")
                .into_rgba8();
            cc.egui_ctx.load_texture(
                breed.label(),
                egui::ColorImage::from_rgba_unmultiplied(
                    [rgba.width() as usize, rgba.height() as usize],
                    rgba.as_raw(),
                ),
                egui::TextureOptions::LINEAR,
            )
        });
        let mut prefs = if launch.demo || launch.capture.is_some() || launch.audit.is_some() {
            Preferences::default()
        } else {
            cc.storage
                .and_then(|s| eframe::get_value(s, "cikung-v2-settings"))
                .unwrap_or_default()
        };
        let first_startup = !std::mem::replace(&mut prefs.startup_initialized, true);
        if !(launch.demo || launch.capture.is_some() || launch.audit.is_some()) {
            native::install_menu_bar_item(&cc.egui_ctx);
        }
        let tools = if launch.demo || launch.capture.is_some() || launch.audit.is_some() {
            Toolkit::default()
        } else {
            cc.storage
                .and_then(|s| eframe::get_value(s, "cikung-v2-tools"))
                .unwrap_or_default()
        };
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        style.visuals.override_text_color = Some(WHITE);
        style.visuals.panel_fill = Color32::BLACK;
        style.visuals.window_fill = Color32::BLACK;
        style.visuals.widgets.inactive.bg_fill = Color32::from_gray(18);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_gray(18);
        style.visuals.widgets.hovered.bg_fill = Color32::from_gray(36);
        style.visuals.widgets.active.bg_fill = Color32::from_gray(45);
        style.visuals.selection.bg_fill = Color32::from_rgb(105, 66, 30);
        style.spacing.item_spacing = Vec2::new(8.0, 7.0);
        style.spacing.button_padding = Vec2::new(10.0, 6.0);
        cc.egui_ctx.set_style(style);
        // Figtree Bold everywhere: free (OFL) lookalike of Spotify's proprietary Spotify Mix.
        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "Figtree Bold".into(),
            egui::FontData::from_static(include_bytes!("../assets/fonts/Figtree-Bold.ttf")).into(),
        );
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "Figtree Bold".into());
        cc.egui_ctx.set_fonts(fonts);
        let snapshot = Snapshot {
            sessions: if launch.demo {
                vec![MediaSession {
                    id: "demo".into(),
                    source: "Spotify".into(),
                    title: "A little sunshine".into(),
                    artist: "Cikung's listening room".into(),
                    album: String::new(),
                    state: PlaybackState::Playing,
                    position_secs: 73.0,
                    duration_secs: 226.0,
                    shuffle: Some(false),
                    repeat: Some(false),
                    can_seek: true,
                    artwork: None,
                }]
            } else {
                vec![]
            },
            error: None,
            control_error: None,
            ready: launch.demo,
            updated: Instant::now(),
        };
        let monitor = (!launch.demo).then(|| Monitor::start(cc.egui_ctx.clone()));
        let mut notices = VecDeque::new();
        if let Some(warning) = &launch.startup_warning {
            notices.push_back(Notice {
                title: "Saved data".into(),
                text: warning.clone(),
                until: Instant::now() + Duration::from_secs(20),
            });
        }
        let bridge = if launch.demo || launch.capture.is_some() {
            None
        } else {
            match Bridge::start(cc.egui_ctx.clone()) {
                Ok(b) => Some(b),
                Err(e) => {
                    notices.push_back(Notice {
                        title: "Agent connection".into(),
                        text: e,
                        until: Instant::now() + Duration::from_secs(10),
                    });
                    None
                }
            }
        };
        let shortcut = if launch.demo || launch.capture.is_some() {
            None
        } else {
            native::GlobalShortcut::new(
                cc.egui_ctx.clone(),
                prefs
                    .shortcut
                    .bytes()
                    .next()
                    .unwrap_or(b'C')
                    .to_ascii_uppercase(),
            )
            .ok()
        };
        let requested = launch.page.as_deref().map(Page::parse).unwrap_or_default();
        let hub_page = launch
            .page
            .as_deref()
            .and_then(settings_pages::SettingsPage::from_request);
        let settings_open = hub_page.is_some();
        let page = if settings_open {
            Page::Overview
        } else {
            requested
        };
        let expanded = launch.expanded || (launch.page.is_some() && !settings_open);
        let collapsed = notch::COLLAPSED;
        Self {
            requests: crate::instance::Requests::new(
                launch.demo || launch.capture.is_some() || launch.audit.is_some(),
            ),
            last_feature: if matches!(page, Page::Style | Page::Settings) {
                Page::Overview
            } else {
                page
            },
            was_open: expanded,
            settings_open,
            settings_tab: hub_page.unwrap_or_default(),
            settings_focus: settings_open,
            settings_save: false,
            capture_viewport: if settings_open {
                settings_window::viewport_id()
            } else {
                egui::ViewportId::ROOT
            },
            agent_manager: crate::agent_connect::Manager::new(),
            desktop_manager: crate::desktop_apps::Manager::new(
                cc.egui_ctx.clone(),
                !launch.demo && launch.capture.is_none() && launch.audit.is_none(),
                prefs.desktop_apps.enabled,
            ),
            completion_player: crate::completion_sound::Player::default(),
            desktop_draft: String::new(),
            cat_textures,
            prefs,
            tools,
            monitor,
            bluetooth: None,
            system: None,
            volume: None,
            volume_checked: None,
            whatsapp: None,
            notification_access: None,
            game: game::GameAssistant::default(),
            snapshot,
            selected: None,
            previous: None,
            session: None,
            cloud: CloudClient::new(),
            bridge,
            bridge_view: None,
            dictation: DictationWorker::new(),
            recorder: RecordingWorker::new(),
            shell: if launch.demo || launch.capture.is_some() {
                None
            } else {
                crate::shell::Shell::start(cc.egui_ctx.clone()).ok()
            },
            autostart: if launch.demo || launch.capture.is_some() || launch.audit.is_some() {
                false
            } else {
                crate::shell::ensure_autostart(first_startup)
            },
            vlc_password: String::new(),
            artwork: None,
            tint: Color32::BLACK,
            page,
            expanded,
            demo: launch.demo,
            started: Instant::now(),
            motion: Motion::new(if expanded {
                Vec2::new(notch::OPEN_WIDTH, page.height())
            } else {
                collapsed
            }),
            hover: HoverIntent::default(),
            content_motion: notch::ContentTransition::default(),
            file_hover: None,
            more_open: launch.page.as_deref() == Some("more"),
            last_shape: None,
            last_trim: None,
            last_anchor: Instant::now() - Duration::from_secs(2),
            fullscreen: false,
            handle: native::window_handle(cc),
            shortcut,
            notices,
            chat: vec![],
            draft: String::new(),
            chat_busy: false,
            pending: HashMap::new(),
            next_request: 1,
            chat_generation: 0,
            active_chat: None,
            lyrics_generation: 0,
            active_lyrics: None,
            lyrics_identity: String::new(),
            auto_lyrics: auto_lyrics::AutomaticLyrics::default(),
            service_requests: HashMap::new(),
            service_fingerprints: HashMap::new(),
            service_index: 0,
            service_cards: HashMap::new(),
            secret_input: String::new(),
            service_secret: String::new(),
            task_input: String::new(),
            task_due_input: String::new(),
            task_edit: None,
            note_title: String::new(),
            note_text: String::new(),
            note_selected: None,
            recording_files: vec![],
            record_started: None,
            dictation_to_notes: false,
            lyrics_text: String::new(),
            lyrics_for: String::new(),
            lyrics_busy: false,
            show_agent_setup: false,
            show_agent_token: false,
            selected_agent: None,
            qr: None,
            poke_until: Instant::now(),
            capture: launch.capture,
            audit: launch.audit,
            capture_requested: false,
        }
    }
    pub(crate) fn request(&mut self, kind: Pending) -> u64 {
        let id = self.next_request;
        self.next_request += 1;
        self.pending.insert(id, kind);
        id
    }
    pub(crate) fn toast(&mut self, title: impl Into<String>, text: impl Into<String>) {
        self.toast_for(title, text, Duration::from_secs(7));
    }
    pub(crate) fn toast_for(
        &mut self,
        title: impl Into<String>,
        text: impl Into<String>,
        shown: Duration,
    ) {
        self.notices.push_back(Notice {
            title: title.into(),
            text: text.into(),
            until: Instant::now() + shown,
        });
        while self.notices.len() > 8 {
            self.notices.pop_front();
        }
    }
    pub(crate) fn report(&mut self, result: Result<(), String>, ok: &str) {
        match result {
            Ok(()) => self.toast("Cikung", ok),
            Err(e) => self.toast("Couldn't finish", e),
        }
    }
    pub(crate) fn stats(&mut self) -> &mut DailyStats {
        let day = chrono::Local::now().format("%Y-%m-%d").to_string();
        self.prefs.days.entry(day).or_default()
    }
    pub(crate) fn control(&mut self, command: MediaCommand) {
        if !self.demo
            && let (Some(monitor), Some(session)) = (&self.monitor, &self.session)
            && let Err(e) = monitor.control(&session.id, command)
        {
            self.toast("Playback", e);
        }
    }
    pub(crate) fn position(&self) -> f64 {
        self.session
            .as_ref()
            .map(|s| {
                let p = s.position_secs
                    + if s.state == PlaybackState::Playing {
                        self.snapshot.updated.elapsed().as_secs_f64().min(3.0)
                    } else {
                        0.0
                    };
                if s.duration_secs > 0.0 {
                    p.clamp(0.0, s.duration_secs)
                } else {
                    p.max(0.0)
                }
            })
            .unwrap_or_default()
    }
    pub(crate) fn reset_chat(&mut self) {
        self.chat_generation = self.chat_generation.wrapping_add(1);
        self.active_chat = None;
        self.chat_busy = false;
        self.chat.clear();
        self.draft.clear();
        self.pending
            .retain(|_, kind| !matches!(kind, Pending::Chat { .. }));
    }
    fn track_identity(session: Option<&MediaSession>) -> String {
        session
            .map(|s| format!("{}\n{}", s.title, s.artist))
            .unwrap_or_default()
    }
    fn invalidate_lyrics(&mut self) {
        self.lyrics_generation = self.lyrics_generation.wrapping_add(1);
        self.active_lyrics = None;
        self.auto_lyrics.automatic_request = None;
        self.lyrics_busy = false;
        self.pending
            .retain(|_, kind| !matches!(kind, Pending::Lyrics { .. }));
    }
    pub(crate) fn request_lyrics(&mut self, session: MediaSession) -> Result<(), String> {
        if self.lyrics_busy {
            return Err("A lyrics lookup is already running.".into());
        }
        let identity = Self::track_identity(Some(&session));
        if identity != self.lyrics_identity {
            self.invalidate_lyrics();
            self.lyrics_identity = identity.clone();
        }
        self.auto_lyrics.attempted = true;
        self.auto_lyrics.error = None;
        let id = self.request(Pending::Lyrics {
            generation: self.lyrics_generation,
            identity,
        });
        match self
            .cloud
            .lookup_lyrics(id, session.title, session.artist, session.duration_secs)
        {
            Ok(()) => {
                self.active_lyrics = Some(id);
                self.lyrics_busy = true;
                Ok(())
            }
            Err(error) => {
                self.pending.remove(&id);
                Err(error)
            }
        }
    }
    pub(crate) fn import_lyrics_file(&mut self, path: &Path) -> Result<(), String> {
        self.tools.import_lyrics(path)?;
        self.invalidate_lyrics();
        self.lyrics_for = Self::track_identity(self.session.as_ref());
        self.lyrics_identity = self.lyrics_for.clone();
        self.lyrics_text = self.tools.lyrics.to_lrc();
        self.auto_lyrics
            .remember(self.lyrics_for.clone(), Ok(self.tools.lyrics.clone()));
        self.auto_lyrics.attempted = true;
        self.auto_lyrics.error = None;
        Ok(())
    }
    fn service_fingerprint(config: &ServiceConfig) -> String {
        // This serializes only account scope and endpoint settings, never credentials.
        serde_json::to_string(config).unwrap_or_default()
    }
    pub(crate) fn clear_service_result(&mut self, service: Service) {
        self.service_cards.remove(service.name());
        self.service_requests.remove(service.name());
        self.pending.retain(|_, kind| !matches!(kind, Pending::Service { service: pending_service, .. } if *pending_service == service));
    }
    fn sync_service_configs(&mut self) {
        let configurations = self
            .prefs
            .services
            .iter()
            .map(|config| (config.service, Self::service_fingerprint(config)))
            .collect::<Vec<_>>();
        for (service, fingerprint) in configurations {
            if self.service_fingerprints.get(service.name()) != Some(&fingerprint) {
                self.clear_service_result(service);
                self.service_fingerprints
                    .insert(service.name().into(), fingerprint);
            }
        }
    }
    pub(crate) fn service_busy(&self, service: Service) -> bool {
        self.service_requests
            .get(service.name())
            .is_some_and(|id| self.pending.contains_key(id))
    }
    pub(crate) fn refresh_current_service(&mut self) -> Result<(), String> {
        self.sync_service_configs();
        let config = self
            .prefs
            .services
            .get(self.service_index)
            .cloned()
            .ok_or("Select a service first.")?;
        if self.service_busy(config.service) {
            return Err("This service refresh is already running.".into());
        }
        let service = config.service;
        let fingerprint = Self::service_fingerprint(&config);
        let id = self.request(Pending::Service {
            service,
            config: fingerprint,
        });
        match self.cloud.refresh_service(id, config) {
            Ok(()) => {
                self.service_requests.insert(service.name().into(), id);
                Ok(())
            }
            Err(error) => {
                self.pending.remove(&id);
                Err(error)
            }
        }
    }
    pub(crate) fn transcription_busy(&self, path: &Path) -> bool {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        self.pending.values().any(|kind| matches!(kind, Pending::Transcription { path: pending_path, .. } if *pending_path == path))
    }
    pub(crate) fn transcribe_file(&mut self, path: PathBuf) -> Result<(), String> {
        let path = path
            .canonicalize()
            .map_err(|_| "The audio file is unavailable.".to_string())?;
        if self.transcription_busy(&path) {
            return Err("This audio file is already being transcribed.".into());
        }
        let source = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let id = self.request(Pending::Transcription {
            source,
            path: path.clone(),
        });
        match self.cloud.transcribe_audio(id, path) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.pending.remove(&id);
                Err(error)
            }
        }
    }
    pub(crate) fn summary_busy(&self, id: u64) -> bool {
        self.pending
            .values()
            .any(|kind| matches!(kind, Pending::Summary(note_id) if *note_id == id))
    }
    pub(crate) fn summarize_note(&mut self, note_id: u64) -> Result<(), String> {
        if self.summary_busy(note_id) {
            return Err("This note is already being summarized.".into());
        }
        let note = self
            .tools
            .notes
            .iter()
            .find(|note| note.id == note_id)
            .ok_or("This note is no longer available.")?;
        let messages = vec![
            ChatMessage { role: "system".into(), content: "Summarize this meeting transcript faithfully. Include key points, decisions only if stated, and explicit action items formatted - [ ] task. Do not invent attendees or facts.".into() },
            ChatMessage { role: "user".into(), content: note.text.clone() },
        ];
        let id = self.request(Pending::Summary(note_id));
        match self.cloud.send_chat(id, self.prefs.chat.clone(), messages) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.pending.remove(&id);
                Err(error)
            }
        }
    }
    pub(crate) fn submit_chat(&mut self) {
        if self.chat_busy || self.draft.trim().is_empty() {
            return;
        }
        self.chat.push(ChatMessage {
            role: "user".into(),
            content: std::mem::take(&mut self.draft),
        });
        let id = self.request(Pending::Chat {
            generation: self.chat_generation,
            config: serde_json::to_string(&self.prefs.chat).unwrap_or_default(),
        });
        match self
            .cloud
            .send_chat(id, self.prefs.chat.clone(), self.chat.clone())
        {
            Ok(()) => {
                self.chat_busy = true;
                self.active_chat = Some(id);
            }
            Err(e) => {
                self.pending.remove(&id);
                self.toast("Chat", e);
            }
        }
    }
    fn tick(&mut self, ctx: &egui::Context) {
        let whatsapp_enabled = self.prefs.whatsapp_notifications
            && cfg!(windows)
            && !self.demo
            && self.capture.is_none()
            && self.audit.is_none();
        if !whatsapp_enabled
            || self
                .whatsapp
                .as_ref()
                .is_some_and(|m| m.preview != self.prefs.whatsapp_preview)
        {
            self.whatsapp = None;
            self.notices.retain(|n| n.title != "WhatsApp");
        }
        if whatsapp_enabled && self.whatsapp.is_none() {
            self.whatsapp = Some(crate::whatsapp::Monitor::start(
                ctx.clone(),
                self.prefs.whatsapp_preview,
            ));
        }
        if let Some(result) = self
            .notification_access
            .as_ref()
            .and_then(|rx| match rx.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(
                    "The Windows permission request ended. Try again.".into(),
                )),
            })
        {
            self.notification_access = None;
            match result {
                Ok(access) => self.toast("Notification access", access.label()),
                Err(error) => self.toast("Notification access", error),
            }
        }
        let messages = self.whatsapp.as_mut().map(|m| m.poll()).unwrap_or_default();
        for message in messages {
            // Mirror only in Cikung, without sending another Windows notification.
            self.toast("WhatsApp", message.text(self.prefs.whatsapp_preview));
        }

        let bluetooth_enabled = self.prefs.bluetooth_notifications
            && cfg!(target_os = "windows")
            && !self.demo
            && self.capture.is_none()
            && self.audit.is_none();
        if bluetooth_enabled && self.bluetooth.is_none() {
            self.bluetooth = Some(crate::bluetooth::Monitor::start(ctx.clone()));
        } else if !bluetooth_enabled {
            self.bluetooth = None;
        }
        let changes = self
            .bluetooth
            .as_mut()
            .map(|m| m.poll())
            .unwrap_or_default();
        for change in changes {
            let message = format!(
                "{} {}",
                change.device.name,
                if change.connected {
                    "connected"
                } else {
                    "disconnected"
                }
            );
            if let Some(shell) = &self.shell {
                shell.notify("Bluetooth", &message);
            }
            self.toast("Bluetooth", message);
        }
        let system_enabled = self.prefs.system_notices
            && cfg!(target_os = "windows")
            && !self.demo
            && self.capture.is_none()
            && self.audit.is_none();
        if system_enabled && self.system.is_none() {
            self.system = Some(crate::system_notices::Monitor::start(ctx.clone()));
        } else if !system_enabled {
            self.system = None;
        }
        let notices = self.system.as_ref().map(|m| m.poll()).unwrap_or_default();
        for notice in notices {
            // Caps Lock answers a key press; battery and internet changes stay a little longer.
            let seconds = if notice.title == "Caps Lock" { 2 } else { 6 };
            self.toast_for(notice.title, notice.text, Duration::from_secs(seconds));
        }
        self.agent_manager.poll();
        self.desktop_manager
            .configure(self.prefs.desktop_apps.enabled);
        for (sent, result) in self.desktop_manager.poll() {
            match result {
                Ok(()) if sent => {
                    self.desktop_draft.clear();
                    self.toast("Desktop app", "Sent to the active app conversation.");
                }
                Ok(()) => {}
                Err(error) => self.toast("Desktop app", error),
            }
        }
        if let Some(page) = self.requests.poll() {
            self.open_request(page.as_deref());
        }
        if !matches!(self.page, Page::Style | Page::Settings) {
            self.last_feature = self.page;
        }
        if let Some(shell) = &self.shell {
            for action in shell.poll() {
                match action {
                    crate::shell::ShellAction::Toggle => {
                        self.expanded = !self.expanded;
                    }
                    crate::shell::ShellAction::Settings => {
                        self.navigate(Page::Settings);
                    }
                    crate::shell::ShellAction::Quit => {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close)
                    }
                }
            }
        }
        if let Some(monitor) = &self.monitor {
            self.snapshot = monitor.snapshot();
        }
        self.session = choose_session(
            &self.snapshot.sessions,
            self.selected.as_deref(),
            self.previous.as_deref(),
        )
        .cloned();
        self.previous = self.session.as_ref().map(|s| s.id.clone());
        let lyrics_identity = Self::track_identity(self.session.as_ref());
        if lyrics_identity != self.lyrics_identity {
            self.invalidate_lyrics();
            self.lyrics_identity = lyrics_identity;
            self.auto_lyrics.track_changed(Instant::now());
        }
        self.poll_automatic_lyrics(Instant::now());
        self.sync_service_configs();
        let notices = self.tools.tick();
        for notice in notices {
            if let Some(shell) = &self.shell {
                shell.notify(&notice.title, &notice.message);
            }
            self.toast(notice.title, notice.message);
            if !self.fullscreen {
                self.expanded = true;
            }
        }
        if self.tools.focus.completed_sessions > self.prefs.last_focus_count {
            let delta = self.tools.focus.completed_sessions - self.prefs.last_focus_count;
            self.stats().focus += delta as u32;
            self.prefs.last_focus_count = self.tools.focus.completed_sessions;
        }
        while self
            .notices
            .front()
            .is_some_and(|n| n.until < Instant::now())
        {
            self.notices.pop_front();
        }
        if let Some(error) = self
            .snapshot
            .control_error
            .as_ref()
            .filter(|(_, when)| when.elapsed() < Duration::from_secs(2))
            .map(|(e, _)| e.clone())
            && !self.notices.iter().any(|n| n.text == error)
        {
            self.toast("Playback", error);
        }
        for event in self.cloud.poll() {
            match event {
                CloudEvent::Chat { request_id, result } => {
                    let pending = self.pending.remove(&request_id);
                    match pending {
                        Some(Pending::Game { prompt, config }) => {
                            self.finish_game_hint(request_id, prompt, config, result)
                        }
                        Some(Pending::Summary(id)) => match result {
                            Ok(text) => {
                                if let Some(note) = self.tools.notes.iter_mut().find(|n| n.id == id)
                                {
                                    note.text.push_str("\n\n--- Summary ---\n");
                                    note.text.push_str(&text);
                                    self.toast("Summary ready", "Saved with your meeting note.");
                                } else {
                                    self.toast(
                                        "Summary discarded",
                                        "The original note was deleted.",
                                    );
                                }
                            }
                            Err(error) => self.toast("Summary", error),
                        },
                        Some(Pending::Chat { generation, config })
                            if self.active_chat == Some(request_id) =>
                        {
                            self.active_chat = None;
                            self.chat_busy = false;
                            if generation != self.chat_generation
                                || config
                                    != serde_json::to_string(&self.prefs.chat).unwrap_or_default()
                            {
                                continue;
                            }
                            match result {
                                Ok(content) => self.chat.push(ChatMessage {
                                    role: "assistant".into(),
                                    content,
                                }),
                                Err(error) => self.toast("Chat", error),
                            }
                        }
                        _ => (),
                    }
                }
                CloudEvent::Service {
                    request_id,
                    service,
                    result,
                } => {
                    let pending = self.pending.remove(&request_id);
                    if self.service_requests.get(service.name()) != Some(&request_id) {
                        continue;
                    }
                    self.service_requests.remove(service.name());
                    let Some(Pending::Service {
                        service: expected_service,
                        config,
                    }) = pending
                    else {
                        continue;
                    };
                    if expected_service != service
                        || !self.prefs.services.iter().any(|current| {
                            current.service == service
                                && Self::service_fingerprint(current) == config
                        })
                    {
                        continue;
                    }
                    match result {
                        Ok(cards) => {
                            self.service_cards.insert(service.name().into(), cards);
                        }
                        Err(e) => self.toast(service.name(), e),
                    }
                }
                CloudEvent::Lyrics { request_id, result } => {
                    let kind = self.pending.remove(&request_id);
                    if self.active_lyrics != Some(request_id) {
                        continue;
                    }
                    self.active_lyrics = None;
                    self.lyrics_busy = false;
                    let Some(Pending::Lyrics {
                        generation,
                        identity,
                    }) = kind
                    else {
                        continue;
                    };
                    if generation != self.lyrics_generation
                        || identity != Self::track_identity(self.session.as_ref())
                    {
                        continue;
                    }
                    let automatic = self.auto_lyrics.automatic_request.take() == Some(request_id);
                    let result = result.and_then(|text| Lyrics::parse(&text));
                    self.auto_lyrics.remember(identity.clone(), result.clone());
                    if let Err(error) = self.apply_lyrics_result(identity, result)
                        && !automatic
                    {
                        self.toast("Lyrics", error);
                    }
                }
                CloudEvent::Transcription { request_id, result } => {
                    let source = match self.pending.remove(&request_id) {
                        Some(Pending::Transcription { source, .. }) => source,
                        _ => continue,
                    };
                    match result {
                        Ok(text) => {
                            let result = self
                                .tools
                                .add_note(&format!("Transcript · {source}"), &text);
                            match result {
                                Ok(id) => {
                                    self.note_selected = Some(id);
                                    self.stats().notes += 1;
                                    self.page = Page::Notes;
                                    self.toast("Transcript ready","Original audio is kept on this PC. The selected track was uploaded to OpenAI; your transcript is saved locally.");
                                }
                                Err(e) => self.toast("Notes", e),
                            }
                        }
                        Err(e) => self.toast("Transcription", e),
                    }
                }
            }
        }
        if let Some(result) = self.dictation.poll() {
            match result {
                Ok(text) => {
                    if self.dictation_to_notes {
                        if !self.note_text.is_empty() {
                            self.note_text.push('\n');
                        }
                        self.note_text.push_str(&text);
                    } else {
                        if !self.draft.is_empty() {
                            self.draft.push(' ');
                        }
                        self.draft.push_str(&text);
                    }
                }
                Err(e) => self.toast("Dictation", e),
            }
        }
        if let Some(result) = self.recorder.poll() {
            self.record_started = None;
            match result {
                Ok(recording) => {
                    self.recording_files = recording
                        .mic_path
                        .into_iter()
                        .chain(recording.system_path)
                        .collect();
                    self.toast(
                        "Recording saved",
                        format!(
                            "{} audio tracks, {:.0} seconds. Choose Transcribe to upload a track.",
                            self.recording_files.len(),
                            recording.duration_secs
                        ),
                    );
                    for warning in recording.warnings {
                        self.toast("Recording", warning);
                    }
                }
                Err(e) => self.toast("Recording", e),
            }
        }
        if let Some(bridge) = &self.bridge {
            bridge.publish_media(serde_json::json!({"sessions":self.snapshot.sessions.iter().map(|s|serde_json::json!({"id":s.id,"title":s.title,"artist":s.artist,"source":s.source,"state":format!("{:?}",s.state)})).collect::<Vec<_>>() }));
            let commands = bridge.take_media_commands();
            self.bridge_view = Some(bridge.snapshot());
            for command in commands {
                let action = match command.command.as_str() {
                    "toggle" | "play_pause" | "toggle_play_pause" => {
                        Some(MediaCommand::TogglePlayPause)
                    }
                    "next" => Some(MediaCommand::Next),
                    "previous" => Some(MediaCommand::Previous),
                    "play" => Some(MediaCommand::Play),
                    "pause" => Some(MediaCommand::Pause),
                    _ => None,
                };
                if let (Some(action), Some(monitor)) = (action, &self.monitor) {
                    let id = command
                        .session_id
                        .or_else(|| self.session.as_ref().map(|s| s.id.clone()));
                    if let Some(id) = id
                        && let Err(e) = monitor.control(&id, action)
                    {
                        self.toast("Phone playback", e);
                    }
                }
            }
        }
        let mut completed: Vec<_> = self
            .desktop_manager
            .take_completions()
            .into_iter()
            .map(|target| target.label().to_owned())
            .collect();
        if let Some(bridge) = &self.bridge {
            completed.extend(bridge.take_completions());
        }
        if !completed.is_empty() && !self.demo && self.capture.is_none() && self.audit.is_none() {
            completed.sort();
            completed.dedup();
            self.toast("AI finished", format!("{} finished", completed.join(" · ")));
            if self.prefs.agent_completion_sound
                && cfg!(any(windows, target_os = "macos"))
                && let Err(error) = self
                    .completion_player
                    .play(Instant::now(), self.prefs.agent_completion_volume)
            {
                self.toast("Completion sound", error);
            }
        }
        if self
            .shortcut
            .as_ref()
            .is_some_and(|shortcut| shortcut.pressed())
        {
            self.expanded = !self.expanded;
        }

        if self.prefs.days.len() > 90
            && let Some(day) = self.prefs.days.keys().next().cloned()
        {
            self.prefs.days.remove(&day);
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.expanded = false;
        }
        if self.page == Page::Music
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Space))
        {
            self.control(MediaCommand::TogglePlayPause);
        }
        let dropped = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .filter_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        if hovering && self.file_hover.is_none() {
            self.file_hover = Some((self.page, self.more_open, self.expanded));
            self.expanded = true;
            self.navigate(Page::Files);
        }
        if !hovering
            && let Some((page, more, expanded)) = self.file_hover.take()
            && dropped.is_empty()
        {
            self.page = page;
            self.more_open = more;
            self.expanded = expanded;
        }
        if !dropped.is_empty() {
            self.file_hover = None;
            self.expanded = true;
            self.navigate(Page::Files);
            for path in dropped {
                if let Err(e) = self.tools.add_file(path) {
                    self.toast("File shelf", e);
                }
            }
        }
    }
    pub(crate) fn draw_cat(&mut self, ui: &mut egui::Ui, size: f32) {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
        let playing = self
            .session
            .as_ref()
            .is_some_and(|s| s.state == PlaybackState::Playing);
        let phase = self.started.elapsed().as_secs_f32();
        let bounce = if self.prefs.reduced_motion {
            0.0
        } else if self.poke_until > Instant::now() {
            (phase * 20.0).sin() * 2.0
        } else if playing {
            (phase * 4.0).sin() * 1.3
        } else {
            (phase * 1.3).sin() * 0.35
        };
        let cat_rect = rect.translate(Vec2::new(0.0, bounce)).shrink(1.0);
        ui.painter().image(
            self.cat_textures[self.active_breed().index()].id(),
            cat_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        Self::paint_outfit(ui, cat_rect, self.active_outfit());
        if response.clicked() {
            self.poke_until = Instant::now() + Duration::from_secs(1);
            self.toast("Mrrp!", "Cikung is keeping you company.");
        }
        response.context_menu(|ui| {
            if ui.button("Dress Cikung").clicked() {
                self.navigate(Page::Style);
                ui.close();
            }
            if ui.button("Quit Cikung").clicked() {
                ui.ctx()
                    .send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Close);
            }
        });
    }
}

impl eframe::App for Cikung {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0; 4]
    }
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.last_anchor.elapsed() > Duration::from_secs(1) {
            let covered = self.capture.is_none()
                && self.audit.is_none()
                && native::fullscreen_on_display(&self.prefs.display);
            let fullscreen = covered && self.prefs.tuck_in_fullscreen;
            if fullscreen && !self.fullscreen {
                self.expanded = false;
            }
            self.fullscreen = fullscreen;
            // A fullscreen app can be topmost itself; step back in front of it.
            if covered && !fullscreen && self.prefs.always_on_top {
                native::raise(self.handle, true);
            }
        }
        self.tick(ctx);
        // Mac menu-bar cat: Open Cikung, Settings… or Quit Cikung.
        match native::take_menu_request() {
            1 => self.expanded = true,
            2 => self.navigate(Page::Settings),
            3 => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            _ => {}
        }
        self.sync_artwork(ctx);
        let now = self.started.elapsed().as_secs_f64();
        let scale = ctx.pixels_per_point();
        let host_width = if cfg!(target_os = "windows") {
            notch::HOST.x
        } else {
            ctx.content_rect().width()
        };
        let center = host_width / 2.0;
        let old_surface = notch::aligned_surface_rect(center * 2.0, self.motion.sample(now), scale);
        let pointer = native::pointer_in_window(self.handle, scale)
            .or_else(|| ctx.input(|i| i.pointer.hover_pos()));
        let inside = pointer.is_some_and(|p| {
            old_surface
                .expand(if self.expanded { 6.0 } else { 0.0 })
                .contains(p)
        });
        let held = self.capture.is_some()
            || self.file_hover.is_some()
            || egui::Popup::is_any_open(ctx)
            || ctx.input(|i| i.pointer.any_down())
            || ctx.wants_keyboard_input()
            || self.notices.iter().any(|n| n.until > Instant::now());
        let previously_expanded = self.expanded;
        self.expanded = self.hover.update(
            now,
            inside,
            self.expanded,
            HoverPolicy {
                enabled: self.prefs.hover_open && self.capture.is_none() && !self.fullscreen,
                auto_close: self.prefs.auto_collapse,
                pinned: false,
                held,
            },
        );
        self.follow_music_on_open(previously_expanded);
        let page_height = if self.more_open {
            420.0
        } else {
            self.page.height()
        };
        let size = self.motion.retarget(
            if self.expanded {
                Vec2::new(notch::OPEN_WIDTH, page_height)
            } else if !self.demo && self.current_lyric().is_some() {
                notch::COLLAPSED_LYRIC
            } else if self.minimal_idle() {
                notch::COLLAPSED_MINIMAL
            } else {
                notch::COLLAPSED
            },
            now,
            self.prefs.reduced_motion,
        );
        let openness = self.motion.openness(now);
        let host_size = if cfg!(target_os = "windows") {
            notch::HOST
        } else {
            size
        };
        // macOS: AppKit pins the window to the top centre, above the menu bar.
        #[cfg(target_os = "macos")]
        native::place_on_top(self.handle, host_size.x, host_size.y);
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(host_size));
        if self.last_anchor.elapsed() > Duration::from_secs(1) {
            let _ = native::anchor(
                self.handle,
                &self.prefs.display,
                host_size.x,
                host_size.y,
                scale,
                self.prefs.always_on_top,
                0.0,
            );
            self.last_anchor = Instant::now();
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        if let Some(size) = ctx.input(|i| i.viewport().monitor_size) {
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(Pos2::new(
                (size.x - host_size.x) / 2.0,
                0.0,
            )));
        }
        if self.expanded && !self.was_open {
            native::raise(self.handle, self.prefs.always_on_top);
        }
        self.was_open = self.expanded;
        let center = host_width / 2.0;
        let surface = notch::aligned_surface_rect(center * 2.0, size, scale);
        let radius = 15.0 + openness * 7.0;
        // The notch draws in the chosen theme; Settings is reset to dark further down.
        set_painting_theme(self.prefs.theme);
        let base = paper();
        // Album colour keeps the cover tint for music; everything else fades to the base.
        let tint = if self.prefs.theme != Theme::Album
            || (self.expanded && (self.page != Page::Music || self.more_open))
        {
            base
        } else {
            self.tint
        };
        let fade = if self.prefs.reduced_motion { 0.0 } else { 0.6 };
        let [r, g, b] = [
            ("tint-r", tint.r()),
            ("tint-g", tint.g()),
            ("tint-b", tint.b()),
        ]
        .map(|(id, v)| {
            ctx.animate_value_with_time(egui::Id::new(id), f32::from(v), fade)
                .round() as u8
        });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                theme_visuals(ui, self.prefs.theme);
                notch::paint_surface(
                    ui.painter(),
                    surface,
                    radius,
                    base,
                    Color32::from_rgb(r, g, b),
                );
                // Never taller than the pill, so the minimal notch's row stays centred.
                let header_rect = Rect::from_min_size(
                    surface.min + Vec2::new(14.0, 2.0),
                    Vec2::new((size.x - 28.0).max(1.0), (size.y - 4.0).clamp(1.0, 36.0)),
                );
                let mut header = ui.new_child(
                    egui::UiBuilder::new()
                        .id_salt("notch-header")
                        .max_rect(header_rect),
                );
                header.set_clip_rect(surface.intersect(ui.clip_rect()));
                self.header(&mut header);
                let reveal = ((openness - 0.3) / 0.7).clamp(0.0, 1.0);
                let opacity = reveal * reveal * (3.0 - 2.0 * reveal);
                let progress = self.content_motion.begin(
                    egui::Id::new((self.page as u8, self.more_open)),
                    now,
                    self.prefs.reduced_motion,
                );
                if progress.moving {
                    ctx.request_repaint_after(Duration::from_millis(8));
                }
                if opacity > 0.0 {
                    // Keep the final content width during the reveal. Only its clip
                    // and opacity change; labels and controls never reflow mid-hover.
                    let start =
                        ctx.graphics(|g| g.get(ui.layer_id()).map_or(0, |l| l.next_idx().0));
                    let clip = surface
                        .shrink2(Vec2::new(3.0, 5.0))
                        .intersect(ui.clip_rect());
                    self.content_motion
                        .paint_previous(ui.painter(), clip, opacity, &progress);
                    let body_rect = Rect::from_min_size(
                        Pos2::new(
                            // 26 px sides leave room for the carousel arrows.
                            (host_width - notch::OPEN_WIDTH) / 2.0 + 26.0 + progress.offset,
                            50.0 + 6.0 * (1.0 - openness),
                        ),
                        Vec2::new(notch::OPEN_WIDTH - 52.0, page_height - 62.0),
                    );
                    let mut body = ui.new_child(
                        egui::UiBuilder::new()
                            .id_salt("notch-body")
                            .max_rect(body_rect),
                    );
                    body.set_clip_rect(
                        surface
                            .shrink2(Vec2::new(3.0, 5.0))
                            .intersect(ui.clip_rect()),
                    );
                    body.multiply_opacity(opacity * progress.opacity);
                    if !self.expanded || openness <= 0.98 || progress.moving {
                        body.disable();
                        body.set_opacity(opacity * progress.opacity);
                    }
                    self.content(&mut body);
                    self.content_motion.remember(
                        ui.painter(),
                        start,
                        self.artwork.as_ref().map(|(_, texture)| texture.clone()),
                    );
                    self.carousel_arrows(ui, surface, opacity);
                }
            });
        if inside && ctx.input(|i| i.pointer.any_click()) {
            self.hover.interact();
        }
        let tucked = self.fullscreen && !self.expanded;
        let shape = if tucked {
            Rect::ZERO
        } else if egui::Popup::is_any_open(ctx) {
            Rect::from_min_size(Pos2::ZERO, host_size)
        } else {
            surface
        };
        let shape_radius = if tucked || egui::Popup::is_any_open(ctx) {
            0.0
        } else {
            radius
        };
        let shape_key = [
            (shape.left() * scale).round() as i32,
            (shape.top() * scale).round() as i32,
            (shape.right() * scale).round() as i32,
            (shape.bottom() * scale).round() as i32,
            (shape_radius * scale).round() as i32,
        ];
        if self.last_shape != Some(shape_key)
            && native::shape_window(self.handle, shape, scale, shape_radius)
        {
            self.last_shape = Some(shape_key);
        }
        if let Some(path) = &self.audit
            && self.started.elapsed() > Duration::from_secs(2)
        {
            let displays = native::displays();
            let display = displays
                .iter()
                .find(|d| d.id == self.prefs.display)
                .or(displays.first());
            let actual = native::window_rect(self.handle);
            let expected = display.map(|d| {
                let (x, y, w, h) = native::anchor_rect(d, host_size.x, host_size.y, scale);
                [x, y, x + w, y + h]
            });
            let report = serde_json::json!({"window_rect":actual,"expected_rect":expected,"anchored":actual.is_some()&&actual==expected,"page":self.page.label(),"settings_open":self.settings_open,"visible_notch_logical":[surface.left(),surface.top(),surface.right(),surface.bottom()],"shape_applied":self.last_shape.is_some(),"tray_initialized":self.shell.is_some(),"shortcut_registered":self.shortcut.is_some(),"agent_server_running":self.bridge.is_some(),"phone_enabled":self.bridge.as_ref().is_some_and(|b|b.snapshot().phone_enabled),"recording_on_startup":self.recorder.is_recording(),"dictation_on_startup":self.dictation.busy()});
            std::fs::write(path, serde_json::to_string_pretty(&report).unwrap())
                .expect("save native audit");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        // Settings and captures keep the dark palette whatever the notch theme is.
        set_painting_theme(Theme::Album);
        self.capture_window(ctx, Some(surface));
        self.settings_window(ctx);
        if self.settings_save {
            self.settings_save = false;
            if let Some(storage) = frame.storage_mut() {
                eframe::App::save(self, storage);
                storage.flush();
            }
        }
        // While the notch rests collapsed, hand idle memory back right away and every 10 s,
        // and let the desktop-app watcher poll less often.
        let resting = !self.expanded && !self.motion.moving(now);
        self.desktop_manager.set_resting(resting);
        if !resting {
            self.last_trim = None;
        } else if now > 3.0 && self.last_trim.is_none_or(|at| now - at >= 10.0) {
            native::trim_memory();
            self.last_trim = Some(now);
        }
        if self.motion.moving(now) {
            ctx.request_repaint_after(Duration::from_millis(8));
        } else {
            ctx.request_repaint_after(Duration::from_millis(
                if !self.expanded && self.prefs.hover_open && self.hover.waiting() {
                    8
                } else if !self.prefs.reduced_motion
                    && self
                        .session
                        .as_ref()
                        .is_some_and(|s| s.state == PlaybackState::Playing)
                {
                    // Collapsed, only the tiny EQ bars move: 15 fps keeps memory and CPU low.
                    if self.expanded { 33 } else { 66 }
                } else if self.expanded {
                    100
                } else {
                    // Resting notch: mouse entry wakes it instantly; timers and the
                    // fullscreen check only need about one frame a second.
                    1000
                },
            ));
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if !self.demo && self.capture.is_none() && self.audit.is_none() {
            eframe::set_value(storage, "cikung-v2-settings", &self.prefs);
            eframe::set_value(storage, "cikung-v2-tools", &self.tools);
        }
    }
}

pub(crate) fn heading(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.add_space(9.0);
    ui.label(RichText::new(title).size(18.0).color(ink()));
    if !subtitle.is_empty() {
        ui.label(RichText::new(subtitle).size(12.0).color(muted()));
    }
    ui.add_space(9.0);
}
pub(crate) fn clock(value: f64) -> String {
    let n = if value.is_finite() {
        value.max(0.0) as u64
    } else {
        0
    };
    if n >= 3600 {
        format!("{}:{:02}:{:02}", n / 3600, (n / 60) % 60, n % 60)
    } else {
        format!("{}:{:02}", n / 60, n % 60)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn app() -> (egui::Context, Cikung) {
        let ctx = egui::Context::default();
        let cc = eframe::CreationContext::_new_kittest(ctx.clone());
        let mut app = Cikung::new(
            &cc,
            Launch {
                demo: true,
                capture: None,
                expanded: true,
                page: None,
                audit: None,
                startup_warning: None,
            },
        );
        app.tick(&ctx);
        (ctx, app)
    }
    #[test]
    fn hover_follows_playing_music_without_interrupting_an_open_tool() {
        let (_ctx, mut app) = app();
        app.page = Page::Focus;
        app.expanded = true;
        app.follow_music_on_open(true);
        assert_eq!(app.page, Page::Focus);
        app.follow_music_on_open(false);
        assert_eq!(app.page, Page::Music);
        app.page = Page::Focus;
        app.prefs.music_on_hover = false;
        app.follow_music_on_open(false);
        assert_eq!(app.page, Page::Focus);
        app.prefs.music_on_hover = true;
        for state in [PlaybackState::Paused, PlaybackState::Stopped] {
            app.session.as_mut().unwrap().state = state;
            app.follow_music_on_open(false);
            assert_eq!(app.page, Page::Focus);
        }
        app.session = None;
        app.follow_music_on_open(false);
        assert_eq!(app.page, Page::Focus);
    }
    #[test]
    fn settings_and_cats_share_a_popup_without_replacing_the_notch() {
        let (_ctx, mut app) = app();
        app.page = Page::Music;
        app.expanded = false;
        app.navigate(Page::Settings);
        assert!(app.settings_open);
        assert_eq!(app.settings_tab, settings_pages::SettingsPage::General);
        assert_eq!(app.page, Page::Music);
        assert!(!app.expanded);
        let id = settings_window::viewport_id();
        assert_ne!(id, egui::ViewportId::ROOT);
        app.navigate(Page::Style);
        assert_eq!(app.settings_tab, settings_pages::SettingsPage::Cats);
        assert_eq!(settings_window::viewport_id(), id);
        assert_eq!(app.page, Page::Music);
    }
    #[test]
    fn hub_deep_links_keep_the_notch_and_desktop_draft_intact() {
        use settings_pages::SettingsPage;
        let (_ctx, mut app) = app();
        app.page = Page::Music;
        app.expanded = false;
        app.desktop_draft = "Keep my unsent thought".into();
        app.open_request(Some("settings/agents"));
        assert!(app.settings_open);
        assert_eq!(app.settings_tab, SettingsPage::Agents);
        assert_eq!(app.page, Page::Music);
        assert!(!app.expanded);
        app.open_chat_settings();
        assert_eq!(app.settings_tab, SettingsPage::Chat);
        app.open_request(Some("settings/shortcuts"));
        assert_eq!(app.settings_tab, SettingsPage::Shortcuts);
        assert_eq!(app.desktop_draft, "Keep my unsent thought");
        app.open_request(Some("focus"));
        assert_eq!(app.page, Page::Focus);
        assert!(app.expanded);
    }
    #[test]
    fn music_preview_never_shows_lyrics_from_another_track() {
        let (ctx, mut app) = app();
        app.page = Page::Music;
        app.tools.lyrics = Lyrics::parse("[00:00]Other track private line").unwrap();
        app.lyrics_for = "A different track".into();
        fn text(shape: &egui::Shape, out: &mut String) {
            match shape {
                egui::Shape::Text(t) => out.push_str(&t.galley.job.text),
                egui::Shape::Vec(shapes) => {
                    for s in shapes {
                        text(s, out);
                    }
                }
                _ => {}
            }
        }
        for matching in [false, true] {
            if matching {
                app.lyrics_for = Cikung::track_identity(app.session.as_ref());
            }
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, notch::HOST)),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| app.music(ui));
                },
            );
            let mut rendered = String::new();
            for shape in output.shapes {
                text(&shape.shape, &mut rendered);
            }
            assert_eq!(rendered.contains("Other track private line"), matching);
            assert_eq!(rendered.contains("Lyrics not available yet"), !matching);
        }
    }
    #[test]
    fn file_drag_opens_pocket_cancel_restores_and_drop_preserves_original() {
        let (ctx, mut app) = app();
        app.page = Page::Music;
        app.expanded = false;
        app.more_open = true;
        let path = std::env::temp_dir().join(format!("cikung-pocket-{}.txt", std::process::id()));
        std::fs::write(&path, "Original file").unwrap();
        let hovering = egui::RawInput {
            hovered_files: vec![egui::HoveredFile {
                path: Some(path.clone()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let before = app.tools.shelf.len();
        let _ = ctx.run(hovering.clone(), |ctx| app.tick(ctx));
        assert_eq!(app.page, Page::Files);
        assert!(app.expanded && !app.more_open && app.file_hover.is_some());
        assert_eq!(app.tools.shelf.len(), before);
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.tick(ctx));
        assert_eq!(app.page, Page::Music);
        assert!(!app.expanded && app.more_open && app.file_hover.is_none());
        let _ = ctx.run(hovering, |ctx| app.tick(ctx));
        let _ = ctx.run(
            egui::RawInput {
                dropped_files: vec![egui::DroppedFile {
                    path: Some(path.clone()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            |ctx| app.tick(ctx),
        );
        assert_eq!(app.page, Page::Files);
        assert!(app.expanded && !app.more_open && app.file_hover.is_none());
        assert_eq!(app.tools.shelf.len(), before + 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Original file");
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn equal_height_panels_crossfade_in_the_actual_notch_renderer() {
        use eframe::App;
        let (ctx, mut app) = app();
        app.page = Page::Files;
        app.motion = Motion::new(Vec2::new(notch::OPEN_WIDTH, Page::Files.height()));
        let mut frame = eframe::Frame::_new_kittest();
        fn render(
            ctx: &egui::Context,
            app: &mut Cikung,
            frame: &mut eframe::Frame,
            time: f64,
        ) -> String {
            app.started = Instant::now() - Duration::from_secs_f64(time);
            app.last_anchor = Instant::now();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, notch::HOST)),
                    time: Some(time),
                    ..Default::default()
                },
                |ctx| app.update(ctx, frame),
            );
            let mut text = String::new();
            for entry in output.shapes {
                if let egui::Shape::Text(shape) = entry.shape {
                    text.push_str(&shape.galley.job.text);
                }
            }
            text
        }
        let files = "Drag out anytime · click to open";
        assert!(render(&ctx, &mut app, &mut frame, 0.0).contains(files));
        app.page = Page::Water;
        let first = render(&ctx, &mut app, &mut frame, 1.0);
        assert!(first.contains(files));
        let middle = render(&ctx, &mut app, &mut frame, 1.12);
        assert!(!middle.contains(files));
        assert!(middle.contains("Water"));
        let last = render(&ctx, &mut app, &mut frame, 1.3);
        assert!(!last.contains(files));
        assert!(last.contains("Water"));
        assert!(!app.motion.moving(1.3));
    }
    #[test]
    fn whatsapp_preferences_migrate_without_private_history_and_demo_does_not_read_notifications() {
        let prefs: Preferences = ron::from_str("(hover_open:true)").unwrap();
        assert!(prefs.whatsapp_notifications);
        assert!(!prefs.whatsapp_preview);
        assert!(prefs.agent_completion_sound);
        assert_eq!(prefs.agent_completion_volume, 100);
        let saved_volume: Preferences = ron::from_str("(agent_completion_volume:35)").unwrap();
        assert_eq!(
            ron::from_str::<Preferences>(&ron::to_string(&saved_volume).unwrap())
                .unwrap()
                .agent_completion_volume,
            35
        );
        let muted: Preferences = ron::from_str("(agent_completion_sound:false)").unwrap();
        assert!(
            !ron::from_str::<Preferences>(&ron::to_string(&muted).unwrap())
                .unwrap()
                .agent_completion_sound
        );
        let (ctx, mut app) = app();
        app.open_request(Some("settings/notifications"));
        assert_eq!(
            app.settings_tab,
            settings_pages::SettingsPage::Notifications
        );
        app.toast("WhatsApp", "Private transient message");
        app.tick(&ctx);
        assert!(app.whatsapp.is_none());
        assert!(app.notices.iter().all(|n| n.title != "WhatsApp"));
        app.toast("WhatsApp test", "Test notification from Cikung");
        app.tick(&ctx);
        assert!(app.notices.iter().any(|n| n.title == "WhatsApp test"));
        let saved = ron::to_string(&app.prefs).unwrap();
        assert!(!saved.contains("Private transient"));
    }
    #[test]
    fn formats_durations() {
        assert_eq!(super::clock(3661.0), "1:01:01");
        assert_eq!(super::clock(f64::NAN), "0:00");
    }
    #[test]
    fn lyric_seeks_require_matching_synced_seekable_real_media() {
        let (_ctx, mut app) = app();
        app.demo = false;
        app.tools.lyrics = Lyrics::parse("[00:10]A line").unwrap();
        app.lyrics_for = Cikung::track_identity(app.session.as_ref());
        assert_eq!(app.lyric_seek_target(10.0), Some(10.0));
        for invalid in [f64::NAN, -1.0, 1000.0] {
            assert!(app.lyric_seek_target(invalid).is_none());
        }
        app.lyrics_for = "Different song".into();
        assert!(app.lyric_seek_target(10.0).is_none());
        app.lyrics_for = Cikung::track_identity(app.session.as_ref());
        app.tools.lyrics.synced = false;
        assert!(app.lyric_seek_target(10.0).is_none());
        app.tools.lyrics.synced = true;
        app.session.as_mut().unwrap().can_seek = false;
        assert!(app.lyric_seek_target(10.0).is_none());
        app.session.as_mut().unwrap().can_seek = true;
        app.demo = true;
        assert!(app.lyric_seek_target(10.0).is_none());
        app.demo = false;
        app.capture = Some("preview.png".into());
        assert!(app.lyric_seek_target(10.0).is_none());
        app.capture = None;
        app.audit = Some("audit.json".into());
        assert!(app.lyric_seek_target(10.0).is_none());
    }
    #[test]
    fn feature_cats_and_agent_cats_switch_inside_the_same_app() {
        use crate::{agent_connect::Agent, cats::Breed};
        let (_ctx, mut app) = app();
        app.page = Page::Music;
        assert_eq!(app.active_breed(), Breed::Orange);
        app.page = Page::Agents;
        assert_eq!(app.active_breed(), Breed::Calico);
        app.prefs.agents.selected = Agent::Codex;
        assert_eq!(app.active_breed(), Breed::Tuxedo);
        app.prefs.agents.selected = Agent::Omp;
        assert_eq!(app.active_breed(), Breed::Silver);
        app.page = Page::Focus;
        assert_eq!(app.active_breed(), Breed::Siamese);
        app.last_feature = Page::Focus;
        app.page = Page::Style;
        assert_eq!(app.active_breed(), Breed::Siamese);
        app.prefs.auto_cat = false;
        app.prefs.cat_breed = Breed::White;
        for page in Page::ALL {
            app.page = page;
            assert_eq!(app.active_breed(), Breed::White);
        }
    }
    #[test]
    fn feature_cat_choices_and_manual_choice_survive_storage() {
        use crate::cats::Breed;
        let (_ctx, mut app) = app();
        app.prefs.feature_cats.insert(Page::Notes, Breed::Tuxedo);
        let saved = ron::to_string(&app.prefs).unwrap();
        app.prefs = ron::from_str(&saved).unwrap();
        assert_eq!(app.breed_for(Page::Notes), Breed::Tuxedo);
        let appearances: std::collections::HashSet<_> = cat_gallery::FEATURES
            .into_iter()
            .map(|page| {
                format!(
                    "{}|{}",
                    app.breed_for(page).label(),
                    app.outfit_for(page).label()
                )
            })
            .collect();
        assert!(appearances.len() >= 10);
        app.prefs.auto_cat = false;
        app.prefs.cat_breed = Breed::White;
        app.prefs = ron::from_str(&ron::to_string(&app.prefs).unwrap()).unwrap();
        assert_eq!(app.breed_for(Page::Notes), Breed::White);
    }
    #[test]
    fn summary_completion_does_not_unlock_an_active_chat() {
        let (ctx, mut app) = app();
        let note = app
            .tools
            .add_note("Meeting", "Original transcript")
            .unwrap();
        let chat_id = app.request(Pending::Chat {
            generation: app.chat_generation,
            config: serde_json::to_string(&app.prefs.chat).unwrap(),
        });
        app.active_chat = Some(chat_id);
        app.chat_busy = true;
        let summary_id = app.request(Pending::Summary(note));
        app.cloud.inject_event(CloudEvent::Chat {
            request_id: summary_id,
            result: Ok("A faithful summary".into()),
        });
        app.tick(&ctx);
        assert!(app.chat_busy);
        assert_eq!(app.active_chat, Some(chat_id));
        assert!(app.tools.notes[0].text.contains("A faithful summary"));
    }
    #[test]
    fn a_new_conversation_discards_old_async_replies() {
        let (ctx, mut app) = app();
        let id = app.request(Pending::Chat {
            generation: app.chat_generation,
            config: serde_json::to_string(&app.prefs.chat).unwrap(),
        });
        app.active_chat = Some(id);
        app.chat_busy = true;
        app.reset_chat();
        app.cloud.inject_event(CloudEvent::Chat {
            request_id: id,
            result: Ok("Old conversation".into()),
        });
        app.tick(&ctx);
        assert!(app.chat.is_empty());
        assert!(!app.chat_busy);
    }
    #[test]
    fn imported_lyrics_win_over_an_earlier_lookup() {
        let (ctx, mut app) = app();
        let identity = Cikung::track_identity(app.session.as_ref());
        let id = app.request(Pending::Lyrics {
            generation: app.lyrics_generation,
            identity,
        });
        app.active_lyrics = Some(id);
        app.lyrics_busy = true;
        let path = std::env::temp_dir().join(format!(
            "cikung-lyrics-{}-{}.lrc",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        std::fs::write(&path, "[00:00]Imported line").unwrap();
        app.import_lyrics_file(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        app.cloud.inject_event(CloudEvent::Lyrics {
            request_id: id,
            result: Ok("[00:00]Old lookup".into()),
        });
        app.tick(&ctx);
        assert_eq!(app.tools.lyrics.lines[0].text, "Imported line");
        assert!(!app.lyrics_busy);
    }
    #[test]
    fn disconnecting_a_service_discards_its_pending_result() {
        let (ctx, mut app) = app();
        let service = Service::GitHub;
        let config = Cikung::service_fingerprint(&app.prefs.services[0]);
        let id = app.request(Pending::Service { service, config });
        app.service_requests.insert(service.name().into(), id);
        app.clear_service_result(service);
        app.cloud.inject_event(CloudEvent::Service {
            request_id: id,
            service,
            result: Ok(vec![ServiceCard {
                title: "Stale account".into(),
                detail: String::new(),
                url: String::new(),
            }]),
        });
        app.tick(&ctx);
        assert!(!app.service_cards.contains_key(service.name()));
    }
    #[test]
    fn duplicate_transcriptions_are_rejected_before_upload() {
        let (_ctx, mut app) = app();
        let path = std::env::current_exe().unwrap().canonicalize().unwrap();
        app.request(Pending::Transcription {
            source: "test".into(),
            path: path.clone(),
        });
        assert!(app.transcription_busy(&path));
        assert!(app.transcribe_file(path).unwrap_err().contains("already"));
    }
    #[test]
    fn date_clock_and_header_actions_fit_collapsed_and_expanded_notches() {
        let (ctx, mut app) = app();
        app.session.as_mut().unwrap().title = "A very long track title ".repeat(20);
        for page in [Page::Music, Page::Overview] {
            app.page = page;
            for width in [252.0, 420.0, 460.0, 500.0, 520.0, 580.0] {
                app.motion = Motion::new(Vec2::new(width, 340.0));
                let header =
                    Rect::from_min_size(Pos2::new(14.0, 2.0), Vec2::new(width - 28.0, 36.0));
                let mut used = Rect::NOTHING;
                let output = ctx.run(
                    egui::RawInput {
                        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, notch::HOST)),
                        ..Default::default()
                    },
                    |ctx| {
                        egui::CentralPanel::default()
                            .frame(egui::Frame::NONE)
                            .show(ctx, |ui| {
                                let mut child =
                                    ui.new_child(egui::UiBuilder::new().max_rect(header));
                                child.set_clip_rect(header);
                                app.header(&mut child);
                                used = child.min_rect();
                            });
                    },
                );
                assert!(
                    used.right() <= header.right() + 1.0,
                    "{} at {width}: {used:?} outside {header:?}",
                    page.label()
                );
                let mut clock_count = 0;
                for entry in output.shapes {
                    if let egui::Shape::Text(text) = entry.shape {
                        let label = &text.galley.job.text;
                        if label.len() == 5 && label.as_bytes()[2] == b':' {
                            clock_count += 1;
                            assert!(
                                header.contains_rect(Rect::from_min_size(
                                    text.pos,
                                    text.galley.size()
                                ))
                            );
                        }
                    }
                }
                assert_eq!(clock_count, 1, "Clock must remain visible at {width}");
            }
        }
    }
    #[test]
    fn every_panel_renders_inside_the_notch_width() {
        let (ctx, mut app) = app();
        let task = app
            .tools
            .add_task(&"A long task that still needs to fit in the notch. ".repeat(30))
            .unwrap();
        let text = app.tools.tasks[0].text.clone();
        app.tools
            .edit_task(task, &text, Some(2_000_000_000))
            .unwrap();
        app.tools.water.drink();
        for page in Page::ALL {
            app.page = page;
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(580.0, 440.0))),
                ..Default::default()
            };
            let mut layout_rect = Rect::NOTHING;
            let output = ctx.run(input, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    app.header(ui);
                    app.content(ui);
                    layout_rect = ui.min_rect();
                });
            });
            assert!(!output.shapes.is_empty(), "{}", page.label());
            assert!(
                layout_rect.width() <= 581.0,
                "{} overflowed horizontally: {:?}",
                page.label(),
                ctx.used_rect()
            );
        }
    }
    #[test]
    fn music_time_row_remains_visible_in_the_compact_panel() {
        let (ctx, mut app) = app();
        app.page = Page::Music;
        app.tools.lyrics =
            Lyrics::parse(&format!("[00:00]{}", "A long lyric line ".repeat(120))).unwrap();
        app.lyrics_for = Cikung::track_identity(app.session.as_ref());
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, notch::HOST)),
            ..Default::default()
        };
        let surface = Rect::from_min_size(
            Pos2::ZERO,
            Vec2::new(notch::OPEN_WIDTH, Page::Music.height()),
        );
        let output = ctx.run(input, |ctx| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    let mut body =
                        ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(
                            Pos2::new(26.0, 50.0),
                            Vec2::new(notch::OPEN_WIDTH - 52.0, Page::Music.height() - 62.0),
                        )));
                    body.set_clip_rect(surface);
                    app.content(&mut body);
                });
        });
        fn locate(shape: &egui::Shape, clip: Rect, found: &mut Vec<(Rect, Rect)>) {
            match shape {
                egui::Shape::Vec(shapes) => {
                    for shape in shapes {
                        locate(shape, clip, found);
                    }
                }
                // The remaining-time label ("−m:ss") sits on the inline progress row.
                egui::Shape::Text(text) if text.galley.job.text.starts_with('−') => {
                    found.push((Rect::from_min_size(text.pos, text.galley.size()), clip));
                }
                _ => {}
            }
        }
        let mut found = Vec::new();
        for clipped in &output.shapes {
            locate(&clipped.shape, clipped.clip_rect, &mut found);
        }
        assert_eq!(found.len(), 1, "The time row must be visible");
        assert!(
            found[0].1.contains_rect(found[0].0),
            "The time row was clipped: {:?}",
            found[0]
        );
        assert!(
            found[0].0.bottom() < surface.bottom() - 15.0,
            "The time row overlaps the bottom edge: {:?}",
            found[0]
        );
        assert!(
            !format!("{:?}", output.shapes).contains("Playback options"),
            "Playback options now live in Settings"
        );
    }
}
