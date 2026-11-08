//! Real-time dictation over the Gemini Live API.
//!
//! The Live API accepts 16 kHz mono PCM over a WebSocket and emits interim and
//! finalized input transcriptions while the user is still speaking. A dedicated
//! session thread owns the socket and the microphone so the UI stays responsive.

use crate::app::audio::LiveAudio;
use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

/// Dedicated low-latency speech-to-text model for streaming dictation.
pub const LIVE_MODEL: &str = "gemini-3.5-transcribe-live";

const WS_URL: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";

/// How long to keep listening for a final transcript after Stop is pressed.
const FINALIZE_TIMEOUT: Duration = Duration::from_secs(3);

type WsStream =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// Events forwarded from the Live session thread to the UI.
pub enum LiveEvent {
    /// A speculative partial transcription that replaces the previous one.
    Interim(String),
    /// An authoritative transcription for the current speech segment.
    Final(String),
    /// The session failed; the payload is the raw error.
    Error(String),
    /// The socket closed (normally or after an error).
    Closed,
}

/// A live dictation session in progress.
pub struct LiveSession {
    stop: Arc<AtomicBool>,
    rx: Receiver<LiveEvent>,
    started: Instant,
    closed: AtomicBool,
}

impl LiveSession {
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// Ask the session to finalize the current turn and close.
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Non-blocking check for the next event.
    pub fn poll(&self) -> Option<LiveEvent> {
        match self.rx.try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                // The worker is gone. Report the close exactly once so callers
                // that drain in a loop always terminate.
                if self.closed.swap(true, Ordering::Relaxed) {
                    None
                } else {
                    Some(LiveEvent::Closed)
                }
            }
        }
    }
}

/// Start streaming dictation. Connection and microphone errors are reported as
/// [`LiveEvent::Error`] so the UI can toast them.
pub fn start(api_key: String) -> LiveSession {
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel();
    let thread_stop = Arc::clone(&stop);
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let result = match runtime {
            Ok(runtime) => runtime.block_on(run_session(&api_key, &thread_stop, &tx)),
            Err(err) => Err(format!("Could not start the Live session: {err}")),
        };
        if let Err(err) = result {
            let _ = tx.send(LiveEvent::Error(err));
        }
        let _ = tx.send(LiveEvent::Closed);
    });
    LiveSession {
        stop,
        rx,
        started: Instant::now(),
        closed: AtomicBool::new(false),
    }
}

/// What a server message means for the session loop.
enum MessageAction {
    Continue,
    TurnComplete,
    Closed,
}

async fn run_session(
    api_key: &str,
    stop: &AtomicBool,
    tx: &mpsc::Sender<LiveEvent>,
) -> Result<(), String> {
    let url = format!("{WS_URL}?key={}", api_key.trim());
    let (mut socket, _response) =
        tokio::time::timeout(Duration::from_secs(10), connect_async(url.as_str()))
            .await
            .map_err(|_| "Timed out connecting to Gemini Live.".to_string())?
            .map_err(|err| format!("Could not connect to Gemini Live: {err}"))?;

    let setup = serde_json::json!({
        "setup": {
            "model": format!("models/{LIVE_MODEL}"),
            "generationConfig": { "responseModalities": ["TEXT"] },
            "inputAudioTranscription": {
                "languageCodes": [],
                "mode": "SMART"
            },
            // Push-to-talk: the app owns the speech boundaries.
            "realtimeInputConfig": {
                "automaticActivityDetection": { "disabled": true }
            }
        }
    });
    send_json(&mut socket, &setup).await?;

    wait_for_setup(&mut socket, tx).await?;

    let mut audio = LiveAudio::start().map_err(|err| format!("Microphone error: {err}"))?;
    send_json(
        &mut socket,
        &serde_json::json!({ "realtimeInput": { "activityStart": {} } }),
    )
    .await?;

    loop {
        while let Some(chunk) = audio.take_chunk()? {
            socket
                .send(audio_message(&chunk))
                .await
                .map_err(|err| format!("Could not send audio to Gemini Live: {err}"))?;
        }

        if stop.load(Ordering::Relaxed) {
            finalize_turn(&mut socket, tx).await;
            break;
        }

        // Wake up regularly to forward microphone audio, even when Gemini has
        // nothing to say.
        match tokio::time::timeout(Duration::from_millis(30), socket.next()).await {
            Ok(Some(Ok(message))) => {
                if !matches!(handle_message(message, tx)?, MessageAction::Continue) {
                    break;
                }
            }
            Ok(Some(Err(err))) => return Err(format!("Gemini Live session failed: {err}")),
            Ok(None) => break,
            // Nothing arrived; loop and send more audio.
            Err(_) => {}
        }
    }

    let _ = socket.close(None).await;
    Ok(())
}

/// Wait for Gemini's `setupComplete` acknowledgement.
async fn wait_for_setup(socket: &mut WsStream, tx: &mpsc::Sender<LiveEvent>) -> Result<(), String> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return Err(String::from(
                "Gemini did not acknowledge the Live dictation session.",
            ));
        }
        match tokio::time::timeout(remaining, socket.next()).await {
            Err(_) => {
                return Err(String::from(
                    "Gemini did not acknowledge the Live dictation session.",
                ));
            }
            Ok(None) => return Err(String::from("Gemini closed the Live dictation session.")),
            Ok(Some(Err(err))) => return Err(format!("Gemini Live setup failed: {err}")),
            Ok(Some(Ok(message))) => {
                let text = message_text(message)?;
                let Some(text) = text else {
                    continue;
                };
                let json: serde_json::Value = serde_json::from_str(&text)
                    .map_err(|err| format!("Bad Live response from Gemini: {err}"))?;
                if json.get("setupComplete").is_some() {
                    return Ok(());
                }
                if let Some(err) = server_error(&json) {
                    return Err(err);
                }
                handle_server_content(&json, tx);
            }
        }
    }
}

/// Signal the end of speech and wait briefly for the final transcript.
async fn finalize_turn(socket: &mut WsStream, tx: &mpsc::Sender<LiveEvent>) {
    if send_json(
        socket,
        &serde_json::json!({ "realtimeInput": { "activityEnd": {} } }),
    )
    .await
    .is_err()
    {
        return;
    }
    let _ = send_json(
        socket,
        &serde_json::json!({ "realtimeInput": { "audioStreamEnd": true } }),
    )
    .await;

    let deadline = tokio::time::Instant::now() + FINALIZE_TIMEOUT;
    while !deadline
        .saturating_duration_since(tokio::time::Instant::now())
        .is_zero()
    {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        match tokio::time::timeout(remaining, socket.next()).await {
            Ok(Some(Ok(message))) => match handle_message(message, tx) {
                Ok(MessageAction::Continue) => {}
                _ => break,
            },
            _ => break,
        }
    }
}

/// Forward a server message and report whether the turn is finished.
fn handle_message(message: Message, tx: &mpsc::Sender<LiveEvent>) -> Result<MessageAction, String> {
    if matches!(&message, Message::Close(_)) {
        return Ok(MessageAction::Closed);
    }
    let Some(text) = message_text(message)? else {
        return Ok(MessageAction::Continue);
    };
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|err| format!("Bad Live response from Gemini: {err}"))?;
    if let Some(err) = server_error(&json) {
        return Err(err);
    }
    handle_server_content(&json, tx);
    if json
        .pointer("/serverContent/turnComplete")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        Ok(MessageAction::TurnComplete)
    } else {
        Ok(MessageAction::Continue)
    }
}

fn handle_server_content(json: &serde_json::Value, tx: &mpsc::Sender<LiveEvent>) {
    let Some(content) = json.get("serverContent") else {
        return;
    };
    if let Some(text) = content
        .pointer("/interimInputTranscription/text")
        .and_then(|value| value.as_str())
        && !text.is_empty()
    {
        let _ = tx.send(LiveEvent::Interim(text.to_string()));
    }
    if let Some(text) = content
        .pointer("/inputTranscription/text")
        .and_then(|value| value.as_str())
        && !text.is_empty()
    {
        let _ = tx.send(LiveEvent::Final(text.to_string()));
    }
}

fn message_text(message: Message) -> Result<Option<String>, String> {
    match message {
        Message::Text(text) => Ok(Some(text.to_string())),
        Message::Binary(bytes) => String::from_utf8(bytes.to_vec())
            .map(Some)
            .map_err(|err| format!("Bad Live response from Gemini: {err}")),
        _ => Ok(None),
    }
}

fn server_error(json: &serde_json::Value) -> Option<String> {
    json.get("error")
        .and_then(|error| error.get("message"))
        .and_then(|message| message.as_str())
        .map(|message| message.to_string())
}

async fn send_json(socket: &mut WsStream, value: &serde_json::Value) -> Result<(), String> {
    socket
        .send(Message::Text(value.to_string()))
        .await
        .map_err(|err| format!("Could not talk to Gemini Live: {err}"))
}

fn audio_message(chunk: &[u8]) -> Message {
    let data = base64::engine::general_purpose::STANDARD.encode(chunk);
    let value = serde_json::json!({
        "realtimeInput": {
            "audio": {
                "data": data,
                "mimeType": "audio/pcm;rate=16000"
            }
        }
    });
    Message::Text(value.to_string())
}
