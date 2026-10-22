//! Explicit, local-only Windows microphone and system-loopback recording.
//! No capture is started by construction, and no recording is uploaded here.
use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::JoinHandle,
};

#[cfg(windows)]
use std::thread;

#[derive(Debug)]
pub struct RecordingResult {
    pub mic_path: Option<PathBuf>,
    pub system_path: Option<PathBuf>,
    pub duration_secs: f64,
    pub warnings: Vec<String>,
}
pub struct RecordingWorker {
    stop: Arc<AtomicBool>,
    receiver: Option<Receiver<Result<RecordingResult, String>>>,
    handle: Option<JoinHandle<()>>,
}
impl Default for RecordingWorker {
    fn default() -> Self {
        Self::new()
    }
}
impl RecordingWorker {
    pub fn new() -> Self {
        Self {
            stop: Arc::new(AtomicBool::new(false)),
            receiver: None,
            handle: None,
        }
    }
    pub fn is_recording(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn start(&mut self, output_dir: &Path) -> Result<(), String> {
        if self.is_recording() {
            return Err("A recording is already running or finishing.".into());
        }
        #[cfg(not(windows))]
        {
            let _ = output_dir;
            Err("Microphone and PC-audio recording is currently available on Windows. Import a transcript or audio file on this platform.".into())
        }
        #[cfg(windows)]
        {
            if !output_dir.is_dir() {
                return Err("Choose an existing folder for the recording.".into());
            }
            let output_dir = std::fs::canonicalize(output_dir)
                .map_err(|e| format!("Cannot access the recording folder: {e}"))?;
            self.stop = Arc::new(AtomicBool::new(false));
            let stop = self.stop.clone();
            let (tx, rx) = mpsc::channel();
            self.handle = Some(
                thread::Builder::new()
                    .name("cikung-meeting-recorder".into())
                    .spawn(move || {
                        let _ = tx.send(native::record(&output_dir, stop));
                    })
                    .map_err(|e| format!("Cannot start the recorder: {e}"))?,
            );
            self.receiver = Some(rx);
            Ok(())
        }
    }
    pub fn stop(&mut self) -> Result<(), String> {
        if !self.is_recording() {
            return Err("There is no active recording.".into());
        }
        self.stop.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub fn poll(&mut self) -> Option<Result<RecordingResult, String>> {
        let receiver = self.receiver.as_ref()?;
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Err("The recording worker stopped unexpectedly. Check the selected output folder for recoverable WAV files.".into()),
        };
        self.receiver = None;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        Some(result)
    }
}
impl Drop for RecordingWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(windows)]
mod native {
    use super::*;
    use std::{
        fs::{File, OpenOptions},
        io::BufWriter,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use windows::Win32::{
        Media::Audio::{
            AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY, AUDCLNT_BUFFERFLAGS_SILENT,
            AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, IAudioCaptureClient,
            IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture, eConsole, eRender,
        },
        System::Com::{CLSCTX_ALL, CoCreateInstance, CoTaskMemFree},
    };

    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            wasapi::deinitialize();
        }
    }
    struct Capture {
        client: IAudioClient,
        reader: IAudioCaptureClient,
        writer: Option<hound::WavWriter<BufWriter<File>>>,
        path: PathBuf,
        rate: u32,
        channels: u16,
        sample_bytes: usize,
        float: bool,
        written_frames: u64,
        last_device_frame: Option<u64>,
        discontinuities: u64,
    }
    impl Capture {
        fn open(
            enumerator: &IMMDeviceEnumerator,
            path: PathBuf,
            loopback: bool,
        ) -> Result<Self, String> {
            let device = unsafe {
                enumerator
                    .GetDefaultAudioEndpoint(if loopback { eRender } else { eCapture }, eConsole)
            }
            .map_err(|e| e.to_string())?;
            let client: IAudioClient =
                unsafe { device.Activate(CLSCTX_ALL, None) }.map_err(|e| e.to_string())?;
            let raw_format = unsafe { client.GetMixFormat() }.map_err(|e| e.to_string())?;
            if raw_format.is_null() {
                return Err("The audio device returned no format.".into());
            }
            // GetMixFormat owns a CoTaskMem allocation. Parse before freeing it.
            let parsed = wasapi::WaveFormat::parse(unsafe { &*raw_format });
            unsafe {
                CoTaskMemFree(Some(raw_format.cast()));
            }
            let format = parsed.map_err(|e| format!("Unsupported audio device format: {e}"))?;
            let channels = format.get_nchannels();
            let rate = format.get_samplespersec();
            let bits = format.get_bitspersample();
            let float =
                format.get_subformat().map_err(|e| e.to_string())? == wasapi::SampleType::Float;
            if channels == 0
                || channels > 32
                || !(8_000..=384_000).contains(&rate)
                || (float && bits != 32)
                || (!float && ![16, 24, 32].contains(&bits))
            {
                return Err(format!(
                    "Unsupported device format: {channels} channels, {rate} Hz, {bits} bits."
                ));
            }
            let flags = if loopback {
                AUDCLNT_STREAMFLAGS_LOOPBACK
            } else {
                0
            };
            unsafe {
                client.Initialize(
                    AUDCLNT_SHAREMODE_SHARED,
                    flags,
                    1_000_000,
                    0,
                    format.as_waveformatex_ref(),
                    None,
                )
            }
            .map_err(|e| format!("Cannot initialize audio capture: {e}"))?;
            let reader: IAudioCaptureClient =
                unsafe { client.GetService() }.map_err(|e| e.to_string())?;
            let file = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .map_err(|e| format!("Cannot create {}: {e}", path.display()))?;
            let writer = hound::WavWriter::new(
                BufWriter::new(file),
                hound::WavSpec {
                    channels,
                    sample_rate: rate,
                    bits_per_sample: 16,
                    sample_format: hound::SampleFormat::Int,
                },
            )
            .map_err(|e| e.to_string())?;
            let capture = Self {
                client,
                reader,
                writer: Some(writer),
                path,
                rate,
                channels,
                sample_bytes: usize::from(bits / 8),
                float,
                written_frames: 0,
                last_device_frame: None,
                discontinuities: 0,
            };
            unsafe { capture.client.Start() }
                .map_err(|e| format!("Cannot start audio capture: {e}"))?;
            Ok(capture)
        }
        fn silence(&mut self, frames: u64) -> Result<(), String> {
            self.check_size(frames)?;
            // Write in bounded chunks; a long silent desktop must not consume memory.
            let writer = self.writer.as_mut().ok_or("Recording file is closed.")?;
            for _ in 0..frames.saturating_mul(u64::from(self.channels)) {
                writer
                    .write_sample(0i16)
                    .map_err(|e| format!("Cannot write recording: {e}"))?;
            }
            self.written_frames += frames;
            Ok(())
        }
        fn check_size(&self, extra_frames: u64) -> Result<(), String> {
            if self
                .written_frames
                .saturating_add(extra_frames)
                .saturating_mul(u64::from(self.channels) * 2)
                > 3_500_000_000
            {
                return Err("This recording reached the 3.5 GB WAV limit. Start a new recording to continue.".into());
            }
            Ok(())
        }
        fn drain(&mut self, elapsed: Duration) -> Result<(), String> {
            // Limit each iteration, so stop remains responsive even with a faulty device.
            for _ in 0..128 {
                let available = unsafe { self.reader.GetNextPacketSize() }
                    .map_err(|e| format!("Audio device disconnected: {e}"))?;
                if available == 0 {
                    break;
                }
                let mut data = std::ptr::null_mut();
                let mut frames = 0;
                let mut flags = 0;
                let mut device_frame = 0;
                unsafe {
                    self.reader.GetBuffer(
                        &mut data,
                        &mut frames,
                        &mut flags,
                        Some(&mut device_frame),
                        None,
                    )
                }
                .map_err(|e| format!("Cannot read audio packet: {e}"))?;
                struct Packet<'a>(&'a IAudioCaptureClient, u32);
                impl Drop for Packet<'_> {
                    fn drop(&mut self) {
                        let _ = unsafe { self.0.ReleaseBuffer(self.1) };
                    }
                }
                let packet = Packet(&self.reader, frames);
                if frames > self.rate * 10 {
                    return Err("Audio device returned an invalid packet length.".into());
                }
                let silent = flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0;
                let samples = if silent || frames == 0 {
                    Vec::new()
                } else {
                    self.check_size(u64::from(frames))?;
                    if data.is_null() {
                        return Err("Audio device returned a null sample buffer.".into());
                    }
                    let byte_count =
                        frames as usize * usize::from(self.channels) * self.sample_bytes;
                    // Valid only until ReleaseBuffer; convert before releasing the lease.
                    let bytes = unsafe { std::slice::from_raw_parts(data, byte_count) };
                    bytes
                        .chunks_exact(self.sample_bytes)
                        .map(|sample| pcm16(sample, self.float))
                        .collect::<Vec<_>>()
                };
                drop(packet);
                if flags & AUDCLNT_BUFFERFLAGS_DATA_DISCONTINUITY.0 as u32 != 0 {
                    self.discontinuities += 1;
                }
                let gap = if let Some(expected) = self.last_device_frame {
                    device_frame
                        .saturating_sub(expected)
                        .min(u64::from(self.rate) * 60 * 60 * 2)
                } else {
                    (elapsed.as_secs_f64() * f64::from(self.rate)) as u64
                        - u64::from(frames)
                            .min((elapsed.as_secs_f64() * f64::from(self.rate)) as u64)
                };
                if gap > 0 {
                    self.silence(gap)?;
                }
                self.last_device_frame = Some(device_frame + u64::from(frames));
                if silent {
                    self.silence(u64::from(frames))?;
                } else {
                    self.check_size(u64::from(frames))?;
                    let writer = self.writer.as_mut().ok_or("Recording file is closed.")?;
                    for sample in samples {
                        writer
                            .write_sample(sample)
                            .map_err(|e| format!("Cannot write recording: {e}"))?;
                    }
                    self.written_frames += u64::from(frames);
                }
            }
            Ok(())
        }
        fn finish(mut self, duration: Duration) -> Result<(PathBuf, u64, Option<String>), String> {
            let _ = unsafe { self.client.Stop() };
            let target_frames = (duration.as_secs_f64() * f64::from(self.rate)) as u64;
            let padding_warning = if self.written_frames < target_frames {
                self.silence(target_frames - self.written_frames).err()
            } else {
                None
            };
            if let Some(writer) = self.writer.take() {
                writer
                    .finalize()
                    .map_err(|e| format!("Cannot finish {}: {e}", self.path.display()))?;
            }
            Ok((self.path.clone(), self.discontinuities, padding_warning))
        }
    }
    impl Drop for Capture {
        fn drop(&mut self) {
            let _ = unsafe { self.client.Stop() };
            if let Some(writer) = self.writer.take() {
                let _ = writer.finalize();
            }
        }
    }
    pub(super) fn record(
        output_dir: &Path,
        stop: Arc<AtomicBool>,
    ) -> Result<RecordingResult, String> {
        wasapi::initialize_mta()
            .ok()
            .map_err(|e| format!("Cannot initialize Windows audio: {e}"))?;
        let _apartment = Apartment;
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .map_err(|e| format!("Cannot list audio devices: {e}"))?;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let directory = output_dir.join(format!(
            "Cikung-{}-{nonce}",
            chrono::Local::now().format("%Y%m%d-%H%M%S")
        ));
        std::fs::create_dir(&directory)
            .map_err(|e| format!("Cannot create recording directory: {e}"))?;
        let mut warnings = Vec::new();
        let started = Instant::now();
        let mut mic = match Capture::open(&enumerator, directory.join("microphone.wav"), false) {
            Ok(c) => Some(c),
            Err(e) => {
                warnings.push(format!("Microphone unavailable: {e}"));
                None
            }
        };
        let mut system =
            match Capture::open(&enumerator, directory.join("computer-audio.wav"), true) {
                Ok(c) => Some(c),
                Err(e) => {
                    warnings.push(format!("PC audio unavailable: {e}"));
                    None
                }
            };
        if mic.is_none() && system.is_none() {
            return Err(format!(
                "Could not record either audio source. {}",
                warnings.join(" ")
            ));
        }
        let mut mic_failed = false;
        let mut system_failed = false;
        while !stop.load(Ordering::Relaxed) {
            let elapsed = started.elapsed();
            if elapsed > Duration::from_secs(7200) {
                warnings.push("Recording stopped at the two-hour WAV safety limit.".into());
                break;
            }
            if let Some(capture) = mic.as_mut().filter(|_| !mic_failed)
                && let Err(e) = capture.drain(elapsed)
            {
                warnings.push(format!("Microphone capture stopped: {e}"));
                mic_failed = true;
            }
            if let Some(capture) = system.as_mut().filter(|_| !system_failed)
                && let Err(e) = capture.drain(elapsed)
            {
                warnings.push(format!("PC audio capture stopped: {e}"));
                system_failed = true;
            }
            if (mic.is_none() || mic_failed) && (system.is_none() || system_failed) {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let duration = started.elapsed();
        let mut finish = |capture: Option<Capture>, label: &str| -> Option<PathBuf> {
            match capture.map(|capture| capture.finish(duration)) {
                Some(Ok((path, gaps, padding_warning))) => {
                    if let Some(warning) = padding_warning {
                        warnings.push(format!(
                            "{} was saved with a shorter silent tail: {warning}",
                            path.display()
                        ));
                    }
                    if gaps > 1 {
                        warnings.push(format!("{label} reported {gaps} audio discontinuities. Some speech may be missing."));
                    }
                    Some(path)
                }
                Some(Err(e)) => {
                    warnings.push(e);
                    None
                }
                None => None,
            }
        };
        let mic_path = finish(mic, "Microphone");
        let system_path = finish(system, "PC audio");
        if mic_path.is_none() && system_path.is_none() {
            return Err(format!(
                "The recording files could not be saved. {}",
                warnings.join(" ")
            ));
        }
        Ok(RecordingResult {
            mic_path,
            system_path,
            duration_secs: duration.as_secs_f64(),
            warnings,
        })
    }
    fn pcm16(bytes: &[u8], float: bool) -> i16 {
        if float {
            let value = f32::from_le_bytes(bytes.try_into().unwrap_or([0; 4]));
            if !value.is_finite() {
                return 0;
            }
            (value.clamp(-1., 1.) * 32767.).round() as i16
        } else {
            match bytes.len() {
                2 => i16::from_le_bytes([bytes[0], bytes[1]]),
                3 => i16::from_le_bytes([bytes[1], bytes[2]]),
                4 => i16::from_le_bytes([bytes[2], bytes[3]]),
                _ => 0,
            }
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn converts_native_audio_without_invalid_float_or_pcm_overflow() {
            assert_eq!(pcm16(&1f32.to_le_bytes(), true), 32767);
            assert_eq!(pcm16(&(-1f32).to_le_bytes(), true), -32767);
            assert_eq!(pcm16(&f32::NAN.to_le_bytes(), true), 0);
            assert_eq!(pcm16(&i16::MIN.to_le_bytes(), false), i16::MIN);
            assert_eq!(pcm16(&[0, 0, 128], false), i16::MIN);
            assert_eq!(pcm16(&i32::MAX.to_le_bytes(), false), i16::MAX);
        }
    }
}
