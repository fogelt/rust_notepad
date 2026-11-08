//! Microphone capture for document dictation.
//!
//! Records from the default input device on a background thread, then sends a
//! mono 16-bit WAV to the UI thread. Gemini accepts that WAV directly as inline
//! audio, so no external transcriber or encoder is needed. The transcription is
//! inserted into the open document at the editor cursor.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Hard cap so a forgotten recording cannot grow without bound.
pub const MAX_SECONDS: u64 = 60;

/// A recording in progress. Poll [`RecordingSession::poll`] until it returns
/// `Some`, which means the microphone thread finished and produced WAV bytes.
pub struct RecordingSession {
    stop: Arc<AtomicBool>,
    rx: Receiver<Result<Vec<u8>, String>>,
    started: Instant,
}

impl RecordingSession {
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Ask the recording thread to stop and finalize the WAV.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Non-blocking check for the finished recording.
    pub fn poll(&self) -> Option<Result<Vec<u8>, String>> {
        match self.rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(
                "The microphone thread stopped unexpectedly.".to_string(),
            )),
        }
    }
}

/// Start recording from the default input device. Device errors are reported
/// through the session's receiver, so this never blocks the UI.
pub fn start() -> RecordingSession {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        let _ = tx.send(record_blocking(thread_stop));
    });
    RecordingSession {
        stop,
        rx,
        started: Instant::now(),
    }
}

fn record_blocking(stop: Arc<AtomicBool>) -> Result<Vec<u8>, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No microphone found. Plug one in and try again.".to_string())?;
    let supported = device
        .default_input_config()
        .map_err(|e| format!("Could not query the microphone: {e}"))?;

    let sample_format = supported.sample_format();
    let config = supported.config();
    let channels = config.channels;
    let sample_rate = config.sample_rate.0;

    let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(&device, &config, &buffer, &error, |s| s),
        SampleFormat::F64 => build_stream::<f64>(&device, &config, &buffer, &error, |s| s as f32),
        SampleFormat::I8 => {
            build_stream::<i8>(&device, &config, &buffer, &error, |s| s as f32 / 128.0)
        }
        SampleFormat::I16 => {
            build_stream::<i16>(&device, &config, &buffer, &error, |s| s as f32 / 32_768.0)
        }
        SampleFormat::I32 => build_stream::<i32>(&device, &config, &buffer, &error, |s| {
            (s as f64 / 2_147_483_648.0) as f32
        }),
        SampleFormat::I64 => build_stream::<i64>(&device, &config, &buffer, &error, |s| {
            (s as f64 / 9_223_372_036_854_775_808.0) as f32
        }),
        SampleFormat::U8 => build_stream::<u8>(&device, &config, &buffer, &error, |s| {
            (s as f32 - 128.0) / 128.0
        }),
        SampleFormat::U16 => build_stream::<u16>(&device, &config, &buffer, &error, |s| {
            (s as f32 - 32_768.0) / 32_768.0
        }),
        SampleFormat::U32 => build_stream::<u32>(&device, &config, &buffer, &error, |s| {
            ((s as f64 - 2_147_483_648.0) / 2_147_483_648.0) as f32
        }),
        SampleFormat::U64 => build_stream::<u64>(&device, &config, &buffer, &error, |s| {
            ((s as f64 - 9_223_372_036_854_775_808.0) / 9_223_372_036_854_775_808.0) as f32
        }),
        other => return Err(format!("Unsupported microphone sample format: {other:?}")),
    }?;

    stream
        .play()
        .map_err(|e| format!("Could not start the microphone: {e}"))?;

    let started = Instant::now();
    while !stop.load(Ordering::Relaxed) && started.elapsed() < Duration::from_secs(MAX_SECONDS) {
        std::thread::sleep(Duration::from_millis(40));
    }
    drop(stream);

    if let Some(err) = error
        .lock()
        .map_err(|_| "Microphone state was poisoned.".to_string())?
        .take()
    {
        return Err(err);
    }
    let samples = buffer
        .lock()
        .map_err(|_| "Microphone buffer was poisoned.".to_string())?
        .clone();
    if samples.is_empty() {
        return Err("No audio was captured. Check your microphone permission.".to_string());
    }

    Ok(encode_wav(&samples, channels, sample_rate))
}

fn build_stream<T: SizedSample + 'static>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: &Arc<Mutex<Vec<f32>>>,
    error: &Arc<Mutex<Option<String>>>,
    convert: fn(T) -> f32,
) -> Result<cpal::Stream, String> {
    let buffer = Arc::clone(buffer);
    let error = Arc::clone(error);
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                if let Ok(mut samples) = buffer.lock() {
                    samples.reserve(data.len());
                    for &sample in data {
                        samples.push(convert(sample));
                    }
                }
            },
            move |err| {
                if let Ok(mut slot) = error.lock() {
                    *slot = Some(format!("Microphone error: {err}"));
                }
            },
            None,
        )
        .map_err(|e| format!("Could not open the microphone: {e}"))
}

/// Downmix interleaved samples to mono and encode a 16-bit PCM WAV.
fn encode_wav(samples: &[f32], channels: u16, sample_rate: u32) -> Vec<u8> {
    let channels = channels.max(1) as usize;
    let mono: Vec<f32> = if channels == 1 {
        samples.to_vec()
    } else {
        samples
            .chunks(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
            .collect()
    };

    let data_len = (mono.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in mono {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_a_mono_wav_header() {
        let wav = encode_wav(&[0.0, 1.0, -1.0, 0.0], 2, 48_000);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[20], wav[21]]), 1);
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 1);
        assert_eq!(
            u32::from_le_bytes([wav[24], wav[25], wav[26], wav[27]]),
            48_000
        );
        assert_eq!(u16::from_le_bytes([wav[34], wav[35]]), 16);
        assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 4);
        assert_eq!(wav.len(), 44 + 4);
    }

    #[test]
    fn downmixes_stereo_to_mono() {
        let wav = encode_wav(&[1.0, 1.0, -1.0, -1.0], 2, 16_000);
        assert_eq!(u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]), 4);
        let first = i16::from_le_bytes([wav[44], wav[45]]);
        assert!(first > 32_000);
    }

    #[test]
    #[ignore = "requires a working microphone"]
    fn records_a_short_clip() {
        let session = start();
        std::thread::sleep(Duration::from_millis(400));
        session.request_stop();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(result) = session.poll() {
                let wav = result.expect("recording should succeed");
                assert!(wav.len() > 44);
                return;
            }
            assert!(Instant::now() < deadline, "recording timed out");
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}
