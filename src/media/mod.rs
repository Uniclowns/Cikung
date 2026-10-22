//! Native, local media-session discovery. No audio, screenshots, or browser history are read.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod macos;
mod vlc;
#[cfg(target_os = "windows")]
mod windows;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MediaSession {
    pub id: String,
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub state: PlaybackState,
    pub position_secs: f64,
    pub duration_secs: f64,
    pub shuffle: Option<bool>,
    pub repeat: Option<bool>,
    pub can_seek: bool,
    pub artwork: Option<std::sync::Arc<Vec<u8>>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MediaCommand {
    TogglePlayPause,
    Play,
    Pause,
    Next,
    Previous,
    Seek(f64),
    Shuffle(bool),
    Repeat(bool),
}

pub trait MediaBackend {
    fn poll(&mut self) -> Result<Vec<MediaSession>, String>;
    fn control(&mut self, session_id: &str, command: MediaCommand) -> Result<(), String>;
}

/// Construct on the worker thread: Windows Runtime and D-Bus connections belong there.
pub fn create_backend() -> Result<Box<dyn MediaBackend>, String> {
    vlc::wrap(create_native_backend())
}

fn create_native_backend() -> Result<Box<dyn MediaBackend>, String> {
    #[cfg(target_os = "windows")]
    return windows::create();
    #[cfg(target_os = "linux")]
    return linux::create();
    #[cfg(target_os = "macos")]
    return macos::create();
    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    Err("Cikung supports Windows, Linux, and macOS.".into())
}
