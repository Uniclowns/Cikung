use mpris::{LoopStatus, PlaybackStatus, PlayerFinder};

use super::{MediaBackend, MediaCommand, MediaSession, PlaybackState};

struct LinuxBackend {
    finder: PlayerFinder,
}

pub fn create() -> Result<Box<dyn MediaBackend>, String> {
    let mut finder = PlayerFinder::new()
        .map_err(|error| format!("Could not connect to the desktop media bus (MPRIS): {error}"))?;
    finder.set_player_timeout_ms(500);
    Ok(Box::new(LinuxBackend { finder }))
}

impl MediaBackend for LinuxBackend {
    fn poll(&mut self) -> Result<Vec<MediaSession>, String> {
        let players = self
            .finder
            .iter_players()
            .map_err(|error| format!("Could not discover MPRIS players: {error}"))?;
        let mut sessions = Vec::new();
        for player in players.filter_map(Result::ok) {
            // Disappearing or nonresponsive players should not hide healthy ones.
            let Ok(status) = player.get_playback_status() else {
                continue;
            };
            let metadata = player.get_metadata().ok();
            sessions.push(MediaSession {
                id: player.bus_name().into(),
                source: player.identity().into(),
                title: metadata
                    .as_ref()
                    .and_then(|value| value.title())
                    .unwrap_or_default()
                    .into(),
                artist: metadata
                    .as_ref()
                    .and_then(|value| value.artists())
                    .unwrap_or_default()
                    .join(", "),
                album: metadata
                    .as_ref()
                    .and_then(|value| value.album_name())
                    .unwrap_or_default()
                    .into(),
                state: match status {
                    PlaybackStatus::Playing => PlaybackState::Playing,
                    PlaybackStatus::Paused => PlaybackState::Paused,
                    PlaybackStatus::Stopped => PlaybackState::Stopped,
                },
                position_secs: player
                    .get_position()
                    .map(|time| time.as_secs_f64())
                    .unwrap_or_default(),
                duration_secs: metadata
                    .as_ref()
                    .and_then(|value| value.length())
                    .map(|time| time.as_secs_f64())
                    .unwrap_or_default(),
                shuffle: player.get_shuffle().ok(),
                repeat: player
                    .get_loop_status()
                    .ok()
                    .map(|state| state != LoopStatus::None),
                can_seek: player.can_seek().unwrap_or(false),
                artwork: None,
            });
        }
        Ok(sessions)
    }

    fn control(&mut self, session_id: &str, command: MediaCommand) -> Result<(), String> {
        let players = self
            .finder
            .iter_players()
            .map_err(|error| format!("Could not discover MPRIS players: {error}"))?;
        let player = players
            .filter_map(Result::ok)
            .find(|player| player.bus_name() == session_id)
            .ok_or_else(|| {
                "This player has closed. Wait for the media list to refresh.".to_owned()
            })?;
        let result = match command {
            MediaCommand::TogglePlayPause => player.play_pause(),
            MediaCommand::Play => player.play(),
            MediaCommand::Pause => player.pause(),
            MediaCommand::Next => player.next(),
            MediaCommand::Previous => player.previous(),
            MediaCommand::Shuffle(enabled) => player.set_shuffle(enabled),
            MediaCommand::Repeat(enabled) => player.set_loop_status(if enabled {
                LoopStatus::Playlist
            } else {
                LoopStatus::None
            }),
            MediaCommand::Seek(seconds) => {
                if !seconds.is_finite() {
                    return Err("Seek time must be a finite number.".into());
                }
                if !player.can_seek().unwrap_or(false) {
                    return Err("This player does not support seeking.".into());
                }
                let metadata = player
                    .get_metadata()
                    .map_err(|e| format!("Cannot read the current track: {e}"))?;
                let track = metadata
                    .track_id()
                    .ok_or("This player does not provide a current-track ID for safe seeking.")?;
                let duration = metadata
                    .length()
                    .map(|duration| duration.as_secs_f64())
                    .unwrap_or(f64::MAX);
                let seconds = seconds
                    .max(0.)
                    .min(duration)
                    .min(u64::MAX as f64 / 1_000_000.);
                player.set_position(track, &std::time::Duration::from_secs_f64(seconds))
            }
        };
        result.map_err(|error| format!("The player could not apply that control: {error}"))
    }
}
