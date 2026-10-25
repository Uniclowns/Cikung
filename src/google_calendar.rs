//! Google Calendar for tasks with a due time. Uses the user's own OAuth client (Desktop app
//! type) with a loopback sign-in and PKCE, and keeps the refresh token in the system
//! credential store. Only the `calendar.events` scope: Cikung adds, moves and removes the
//! events it made, in the primary calendar.
use crate::cloud::{http, read_secret, remove_secret, save_secret};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use reqwest::blocking::RequestBuilder;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

pub const CLIENT_ID: &str = "google-client-id";
const CLIENT_SECRET: &str = "google-client-secret";
const REFRESH_TOKEN: &str = "google-refresh-token";
const SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";
const AUTH: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const EVENTS: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
/// An event lasts half an hour from the task's due time.
const LENGTH_SECS: i64 = 30 * 60;

static ACCESS: Mutex<Option<(String, Instant)>> = Mutex::new(None);

pub fn connected() -> bool {
    read_secret(REFRESH_TOKEN)
        .ok()
        .flatten()
        .is_some_and(|token| !token.is_empty())
}

fn random(bytes: usize) -> Result<String, String> {
    let mut buffer = vec![0; bytes];
    getrandom::fill(&mut buffer).map_err(|_| "Could not create a sign-in code.".to_owned())?;
    Ok(URL_SAFE_NO_PAD.encode(buffer))
}

/// PKCE S256: the browser only ever sees this hash of the verifier.
fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

/// A started sign-in: the page to open, and the loopback listener Google sends the browser to.
pub struct SignIn {
    pub url: String,
    server: tiny_http::Server,
    redirect: String,
    verifier: String,
    state: String,
    client_id: String,
    client_secret: String,
}

pub fn begin_sign_in(client_id: &str, client_secret: &str) -> Result<SignIn, String> {
    let (client_id, client_secret) = (client_id.trim(), client_secret.trim());
    if client_id.is_empty() || client_secret.is_empty() {
        return Err("Enter the OAuth client ID and client secret first.".into());
    }
    let server = tiny_http::Server::http("127.0.0.1:0")
        .map_err(|_| "Could not open a local port for the Google sign-in.".to_owned())?;
    let port = server
        .server_addr()
        .to_ip()
        .map(|address| address.port())
        .ok_or("Could not open a local port for the Google sign-in.")?;
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = random(32)?;
    let state = random(16)?;
    let url = reqwest::Url::parse_with_params(
        AUTH,
        &[
            ("client_id", client_id),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", SCOPE),
            ("code_challenge", &challenge(&verifier)),
            ("code_challenge_method", "S256"),
            ("state", &state),
            // A refresh token, so the link outlives this session.
            ("access_type", "offline"),
            ("prompt", "consent"),
        ],
    )
    .map_err(|_| "Could not build the Google sign-in link.".to_owned())?;
    Ok(SignIn {
        url: url.into(),
        server,
        redirect,
        verifier,
        state,
        client_id: client_id.into(),
        client_secret: client_secret.into(),
    })
}

impl SignIn {
    /// Waits up to five minutes for Google to send the browser back, then keeps the sign-in.
    pub fn finish(self) -> Result<(), String> {
        let code = self.wait_for_code()?;
        let reply = token_request(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", &self.redirect),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("code_verifier", &self.verifier),
        ])?;
        let refresh = reply["refresh_token"]
            .as_str()
            .filter(|token| !token.is_empty())
            .ok_or("Google did not return a lasting sign-in. Remove Cikung under Third-party access in your Google account, then connect again.")?;
        save_secret(CLIENT_ID, &self.client_id)?;
        save_secret(CLIENT_SECRET, &self.client_secret)?;
        save_secret(REFRESH_TOKEN, refresh)?;
        keep_access(&reply);
        Ok(())
    }

    fn wait_for_code(&self) -> Result<String, String> {
        let timed_out = || "Google sign-in timed out. Press Connect Google again.".to_owned();
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            let left = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(timed_out)?;
            let request = self
                .server
                .recv_timeout(left)
                .ok()
                .flatten()
                .ok_or_else(timed_out)?;
            let url = reqwest::Url::parse(&format!("http://127.0.0.1{}", request.url())).ok();
            let query = |key: &str| {
                url.as_ref().and_then(|url| {
                    url.query_pairs()
                        .find(|(k, _)| k == key)
                        .map(|(_, v)| v.into_owned())
                })
            };
            // Anything without our state (a favicon, a stray tab) is not the answer.
            if query("state").as_deref() != Some(self.state.as_str()) {
                let _ = request.respond(tiny_http::Response::empty(404));
                continue;
            }
            let code = query("code");
            let page = if code.is_some() {
                "Cikung is connected to Google Calendar. You can close this tab."
            } else {
                "Google sign-in was cancelled. You can close this tab."
            };
            let _ = request.respond(tiny_http::Response::from_string(page));
            return code.ok_or_else(|| "Google sign-in was cancelled.".into());
        }
    }
}

/// Forgets the sign-in here and asks Google to revoke it too.
pub fn disconnect() -> Result<(), String> {
    if let Ok(Some(token)) = read_secret(REFRESH_TOKEN) {
        std::thread::spawn(move || {
            if let Ok(client) = http() {
                let _ = client
                    .post("https://oauth2.googleapis.com/revoke")
                    .form(&[("token", token)])
                    .send();
            }
        });
    }
    if let Ok(mut access) = ACCESS.lock() {
        *access = None;
    }
    remove_secret(REFRESH_TOKEN)
}

fn token_request(form: &[(&str, &str)]) -> Result<Value, String> {
    let response = http()?
        .post(TOKEN)
        .form(form)
        .send()
        .map_err(|_| "Could not reach Google. Check the connection.".to_owned())?;
    let status = response.status();
    let body: Value = response.json().unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(body);
    }
    Err(match body["error"].as_str() {
        Some("invalid_grant") => {
            "Google sign-in expired or was revoked. Connect Google again in Settings → Tasks."
                .into()
        }
        Some("invalid_client" | "unauthorized_client") => {
            "Google rejected the OAuth client. Check the client ID and secret.".into()
        }
        _ => format!("Google sign-in failed (HTTP {}).", status.as_u16()),
    })
}

fn keep_access(reply: &Value) {
    if let (Some(token), Ok(mut access)) = (reply["access_token"].as_str(), ACCESS.lock()) {
        let life = reply["expires_in"]
            .as_u64()
            .unwrap_or(3600)
            .saturating_sub(60);
        *access = Some((token.to_owned(), Instant::now() + Duration::from_secs(life)));
    }
}

fn access_token() -> Result<String, String> {
    if let Ok(access) = ACCESS.lock()
        && let Some((token, until)) = access.as_ref()
        && *until > Instant::now()
    {
        return Ok(token.clone());
    }
    let missing = || "Connect Google in Settings → Tasks first.".to_owned();
    let refresh = read_secret(REFRESH_TOKEN)?.ok_or_else(missing)?;
    let client_id = read_secret(CLIENT_ID)?.ok_or_else(missing)?;
    let client_secret = read_secret(CLIENT_SECRET)?.ok_or_else(missing)?;
    let reply = token_request(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh),
        ("client_id", &client_id),
        ("client_secret", &client_secret),
    ])?;
    keep_access(&reply);
    reply["access_token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "Google returned no access token.".into())
}

/// `None` when the event no longer exists (deleted in Google Calendar).
fn send(request: RequestBuilder) -> Result<Option<Value>, String> {
    let response = request
        .send()
        .map_err(|_| "Could not reach Google Calendar. Check the connection.".to_owned())?;
    match response.status().as_u16() {
        200..=299 => Ok(Some(response.json().unwrap_or(Value::Null))),
        404 | 410 => Ok(None),
        401 => {
            if let Ok(mut access) = ACCESS.lock() {
                *access = None;
            }
            Err("Google sign-in expired. Connect Google again in Settings → Tasks.".into())
        }
        403 => Err("Google refused the request. Check that the Google Calendar API is enabled in your Google Cloud project.".into()),
        429 => Err("Google Calendar rate limit reached. Cikung will try again shortly.".into()),
        code => Err(format!("Google Calendar returned HTTP {code}.")),
    }
}

/// Event IDs Google issues are base32hex; anything else never reaches a URL.
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 1024 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

/// Creates the task's event, or moves the existing one. Returns the event ID.
pub fn save_event(existing: Option<&str>, summary: &str, due_unix: u64) -> Result<String, String> {
    let start = i64::try_from(due_unix)
        .ok()
        .and_then(|secs| chrono::DateTime::from_timestamp(secs, 0))
        .ok_or("The task's due time is invalid.")?
        .with_timezone(&chrono::Local);
    let end = start + chrono::Duration::seconds(LENGTH_SECS);
    let body = json!({
        "summary": summary,
        "description": "Task from Cikung",
        "start": {"dateTime": start.to_rfc3339()},
        "end": {"dateTime": end.to_rfc3339()},
    });
    let client = http()?;
    let token = access_token()?;
    if let Some(id) = existing.filter(|id| valid_id(id))
        && let Some(event) = send(
            client
                .patch(format!("{EVENTS}/{id}"))
                .bearer_auth(&token)
                .json(&body),
        )?
    {
        return event_id(&event);
    }
    let event = send(client.post(EVENTS).bearer_auth(&token).json(&body))?
        .ok_or("Your primary Google Calendar was not found.")?;
    event_id(&event)
}

fn event_id(event: &Value) -> Result<String, String> {
    event["id"]
        .as_str()
        .filter(|id| valid_id(id))
        .map(str::to_owned)
        .ok_or_else(|| "Google Calendar returned an unexpected response.".into())
}

/// Removes an event Cikung made. One already gone counts as removed.
pub fn delete_event(id: &str) -> Result<(), String> {
    if !valid_id(id) {
        return Ok(());
    }
    send(
        http()?
            .delete(format!("{EVENTS}/{id}"))
            .bearer_auth(access_token()?),
    )
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc_7636_and_ids_are_checked() {
        // The worked example in RFC 7636, appendix B.
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert!(random(32).unwrap().len() >= 43);
        assert!(valid_id("a1b2c3d4e5"));
        assert!(!valid_id("../calendars"));
        assert!(!valid_id(""));
        assert!(begin_sign_in(" ", "secret").is_err());
    }
}
