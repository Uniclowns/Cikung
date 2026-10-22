use super::*;

pub(super) struct AutomaticLyrics {
    changed_at: Instant,
    pub attempted: bool,
    pub automatic_request: Option<u64>,
    pub error: Option<String>,
    cache: VecDeque<(String, Result<Lyrics, String>)>,
}
impl Default for AutomaticLyrics {
    fn default() -> Self {
        Self {
            changed_at: Instant::now(),
            attempted: false,
            automatic_request: None,
            error: None,
            cache: VecDeque::new(),
        }
    }
}
impl AutomaticLyrics {
    pub fn track_changed(&mut self, now: Instant) {
        self.changed_at = now;
        self.attempted = false;
        self.automatic_request = None;
        self.error = None;
    }
    pub fn remember(&mut self, identity: String, result: Result<Lyrics, String>) {
        self.cache.retain(|(key, _)| key != &identity);
        self.cache.push_back((identity, result));
        while self.cache.len() > 16 {
            self.cache.pop_front();
        }
    }
}

enum Action {
    Fetch(MediaSession),
    Cached(String, Result<Lyrics, String>),
}
impl Cikung {
    fn automatic_lyrics_action(&mut self, now: Instant) -> Option<Action> {
        if self.demo
            || self.capture.is_some()
            || self.audit.is_some()
            || !self.prefs.auto_lyrics
            || self.lyrics_busy
            || self.auto_lyrics.attempted
            || now.saturating_duration_since(self.auto_lyrics.changed_at) < Duration::from_secs(1)
        {
            return None;
        }
        let session = self.session.as_ref()?;
        if session.state != PlaybackState::Playing
            || session.title.trim().is_empty()
            || session.artist.trim().is_empty()
        {
            return None;
        }
        let identity = Self::track_identity(Some(session));
        if self.lyrics_for == identity && !self.tools.lyrics.lines.is_empty() {
            return None;
        }
        self.auto_lyrics.attempted = true;
        if let Some((_, result)) = self
            .auto_lyrics
            .cache
            .iter()
            .find(|(key, _)| key == &identity)
        {
            Some(Action::Cached(identity, result.clone()))
        } else {
            Some(Action::Fetch(session.clone()))
        }
    }
    pub(super) fn poll_automatic_lyrics(&mut self, now: Instant) {
        match self.automatic_lyrics_action(now) {
            Some(Action::Fetch(session)) => match self.request_lyrics(session) {
                Ok(()) => self.auto_lyrics.automatic_request = self.active_lyrics,
                Err(error) => self.auto_lyrics.error = Some(error),
            },
            Some(Action::Cached(identity, result)) => {
                let _ = self.apply_lyrics_result(identity, result);
            }
            None => {}
        }
    }
    pub(super) fn apply_lyrics_result(
        &mut self,
        identity: String,
        result: Result<Lyrics, String>,
    ) -> Result<(), String> {
        match result {
            Ok(lyrics) => {
                self.lyrics_text = lyrics.to_lrc();
                self.tools.lyrics = lyrics;
                self.lyrics_for = identity;
                self.auto_lyrics.error = None;
                Ok(())
            }
            Err(error) => {
                self.auto_lyrics.error = Some(error.clone());
                Err(error)
            }
        }
    }
    pub(crate) fn lyrics_status(&self) -> &'static str {
        if self.lyrics_busy {
            "Finding the words…"
        } else if self.auto_lyrics.error.is_some() {
            "Lyrics not available for this track"
        } else if self
            .session
            .as_ref()
            .is_some_and(|s| s.artist.trim().is_empty() || s.title.trim().is_empty())
        {
            "Waiting for song details"
        } else if self.prefs.auto_lyrics
            && !self.demo
            && self
                .session
                .as_ref()
                .is_none_or(|s| s.state != PlaybackState::Playing)
        {
            "Play a song to find lyrics"
        } else if self.prefs.auto_lyrics && !self.demo {
            "Getting your lyrics ready…"
        } else {
            "Lyrics not available yet"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> (egui::Context, Cikung, Instant) {
        let (ctx, mut app) = super::super::tests::app();
        // Test the scheduling decision without launching a network worker.
        app.demo = false;
        let now = Instant::now();
        app.auto_lyrics.track_changed(now);
        (ctx, app, now)
    }
    #[test]
    fn stable_playing_track_is_fetched_once_without_a_click() {
        let (_, mut app, now) = app();
        assert!(
            app.automatic_lyrics_action(now + Duration::from_millis(999))
                .is_none()
        );
        assert!(matches!(
            app.automatic_lyrics_action(now + Duration::from_secs(1)),
            Some(Action::Fetch(_))
        ));
        assert!(
            app.automatic_lyrics_action(now + Duration::from_secs(30))
                .is_none()
        );
        app.auto_lyrics.track_changed(now + Duration::from_secs(30));
        assert!(
            app.automatic_lyrics_action(now + Duration::from_millis(30500))
                .is_none()
        );
        assert!(matches!(
            app.automatic_lyrics_action(now + Duration::from_secs(31)),
            Some(Action::Fetch(_))
        ));
    }
    #[test]
    fn disabled_paused_incomplete_imported_and_preview_media_do_not_fetch() {
        let (_, mut app, now) = app();
        let ready = now + Duration::from_secs(2);
        app.prefs.auto_lyrics = false;
        assert!(app.automatic_lyrics_action(ready).is_none());
        app.prefs.auto_lyrics = true;
        app.session.as_mut().unwrap().state = PlaybackState::Paused;
        assert!(app.automatic_lyrics_action(ready).is_none());
        app.session.as_mut().unwrap().state = PlaybackState::Playing;
        let artist = std::mem::take(&mut app.session.as_mut().unwrap().artist);
        assert!(app.automatic_lyrics_action(ready).is_none());
        app.session.as_mut().unwrap().artist = artist;
        app.demo = true;
        assert!(app.automatic_lyrics_action(ready).is_none());
        app.demo = false;
        app.lyrics_for = Cikung::track_identity(app.session.as_ref());
        app.tools.lyrics = Lyrics::parse("[00:00]Imported fixture").unwrap();
        assert!(app.automatic_lyrics_action(ready).is_none());
    }
    #[test]
    fn cached_success_and_failure_prevent_repeated_requests() {
        let (_, mut app, now) = app();
        let identity = Cikung::track_identity(app.session.as_ref());
        app.auto_lyrics.remember(
            identity.clone(),
            Ok(Lyrics::parse("[00:00]Fixture").unwrap()),
        );
        app.poll_automatic_lyrics(now + Duration::from_secs(2));
        assert_eq!(app.lyrics_for, identity);
        assert_eq!(app.tools.lyrics.lines[0].text, "Fixture");
        assert!(!app.lyrics_busy);
        app.lyrics_for.clear();
        app.auto_lyrics.track_changed(now);
        app.auto_lyrics.remember(identity, Err("No match".into()));
        app.poll_automatic_lyrics(now + Duration::from_secs(2));
        assert_eq!(app.auto_lyrics.error.as_deref(), Some("No match"));
        assert!(
            app.automatic_lyrics_action(now + Duration::from_secs(60))
                .is_none()
        );
        for i in 0..50 {
            app.auto_lyrics
                .remember(format!("track{i}"), Err("No match".into()));
        }
        assert_eq!(app.auto_lyrics.cache.len(), 16);
    }
    #[test]
    fn automatic_failures_are_quiet_and_stale_completions_cannot_replace_a_new_track() {
        let (ctx, mut app, now) = app();
        app.demo = true; // tick must not launch real lookups in this fixture.
        let identity = Cikung::track_identity(app.session.as_ref());
        let id = app.request(Pending::Lyrics {
            generation: app.lyrics_generation,
            identity,
        });
        app.active_lyrics = Some(id);
        app.auto_lyrics.automatic_request = Some(id);
        app.lyrics_busy = true;
        app.cloud.inject_event(CloudEvent::Lyrics {
            request_id: id,
            result: Err("No match".into()),
        });
        app.tick(&ctx);
        assert_eq!(app.auto_lyrics.error.as_deref(), Some("No match"));
        assert!(app.notices.is_empty());
        let old = app.request(Pending::Lyrics {
            generation: app.lyrics_generation,
            identity: app.lyrics_identity.clone(),
        });
        app.active_lyrics = Some(old);
        app.snapshot.sessions[0].title = "The next song".into();
        app.cloud.inject_event(CloudEvent::Lyrics {
            request_id: old,
            result: Ok("[00:00]Stale fixture".into()),
        });
        app.tick(&ctx);
        assert!(!app.lyrics_busy);
        assert_ne!(app.lyrics_for, Cikung::track_identity(app.session.as_ref()));
        assert!(app.tools.lyrics.lines.is_empty());
        app.auto_lyrics.track_changed(now);
    }
}
