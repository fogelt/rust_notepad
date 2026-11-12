# Architecture

Rusty Notepad is a small egui/eframe desktop app. This document describes the
module boundaries so changes stay local and the pure logic stays testable.

## Binary crates

| Crate | Role |
| --- | --- |
| `rust_notepad` (`src/main.rs`) | The desktop app. |
| `rmd` (`src/bin/rmd.rs`) | Tiny console helper that forwards to `rmd_cli`. |

Both crate roots use `#![forbid(unsafe_code)]`. The only `unsafe` in the
workspace lives in the vendored litehtml FFI bindings (`vendor/litehtml`,
`vendor/litehtml-sys`), which are kept behind their public safe APIs.

## App layers

```
src/app/
  ai/                  pure assistant infrastructure
    mod.rs
    provider.rs        provider metadata (labels, models, env vars)
    prompt.rs          assistant + visual-report prompts
    text.rs            TextBackend trait and provider transports
    report.rs          HTML extraction, Commons photos, responsive cleanup
  components/          egui views and widgets
    ai_chat.rs         assistant panel + report orchestration
    editor.rs          editor / preview split, Markdown preview
    html_preview.rs    litehtml rasterizer, image cache, render worker
    toolbar.rs         Markdown toolbar and dictation button
    top_bar.rs         menu, tabs, settings tables
    ...
  dictation.rs         microphone/transcription state machine
  models/              persisted app state
  audio.rs             microphone capture and resampling
  local_stt.rs         Whisper model catalog, download, inference
  provider_stt.rs      Gemini Live / OpenAI transcription
  gemini_live.rs       Gemini Live WebSocket client
  config.rs            JSON settings (no API keys)
  keys.rs              OS keychain access
  theme.rs             colors, spacing, shared widgets
```

### Assistant (`app/ai`)

- `prompt` and `report` are deterministic string transformations. They have no
  `NotepadApp` dependency and are unit-tested directly.
- `text` owns the `TextBackend` trait. Each provider is one implementation;
  adding a provider does not touch request/response plumbing.
- `components::ai_chat` only orchestrates: it moves results between channels
  and app state, and renders the panel. It never builds HTTP requests itself.
- Any provider accepts a hand-typed model ID; curated models are only presets.

### Adding a provider

1. Add the enum variant in `ai/provider.rs` and one `ProviderSpec` entry in
   `SPECS` (id, labels, env vars, key metadata, curated models).
2. Add its transport in `ai/text.rs`. OpenAI-compatible servers share
   `ChatCompletionsBackend`; Gemini, OpenAI Responses, and Anthropic have their
   own backends.
3. If it supports transcription, update `provider_stt`.
4. Add the key prefix to `KEY_PREFIXES` if the provider has a recognizable one.

### Custom / OpenAI-compatible provider

`Provider::Custom` speaks the OpenAI chat-completions API, which covers Ollama,
LM Studio, llama.cpp, vLLM, and hosted gateways that use the same shape:

- The server URL lives in config (`custom_endpoint`); `custom_chat_endpoint`
  accepts a bare host, a `/v1` base, or the full `/v1/chat/completions` URL.
- `ChatCompletionsBackend` sends `Authorization` only when a key is configured,
  so local servers work without one while gateways can still require a key.
- `ProviderSpec::requires_api_key` is false and `allows_api_key` is true: the
  settings key controls are optional rather than hidden.
- `provider_stt` stays provider-specific; local chat models are not audio
  transcribers.

### Dictation (`app/dictation`)

The streamed-edit logic (interim/final text, selection replacement, cursor
restoration) lives together. `components::ai_chat` starts/stops sessions and
polls the shared state machine; it does not know how a transcript is merged
into the buffer.

### Rendering (`components::html_preview`)

Rendering runs on a background worker. The worker owns an `Arc<str>` of the
HTML and returns `ColorImage` pixels; the UI thread only uploads textures and
draws. The remote-image cache uses `Arc<Vec<u8>>` so cached bytes are shared
instead of cloned.

## Design rules

- **Single responsibility**: pure transformations live in `ai/`; UI and app
  state live in `components/`; persistence lives in `config`/`keys`.
- **Open for extension, closed for modification**: provider behavior is
  selected through the `TextBackend` trait. Providers are data (metadata) plus
  one transport implementation.
- **Errors at the edges**: provider/network failures become human-readable
  strings only at the UI boundary (`friendly_*` helpers). Pure helpers return
  `Option`/`Result` instead of panicking.
- **No panics on user input**: UI paths use `let-else`/`Option` for missing
  state. Tests may `unwrap`; production code does not.
- **No global mutable state** except the intentionally shared remote-image
  cache, which is guarded by a `Mutex` and `OnceLock`.
