# Rusty Notepad

A lightweight Markdown text editor built in Rust on [eframe](https://github.com/emilk/egui)/egui.

## Run

```bash
cargo run
```

Open a file directly with `cargo run -- <file.md>`.

## Configure (optional)

The Gemini assistant needs an API key ([get one here](https://aistudio.google.com/api-keys)).
For development, put it in a `.env` file (gitignored):

```env
GEMINI_API_KEY=…
GEMINI_MODEL=gemini-2.5-flash
```

At runtime the Assistant tab can also save both values to `%APPDATA%\rust_notepad\config.json`
(`~/.config/rust_notepad/config.json` elsewhere); saved values take precedence over `.env`.

## Checks

```bash
cargo test      # unit tests
cargo fmt       # formatting (required clean)
cargo clippy    # lint (only pre-existing File-menu notes should remain)
```

## Build / package

```bash
cargo build --release
cargo packager --release   # nsis / deb / appimage (needs cargo-packager)
```

Windows executables embed `assets/icon.ico` via `build.rs`.

## Layout

* `src/main.rs` — window/viewport setup, font + `.env` loading
* `src/app/mod.rs` — `eframe::App` update loop, panel + window-size management
* `src/app/components/` — `top_bar`, `toolbar`, `editor`, `ai_chat`, `bottom_bar`
* `src/app/models/notepad.rs` — app state
* `src/app/theme.rs` — shared accent color, panel metrics, panel helpers
* `src/app/config.rs` — assistant settings persistence
