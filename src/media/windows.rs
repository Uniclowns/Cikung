use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use windows::{
    Media::{
        Control::{
            GlobalSystemMediaTransportControlsSession as Session,
            GlobalSystemMediaTransportControlsSessionManager as SessionManager,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
        },
        MediaPlaybackAutoRepeatMode,
    },
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{IUnknown, Interface},
};

use super::{MediaBackend, MediaCommand, MediaSession, PlaybackState};

type ArtworkCache = HashMap<String, (String, Option<std::sync::Arc<Vec<u8>>>)>;
struct WindowsBackend {
    // COM objects must be released before RoUninitialize (in ComApartment::drop).
    manager: SessionManager,
    sessions: Vec<(String, Session)>,
    artwork: ArtworkCache,
    _apartment: ComApartment,
}

struct ComApartment;

impl Drop for ComApartment {
    fn drop(&mut self) {
        // This guard is only created after a successful initialization on this thread.
        unsafe { RoUninitialize() };
    }
}

pub fn create() -> Result<Box<dyn MediaBackend>, String> {
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|error| format!("Could not initialize Windows media access: {error}"))?;
    let apartment = ComApartment;
    let manager = SessionManager::RequestAsync()
        .and_then(|request| request.join())
        .map_err(|error| format!("Could not connect to Windows media sessions: {error}"))?;
    Ok(Box::new(WindowsBackend {
        manager,
        sessions: Vec::new(),
        artwork: HashMap::new(),
        _apartment: apartment,
    }))
}

impl MediaBackend for WindowsBackend {
    fn poll(&mut self) -> Result<Vec<MediaSession>, String> {
        let native_sessions = self
            .manager
            .GetSessions()
            .map_err(|error| format!("Could not list Windows media sessions: {error}"))?;
        let count = native_sessions
            .Size()
            .map_err(|error| format!("Could not count Windows media sessions: {error}"))?;
        let mut result = Vec::new();
        let mut handles = Vec::new();
        for index in 0..count {
            // A closing player must not hide every other player.
            let Ok(session) = native_sessions.GetAt(index) else {
                continue;
            };
            let Ok(source_id) = session.SourceAppUserModelId() else {
                continue;
            };
            let source_id = source_id.to_string_lossy();
            // COM's canonical IUnknown identity stays stable within this apartment,
            // unlike list indices when another browser/player session closes.
            let id = match session.cast::<IUnknown>() {
                Ok(identity) => format!("{source_id}@{:p}", identity.as_raw()),
                Err(_) => format!("{source_id}#{index}"),
            };
            let state = session
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .map(|status| match status {
                    Status::Playing => PlaybackState::Playing,
                    Status::Paused => PlaybackState::Paused,
                    _ => PlaybackState::Stopped,
                })
                .unwrap_or(PlaybackState::Stopped);
            let properties = session
                .TryGetMediaPropertiesAsync()
                .and_then(|request| request.join())
                .ok();
            let title = properties
                .as_ref()
                .and_then(|value| value.Title().ok())
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            let artist = properties
                .as_ref()
                .and_then(|value| value.Artist().ok())
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            let album = properties
                .as_ref()
                .and_then(|value| value.AlbumTitle().ok())
                .map(|value| value.to_string_lossy())
                .unwrap_or_default();
            let (position_secs, duration_secs) = session
                .GetTimelineProperties()
                .ok()
                .map(|timeline| {
                    let start = timeline.StartTime().map(|t| t.Duration).unwrap_or(0);
                    let end = timeline.EndTime().map(|t| t.Duration).unwrap_or(0);
                    let position = timeline.Position().map(|t| t.Duration).unwrap_or(start);
                    let duration_secs = end.saturating_sub(start).max(0) as f64 / TICKS_PER_SECOND;
                    let position_secs =
                        position.saturating_sub(start).max(0) as f64 / TICKS_PER_SECOND;
                    // Windows can retain one position snapshot across many polls.
                    // Advance from its timestamp, not from our most recent poll.
                    let position_secs = current_position(
                        position_secs,
                        duration_secs,
                        state,
                        timeline
                            .LastUpdatedTime()
                            .ok()
                            .map(|time| time.UniversalTime),
                        windows_time_ticks(SystemTime::now()),
                    );
                    (position_secs, duration_secs)
                })
                .unwrap_or_default();
            let playback_info = session.GetPlaybackInfo().ok();
            let controls = playback_info.as_ref().and_then(|info| info.Controls().ok());
            let can_seek = controls
                .as_ref()
                .and_then(|controls| controls.IsPlaybackPositionEnabled().ok())
                .unwrap_or(false);
            let shuffle = controls
                .as_ref()
                .filter(|controls| controls.IsShuffleEnabled().unwrap_or(false))
                .and_then(|_| playback_info.as_ref()?.IsShuffleActive().ok()?.Value().ok());
            let repeat = controls
                .as_ref()
                .filter(|controls| controls.IsRepeatEnabled().unwrap_or(false))
                .and_then(|_| playback_info.as_ref()?.AutoRepeatMode().ok()?.Value().ok())
                .map(|mode| mode != MediaPlaybackAutoRepeatMode::None);
            let track_key = format!("{title}\u{1f}{artist}\u{1f}{album}");
            if !self
                .artwork
                .get(&id)
                .is_some_and(|(key, _)| key == &track_key)
            {
                let bytes = properties
                    .as_ref()
                    .and_then(read_artwork)
                    .map(std::sync::Arc::new);
                self.artwork.insert(id.clone(), (track_key, bytes));
            }
            let artwork = self
                .artwork
                .get(&id)
                .and_then(|(_, artwork)| artwork.clone());
            result.push(MediaSession {
                id: id.clone(),
                source: friendly_source(&source_id),
                title,
                artist,
                album,
                state,
                position_secs,
                duration_secs,
                shuffle,
                repeat,
                can_seek,
                artwork,
            });
            handles.push((id, session));
        }
        self.artwork
            .retain(|id, _| handles.iter().any(|(active, _)| active == id));
        self.sessions = handles;
        Ok(result)
    }

    fn control(&mut self, session_id: &str, command: MediaCommand) -> Result<(), String> {
        let session = self
            .sessions
            .iter()
            .find(|(id, _)| id == session_id)
            .map(|(_, session)| session)
            .ok_or_else(|| {
                "This player has closed. Wait for the media list to refresh.".to_owned()
            })?;
        if let Ok(controls) = session.GetPlaybackInfo().and_then(|info| info.Controls()) {
            let enabled = match command {
                MediaCommand::Next => controls.IsNextEnabled(),
                MediaCommand::Previous => controls.IsPreviousEnabled(),
                MediaCommand::TogglePlayPause => controls.IsPlayPauseToggleEnabled(),
                MediaCommand::Play => controls.IsPlayEnabled(),
                MediaCommand::Pause => controls.IsPauseEnabled(),
                MediaCommand::Seek(_) => controls.IsPlaybackPositionEnabled(),
                MediaCommand::Shuffle(_) => controls.IsShuffleEnabled(),
                MediaCommand::Repeat(_) => controls.IsRepeatEnabled(),
            };
            if matches!(enabled, Ok(false)) {
                return Err("This player does not support that playback control.".into());
            }
        }
        let operation = match command {
            MediaCommand::TogglePlayPause => session.TryTogglePlayPauseAsync(),
            MediaCommand::Next => session.TrySkipNextAsync(),
            MediaCommand::Previous => session.TrySkipPreviousAsync(),
            MediaCommand::Play => session.TryPlayAsync(),
            MediaCommand::Pause => session.TryPauseAsync(),
            MediaCommand::Shuffle(enabled) => session.TryChangeShuffleActiveAsync(enabled),
            MediaCommand::Repeat(enabled) => session.TryChangeAutoRepeatModeAsync(if enabled {
                MediaPlaybackAutoRepeatMode::List
            } else {
                MediaPlaybackAutoRepeatMode::None
            }),
            MediaCommand::Seek(seconds) => {
                let timeline = session
                    .GetTimelineProperties()
                    .map_err(|e| format!("Cannot read the current media timeline: {e}"))?;
                let start = timeline.StartTime().map_err(|e| e.to_string())?.Duration;
                let end = timeline.EndTime().map_err(|e| e.to_string())?.Duration;
                session.TryChangePlaybackPositionAsync(seek_ticks(seconds, start, end)?)
            }
        };
        match operation.and_then(|operation| operation.join()) {
            Ok(true) => Ok(()),
            Ok(false) => Err("This player does not support that playback control.".into()),
            Err(error) => Err(format!("The player could not apply that control: {error}")),
        }
    }
}

fn seek_ticks(seconds: f64, start: i64, end: i64) -> Result<i64, String> {
    if !seconds.is_finite() {
        return Err("Seek time must be finite.".into());
    }
    let duration = end.saturating_sub(start).max(0) as f64 / TICKS_PER_SECOND;
    let seconds = if duration > 0. {
        seconds.clamp(0., duration)
    } else {
        seconds.clamp(0., MAX_TIMELINE_SECONDS)
    };
    Ok(start.saturating_add((seconds * TICKS_PER_SECOND) as i64))
}

fn read_artwork(
    properties: &windows::Media::Control::GlobalSystemMediaTransportControlsSessionMediaProperties,
) -> Option<Vec<u8>> {
    use windows::Storage::Streams::DataReader;
    let stream = properties
        .Thumbnail()
        .ok()?
        .OpenReadAsync()
        .ok()?
        .join()
        .ok()?;
    let length = stream.Size().ok()?;
    if length == 0 || length > 2_000_000 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    let bytes = (|| {
        let loaded = reader.LoadAsync(length as u32).ok()?.join().ok()?;
        if u64::from(loaded) != length {
            return None;
        }
        let mut bytes = vec![0; length as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        Some(bytes)
    })();
    let _ = reader.Close();
    bytes
}

const TICKS_PER_SECOND: f64 = 10_000_000.0;
const WINDOWS_UNIX_EPOCH_TICKS: u128 = 116_444_736_000_000_000;
const MAX_TIMELINE_SECONDS: f64 = i64::MAX as f64 / TICKS_PER_SECOND;

fn windows_time_ticks(time: SystemTime) -> Option<i64> {
    let since_unix = time.duration_since(UNIX_EPOCH).ok()?;
    // WinRT DateTime counts 100-nanosecond ticks since 1601-01-01 UTC.
    let ticks = WINDOWS_UNIX_EPOCH_TICKS.checked_add(since_unix.as_nanos() / 100)?;
    ticks.try_into().ok()
}

fn current_position(
    position_secs: f64,
    duration_secs: f64,
    state: PlaybackState,
    updated_ticks: Option<i64>,
    now_ticks: Option<i64>,
) -> f64 {
    let safe_seconds = |value: f64| {
        if value.is_finite() {
            value.clamp(0.0, MAX_TIMELINE_SECONDS)
        } else {
            0.0
        }
    };
    let mut position = safe_seconds(position_secs);
    // Zero/missing dates are unspecified. A future date must not rewind.
    if state == PlaybackState::Playing
        && let (Some(updated), Some(now)) = (updated_ticks, now_ticks)
        && updated > 0
        && now > 0
    {
        let elapsed = now.saturating_sub(updated).max(0) as f64 / TICKS_PER_SECOND;
        position = (position + elapsed).min(MAX_TIMELINE_SECONDS);
    }
    let duration = safe_seconds(duration_secs);
    if duration > 0.0 {
        position.min(duration)
    } else {
        position
    }
}

fn friendly_source(source: &str) -> String {
    let lower = source.to_ascii_lowercase();
    // Firefox's Start-menu AppUserModelId on this Windows installation.
    if lower == "308046b0af4a39cb" {
        return "Firefox".into();
    }
    if source.len() >= 8 && source.bytes().all(|c| c.is_ascii_hexdigit()) {
        return "Media player".into();
    }
    for (needle, label) in [
        ("spotify", "Spotify"),
        ("vlc", "VLC"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("brave", "Brave"),
        ("music", "Music"),
    ] {
        if lower.contains(needle) {
            return label.into();
        }
    }
    source.strip_suffix(".exe").unwrap_or(source).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPDATED: i64 = 116_444_736_000_000_000;
    const TEN_SECONDS: i64 = 100_000_000;
    #[test]
    fn media_source_labels_hide_opaque_system_identifiers() {
        assert_eq!(friendly_source("308046B0AF4A39CB"), "Firefox");
        assert_eq!(friendly_source("0123456789ABCDEF"), "Media player");
        assert_eq!(friendly_source("Spotify.exe"), "Spotify");
        assert_eq!(friendly_source("VLC.exe"), "VLC");
        assert_eq!(friendly_source("chrome.exe"), "Chrome");
        assert_eq!(friendly_source("Custom Player.exe"), "Custom Player");
    }
    #[test]
    fn seeks_are_finite_clamped_and_relative_to_timeline_start() {
        assert!(seek_ticks(f64::NAN, 0, 100).is_err());
        assert_eq!(seek_ticks(-4., 100, 20_000_100).unwrap(), 100);
        assert_eq!(seek_ticks(40., 100, 20_000_100).unwrap(), 20_000_100);
    }

    #[test]
    fn advances_from_the_windows_snapshot_across_polls() {
        assert_eq!(
            current_position(
                30.0,
                200.0,
                PlaybackState::Playing,
                Some(UPDATED),
                Some(UPDATED + TEN_SECONDS)
            ),
            40.0
        );
        assert_eq!(
            current_position(
                30.0,
                200.0,
                PlaybackState::Playing,
                Some(UPDATED),
                Some(UPDATED + 2 * TEN_SECONDS)
            ),
            50.0
        );
    }

    #[test]
    fn does_not_advance_paused_or_stopped_sessions() {
        for state in [PlaybackState::Paused, PlaybackState::Stopped] {
            assert_eq!(
                current_position(
                    30.0,
                    200.0,
                    state,
                    Some(UPDATED),
                    Some(UPDATED + TEN_SECONDS)
                ),
                30.0
            );
        }
    }

    #[test]
    fn ignores_future_or_missing_snapshot_times() {
        for updated in [None, Some(0), Some(UPDATED + TEN_SECONDS)] {
            assert_eq!(
                current_position(30.0, 200.0, PlaybackState::Playing, updated, Some(UPDATED)),
                30.0
            );
        }
        assert_eq!(
            current_position(30.0, 200.0, PlaybackState::Playing, Some(UPDATED), None),
            30.0
        );
    }

    #[test]
    fn clamps_at_the_track_end_and_keeps_unknown_duration_finite() {
        assert_eq!(
            current_position(
                195.0,
                200.0,
                PlaybackState::Playing,
                Some(UPDATED),
                Some(UPDATED + TEN_SECONDS)
            ),
            200.0
        );
        assert_eq!(
            current_position(
                30.0,
                0.0,
                PlaybackState::Playing,
                Some(UPDATED),
                Some(UPDATED + TEN_SECONDS)
            ),
            40.0
        );
        let position = current_position(
            f64::INFINITY,
            f64::NAN,
            PlaybackState::Playing,
            Some(1),
            Some(i64::MAX),
        );
        assert!(position.is_finite() && (0.0..=MAX_TIMELINE_SECONDS).contains(&position));
    }

    #[test]
    fn converts_system_time_to_the_windows_epoch() {
        assert_eq!(windows_time_ticks(UNIX_EPOCH), Some(UPDATED));
        assert_eq!(
            windows_time_ticks(UNIX_EPOCH + std::time::Duration::from_secs(10)),
            Some(UPDATED + TEN_SECONDS)
        );
    }
}
