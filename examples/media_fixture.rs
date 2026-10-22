//! Developer-only Windows playback fixture. Run alongside Cikung with
//! `cargo run --example media_fixture`.
//!
//! This creates a real, muted WinRT MediaPlayer and publishes its own SMTC
//! metadata. It does not change Cikung's state or launch another media app.
//! Some Windows desktop configurations do not expose an unpackaged executable's
//! SMTC session. An absent session is reported explicitly, not counted as a pass.

#[cfg(target_os = "windows")]
#[allow(dead_code)]
#[path = "../src/media/mod.rs"]
mod media;

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("The native media fixture is available on Windows only.");
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use windows::Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize};

    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // The guard is created only after this thread successfully initializes.
            unsafe { RoUninitialize() };
        }
    }

    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }?;
    let _apartment = Apartment;
    let result = native::run();
    if let Err(error) = &result {
        eprintln!("Native fixture failed: {error}");
        eprintln!(
            "If Windows rejects SMTC for this unpackaged desktop process, \
             this run cannot validate session discovery. No helper was installed."
        );
    }
    result
}

#[cfg(target_os = "windows")]
mod native {
    use std::{
        error::Error,
        fs::{self, OpenOptions},
        io::Write,
        path::PathBuf,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use windows::{
        Foundation::TypedEventHandler,
        Media::{
            Core::MediaSource, MediaPlaybackStatus, MediaPlaybackType, Playback::MediaPlayer,
            SystemMediaTransportControls, SystemMediaTransportControlsButton,
            SystemMediaTransportControlsButtonPressedEventArgs,
        },
        Storage::StorageFile,
        core::HSTRING,
    };

    const TITLE: &str = "Cikung integration test";
    const ARTIST: &str = "Local silent fixture";

    use crate::media::{self, MediaBackend, MediaCommand, MediaSession, PlaybackState};

    struct ButtonSubscription {
        controls: SystemMediaTransportControls,
        token: i64,
    }

    impl Drop for ButtonSubscription {
        fn drop(&mut self) {
            // Break the controls -> handler -> player reference cycle first.
            let _ = self.controls.RemoveButtonPressed(self.token);
        }
    }

    fn wait_for_fixture(
        backend: &mut dyn MediaBackend,
        album: &str,
        state: PlaybackState,
    ) -> Result<MediaSession, Box<dyn Error>> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut last_error = String::from("fixture not discovered");
        while Instant::now() < deadline {
            match backend.poll() {
                Ok(sessions) => {
                    let matches: Vec<_> = sessions
                        .into_iter()
                        .filter(|session| session.title == TITLE && session.album == album)
                        .collect();
                    if matches.len() > 1 {
                        return Err(
                            "Ambiguous fixture sessions; no playback control was sent".into()
                        );
                    }
                    if let Some(session) = matches.into_iter().next() {
                        if session.artist != ARTIST {
                            return Err(
                                "Fixture artist did not match its published metadata".into()
                            );
                        }
                        if session.state == state {
                            return Ok(session);
                        }
                        last_error = format!("expected {state:?}, observed {:?}", session.state);
                    }
                }
                Err(error) => last_error = error,
            }
            thread::sleep(Duration::from_millis(100));
        }
        Err(format!("Native backend integration failed: {last_error}").into())
    }

    struct SilentWave(PathBuf);

    impl SilentWave {
        fn create() -> Result<Self, Box<dyn Error>> {
            let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
            let path = std::env::temp_dir().join(format!(
                "cikung-media-fixture-{}-{nonce}.wav",
                std::process::id()
            ));
            // create_new ensures an existing user's file is never overwritten.
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)?;
            let wave = Self(path);
            let written = (|| -> std::io::Result<()> {
                let data_size: u32 = 16_000 * 2 * 2; // 2 seconds, 16 kHz, mono PCM16.
                file.write_all(b"RIFF")?;
                file.write_all(&(36 + data_size).to_le_bytes())?;
                file.write_all(b"WAVEfmt ")?;
                file.write_all(&16_u32.to_le_bytes())?;
                file.write_all(&1_u16.to_le_bytes())?; // PCM.
                file.write_all(&1_u16.to_le_bytes())?; // Mono.
                file.write_all(&16_000_u32.to_le_bytes())?;
                file.write_all(&32_000_u32.to_le_bytes())?; // Bytes per second.
                file.write_all(&2_u16.to_le_bytes())?; // Block alignment.
                file.write_all(&16_u16.to_le_bytes())?;
                file.write_all(b"data")?;
                file.write_all(&data_size.to_le_bytes())?;
                file.write_all(&vec![0_u8; data_size as usize])?;
                file.flush()
            })();
            // Release the file handle before the cleanup guard runs on failure.
            drop(file);
            written?;
            Ok(wave)
        }
    }

    impl Drop for SilentWave {
        fn drop(&mut self) {
            if let Err(error) = fs::remove_file(&self.0) {
                eprintln!("Could not remove fixture {}: {error}", self.0.display());
            }
        }
    }

    struct ClosingPlayer(MediaPlayer);
    impl Drop for ClosingPlayer {
        fn drop(&mut self) {
            let _ = self.0.Pause();
            let _ = self.0.Close();
        }
    }

    struct ClosingSource(MediaSource);
    impl Drop for ClosingSource {
        fn drop(&mut self) {
            let _ = self.0.Close();
        }
    }

    pub fn run() -> Result<(), Box<dyn Error>> {
        // Reverse declaration order releases player, source and StorageFile
        // before deleting the temporary WAV, including on early errors.
        let wave = SilentWave::create()?;
        let file =
            StorageFile::GetFileFromPathAsync(&HSTRING::from(wave.0.to_string_lossy().as_ref()))?
                .join()?;
        let source = ClosingSource(MediaSource::CreateFromStorageFile(&file)?);
        let player = ClosingPlayer(MediaPlayer::new()?);
        player.0.SetAutoPlay(false)?;
        player.0.SetIsMuted(true)?;
        player.0.SetIsLoopingEnabled(true)?;
        player.0.SetSource(&source.0)?;
        // Manage SMTC explicitly so it does not replace the fixture metadata
        // with the temporary file's name.
        player.0.CommandManager()?.SetIsEnabled(false)?;
        let controls = player.0.SystemMediaTransportControls()?;
        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsNextEnabled(false)?;
        controls.SetIsPreviousEnabled(false)?;
        let callback_player = player.0.clone();
        let handler = TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |sender, args| {
            if let (Some(sender), Some(args)) = (sender.as_ref(), args.as_ref()) {
                match args.Button()? {
                    SystemMediaTransportControlsButton::Play => {
                        callback_player.Play()?;
                        sender.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
                    }
                    SystemMediaTransportControlsButton::Pause => {
                        callback_player.Pause()?;
                        sender.SetPlaybackStatus(MediaPlaybackStatus::Paused)?;
                    }
                    _ => (),
                }
            }
            Ok(())
        });
        let _subscription = ButtonSubscription {
            token: controls.ButtonPressed(&handler)?,
            controls: controls.clone(),
        };
        let updater = controls.DisplayUpdater()?;
        updater.SetType(MediaPlaybackType::Music)?;
        let music = updater.MusicProperties()?;
        music.SetTitle(&HSTRING::from(TITLE))?;
        music.SetArtist(&HSTRING::from(ARTIST))?;
        // This run-specific marker prevents controlling a different fixture or player.
        let album = format!("Cikung fixture PID {}", std::process::id());
        music.SetAlbumTitle(&HSTRING::from(&album))?;
        updater.Update()?;

        player.0.Play()?;
        controls.SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
        println!("Playing a muted, looping native WinRT source for 25 seconds.");
        println!("Requested SMTC title: {TITLE}");

        let start = Instant::now();
        let mut backend = media::create_backend()
            .map_err(|error| format!("Could not create the real media backend: {error}"))?;
        let fixture = wait_for_fixture(backend.as_mut(), &album, PlaybackState::Playing)?;
        println!(
            "Backend discovered fixture: source={}, title={}, artist={}, album={}, state={:?}",
            fixture.source, fixture.title, fixture.artist, fixture.album, fixture.state
        );
        backend.control(&fixture.id, MediaCommand::TogglePlayPause)?;
        let fixture = wait_for_fixture(backend.as_mut(), &album, PlaybackState::Paused)?;
        println!("Backend TogglePlayPause delivered to real MediaPlayer: Paused");
        backend.control(&fixture.id, MediaCommand::TogglePlayPause)?;
        let fixture = wait_for_fixture(backend.as_mut(), &album, PlaybackState::Playing)?;
        println!("Backend TogglePlayPause delivered to real MediaPlayer: Playing");
        for command in [MediaCommand::Next, MediaCommand::Previous] {
            match backend.control(&fixture.id, command) {
                Err(error) => {
                    println!("Backend rejected fixture's disabled {command:?} control: {error}")
                }
                Ok(()) => {
                    return Err(
                        format!("Disabled fixture {command:?} unexpectedly succeeded").into(),
                    );
                }
            }
        }
        println!(
            "INTEGRATION PASS: native discovery, metadata, pause/resume, and unsupported Next/Previous"
        );
        let mut paused = false;
        let mut last_report = None;
        while start.elapsed() < Duration::from_secs(45) {
            if !paused && start.elapsed() >= Duration::from_secs(25) {
                player.0.Pause()?;
                controls.SetPlaybackStatus(MediaPlaybackStatus::Paused)?;
                paused = true;
                println!("Paused the real MediaPlayer; waiting 20 seconds before cleanup.");
            }
            if let Ok(sessions) = backend.poll() {
                for session in sessions {
                    if session.title == TITLE && session.album == album {
                        let status = session.state;
                        if last_report != Some(status) {
                            println!(
                                "GSMTC observed fixture: source={}, status={status:?}",
                                session.source
                            );
                            last_report = Some(status);
                        }
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
        controls.SetPlaybackStatus(MediaPlaybackStatus::Stopped)?;
        controls.SetIsEnabled(false)?;
        println!("Fixture completed; disposing native playback and removing its temporary WAV.");
        Ok(())
    }
}
