//! Speech starts only after a user action. No audio is collected on app startup.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread,
};

pub struct DictationWorker {
    receiver: Option<Receiver<Result<String, String>>>,
    cancel: Arc<AtomicBool>,
}
impl Default for DictationWorker {
    fn default() -> Self {
        Self::new()
    }
}
impl DictationWorker {
    pub fn new() -> Self {
        Self {
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    pub fn busy(&self) -> bool {
        self.receiver.is_some()
    }
    fn spawn(
        &mut self,
        work: impl FnOnce(Arc<AtomicBool>) -> Result<String, String> + Send + 'static,
    ) -> Result<(), String> {
        if self.busy() {
            return Err("A transcription is already running.".into());
        }
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("cikung-dictation".into())
            .spawn(move || {
                let _ = tx.send(work(cancel));
            })
            .map_err(|e| format!("Could not start dictation: {e}"))?;
        self.receiver = Some(rx);
        Ok(())
    }
    /// Windows displays its own recognition consent/listening UI in the system language.
    pub fn start(&mut self) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.spawn(recognize_windows)
        }
        #[cfg(not(windows))]
        {
            Err("Native dictation is currently available on Windows. Select a local whisper.cpp executable and an audio file for offline transcription on this platform.".into())
        }
    }
    /// Run an explicitly selected whisper.cpp CLI; no download, shell, or model setup.
    pub fn transcribe_audio_with_model(
        &mut self,
        executable: &Path,
        audio: &Path,
        model: Option<&Path>,
    ) -> Result<(), String> {
        let executable = checked_file(executable, "whisper.cpp executable")?;
        let audio = checked_file(audio, "audio file")?;
        if std::fs::metadata(&audio).map_err(|e| e.to_string())?.len() > 2_000_000_000 {
            return Err("Choose an audio file below 2 GB.".into());
        }
        let model = model
            .map(|path| checked_file(path, "Whisper model"))
            .transpose()?;
        self.spawn(move |cancel| run_whisper(executable, audio, model, cancel))
    }
    pub fn poll(&mut self) -> Option<Result<String, String>> {
        let receiver = self.receiver.as_ref()?;
        match receiver.try_recv() {
            Ok(result) => {
                self.receiver = None;
                Some(result)
            }
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                Some(Err("The speech worker stopped unexpectedly.".into()))
            }
        }
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Drop for DictationWorker {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn checked_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let path = std::fs::canonicalize(path).map_err(|e| format!("Cannot access {label}: {e}"))?;
    if !path.is_file() {
        return Err(format!("Choose a file for {label}."));
    }
    Ok(path)
}
fn run_whisper(
    executable: PathBuf,
    audio: PathBuf,
    model: Option<PathBuf>,
    cancel: Arc<AtomicBool>,
) -> Result<String, String> {
    use std::{
        fs::{self, File},
        process::{Command, Stdio},
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("cikung-speech-{}-{nonce}", std::process::id()));
    fs::create_dir(&directory)
        .map_err(|e| format!("Cannot create transcription workspace: {e}"))?;
    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(self.0.join("transcript.txt"));
            let _ = std::fs::remove_file(self.0.join("stderr.log"));
            let _ = std::fs::remove_dir(&self.0);
        }
    }
    let _cleanup = Cleanup(directory.clone());
    let prefix = directory.join("transcript");
    let stderr_path = directory.join("stderr.log");
    let stderr = File::create(&stderr_path).map_err(|e| e.to_string())?;
    let mut command = Command::new(executable);
    command
        .arg("-f")
        .arg(audio)
        .args(["-otxt", "-of"])
        .arg(&prefix)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr);
    if let Some(model) = model {
        command.arg("-m").arg(model);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("Could not run the selected whisper.cpp executable: {e}"))?;
    let started = Instant::now();
    let status = loop {
        if cancel.load(Ordering::Relaxed) || started.elapsed() > Duration::from_secs(1800) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Transcription was canceled or exceeded 30 minutes.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Could not monitor transcription: {e}"));
            }
        }
    };
    if !status.success() {
        let error = fs::read_to_string(&stderr_path).unwrap_or_default();
        return Err(format!(
            "whisper.cpp exited with {status}. {}",
            error.chars().take(2_000).collect::<String>()
        ));
    }
    let transcript_path = prefix.with_extension("txt");
    if fs::metadata(&transcript_path)
        .map_err(|e| format!("The selected program did not produce a whisper.cpp transcript: {e}"))?
        .len()
        > 2_000_000
    {
        return Err("The generated transcript exceeds 2 MB.".into());
    }
    let text = fs::read_to_string(&transcript_path)
        .map_err(|e| format!("Cannot read generated transcript: {e}"))?;
    if text.trim().is_empty() {
        return Err("The audio produced no recognized speech.".into());
    }
    Ok(text.trim().into())
}

#[cfg(windows)]
fn recognize_windows(cancel: Arc<AtomicBool>) -> Result<String, String> {
    use windows::{
        Media::SpeechRecognition::{
            SpeechRecognitionResultStatus as Status, SpeechRecognitionScenario,
            SpeechRecognitionTopicConstraint, SpeechRecognizer,
        },
        Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
        core::HSTRING,
    };
    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|e| format!("Windows speech could not initialize: {e}"))?;
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                RoUninitialize();
            }
        }
    }
    let _apartment = Apartment;
    let recognizer = SpeechRecognizer::new().map_err(|e| format!("Windows speech recognition is unavailable: {e}. Enable a speech language and microphone access in Windows Settings."))?;
    let result = (|| -> windows::core::Result<String> {
        let constraint = SpeechRecognitionTopicConstraint::Create(
            SpeechRecognitionScenario::Dictation,
            &HSTRING::new(),
        )?;
        recognizer.Constraints()?.Append(&constraint)?;
        let compilation = recognizer.CompileConstraintsAsync()?.join()?;
        if compilation.Status()? != Status::Success {
            return Ok(String::new());
        }
        let operation = recognizer.RecognizeWithUIAsync()?;
        // WinRT AsyncStatus::Started has ABI value 0. windows-rs exposes its
        // concrete type from windows-future rather than Windows::Foundation.
        while operation.Status()?.0 == 0 {
            if cancel.load(Ordering::Relaxed) {
                operation.Cancel()?;
                return Ok(String::new());
            }
            thread::sleep(std::time::Duration::from_millis(50));
        }
        let result = operation.join()?;
        let status = result.Status()?;
        if status == Status::UserCanceled {
            return Ok(String::new());
        }
        if status != Status::Success {
            return Err(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                format!("Speech recognition status {}", status.0),
            ));
        }
        Ok(result.Text()?.to_string_lossy())
    })();
    let _ = recognizer.Close();
    let text = result.map_err(|e| format!("Windows dictation could not finish: {e}. Check microphone permissions, speech language, and Windows online speech settings; use local Whisper transcription if unavailable."))?;
    if text.trim().is_empty() {
        return Err("Dictation was canceled, no speech was recognized, or the current Windows speech language is unsupported.".into());
    }
    Ok(text.trim().into())
}
