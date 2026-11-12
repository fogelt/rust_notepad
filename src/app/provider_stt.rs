//! Provider-backed dictation.
//!
//! Records (or decodes) 16 kHz mono audio in short windows and sends them to a
//! cloud transcription endpoint. Used when the user chooses to transcribe with
//! their provider instead of the local Whisper model.

use crate::app::ai::http::check_response;
use crate::app::ai::provider::Provider;
use crate::app::audio::{Microphone, decode_audio_file};
use crate::app::gemini_live::{LIVE_MODEL, LiveEvent, LiveSession};
use crate::app::local_stt::{dedupe_tail, sanitize_transcript};
use crate::app::models::DictationEvent;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

const SAMPLE_RATE: usize = 16_000;
const WINDOW_SECONDS: f32 = 4.0;
const OVERLAP_SECONDS: f32 = 0.6;
const FILE_WINDOW_SECONDS: usize = 30;
const FILE_OVERLAP_SECONDS: usize = 1;
const SILENCE_RMS: f32 = 0.006;

/// Whether the provider has a transcription endpoint we can use.
pub fn supports(provider: Provider) -> bool {
    matches!(provider, Provider::Gemini | Provider::OpenAi)
}

/// Model name to show for provider dictation.
pub fn transcription_model(provider: Provider) -> &'static str {
    match provider {
        Provider::Gemini => LIVE_MODEL,
        Provider::OpenAi => "whisper-1",
        _ => "",
    }
}

/// Model used for dropped audio files. Gemini's Live model is only available
/// over the streaming API, so files use the selected audio-capable chat model.
pub fn file_model(provider: Provider, chat_model: &str) -> String {
    match provider {
        Provider::Gemini => chat_model.trim().to_string(),
        Provider::OpenAi => String::from("whisper-1"),
        _ => String::new(),
    }
}

enum SessionKind {
    Http {
        cancel: Arc<AtomicBool>,
        events: Receiver<DictationEvent>,
    },
    Live(LiveSession),
}

pub struct ProviderSession {
    kind: SessionKind,
    started: Instant,
}

impl ProviderSession {
    pub fn start_microphone(provider: Provider, api_key: String, model: String) -> Self {
        if provider == Provider::Gemini {
            return Self {
                kind: SessionKind::Live(crate::app::gemini_live::start(api_key)),
                started: Instant::now(),
            };
        }
        let (tx, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        std::thread::spawn(move || run_microphone(provider, api_key, model, tx, worker_cancel));
        Self {
            kind: SessionKind::Http { cancel, events },
            started: Instant::now(),
        }
    }

    pub fn start_file(provider: Provider, api_key: String, model: String, path: PathBuf) -> Self {
        let (tx, events) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_cancel = Arc::clone(&cancel);
        std::thread::spawn(move || run_file(provider, api_key, model, path, tx, worker_cancel));
        Self {
            kind: SessionKind::Http { cancel, events },
            started: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        match &self.kind {
            SessionKind::Http { .. } => self.started.elapsed(),
            SessionKind::Live(session) => session.elapsed(),
        }
    }

    pub fn request_stop(&self) {
        match &self.kind {
            SessionKind::Http { cancel, .. } => cancel.store(true, Ordering::Relaxed),
            SessionKind::Live(session) => session.request_stop(),
        }
    }

    pub fn poll(&self) -> Option<DictationEvent> {
        match &self.kind {
            SessionKind::Live(session) => match session.poll()? {
                LiveEvent::Interim(text) => Some(DictationEvent::Interim(text)),
                LiveEvent::Final(text) => Some(DictationEvent::Final(text)),
                LiveEvent::Error(err) => Some(DictationEvent::Failed(err)),
                LiveEvent::Closed => Some(DictationEvent::Finished),
            },
            SessionKind::Http { events, .. } => match events.try_recv() {
                Ok(event) => Some(event),
                Err(mpsc::TryRecvError::Empty) => None,
                Err(mpsc::TryRecvError::Disconnected) => Some(DictationEvent::Failed(
                    String::from("The transcription worker stopped unexpectedly."),
                )),
            },
        }
    }
}

fn run_microphone(
    provider: Provider,
    api_key: String,
    model: String,
    tx: Sender<DictationEvent>,
    stop: Arc<AtomicBool>,
) {
    let mut audio = match Microphone::start() {
        Ok(audio) => audio,
        Err(err) => {
            let _ = tx.send(DictationEvent::Failed(err));
            return;
        }
    };

    let window = (SAMPLE_RATE as f32 * WINDOW_SECONDS) as usize * 2;
    let overlap = (SAMPLE_RATE as f32 * OVERLAP_SECONDS) as usize * 2;
    let mut buffer: Vec<u8> = Vec::with_capacity(window * 2);
    let mut tail: Vec<String> = Vec::new();
    let mut first = true;

    loop {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        match audio.take_chunk() {
            Ok(Some(pcm)) => buffer.extend_from_slice(&pcm),
            Ok(None) => {}
            Err(err) => {
                let _ = tx.send(DictationEvent::Failed(err));
                return;
            }
        }
        while buffer.len() >= window {
            let chunk = buffer[..window].to_vec();
            buffer.drain(..window - overlap);
            if is_silent_pcm(&chunk) {
                tail.clear();
                continue;
            }
            if let Err(err) = transcribe_chunk(
                provider, &api_key, &model, &chunk, &tx, &mut tail, &mut first,
            ) {
                let _ = tx.send(DictationEvent::Failed(err));
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    if buffer.len() >= SAMPLE_RATE && !is_silent_pcm(&buffer) {
        let _ = transcribe_chunk(
            provider, &api_key, &model, &buffer, &tx, &mut tail, &mut first,
        );
    }
    let _ = tx.send(DictationEvent::Finished);
}

fn run_file(
    provider: Provider,
    api_key: String,
    model: String,
    path: PathBuf,
    tx: Sender<DictationEvent>,
    stop: Arc<AtomicBool>,
) {
    let samples = match decode_audio_file(&path) {
        Ok(samples) => samples,
        Err(err) => {
            let _ = tx.send(DictationEvent::Failed(err));
            return;
        }
    };

    let window = SAMPLE_RATE * FILE_WINDOW_SECONDS;
    let overlap = SAMPLE_RATE * FILE_OVERLAP_SECONDS;
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
            let pcm = f32_to_pcm16(chunk);
            if let Err(err) =
                transcribe_chunk(provider, &api_key, &model, &pcm, &tx, &mut tail, &mut first)
            {
                let _ = tx.send(DictationEvent::Failed(err));
                return;
            }
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

fn transcribe_chunk(
    provider: Provider,
    api_key: &str,
    model: &str,
    pcm: &[u8],
    tx: &Sender<DictationEvent>,
    tail: &mut Vec<String>,
    first: &mut bool,
) -> Result<(), String> {
    let wav = wav_bytes(pcm);
    let raw = transcribe(provider, api_key, model, wav)?;
    let text = sanitize_transcript(&raw);
    let words: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    if words.is_empty() {
        return Ok(());
    }
    let fresh = dedupe_tail(tail, &words);
    if fresh.is_empty() {
        *tail = words;
        return Ok(());
    }
    let mut text = fresh.join(" ");
    if !*first {
        text.insert(0, ' ');
    }
    *first = false;
    *tail = words;
    let _ = tx.send(DictationEvent::Delta(text));
    Ok(())
}

fn transcribe(
    provider: Provider,
    api_key: &str,
    model: &str,
    wav: Vec<u8>,
) -> Result<String, String> {
    match provider {
        Provider::Gemini => gemini_transcribe(api_key, model, wav),
        Provider::OpenAi => openai_transcribe(api_key, wav),
        _ => Err(String::from(
            "This provider doesn't offer audio transcription.",
        )),
    }
}

fn gemini_transcribe(api_key: &str, model: &str, wav: Vec<u8>) -> Result<String, String> {
    use base64::Engine as _;
    let audio = base64::engine::general_purpose::STANDARD.encode(wav);
    let body = serde_json::json!({
        "contents": [{
            "role": "user",
            "parts": [
                { "inlineData": { "mimeType": "audio/wav", "data": audio } },
                { "text": "Transcribe the speech in this audio exactly as spoken. \
                           Return only the transcription text, with no commentary, labels, \
                           or quotation marks." }
            ]
        }],
        "generationConfig": { "temperature": 0.0, "maxOutputTokens": 1024 }
    });
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
        model.trim()
    );
    let resp = client()?
        .post(&url)
        .query(&[("key", api_key)])
        .json(&body)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let json = check_response(resp, "Gemini")?;
    let parts = json["candidates"][0]["content"]["parts"]
        .as_array()
        .ok_or_else(|| String::from("Gemini returned an empty response."))?;
    let text: String = parts
        .iter()
        .filter(|part| part.get("thought").and_then(|v| v.as_bool()) != Some(true))
        .filter_map(|part| part["text"].as_str())
        .collect();
    if text.trim().is_empty() {
        Err(String::from("Gemini returned an empty response."))
    } else {
        Ok(text)
    }
}

fn openai_transcribe(api_key: &str, wav: Vec<u8>) -> Result<String, String> {
    let form = reqwest::blocking::multipart::Form::new()
        .text("model", "whisper-1")
        .part(
            "file",
            reqwest::blocking::multipart::Part::bytes(wav)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| format!("Request failed: {e}"))?,
        );
    let resp = client()?
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;
    let json = check_response(resp, "OpenAI")?;
    let text = json["text"]
        .as_str()
        .ok_or_else(|| String::from("OpenAI returned an empty response."))?;
    if text.trim().is_empty() {
        Err(String::from("OpenAI returned an empty response."))
    } else {
        Ok(text.to_string())
    }
}

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|e| format!("Request failed: {e}"))
}

fn wav_bytes(pcm: &[u8]) -> Vec<u8> {
    let data_len = pcm.len() as u32;
    let byte_rate = (SAMPLE_RATE as u32) * 2;
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1_u16.to_le_bytes()); // mono
    wav.extend_from_slice(&(SAMPLE_RATE as u32).to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm);
    wav
}

fn f32_to_pcm16(samples: &[f32]) -> Vec<u8> {
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&value.to_le_bytes());
    }
    pcm
}

fn is_silent(samples: &[f32]) -> bool {
    if samples.is_empty() {
        return true;
    }
    let energy = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    energy.sqrt() < SILENCE_RMS
}

fn is_silent_pcm(pcm: &[u8]) -> bool {
    let samples: Vec<f32> = pcm
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32_768.0)
        .collect();
    is_silent(&samples)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_audio_capable_providers_support_transcription() {
        assert!(supports(Provider::Gemini));
        assert!(supports(Provider::OpenAi));
        assert!(!supports(Provider::Anthropic));
        assert!(!supports(Provider::OpenRouter));
        assert!(!supports(Provider::DeepSeek));
    }

    #[test]
    fn wav_header_matches_pcm_payload() {
        let wav = wav_bytes(&[0_u8; 8]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(wav.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
    }
}
