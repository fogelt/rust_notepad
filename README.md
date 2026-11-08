# Rusty Notepad

A fast, lightweight Markdown editor for Windows, macOS and Linux. Write on the
left, see the formatted document on the right, and optionally ask Gemini to
rewrite or extend it.

![Rusty Notepad with the editor and live preview side by side](assets/screenshot.png)

## Features

- **Live Markdown preview** that updates as you type.
- **Full-window preview** — hide the editor, or press `F11` for real fullscreen
  (`Esc` leaves it).
- **Formatting toolbar** for headings, lists, quotes, tables, links, images,
  code blocks, and more.
- **Starter templates** in the empty preview: README, meeting notes, blog post,
  changelog, task list, and project brief.
- **Autosave** once the document has a file name.
- **`rmd` command** to open or create notes from any terminal.
- **Optional Gemini assistant** that can rewrite or extend the document.
- **Voice instructions** — record up to 60 seconds, or drop an audio file, and
  Gemini transcribes it into the instruction box.
- **Native installers** for Windows, macOS (Apple Silicon and Intel), Debian/
  Ubuntu, and any Linux via AppImage.

## Install

Download the latest build from the
[releases page](https://github.com/fogelt/rust_notepad/releases).

| Platform | Artifact | Notes |
| --- | --- | --- |
| Windows | `*-setup.exe` | Current-user install, no admin. Adds `rmd` to `PATH` and registers `.md` / `.markdown`. |
| macOS | `.dmg` | Drag **Rusty Notepad** to Applications. |
| Debian/Ubuntu | `.deb` | Installs `rust_notepad` and `rmd` under `/usr/bin`. |
| Any Linux | `.AppImage` | Make it executable and run it. |

### Windows

Run the setup executable. Windows SmartScreen may warn because the installer is
unsigned; choose **More info → Run anyway**.

### macOS

Open the `.dmg` and drag the app to Applications. The build is unsigned, so the
first launch needs **right-click → Open**. macOS asks for microphone access the
first time you record a voice instruction.

### Linux

```bash
# Debian / Ubuntu
sudo apt install ./rust_notepad_*.deb

# AppImage
chmod +x rust_notepad_*.AppImage
./rust_notepad_*.AppImage
```

If your system cannot run AppImages directly, use
`./rust_notepad_*.AppImage --appimage-extract-and-run`.

## Using Rusty Notepad

- **File menu** — New, Open, Save, and Save As. Shortcuts: `Ctrl/Cmd+N`,
  `Ctrl/Cmd+O`, `Ctrl/Cmd+S`, `Ctrl/Cmd+Shift+S`.
- **Panels** — the Assistant, Editor, and Preview tabs toggle each panel. Hiding
  the editor gives the preview the full window.
- **Templates** — when the document is empty, the preview shows template buttons
  that fill the editor with a starting document.
- **Fullscreen** — `F11` toggles it; `Esc` exits.

## `rmd` command

`rmd` opens a note from the terminal, creating it if needed. If no name is
given, it prompts for one. Bare names get a `.md` extension.

```text
$ rmd
Name for the new note: ideas
Created /home/me/ideas.md
Opening /home/me/ideas.md in Rusty Notepad…

$ rmd todo.markdown
Opening /home/me/todo.markdown in Rusty Notepad…
```

On **Windows** and with the Linux **`.deb`**, `rmd` is installed on `PATH`
automatically. On **macOS**, link the helper from the app bundle once:

```bash
sudo ln -sf "/Applications/Rusty Notepad.app/Contents/MacOS/rmd" /usr/local/bin/rmd
```

For the **AppImage**, run the `install-rmd.sh` script included in the release:

```bash
sh install-rmd.sh ./rust_notepad_*.AppImage
```

It creates a `rmd` launcher in `~/.local/bin` (or `/usr/local/bin` when
writable). Use `rmd --help` for options.

## Assistant and voice (optional)

The assistant needs a Gemini API key from
[Google AI Studio](https://aistudio.google.com/api-keys).

1. Open the **Assistant** tab and expand **Settings**.
2. Paste the API key and choose from a few curated Gemini models, then save.
3. Type an instruction, or use **Record** / drop an audio file. The transcription
   appears in the instruction box; review it and press **Send**.

`Ctrl/Cmd+Enter` sends from the instruction box. Voice recordings are capped at
60 seconds and dropped audio files at 14 MB; supported formats include `wav`,
`mp3`, `m4a`, `ogg`, and `flac`. Transcription uses the selected Gemini model,
so pick one with audio input support.

The key and model are stored locally in
`%APPDATA%\rust_notepad\config.json` on Windows and
`~/.config/rust_notepad/config.json` elsewhere.

The built-in free-tier limits are a static snapshot, not live usage; open
AI Studio for current limits. The request counter only counts this app session.

## Development

Requires a recent stable Rust toolchain (1.87 or newer).

### Run and check

```bash
cargo run
cargo run -- <file.md>

cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

The microphone smoke test is ignored by default:

```bash
cargo test --all-targets records_a_short_clip -- --ignored
```

### Package

```bash
cargo build --release      # builds rust_notepad + rmd
cargo install cargo-packager --locked
cargo packager --release   # installer for the host OS
```

Pushing a `v*` tag (for example `v0.1.0`) runs
`.github/workflows/release.yml`, which builds Windows, macOS, and Linux
installers on native runners and attaches them to a GitHub release.

On Linux, the build needs `libasound2-dev` for microphone capture in addition to
the usual eframe dependencies (see `.github/workflows/ci.yml`). The `.deb`
depends on `libasound2` at runtime.

### Notes

- For local development, `GEMINI_API_KEY` and `GEMINI_MODEL` can be placed in a
  gitignored `.env` file.
- `vendor/egui_commonmark` is upstream 0.14.0 with small preview rendering
  patches (task-list bullets, line height, and code-copy UX). See its
  `PATCH.md` before updating the dependency.
- `src/app/audio.rs` handles microphone capture, `src/app/templates.rs` holds
  the starter documents, and `src/rmd_cli.rs` is shared by `rmd` and
  `rust_notepad --rmd`.
