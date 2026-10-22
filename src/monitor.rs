use crate::media::{self, MediaCommand, MediaSession, PlaybackState};
use std::{
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub sessions: Vec<MediaSession>,
    pub error: Option<String>,
    pub control_error: Option<(String, Instant)>,
    pub updated: Instant,
    pub ready: bool,
}

pub struct Monitor {
    latest: Arc<Mutex<Snapshot>>,
    commands: mpsc::SyncSender<(String, MediaCommand)>,
}

impl Monitor {
    pub fn start(ctx: eframe::egui::Context) -> Self {
        let latest = Arc::new(Mutex::new(Snapshot {
            sessions: vec![],
            error: None,
            control_error: None,
            updated: Instant::now(),
            ready: false,
        }));
        let worker_state = latest.clone();
        let (commands, receiver) = mpsc::sync_channel::<(String, MediaCommand)>(8);
        thread::spawn(move || {
            let mut backend = None;
            loop {
                if backend.is_none() {
                    match media::create_backend() {
                        Ok(value) => backend = Some(value),
                        Err(error) => {
                            let mut state = worker_state.lock().unwrap();
                            state.error = Some(error);
                            state.ready = true;
                            state.sessions.clear();
                        }
                    }
                }
                if let Some(backend) = backend.as_mut() {
                    let result = backend.poll();
                    let mut state = worker_state.lock().unwrap();
                    state.ready = true;
                    state.updated = Instant::now();
                    match result {
                        Ok(sessions) => {
                            state.sessions = sessions;
                            state.error = None;
                        }
                        Err(error) => {
                            state.sessions.clear();
                            state.error = Some(error);
                        }
                    }
                }
                ctx.request_repaint();
                match receiver.recv_timeout(Duration::from_millis(900)) {
                    Ok((id, command)) => {
                        if let Some(backend) = backend.as_mut()
                            && let Err(error) = backend.control(&id, command)
                        {
                            worker_state.lock().unwrap().control_error =
                                Some((error, Instant::now()));
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        });
        Self { latest, commands }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.latest.lock().unwrap().clone()
    }

    pub fn control(&self, id: &str, command: MediaCommand) -> Result<(), String> {
        self.commands
            .try_send((id.to_owned(), command))
            .map_err(|_| "Please wait a moment before trying again.".to_owned())
    }
}

pub fn choose_session<'a>(
    sessions: &'a [MediaSession],
    pinned: Option<&str>,
    previous: Option<&str>,
) -> Option<&'a MediaSession> {
    if let Some(pinned) = pinned
        && let Some(session) = sessions.iter().find(|s| s.id == pinned)
    {
        return Some(session);
    }
    sessions
        .iter()
        .find(|s| Some(s.id.as_str()) == previous && s.state == PlaybackState::Playing)
        .or_else(|| sessions.iter().find(|s| s.state == PlaybackState::Playing))
        .or_else(|| sessions.iter().find(|s| Some(s.id.as_str()) == previous))
        .or_else(|| sessions.iter().find(|s| s.state == PlaybackState::Paused))
        .or_else(|| sessions.first())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn session(id: &str, state: PlaybackState) -> MediaSession {
        MediaSession {
            id: id.into(),
            source: id.into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            state,
            position_secs: 0.0,
            duration_secs: 0.0,
            shuffle: None,
            repeat: None,
            can_seek: false,
            artwork: None,
        }
    }
    #[test]
    fn follows_playing_player_and_keeps_it_stable() {
        let sessions = [
            session("browser", PlaybackState::Paused),
            session("spotify", PlaybackState::Playing),
            session("vlc", PlaybackState::Playing),
        ];
        assert_eq!(
            choose_session(&sessions, None, Some("browser")).unwrap().id,
            "spotify"
        );
        assert_eq!(
            choose_session(&sessions, None, Some("vlc")).unwrap().id,
            "vlc"
        );
    }
    #[test]
    fn explicit_selection_can_stay_paused_and_missing_selection_falls_back() {
        let sessions = [
            session("browser", PlaybackState::Paused),
            session("spotify", PlaybackState::Playing),
        ];
        assert_eq!(
            choose_session(&sessions, Some("browser"), None).unwrap().id,
            "browser"
        );
        assert_eq!(
            choose_session(&sessions, Some("gone"), None).unwrap().id,
            "spotify"
        );
        assert!(choose_session(&[], None, None).is_none());
    }
}
