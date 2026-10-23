//! Original, embedded chime; no external player or audio file is required on Windows.
//! macOS plays the same chime through the built-in `afplay`.
use std::time::{Duration, Instant};

const CHIME: &[u8] = include_bytes!("../assets/agent-complete.wav");

#[derive(Default)]
pub struct Player {
    last: Option<Instant>,
}
impl Player {
    pub fn play(&mut self, now: Instant, volume: u8) -> Result<(), String> {
        if volume == 0 {
            return Ok(());
        }
        if self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < Duration::from_millis(750))
        {
            return Ok(());
        }
        play(volume)?;
        self.last = Some(now);
        Ok(())
    }
}

// shortcut: this bundled WAV has a fixed 44-byte PCM16 header; parse chunks if custom sounds are added.
fn scaled_chime(volume: u8) -> Vec<u8> {
    let mut wav = CHIME.to_vec();
    let gain = i32::from(volume.min(100));
    for sample in wav[44..].as_chunks_mut::<2>().0 {
        let value = i32::from(i16::from_le_bytes([sample[0], sample[1]]));
        sample.copy_from_slice(&((value * gain / 100) as i16).to_le_bytes());
    }
    wav
}

#[cfg(windows)]
fn play(volume: u8) -> Result<(), String> {
    use std::sync::Mutex;
    use windows::{
        Win32::Media::Audio::{
            PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT, SND_NOSTOP, SND_SYNC,
        },
        core::PCWSTR,
    };
    // Retain async audio across Player drops; stop playback before replacing its buffer.
    static BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());
    let mut buffer = BUFFER
        .lock()
        .map_err(|_| "The completion sound player is unavailable.")?;
    if !unsafe { PlaySoundW(PCWSTR::null(), None, SND_SYNC) }.as_bool() {
        return Err("Windows could not prepare the completion sound.".into());
    }
    *buffer = scaled_chime(volume);
    if unsafe {
        PlaySoundW(
            PCWSTR(buffer.as_ptr().cast()),
            None,
            SND_ASYNC | SND_MEMORY | SND_NODEFAULT | SND_NOSTOP,
        )
    }
    .as_bool()
    {
        Ok(())
    } else {
        Err("Windows could not play the completion sound. Check your audio output.".into())
    }
}

#[cfg(target_os = "macos")]
fn play(volume: u8) -> Result<(), String> {
    // afplay ships with macOS, so the embedded chime still needs no extra player.
    let path = std::env::temp_dir().join(format!("cikung-chime-{}.wav", std::process::id()));
    std::fs::write(&path, scaled_chime(volume))
        .map_err(|e| format!("Could not prepare the completion sound: {e}"))?;
    let mut child = std::process::Command::new("/usr/bin/afplay")
        .arg(&path)
        .spawn()
        .map_err(|e| format!("macOS could not play the completion sound: {e}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
fn play(volume: u8) -> Result<(), String> {
    let _ = scaled_chime(volume);
    Err("Completion sounds currently require Windows or macOS.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    #[ignore = "Plays one audible chime; run explicitly on the target PC"]
    fn native_playback_is_accepted() {
        play(35).unwrap();
        std::thread::sleep(Duration::from_millis(550));
    }
    #[test]
    fn embedded_chime_is_short_valid_pcm_with_a_quiet_start_and_end() {
        assert_eq!(&CHIME[..4], b"RIFF");
        assert_eq!(&CHIME[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes(CHIME[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(CHIME[34..36].try_into().unwrap()), 16);
        assert_eq!(
            u32::from_le_bytes(CHIME[40..44].try_into().unwrap()) as usize,
            CHIME.len() - 44
        );
        assert!((40_000..45_000).contains(&CHIME.len()));
        let samples: Vec<_> = CHIME[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(i16::from_le_bytes)
            .collect();
        assert_eq!(samples[0], 0);
        assert_eq!(*samples.last().unwrap(), 0);
        let peak = samples.iter().map(|s| s.unsigned_abs()).max().unwrap();
        assert!((1000..12_000).contains(&peak));
    }
    #[test]
    fn volume_scales_only_pcm_samples_and_clamps_to_full_volume() {
        assert_eq!(scaled_chime(100), CHIME);
        assert_eq!(scaled_chime(255), CHIME);
        let silent = scaled_chime(0);
        assert_eq!(&silent[..44], &CHIME[..44]);
        assert!(silent[44..].iter().all(|b| *b == 0));
        let half = scaled_chime(50);
        assert_eq!(&half[..44], &CHIME[..44]);
        for (original, scaled) in CHIME[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .zip(half[44..].as_chunks::<2>().0)
        {
            let original = i16::from_le_bytes(*original);
            let scaled = i16::from_le_bytes(*scaled);
            assert_eq!(scaled, original / 2);
        }
    }
    #[test]
    fn zero_volume_is_silent_and_does_not_consume_the_burst_gate() {
        let mut player = Player::default();
        assert!(player.play(Instant::now(), 0).is_ok());
        assert!(player.last.is_none());
    }
    #[test]
    fn overlapping_completions_do_not_retrigger_playback() {
        let now = Instant::now();
        let mut player = Player { last: Some(now) };
        assert!(player.play(now + Duration::from_millis(100), 50).is_ok());
        assert_eq!(player.last, Some(now));
    }
}
