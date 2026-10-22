#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod agent_connect;
mod app;
mod bluetooth;
mod bridge;
mod cats;
mod cloud;
mod completion_sound;
mod desktop_apps;
mod dictation;
mod instance;
mod media;
mod monitor;
mod native;
mod notch;
mod panels;
mod profile_migration;
mod recording;
mod shell;
mod tools;
mod whatsapp;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--bluetooth-probe") {
        match bluetooth::probe() {
            Ok(devices) => println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({"connected_devices":devices}))
                    .unwrap()
            ),
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--desktop-probe") {
        let result = desktop_apps::probe();
        let failed = result.is_err();
        let value = result.unwrap_or_else(|error| serde_json::json!({"error":error}));
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
        if failed {
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--notifications-probe") {
        let result = whatsapp::probe();
        let failed = result.is_err();
        println!(
            "{}",
            match result {
                Ok(probe) => serde_json::to_string_pretty(&probe).unwrap(),
                Err(error) => serde_json::json!({"error": error}).to_string(),
            }
        );
        if failed {
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--probe") {
        let result = media::create_backend().and_then(|mut backend| backend.poll());
        let failed = result.is_err();
        let output = match result {
            Ok(sessions) => {
                serde_json::json!({"sessions": sessions.iter().map(|s| serde_json::json!({
                "id":s.id,"source":s.source,"title":s.title,"artist":s.artist,"album":s.album,
                "state":format!("{:?}",s.state),"position_secs":s.position_secs,"duration_secs":s.duration_secs
            })).collect::<Vec<_>>() })
            }
            Err(error) => serde_json::json!({"error":error}),
        };
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
        if failed {
            std::process::exit(1);
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "Cikung 0.11.1\n\nOne app, one top-center notch, six cat breeds. Use Cats to choose a cat or assign cats to features.\n--probe    Print actual system media sessions as JSON\n--notifications-probe Print notification access and WhatsApp count (no message contents)\n--bluetooth-probe Print connected Bluetooth devices without changing them\n--desktop-probe  Print desktop app connection status (no message contents)\n--demo     Preview with clearly labeled sample media\n--expanded Start with the notch open\n--page overview|music|lyrics|focus|files|tasks|water|notes|agents|chat|game|services|phone|cats|settings|more\n--page settings/<general|cats|music|agents|chat|integrations|phone|shortcuts|dictation|notes|focus|tasks|water|game|notifications>  Open a hub page\n--screenshot <path.png>  Save a window capture and exit\n\nA second launch opens the requested page in the existing app. Legacy --feature and --workspace flags no longer start separate companions.\nHover to open. Click the header to open or close. Ctrl+Alt+C toggles the notch. Ctrl+Q quits."
        );
        return Ok(());
    }
    let demo = args.iter().any(|a| a == "--demo");
    let capture = args
        .windows(2)
        .find(|w| w[0] == "--screenshot")
        .map(|w| w[1].clone());
    let diagnostic = demo || capture.is_some() || args.iter().any(|a| a == "--native-smoke");
    let page = args
        .windows(2)
        .find(|w| w[0] == "--page" || w[0] == "--feature")
        .map(|w| w[1].clone());
    let _instance = if diagnostic {
        None
    } else {
        match instance::Instance::acquire() {
            Ok(Some(guard)) => Some(guard),
            Ok(None) => {
                let _ = instance::show(page.as_deref());
                return Ok(());
            }
            Err(error) => {
                eprintln!("{error}");
                return Ok(());
            }
        }
    };
    let startup_warning = if diagnostic {
        None
    } else {
        profile_migration::migrate().err()
    };
    let title = "Cikung";
    let app_id = "com.cikung.desktop";
    let preview_storage = diagnostic
        .then(|| std::env::temp_dir().join(format!("cikung-preview-{}.ron", std::process::id())));
    let options = eframe::NativeOptions {
        persistence_path: preview_storage.clone(),
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_app_id(app_id)
            .with_inner_size(if cfg!(target_os = "windows") {
                notch::HOST
            } else {
                notch::COLLAPSED
            })
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/cikung-face.png"))
                    .expect("cat icon"),
            )
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_always_on_top(),
        persist_window: false,
        // DWM composes the transparent host. Avoid a second blocking GL vsync wait;
        // the app schedules frames only while motion or playback needs them.
        vsync: !cfg!(target_os = "windows"),
        centered: false,
        // eframe still reads old window geometry when persist_window is false.
        // Override it after restoration so a previous release cannot shrink the host.
        window_builder: Some(Box::new(|builder| {
            if cfg!(target_os = "windows") {
                builder
                    .with_inner_size(notch::HOST)
                    .with_min_inner_size(notch::HOST)
                    .with_max_inner_size(notch::HOST)
                    .with_maximized(false)
                    .with_fullscreen(false)
            } else {
                builder
            }
        })),
        ..Default::default()
    };
    let result = eframe::run_native(
        title,
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::Cikung::new(
                cc,
                app::Launch {
                    demo,
                    startup_warning,
                    capture,
                    expanded: args.iter().any(|a| a == "--expanded"),
                    page,
                    audit: args
                        .windows(2)
                        .find(|w| w[0] == "--native-smoke")
                        .map(|w| w[1].clone()),
                },
            )))
        }),
    );
    if let Some(path) = preview_storage {
        let _ = std::fs::remove_file(path);
    }
    result
}
