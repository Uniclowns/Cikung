//! macOS has no supported system-wide now-playing reader for third-party apps.
//! Use the public scripting dictionaries of running Spotify, Music, and VLC.
//! Browser/YouTube discovery is deliberately unsupported on this backend.
//! Covers come from Spotify's artwork URL and Music's raw artwork data.

use std::{collections::HashMap, process::Command, sync::Arc, time::Duration};

use super::{MediaBackend, MediaCommand, MediaSession, PlaybackState};

const FIELD_SEPARATOR: char = '\u{1e}';

#[derive(Clone, Copy)]
struct Player {
    name: &'static str,
    bundle: &'static str,
}

const PLAYERS: [Player; 3] = [
    Player {
        name: "Spotify",
        bundle: "com.spotify.client",
    },
    Player {
        name: "Music",
        bundle: "com.apple.Music",
    },
    Player {
        name: "VLC",
        bundle: "org.videolan.vlc",
    },
];

/// Track key and its cover (or the remembered absence of one).
type CachedCover = (String, Option<Arc<Vec<u8>>>);

#[derive(Default)]
struct MacBackend {
    /// Last cover per player, keyed by track, so each song is looked up once.
    artwork: HashMap<&'static str, CachedCover>,
}

pub fn create() -> Result<Box<dyn MediaBackend>, String> {
    Ok(Box::new(MacBackend::default()))
}

impl MacBackend {
    fn artwork_for(&mut self, player: Player, session: &MediaSession) -> Option<Arc<Vec<u8>>> {
        let key = format!("{}\n{}\n{}", session.title, session.artist, session.album);
        if let Some((cached, art)) = self.artwork.get(player.bundle)
            && *cached == key
        {
            return art.clone();
        }
        // A missing cover is remembered for this track, not retried every poll.
        let art = read_artwork(player).map(Arc::new);
        self.artwork.insert(player.bundle, (key, art.clone()));
        art
    }
}

/// Cover bytes for the tinted notch: Spotify's artwork URL, or Music's raw artwork data.
fn read_artwork(player: Player) -> Option<Vec<u8>> {
    match player.name {
        "Spotify" => {
            let url = run_script(
                "with timeout of 2 seconds\ntell application id \"com.spotify.client\" to return artwork url of current track\nend timeout",
            )
            .ok()?;
            let url = url.trim();
            // Only Spotify's own image CDN, never an arbitrary address from script output.
            if !url.starts_with("https://i.scdn.co/") {
                return None;
            }
            let bytes = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .ok()?
                .get(url)
                .send()
                .ok()?
                .error_for_status()
                .ok()?
                .bytes()
                .ok()?;
            (bytes.len() <= 8 * 1024 * 1024).then(|| bytes.to_vec())
        }
        "Music" => {
            let path = std::env::temp_dir().join(format!("cikung-cover-{}", std::process::id()));
            let text = path.to_str()?;
            if text.contains(['"', '\\']) {
                return None;
            }
            let script = format!(
                r#"with timeout of 3 seconds
tell application id "com.apple.Music"
if (count of artworks of current track) is 0 then return "none"
set coverData to raw data of artwork 1 of current track
end tell
end timeout
set coverFile to open for access (POSIX file "{text}") with write permission
try
set eof coverFile to 0
write coverData to coverFile
end try
close access coverFile
return "ok""#
            );
            if run_script(&script).ok()?.trim() != "ok" {
                return None;
            }
            let bytes = std::fs::read(&path).ok();
            let _ = std::fs::remove_file(&path);
            bytes.filter(|bytes| !bytes.is_empty())
        }
        _ => None,
    }
}

impl MediaBackend for MacBackend {
    fn poll(&mut self) -> Result<Vec<MediaSession>, String> {
        let mut sessions = Vec::new();
        let mut failures = Vec::new();
        for player in PLAYERS {
            if !is_running(player)? {
                continue;
            }
            let script = metadata_script(player);
            match run_script(&script).and_then(|output| parse_session(player, &output)) {
                Ok(Some(mut session)) => {
                    session.artwork = self.artwork_for(player, &session);
                    sessions.push(session);
                }
                Ok(None) => (),
                Err(error) => failures.push(format!("{}: {error}", player.name)),
            }
        }
        // A permission failure in one application must not hide healthy players.
        if sessions.is_empty() && !failures.is_empty() {
            Err(format!(
                "macOS media access failed. Check System Settings > Privacy & Security > Automation. {}",
                failures.join("; ")
            ))
        } else {
            Ok(sessions)
        }
    }

    fn control(&mut self, session_id: &str, command: MediaCommand) -> Result<(), String> {
        let player = PLAYERS
            .iter()
            .copied()
            .find(|player| player.bundle == session_id)
            .ok_or_else(|| "Unknown media player.".to_owned())?;
        if !is_running(player)? {
            return Err("This player has closed. Wait for the media list to refresh.".into());
        }
        let instruction = control_instruction(player, command)?;
        // Interpolation is restricted to fixed IDs/verbs, bools, and validated numbers.
        let script = format!(
            "with timeout of 2 seconds\nif application id \"{}\" is running then\ntell application id \"{}\"\n{}\nend tell\nreturn \"ok\"\nelse\nreturn \"not-running\"\nend if\nend timeout",
            player.bundle, player.bundle, instruction
        );
        if run_script(&script)?.trim() == "ok" {
            Ok(())
        } else {
            Err("The player closed before that control could be applied.".into())
        }
    }
}

fn control_instruction(player: Player, command: MediaCommand) -> Result<String, String> {
    let boolean = |value: bool| if value { "true" } else { "false" };
    Ok(match (command, player.name) {
        (MediaCommand::TogglePlayPause, "VLC") => "play".into(),
        (MediaCommand::TogglePlayPause, _) => "playpause".into(),
        (MediaCommand::Play, "VLC") => "if not playing then play".into(),
        (MediaCommand::Pause, "VLC") => "if playing then play".into(),
        (MediaCommand::Play, _) => "play".into(),
        (MediaCommand::Pause, _) => "pause".into(),
        (MediaCommand::Next, "VLC") => "next".into(),
        (MediaCommand::Previous, "VLC") => "previous".into(),
        (MediaCommand::Next, _) => "next track".into(),
        (MediaCommand::Previous, _) => "previous track".into(),
        (MediaCommand::Shuffle(value), "Spotify") => format!("set shuffling to {}", boolean(value)),
        (MediaCommand::Shuffle(value), "Music") => {
            format!("set shuffle enabled to {}", boolean(value))
        }
        (MediaCommand::Shuffle(value), "VLC") => {
            format!("set shuffled playback to {}", boolean(value))
        }
        (MediaCommand::Repeat(value), "Spotify") => format!("set repeating to {}", boolean(value)),
        (MediaCommand::Repeat(value), "Music") => {
            format!("set song repeat to {}", if value { "all" } else { "off" })
        }
        (MediaCommand::Repeat(value), "VLC") => format!(
            "set repeat one to false\nset repeat all to {}",
            boolean(value)
        ),
        (MediaCommand::Seek(seconds), _) => {
            if !seconds.is_finite() {
                return Err("Seek time must be finite.".into());
            }
            let target = seconds.clamp(0., 315_360_000.);
            let duration = match player.name {
                "VLC" => "duration of current item",
                "Spotify" => "((duration of current track) / 1000)",
                _ => "duration of current track",
            };
            let property = if player.name == "VLC" {
                "current time"
            } else {
                "player position"
            };
            format!(
                "set targetPosition to {target:.3}\nset trackDuration to {duration}\nif trackDuration > 0 and targetPosition > trackDuration then set targetPosition to trackDuration\nset {property} to targetPosition"
            )
        }
        _ => return Err("This player does not support that control.".into()),
    })
}

fn is_running(player: Player) -> Result<bool, String> {
    // Checking process presence first prevents AppleScript from launching an app.
    let status = Command::new("/usr/bin/pgrep")
        .args(["-x", player.name])
        .output()
        .map_err(|error| format!("Could not inspect running media players: {error}"))?
        .status;
    match status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(format!(
            "Could not check whether {} is running.",
            player.name
        )),
    }
}

fn run_script(script: &str) -> Result<String, String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .map_err(|error| format!("Could not run the media reader: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().into())
    }
}

fn metadata_script(player: Player) -> String {
    let read = if player.name == "VLC" {
        r#"
set stateText to "paused"
if playing then set stateText to "playing"
try
    set titleText to name of current item as text
    set positionText to current time as text
    set durationText to duration of current item as text
    set shuffleText to shuffled playback as text
    set repeatText to ((repeat all) or (repeat one)) as text
end try
"#
        .to_owned()
    } else {
        let divisor = if player.name == "Spotify" { 1000 } else { 1 };
        let flags = if player.name == "Spotify" {
            "set shuffleText to shuffling as text\nset repeatText to repeating as text"
        } else {
            "set shuffleText to shuffle enabled as text\nset repeatText to ((song repeat) is not off) as text"
        };
        format!(
            r#"
set stateText to player state as text
try
    set titleText to name of current track as text
    set artistText to artist of current track as text
    set albumText to album of current track as text
    set positionText to player position as text
    set durationText to ((duration of current track) / {divisor}) as text
    {flags}
end try
"#
        )
    };
    format!(
        r#"with timeout of 2 seconds
if application id "{bundle}" is running then
tell application id "{bundle}"
set stateText to "stopped"
set titleText to ""
set artistText to ""
set albumText to ""
set positionText to "0"
set durationText to "0"
set shuffleText to ""
set repeatText to ""
{read}
set separatorText to ASCII character 30
return stateText & separatorText & titleText & separatorText & artistText & separatorText & albumText & separatorText & positionText & separatorText & durationText & separatorText & shuffleText & separatorText & repeatText
end tell
else
return "not-running"
end if
end timeout"#,
        bundle = player.bundle,
    )
}

fn parse_session(player: Player, output: &str) -> Result<Option<MediaSession>, String> {
    let output = output.trim_end_matches(['\r', '\n']);
    if output == "not-running" {
        return Ok(None);
    }
    let fields: Vec<&str> = output.split(FIELD_SEPARATOR).collect();
    if fields.len() != 6 && fields.len() != 8 {
        return Err("The player returned incomplete media information.".into());
    }
    let state = match fields[0] {
        "playing" => PlaybackState::Playing,
        "paused" => PlaybackState::Paused,
        _ => PlaybackState::Stopped,
    };
    // A running app with no track is not a media session.
    if fields[1].trim().is_empty() && state != PlaybackState::Playing {
        return Ok(None);
    }
    let seconds = |value: &str| {
        value
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or_default()
    };
    Ok(Some(MediaSession {
        id: player.bundle.into(),
        source: player.name.into(),
        title: fields[1].into(),
        artist: fields[2].into(),
        album: fields[3].into(),
        state,
        position_secs: seconds(fields[4]),
        duration_secs: seconds(fields[5]),
        shuffle: fields.get(6).and_then(|field| field.parse().ok()),
        repeat: fields.get(7).and_then(|field| field.parse().ok()),
        can_seek: seconds(fields[5]) > 0.,
        artwork: None,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_metadata_as_data_and_preserves_newlines() {
        let session = parse_session(
            PLAYERS[0],
            "playing\u{1e}A \"song\"\nsecond line\u{1e}Artist\u{1e}Album\u{1e}12,5\u{1e}180\n",
        )
        .unwrap()
        .unwrap();
        assert_eq!(session.title, "A \"song\"\nsecond line");
        assert_eq!(session.position_secs, 12.5);
        assert_eq!(session.state, PlaybackState::Playing);
    }

    #[test]
    fn rejects_partial_results_and_invalid_times() {
        assert!(parse_session(PLAYERS[0], "playing\u{1e}title").is_err());
        let session = parse_session(PLAYERS[0], "paused\u{1e}Song\u{1e}\u{1e}\u{1e}NaN\u{1e}-1")
            .unwrap()
            .unwrap();
        assert_eq!(session.position_secs, 0.0);
        assert_eq!(session.duration_secs, 0.0);
    }

    #[test]
    fn omits_apps_without_a_track_and_apps_that_closed() {
        assert!(
            parse_session(PLAYERS[2], "paused\u{1e}\u{1e}\u{1e}\u{1e}0\u{1e}0")
                .unwrap()
                .is_none()
        );
        assert!(
            parse_session(PLAYERS[0], "not-running\n")
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn controls_only_interpolate_validated_numeric_values() {
        assert!(control_instruction(PLAYERS[0], MediaCommand::Seek(f64::NAN)).is_err());
        assert!(
            control_instruction(PLAYERS[0], MediaCommand::Seek(-5.))
                .unwrap()
                .contains("0.000")
        );
        assert_eq!(
            control_instruction(PLAYERS[2], MediaCommand::Pause).unwrap(),
            "if playing then play"
        );
        assert!(
            control_instruction(PLAYERS[1], MediaCommand::Repeat(true))
                .unwrap()
                .contains("song repeat to all")
        );
    }
}
