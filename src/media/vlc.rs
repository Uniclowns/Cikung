//! Optional VLC HTTP fallback, restricted to the local VLC endpoint.
use super::{MediaBackend, MediaCommand, MediaSession, PlaybackState};
use reqwest::blocking::Client;
use serde_json::Value;
use std::{
    io::Read,
    time::{Duration, Instant},
};

const ENDPOINT: &str = "http://127.0.0.1:8080/requests/status.json";
const SESSION_ID: &str = "cikung-vlc-http";
const MAX_BODY: u64 = 2_000_000;

pub(super) fn wrap(
    native: Result<Box<dyn MediaBackend>, String>,
) -> Result<Box<dyn MediaBackend>, String> {
    let vlc = match VlcBackend::new() {
        Ok(vlc) => vlc,
        Err(error) => return native.map_err(|native_error| format!("{native_error} {error}")),
    };
    let (native, native_error) = match native {
        Ok(native) => (Some(native), None),
        Err(error) if vlc.password.is_some() => (None, Some(error)),
        Err(error) => return Err(error),
    };
    Ok(Box::new(CombinedBackend {
        native,
        native_error,
        vlc,
    }))
}

struct CombinedBackend {
    native: Option<Box<dyn MediaBackend>>,
    native_error: Option<String>,
    vlc: VlcBackend,
}
impl MediaBackend for CombinedBackend {
    fn poll(&mut self) -> Result<Vec<MediaSession>, String> {
        let mut sessions = match self.native.as_mut().map(|native| native.poll()) {
            Some(Ok(sessions)) => {
                self.native_error = None;
                sessions
            }
            Some(Err(error)) => {
                self.native_error = Some(error);
                Vec::new()
            }
            None => Vec::new(),
        };
        if !sessions
            .iter()
            .any(|session| session.source.to_ascii_lowercase().contains("vlc"))
        {
            match self.vlc.poll() {
                Ok(Some(session)) => sessions.push(session),
                Err(error) if sessions.is_empty() => return Err(error),
                _ => (), // A local VLC outage must not hide other healthy players.
            }
        }
        if sessions.is_empty()
            && let Some(error) = &self.native_error
        {
            return Err(error.clone());
        }
        Ok(sessions)
    }
    fn control(&mut self, id: &str, command: MediaCommand) -> Result<(), String> {
        if id == SESSION_ID {
            return self.vlc.control(command);
        }
        self.native
            .as_mut()
            .ok_or("The native media service is unavailable.")?
            .control(id, command)
    }
}

struct VlcBackend {
    client: Client,
    password: Option<String>,
    credential_checked: Instant,
}
impl VlcBackend {
    fn new() -> Result<Self, String> {
        let client = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_millis(400))
            .timeout(Duration::from_millis(400))
            .build()
            .map_err(|_| "Could not initialize the local VLC connection.".to_owned())?;
        Ok(Self {
            client,
            password: saved_password(),
            credential_checked: Instant::now(),
        })
    }
    fn refresh_credentials(&mut self) {
        if self.credential_checked.elapsed() >= Duration::from_secs(10) {
            self.password = saved_password();
            self.credential_checked = Instant::now();
        }
    }
    fn request(&self, command: Option<(&str, Option<String>)>) -> Result<Value, String> {
        let password = self
            .password
            .as_deref()
            .ok_or("Save the VLC Web interface password in Settings first.")?;
        let mut request = self.client.get(ENDPOINT).basic_auth("", Some(password));
        if let Some((command, value)) = command {
            request = request.query(&[("command", command)]);
            if let Some(value) = value {
                request = request.query(&[("val", value)]);
            }
        }
        let response = request.send().map_err(|_| "Could not connect to VLC at 127.0.0.1:8080. Enable VLC's Web interface or remove the saved VLC password to disable the fallback.".to_owned())?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(
                "VLC rejected the saved Web interface password. Update it in Settings.".into(),
            );
        }
        if !response.status().is_success() {
            return Err(format!(
                "The local VLC interface returned HTTP {}.",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BODY)
        {
            return Err("VLC returned an oversized response.".into());
        }
        let mut bytes = Vec::new();
        response
            .take(MAX_BODY + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Could not read VLC's response.".to_owned())?;
        if bytes.len() as u64 > MAX_BODY {
            return Err("VLC returned an oversized response.".into());
        }
        serde_json::from_slice(&bytes)
            .map_err(|_| "VLC returned an invalid status response.".into())
    }
    fn poll(&mut self) -> Result<Option<MediaSession>, String> {
        self.refresh_credentials();
        // No saved password means no HTTP request and no connection error.
        if self.password.is_none() {
            return Ok(None);
        }
        parse_status(&self.request(None)?)
    }
    fn control(&mut self, command: MediaCommand) -> Result<(), String> {
        self.refresh_credentials();
        let status = self.request(None)?;
        for instruction in control_requests(command, &status)? {
            let _ = self.request(Some((instruction.0, instruction.1)))?;
        }
        Ok(())
    }
}
fn saved_password() -> Option<String> {
    // Same service/account as cloud::save_secret, decoupled from the GUI crate.
    keyring::Entry::new("Cikung", "service-vlc")
        .ok()?
        .get_password()
        .ok()
        .filter(|password| !password.is_empty())
}
fn flag(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| value.as_i64().map(|v| v != 0))
}
fn seconds(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.)
}
fn parse_status(status: &Value) -> Result<Option<MediaSession>, String> {
    let state = match status.get("state").and_then(Value::as_str) {
        Some("playing") => PlaybackState::Playing,
        Some("paused") => PlaybackState::Paused,
        Some("stopped") => PlaybackState::Stopped,
        _ => return Err("VLC returned an unknown playback state.".into()),
    };
    let metadata = status.pointer("/information/category/meta");
    let field = |name: &str| {
        metadata
            .and_then(|metadata| metadata.get(name))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let mut title = field("title");
    if title.is_empty() {
        title = field("filename");
    }
    if title.is_empty() && state == PlaybackState::Stopped {
        return Ok(None);
    }
    let duration_secs = seconds(&status["length"]).unwrap_or(0.);
    let mut position_secs = seconds(&status["time"])
        .or_else(|| seconds(&status["position"]).map(|position| position * duration_secs))
        .unwrap_or(0.);
    if duration_secs > 0. {
        position_secs = position_secs.min(duration_secs);
    }
    Ok(Some(MediaSession {
        id: SESSION_ID.into(),
        source: "VLC".into(),
        title,
        artist: field("artist"),
        album: field("album"),
        state,
        position_secs,
        duration_secs,
        shuffle: flag(&status["random"]),
        repeat: flag(&status["loop"])
            .zip(flag(&status["repeat"]))
            .map(|(all, one)| all || one),
        can_seek: duration_secs > 0.,
        artwork: None,
    }))
}
type Instruction = (&'static str, Option<String>);
fn control_requests(command: MediaCommand, status: &Value) -> Result<Vec<Instruction>, String> {
    let mut result = Vec::new();
    let simple = match command {
        MediaCommand::TogglePlayPause => Some("pl_pause"),
        MediaCommand::Play => Some(if status["state"] == "paused" {
            "pl_forceresume"
        } else {
            "pl_play"
        }),
        MediaCommand::Pause => Some("pl_forcepause"),
        MediaCommand::Next => Some("pl_next"),
        MediaCommand::Previous => Some("pl_previous"),
        _ => None,
    };
    if let Some(command) = simple {
        result.push((command, None));
    }
    match command {
        MediaCommand::Seek(time) => {
            if !time.is_finite() {
                return Err("Seek time must be finite.".into());
            }
            let duration = seconds(&status["length"])
                .filter(|length| *length > 0.)
                .ok_or("This VLC item does not expose a duration for seeking.")?;
            result.push(("seek", Some(format!("{:.3}", time.clamp(0., duration)))));
        }
        MediaCommand::Shuffle(value) => {
            let current =
                flag(&status["random"]).ok_or("VLC does not expose its shuffle state.")?;
            if current != value {
                result.push(("pl_random", None));
            }
        }
        MediaCommand::Repeat(value) => {
            let all =
                flag(&status["loop"]).ok_or("VLC does not expose its playlist-repeat state.")?;
            let one =
                flag(&status["repeat"]).ok_or("VLC does not expose its track-repeat state.")?;
            if one {
                result.push(("pl_repeat", None));
            }
            if all != value {
                result.push(("pl_loop", None));
            }
        }
        _ => (),
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn backend(password: Option<&str>) -> VlcBackend {
        VlcBackend {
            client: Client::builder().no_proxy().build().unwrap(),
            password: password.map(str::to_owned),
            credential_checked: Instant::now(),
        }
    }
    #[test]
    fn disabled_fallback_needs_no_credentials_or_network() {
        assert!(backend(None).poll().unwrap().is_none());
    }
    #[test]
    fn native_vlc_prevents_http_duplicate() {
        struct Native;
        impl MediaBackend for Native {
            fn poll(&mut self) -> Result<Vec<MediaSession>, String> {
                Ok(vec![parse_status(&status())?.unwrap()])
            }
            fn control(&mut self, _: &str, _: MediaCommand) -> Result<(), String> {
                Ok(())
            }
        }
        let mut combined = CombinedBackend {
            native: Some(Box::new(Native)),
            native_error: None,
            vlc: backend(Some("synthetic-test-password-never-sent")),
        };
        assert_eq!(combined.poll().unwrap().len(), 1);
    }
    fn status() -> Value {
        serde_json::json!({"state":"paused","length":180,"time":22,"random":false,"loop":false,"repeat":false,"information":{"category":{"meta":{"title":"แมว 🐈","artist":"An artist","album":"Cats"}}}})
    }
    #[test]
    fn reads_real_metadata_and_flags_and_omits_empty_stopped_player() {
        let session = parse_status(&status()).unwrap().unwrap();
        assert_eq!(session.title, "แมว 🐈");
        assert_eq!(session.position_secs, 22.);
        assert_eq!(session.shuffle, Some(false));
        assert!(session.can_seek);
        assert!(
            parse_status(&serde_json::json!({"state":"stopped"}))
                .unwrap()
                .is_none()
        );
        assert!(parse_status(&serde_json::json!({"state":"unknown"})).is_err());
    }
    #[test]
    fn uses_absolute_seek_and_state_aware_controls() {
        assert_eq!(
            control_requests(MediaCommand::Play, &status()).unwrap()[0].0,
            "pl_forceresume"
        );
        assert_eq!(
            control_requests(MediaCommand::Pause, &status()).unwrap()[0].0,
            "pl_forcepause"
        );
        assert_eq!(
            control_requests(MediaCommand::Seek(500.), &status()).unwrap(),
            vec![("seek", Some("180.000".into()))]
        );
        assert!(control_requests(MediaCommand::Seek(f64::NAN), &status()).is_err());
        assert!(
            control_requests(MediaCommand::Shuffle(false), &status())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            control_requests(MediaCommand::Shuffle(true), &status()).unwrap()[0].0,
            "pl_random"
        );
        let mut current = status();
        current["repeat"] = Value::Bool(true);
        current["loop"] = Value::Bool(true);
        assert_eq!(
            control_requests(MediaCommand::Repeat(false), &current).unwrap(),
            vec![("pl_repeat", None), ("pl_loop", None)]
        );
    }
}
