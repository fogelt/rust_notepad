//! Microphone capture and audio-file decoding for dictation.
//!
//! Captures the default input device on a background audio callback, downmixes
//! to mono and resamples to 16 kHz, which is what Whisper expects. The session
//! thread drains 100 ms chunks while recording.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Dictation runs at 16 kHz mono.
pub const TARGET_SAMPLE_RATE: u32 = 16_000;
/// Capture chunk size: 100 ms of 16-bit mono audio.
pub const CHUNK_BYTES: usize = (TARGET_SAMPLE_RATE as usize / 10) * 2;

/// A microphone capture in progress.
pub struct Microphone {
    _stream: cpal::Stream,
    buffer: Arc<Mutex<Vec<f32>>>,
    error: Arc<Mutex<Option<String>>>,
    channels: usize,
    sample_rate: u32,
    /// Fractional read position for linear resampling across chunks.
    resample_pos: f64,
    pcm: Vec<u8>,
}

impl Microphone {
    /// Open the default microphone and start capturing.
    pub fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "No microphone found. Plug one in and try again.".to_string())?;
        let supported = device
            .default_input_config()
            .map_err(|e| format!("Could not query the microphone: {e}"))?;

        let sample_format = supported.sample_format();
        let config = supported.config();
        let channels = config.channels as usize;
        let sample_rate = config.sample_rate.0;

        let buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
        let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(&device, &config, &buffer, &error, |s| s),
            SampleFormat::F64 => {
                build_stream::<f64>(&device, &config, &buffer, &error, |s| s as f32)
            }
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

        Ok(Self {
            _stream: stream,
            buffer,
            error,
            channels,
            sample_rate,
            resample_pos: 0.0,
            pcm: Vec::with_capacity(CHUNK_BYTES * 2),
        })
    }

    /// Pull available samples and return the next 100 ms PCM chunk, if ready.
    pub fn take_chunk(&mut self) -> Result<Option<Vec<u8>>, String> {
        if let Some(err) = self
            .error
            .lock()
            .map_err(|_| "Microphone state was poisoned.".to_string())?
            .take()
        {
            return Err(err);
        }

        let samples = {
            let mut buffer = self
                .buffer
                .lock()
                .map_err(|_| "Microphone buffer was poisoned.".to_string())?;
            std::mem::take(&mut *buffer)
        };
        if !samples.is_empty() {
            let mono = downmix(&samples, self.channels);
            resample_into(
                &mono,
                self.sample_rate,
                &mut self.resample_pos,
                &mut self.pcm,
            );
        }

        if self.pcm.len() >= CHUNK_BYTES {
            let rest = self.pcm.split_off(CHUNK_BYTES);
            Ok(Some(std::mem::replace(&mut self.pcm, rest)))
        } else {
            Ok(None)
        }
    }
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

/// Average interleaved frames down to mono.
fn downmix(samples: &[f32], channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.to_vec();
    }
    samples
        .chunks(channels)
        .map(|frame| frame.iter().copied().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Linearly resample mono `f32` samples to 16 kHz and append little-endian
/// 16-bit PCM to `out`. `pos` carries the fractional position between calls.
fn resample_into(mono: &[f32], source_rate: u32, pos: &mut f64, out: &mut Vec<u8>) {
    if mono.is_empty() || source_rate == 0 {
        return;
    }
    let step = source_rate as f64 / TARGET_SAMPLE_RATE as f64;
    while *pos < mono.len() as f64 {
        let index = *pos as usize;
        let fraction = *pos - index as f64;
        let a = mono[index];
        let b = mono.get(index + 1).copied().unwrap_or(a);
        let value = a + (b - a) * fraction as f32;
        let pcm = (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        out.extend_from_slice(&pcm.to_le_bytes());
        *pos += step;
    }
    *pos -= mono.len() as f64;
}

/// Decoded files are held in memory, so reject anything unreasonable.
const MAX_AUDIO_BYTES: u64 = 200 * 1024 * 1024;

/// Decode an audio file to 16 kHz mono `f32` samples for Whisper. Supports the
/// formats bundled with symphonia: WAV, MP3, FLAC, Ogg/Vorbis, and MP4/AAC.
pub fn decode_audio_file(path: &Path) -> Result<Vec<f32>, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{CODEC_TYPE_NULL, DecoderOptions};
    use symphonia::core::errors::Error as SymphoniaError;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let size = std::fs::metadata(path)
        .map_err(|e| format!("Could not read the audio file: {e}"))?
        .len();
    if size > MAX_AUDIO_BYTES {
        return Err(format!(
            "That audio file is too large ({} MB). Use one under {} MB.",
            size / (1024 * 1024),
            MAX_AUDIO_BYTES / (1024 * 1024)
        ));
    }
    let file =
        std::fs::File::open(path).map_err(|e| format!("Could not open the audio file: {e}"))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            stream,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("Could not read the audio file: {e}"))?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| String::from("No audio track found in that file."))?;
    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(48_000);
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("Unsupported audio format: {e}"))?;

    let mut mono = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(err) => return Err(format!("Could not decode the audio file: {err}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A single corrupt frame shouldn't lose the rest of the file.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(err) => return Err(format!("Could not decode the audio file: {err}")),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        buffer.copy_interleaved_ref(decoded);
        for frame in buffer.samples().chunks(channels) {
            mono.push(frame.iter().sum::<f32>() / channels as f32);
        }
    }

    if mono.is_empty() {
        return Err(String::from("The audio file is empty."));
    }
    Ok(resample_mono(&mono, sample_rate))
}

/// Linearly resample mono `f32` audio to 16 kHz.
fn resample_mono(samples: &[f32], source_rate: u32) -> Vec<f32> {
    if source_rate == 0 || samples.is_empty() {
        return Vec::new();
    }
    if source_rate == TARGET_SAMPLE_RATE {
        return samples.to_vec();
    }
    let step = source_rate as f64 / TARGET_SAMPLE_RATE as f64;
    let mut out = Vec::with_capacity((samples.len() as f64 / step).ceil() as usize);
    let mut pos = 0.0_f64;
    while pos < samples.len() as f64 {
        let index = pos as usize;
        let fraction = pos - index as f64;
        let a = samples[index];
        let b = samples.get(index + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * fraction as f32);
        pos += step;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_is_one_hundred_milliseconds() {
        assert_eq!(CHUNK_BYTES, 3_200);
    }

    #[test]
    fn downmixes_stereo_to_mono() {
        let mono = downmix(&[1.0, -1.0, 0.5, 0.5], 2);
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn resamples_48k_to_16k() {
        let source = vec![0.0_f32; 4_800];
        let mut pos = 0.0;
        let mut out = Vec::new();
        resample_into(&source, 48_000, &mut pos, &mut out);
        assert_eq!(out.len(), 1_600 * 2);
    }

    #[test]
    fn resampling_carries_position_between_chunks() {
        let mut pos = 0.0;
        let mut out = Vec::new();
        // 481 samples at 48 kHz: the fractional remainder must survive.
        resample_into(&vec![0.0_f32; 481], 48_000, &mut pos, &mut out);
        let after_first = out.len();
        resample_into(&vec![0.0_f32; 479], 48_000, &mut pos, &mut out);
        // 960 input samples at 48 kHz is 320 output samples total.
        assert_eq!(out.len(), 320 * 2);
        assert!(after_first > 0 && after_first < out.len());
    }
}
