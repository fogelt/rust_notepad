//! Offline dictation with a local Whisper model.
//!
//! The model is not bundled with the app. It is downloaded on demand into the
//! user's local app data and then used through sherpa-onnx, so dictation works
//! without an API key or an internet connection.

use crate::app::audio::{Microphone, decode_audio_file};
use crate::app::models::DictationEvent;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

/// A Whisper model that can be downloaded on demand.
pub struct WhisperModel {
    pub id: &'static str,
    pub label: &'static str,
    pub note: &'static str,
    pub url: &'static str,
    pub archive_bytes: u64,
    /// Approximate RAM needed while transcribing.
    pub ram: &'static str,
}

pub const MODELS: &[WhisperModel] = &[
    WhisperModel {
        id: "tiny",
        label: "Whisper Tiny",
        note: "Fastest · multilingual",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-tiny.tar.bz2",
        archive_bytes: 116_204_861,
        ram: "~0.5 GB",
    },
    WhisperModel {
        id: "base",
        label: "Whisper Base",
        note: "Recommended · multilingual",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-base.tar.bz2",
        archive_bytes: 207_557_382,
        ram: "~1 GB",
    },
    WhisperModel {
        id: "small",
        label: "Whisper Small",
        note: "Best accuracy · multilingual",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-small.tar.bz2",
        archive_bytes: 639_387_718,
        ram: "~2 GB",
    },
    WhisperModel {
        id: "medium",
        label: "Whisper Medium",
        note: "Higher accuracy · multilingual · slow on CPU",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-medium.tar.bz2",
        archive_bytes: 1_931_372_882,
        ram: "~4 GB",
    },
    WhisperModel {
        id: "large-v3",
        label: "Whisper Large v3",
        note: "Highest accuracy · multilingual · very slow on CPU",
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/asr-models/sherpa-onnx-whisper-large-v3.tar.bz2",
        archive_bytes: 1_068_482_488,
        ram: "~5 GB",
    },
];

pub const DEFAULT_MODEL: &str = "base";

pub fn model(id: &str) -> &'static WhisperModel {
    MODELS
        .iter()
        .find(|model| model.id == id)
        .unwrap_or_else(|| {
            MODELS
                .iter()
                .find(|model| model.id == DEFAULT_MODEL)
                .unwrap_or(&MODELS[0])
        })
}

/// Audio window fed to Whisper per pass.
const WINDOW_SECONDS: f32 = 4.0;
/// Audio kept from the previous window so words aren't cut at the boundary.
const OVERLAP_SECONDS: f32 = 0.6;
/// Ignore a window quieter than this (about -44 dBFS); Whisper hallucinates on
/// silence and background noise.
const SILENCE_RMS: f32 = 0.006;

/// Audio file extensions the local engine can decode.
pub fn is_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|ext| {
            matches!(
                ext.as_str(),
                "wav" | "wave" | "mp3" | "flac" | "ogg" | "oga" | "m4a" | "aac" | "aiff" | "aif"
            )
        })
}

#[derive(Clone, Debug)]
pub struct ModelPaths {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub tokens: PathBuf,
}

fn models_root() -> PathBuf {
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(local).join("Rusty Notepad").join("models");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("rust_notepad")
            .join("models");
    }
    PathBuf::from("models")
}

pub fn model_dir(model: &WhisperModel) -> PathBuf {
    models_root().join(format!("whisper-{}", model.id))
}

fn model_file_names(model: &WhisperModel) -> (String, String, String) {
    (
        format!("{}-encoder.int8.onnx", model.id),
        format!("{}-decoder.int8.onnx", model.id),
        format!("{}-tokens.txt", model.id),
    )
}

/// The three files needed to run the model, if they are all present.
pub fn model_paths(model: &WhisperModel) -> Option<ModelPaths> {
    let dir = model_dir(model);
    let (encoder_name, decoder_name, tokens_name) = model_file_names(model);
    let encoder = dir.join(encoder_name);
    let decoder = dir.join(decoder_name);
    let tokens = dir.join(tokens_name);
    let complete = [&encoder, &decoder, &tokens]
        .iter()
        .all(|path| path.metadata().is_ok_and(|meta| meta.len() > 0));
    if complete {
        Some(ModelPaths {
            encoder,
            decoder,
            tokens,
        })
    } else {
        None
    }
}

/// All models whose files are fully downloaded.
pub fn installed_models() -> Vec<&'static WhisperModel> {
    MODELS
        .iter()
        .filter(|model| model_paths(model).is_some())
        .collect()
}

/// Pick a usable model: the saved choice if installed, otherwise the first
/// installed model, otherwise the saved/default id.
pub fn resolve_model(saved: &str) -> &'static WhisperModel {
    let saved = model(saved);
    if model_paths(saved).is_some() {
        return saved;
    }
    installed_models().first().copied().unwrap_or(saved)
}

/// Human-readable download size.
pub fn archive_size(model: &WhisperModel) -> String {
    let megabytes = model.archive_bytes as f64 / 1_000_000.0;
    if megabytes >= 1000.0 {
        format!("{:.1} GB", megabytes / 1000.0)
    } else {
        format!("{megabytes:.0} MB")
    }
}

/// Delete the downloaded model files from disk.
pub fn remove_model(model: &WhisperModel) -> Result<(), String> {
    let dir = model_dir(model);
    if !dir.exists() {
        return Ok(());
    }
    std::fs::remove_dir_all(&dir).map_err(|err| format!("Could not remove the model: {err}"))
}

#[derive(Debug)]
pub enum DownloadEvent {
    Progress { downloaded: u64, total: u64 },
    Ready,
    Failed(String),
}

/// A model download in progress, polled by the UI.
pub struct ModelDownload {
    pub model: &'static WhisperModel,
    pub rx: Receiver<DownloadEvent>,
    pub downloaded: u64,
    pub total: u64,
}

impl ModelDownload {
    /// Start downloading and extracting the model on a background thread.
    pub fn start(model: &'static WhisperModel) -> Self {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            if let Err(err) = download_and_extract(model, &tx) {
                cleanup_partial(model);
                let _ = tx.send(DownloadEvent::Failed(err));
            }
        });
        Self {
            model,
            rx,
            downloaded: 0,
            total: model.archive_bytes,
        }
    }

    /// Poll one event. Returns `None` while the job is still running.
    pub fn poll(&mut self) -> Option<DownloadEvent> {
        match self.rx.try_recv() {
            Ok(DownloadEvent::Progress { downloaded, total }) => {
                self.downloaded = downloaded;
                if total > 0 {
                    self.total = total;
                }
                Some(DownloadEvent::Progress { downloaded, total })
            }
            Ok(event) => Some(event),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(DownloadEvent::Failed(String::from(
                "The model download stopped unexpectedly.",
            ))),
        }
    }

    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.downloaded as f32 / self.total as f32).clamp(0.0, 1.0)
        }
    }
}

fn download_and_extract(
    model: &'static WhisperModel,
    tx: &Sender<DownloadEvent>,
) -> Result<(), String> {
    let dir = model_dir(model);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not create the model folder: {e}"))?;

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| format!("Could not start the download: {e}"))?;
    let mut resp = client
        .get(model.url)
        .send()
        .map_err(|e| format!("Could not reach the model download: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "The model download failed with status {}.",
            resp.status()
        ));
    }
    let total = resp.content_length().unwrap_or(model.archive_bytes);

    let archive_path = dir.join(format!("{}.tar.bz2.part", model.id));
    let mut file = File::create(&archive_path)
        .map_err(|e| format!("Could not save the model download: {e}"))?;
    let mut buffer = vec![0_u8; 128 * 1024];
    let mut downloaded = 0_u64;
    let mut last_report = Instant::now();
    loop {
        let read = resp
            .read(&mut buffer)
            .map_err(|e| format!("The model download stopped: {e}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|e| format!("Could not save the model download: {e}"))?;
        downloaded += read as u64;
        if last_report.elapsed() >= Duration::from_millis(120) {
            last_report = Instant::now();
            let _ = tx.send(DownloadEvent::Progress { downloaded, total });
        }
    }
    file.flush()
        .map_err(|e| format!("Could not save the model download: {e}"))?;
    drop(file);
    let _ = tx.send(DownloadEvent::Progress { downloaded, total });

    extract(model, &archive_path, &dir)?;
    let _ = std::fs::remove_file(&archive_path);

    if model_paths(model).is_none() {
        return Err(String::from(
            "The model download finished but the files are incomplete.",
        ));
    }
    let _ = tx.send(DownloadEvent::Ready);
    Ok(())
}

/// Unpack only the int8 encoder, decoder, and token table. The archive also
/// contains the much larger float32 models and sample audio.
fn extract(model: &WhisperModel, archive_path: &Path, dir: &Path) -> Result<(), String> {
    let (encoder, decoder_name, tokens) = model_file_names(model);
    let file = File::open(archive_path)
        .map_err(|e| format!("Could not open the downloaded model: {e}"))?;
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| format!("Could not read the model archive: {e}"))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| format!("Could not read the model archive: {e}"))?;
        let path = entry
            .path()
            .map_err(|e| format!("Could not read the model archive: {e}"))?
            .into_owned();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name != encoder && name != decoder_name && name != tokens {
            continue;
        }
        let target = dir.join(name);
        let mut out =
            File::create(&target).map_err(|e| format!("Could not write the model files: {e}"))?;
        std::io::copy(&mut entry, &mut out)
            .map_err(|e| format!("Could not extract the model files: {e}"))?;
    }
    Ok(())
}

fn cleanup_partial(model: &WhisperModel) {
    let dir = model_dir(model);
    let _ = std::fs::remove_file(dir.join(format!("{}.tar.bz2.part", model.id)));
}

/// A local Whisper session. Audio is captured (or decoded) on a background
/// thread and transcribed in overlapping windows.
pub struct LocalSession {
    cancel: Arc<AtomicBool>,
    events: Receiver<DictationEvent>,
    started: Instant,
}

impl LocalSession {
    /// Transcribe the microphone until [`Self::request_stop`] is called.
    pub fn start_microphone(paths: ModelPaths) -> Self {
        let (tx, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        std::thread::spawn(move || run_microphone(paths, tx, worker_cancel));
        Self {
            cancel,
            events,
            started: Instant::now(),
        }
    }

    /// Transcribe a single audio file into the document.
    pub fn start_file(paths: ModelPaths, path: PathBuf) -> Self {
        let (tx, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        std::thread::spawn(move || run_file(paths, path, tx, worker_cancel));
        Self {
            cancel,
            events,
            started: Instant::now(),
        }
    }

    /// How long this session has been running.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Ask the worker to finish. The microphone flushes its last audio; a file
    /// transcription stops after the current chunk.
    pub fn request_stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Grab the next event, if any.
    pub fn poll(&self) -> Option<DictationEvent> {
        match self.events.try_recv() {
            Ok(event) => Some(event),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(DictationEvent::Failed(String::from(
                "The local dictation worker stopped unexpectedly.",
            ))),
        }
    }
}

fn run_microphone(paths: ModelPaths, tx: Sender<DictationEvent>, stop: Arc<AtomicBool>) {
    let Some(recognizer) = create_recognizer(&paths, &tx) else {
        return;
    };
    let mut audio = match Microphone::start() {
        Ok(audio) => audio,
        Err(err) => {
            let _ = tx.send(DictationEvent::Failed(err));
            return;
        }
    };

    let window = (16_000.0 * WINDOW_SECONDS) as usize;
    let overlap = (16_000.0 * OVERLAP_SECONDS) as usize;
    let mut buffer: Vec<f32> = Vec::with_capacity(window * 2);
    let mut tail: Vec<String> = Vec::new();
    let mut first = true;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match audio.take_chunk() {
            Ok(Some(pcm)) => buffer.extend(pcm_to_f32(&pcm)),
            Ok(None) => {}
            Err(err) => {
                let _ = tx.send(DictationEvent::Failed(err));
                return;
            }
        }
        while buffer.len() >= window {
            let chunk: Vec<f32> = buffer[..window].to_vec();
            buffer.drain(..window - overlap);
            if is_silent(&chunk) {
                tail.clear();
                continue;
            }
            transcribe_and_emit(&recognizer, &chunk, &mut tail, &mut first, &tx);
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Flush whatever is left, ignoring a tiny trailing fragment.
    if buffer.len() >= 8_000 && !is_silent(&buffer) {
        transcribe_and_emit(&recognizer, &buffer, &mut tail, &mut first, &tx);
    }
    let _ = tx.send(DictationEvent::Finished);
}

/// Files are decoded up front and transcribed in 30-second windows. Whisper's
/// encoder is trained on 30-second clips, so going longer hurts accuracy.
fn run_file(paths: ModelPaths, path: PathBuf, tx: Sender<DictationEvent>, stop: Arc<AtomicBool>) {
    let samples = match decode_audio_file(&path) {
        Ok(samples) => samples,
        Err(err) => {
            let _ = tx.send(DictationEvent::Failed(err));
            return;
        }
    };
    let Some(recognizer) = create_recognizer(&paths, &tx) else {
        return;
    };

    let window = 16_000 * 30;
    let overlap = 16_000;
    let mut tail: Vec<String> = Vec::new();
    let mut first = true;
    let mut start = 0;
    while start < samples.len() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let end = (start + window).min(samples.len());
        let chunk = &samples[start..end];
        if !is_silent(chunk) {
            transcribe_and_emit(&recognizer, chunk, &mut tail, &mut first, &tx);
        } else {
            tail.clear();
        }
        if end == samples.len() {
            break;
        }
        start = end - overlap;
    }
    let _ = tx.send(DictationEvent::Finished);
}

fn create_recognizer(
    paths: &ModelPaths,
    tx: &Sender<DictationEvent>,
) -> Option<sherpa_onnx::OfflineRecognizer> {
    let mut config = sherpa_onnx::OfflineRecognizerConfig::default();
    config.model_config.whisper = sherpa_onnx::OfflineWhisperModelConfig {
        encoder: Some(paths.encoder.to_string_lossy().into_owned()),
        decoder: Some(paths.decoder.to_string_lossy().into_owned()),
        language: None,
        task: Some("transcribe".into()),
        tail_paddings: -1,
        ..Default::default()
    };
    config.model_config.tokens = Some(paths.tokens.to_string_lossy().into_owned());
    config.model_config.num_threads = std::thread::available_parallelism()
        .map(|n| n.get().min(4) as i32)
        .unwrap_or(2);
    match sherpa_onnx::OfflineRecognizer::create(&config) {
        Some(recognizer) => Some(recognizer),
        None => {
            let _ = tx.send(DictationEvent::Failed(String::from(
                "Could not load the local dictation model. Try downloading it again.",
            )));
            None
        }
    }
}

fn transcribe_and_emit(
    recognizer: &sherpa_onnx::OfflineRecognizer,
    samples: &[f32],
    tail: &mut Vec<String>,
    first: &mut bool,
    tx: &Sender<DictationEvent>,
) {
    let stream = recognizer.create_stream();
    stream.accept_waveform(16_000, samples);
    recognizer.decode(&stream);
    let Some(result) = stream.get_result() else {
        return;
    };
    let text = sanitize_transcript(&result.text);
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return;
    }
    let fresh = dedupe_tail(tail, &words);
    if fresh.is_empty() {
        *tail = words;
        return;
    }
    let mut text = fresh.join(" ");
    if !*first {
        text.insert(0, ' ');
    }
    *first = false;
    *tail = words;
    let _ = tx.send(DictationEvent::Delta(text));
}

/// Drop words at the start of a transcript that repeat the end of the previous
/// one, so the overlap doesn't produce duplicates.
pub(crate) fn dedupe_tail(previous: &[String], words: &[String]) -> Vec<String> {
    let max = previous.len().min(words.len()).min(12);
    for count in (1..=max).rev() {
        let prev = &previous[previous.len() - count..];
        let head = &words[..count];
        if prev
            .iter()
            .zip(head)
            .all(|(a, b)| normalize_word(a) == normalize_word(b))
        {
            return words[count..].to_vec();
        }
    }
    words.to_vec()
}

pub(crate) fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// Drop control characters, replacement glyphs, and box-drawing "squares"
/// Whisper emits when it hallucinates on noise. Words that contain no letters
/// or digits are removed entirely.
pub(crate) fn sanitize_transcript(text: &str) -> String {
    text.chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(
                    *c,
                    '\u{FFFD}' // replacement character
                        | '\u{25A0}'..='\u{25FF}' // geometric shapes (squares, circles)
                        | '\u{2610}'..='\u{2612}' // ballot boxes
                        | '\u{2B1B}' | '\u{2B1C}' // large squares
                )
        })
        .collect::<String>()
        .split_whitespace()
        .filter(|word| word.chars().any(|c| c.is_alphanumeric()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_silent(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let energy = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    energy.sqrt() < SILENCE_RMS
}

fn pcm_to_f32(pcm: &[u8]) -> impl Iterator<Item = f32> + '_ {
    pcm.chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32_768.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_overlapping_words() {
        let previous = vec!["the".to_string(), "quick".to_string(), "brown".to_string()];
        let next = vec!["brown".to_string(), "fox".to_string(), "jumps".to_string()];
        assert_eq!(dedupe_tail(&previous, &next), vec!["fox", "jumps"]);
    }

    #[test]
    fn dedupe_ignores_case_and_punctuation() {
        let previous = vec!["Brown,".to_string()];
        let next = vec!["brown".to_string(), "fox".to_string()];
        assert_eq!(dedupe_tail(&previous, &next), vec!["fox"]);
    }

    #[test]
    fn recognises_audio_extensions() {
        assert!(is_audio_file(Path::new("memo.WAV")));
        assert!(is_audio_file(Path::new("memo.mp3")));
        assert!(is_audio_file(Path::new("memo.m4a")));
        assert!(!is_audio_file(Path::new("notes.md")));
        assert!(!is_audio_file(Path::new("picture.png")));
    }

    #[test]
    fn silence_is_detected() {
        assert!(is_silent(&vec![0.0; 1_000]));
        assert!(!is_silent(&vec![0.2; 1_000]));
    }

    #[test]
    fn strips_symbol_hallucinations() {
        assert_eq!(sanitize_transcript("□□ □ ■"), "");
        assert_eq!(sanitize_transcript("hello \u{FFFD} world"), "hello world");
        assert_eq!(sanitize_transcript("hello... world!"), "hello... world!");
    }
}
