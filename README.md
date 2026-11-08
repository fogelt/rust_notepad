# Rusty Notepad

A lightweight Markdown editor with a live preview and an optional Gemini
assistant, built in Rust on [eframe](https://github.com/emilk/egui)/egui.

## Features

- Split editor / live Markdown preview, with both panels independently
  toggleable. Hiding the editor gives the preview the full window, and `F11`
  enters real fullscreen (`Esc` leaves it).
- Starter templates in the empty preview: README, meeting notes, blog post,
  changelog, task list, and project brief.
- Formatting toolbar (headings, lists, quotes, tables, links, images, …).
- Optional Gemini assistant that can rewrite or extend the document.
- Voice instructions: record up to 60 seconds or drop an audio file
  (wav, mp3, m4a, ogg, flac, …) and Gemini transcribes it into the instruction
  box.
- `rmd` command to open or create notes from any terminal, on Windows, macOS
  and Linux.
- Autosave once a document has a file path.
- Double-click `.md` / `.markdown` files after installing.

## Install

Download the artifact for your platform from the
[releases page](https://github.com/fogelt/rust_notepad/releases):

| Platform | Artifact | Notes |
| --- | --- | --- |
| Windows | `*-setup.exe` (NSIS) | Installs the app, `rmd` on `PATH`, and `.md` associations (current user, no admin). |
| macOS | `.dmg` | Drag **Rusty Notepad** to Applications. Builds are unsigned, so right-click → Open the first time. |
| Debian/Ubuntu | `.deb` | Installs `rust_notepad` and `rmd` under `/usr/bin`. |
| Any Linux | `.AppImage` | Make it executable and run it. See `install-rmd.sh` below for the CLI helper. |

Run `cargo packager --release` yourself to build the installers for the host
OS (see [Build / package](#build--package)).

## Run

```bash
cargo run
```

Open a file directly with `cargo run -- <file.md>`. A missing path opens as a
new buffer that saves there on first Save.

## `rmd` command

`rmd` is a small console binary built next to the GUI. It accepts one file,
adds `.md` when the name has no extension, creates the file if it is missing,
prints what it did, and launches the editor:

```text
$ rmd
Name for the new note: ideas
Created /home/me/ideas.md
Opening /home/me/ideas.md in Rusty Notepad…

$ rmd todo.markdown
Opening /home/me/todo.markdown in Rusty Notepad…

$ rmd
Name for the new note:
rmd: no filename given.
```

`rmd --help` documents the options, and `rmd --version` prints the version.
Passing more than one file is rejected instead of silently ignoring the rest.

Where it is installed:

- **Windows** — the installer adds the install folder to the user `PATH`, so
  `rmd` works in new terminals.
- **Linux (.deb)** — both binaries are installed in `/usr/bin`.
- **macOS** — `rmd` lives inside the app bundle at
  `/Applications/Rusty Notepad.app/Contents/MacOS/rmd`. Link it once:

  ```bash
  sudo ln -sf "/Applications/Rusty Notepad.app/Contents/MacOS/rmd" /usr/local/bin/rmd
  ```

- **Linux (AppImage)** — run the helper script shipped with the release:

  ```bash
  sh install-rmd.sh ./Rusty-Notepad.AppImage
  ```

  It writes a `rmd` launcher to `~/.local/bin` (or `/usr/local/bin` when
  writable) that calls the AppImage in `--rmd` mode.

For a local build, add the build folder to `PATH`, or run
`scripts/install-rmd.sh` (Unix) / `scripts/setup-rmd.ps1` (Windows, also
handles `.md` associations):

```powershell
.\scripts\setup-rmd.ps1
.\scripts\setup-rmd.ps1 -AssociateMd
.\scripts\setup-rmd.ps1 -Uninstall
```

```bash
sh scripts/install-rmd.sh
sh scripts/install-rmd.sh --uninstall
```

## Configure (optional)

The Gemini assistant needs an API key ([get one here](https://aistudio.google.com/api-keys)).
For development, put it in a `.env` file (gitignored):

```env
GEMINI_API_KEY=…
GEMINI_MODEL=gemini-2.5-flash
```

At runtime the Assistant tab can also save both values to
`%APPDATA%\rust_notepad\config.json` (`~/.config/rust_notepad/config.json`
elsewhere); saved values take precedence over `.env`.

### Voice instructions

Open the Assistant tab, expand **Settings** once to add the API key and model,
then use **Record** (up to 60 seconds) or simply drop an audio file anywhere on
the window. Gemini transcribes the speech into the instruction box; review it
and press **Send** to apply the edit. On macOS the app asks for microphone
access the first time you record.

The transcription uses the selected Gemini model, so pick one that supports
audio input (e.g. a Gemini Flash model).

## Checks

```bash
cargo test          # unit tests (GUI + rmd)
cargo test --all-targets records_a_short_clip -- --ignored   # mic smoke test
cargo fmt           # formatting
cargo clippy --all-targets
```

CI runs all three on Windows, macOS and Linux (see `.github/workflows/ci.yml`).

## Build / package

```bash
cargo build --release      # builds rust_notepad + rmd
cargo install cargo-packager --locked
cargo packager --release   # installer for the host OS
```

`cargo packager` reads `[package.metadata.packager]` in `Cargo.toml` and builds
the formats supported by the host:

- Windows → NSIS `setup.exe` (`rmd` on `PATH`, file associations)
- macOS → `.dmg`
- Linux → `.deb` and `.AppImage`

Tagging a commit `v*` runs `.github/workflows/release.yml`, which builds all
installers on their native runners and attaches them to a GitHub release.
Windows executables embed `assets/icon.ico` via `build.rs`.

On Linux, building needs `libasound2-dev` for microphone capture (the CI
workflows install it). The `.deb` depends on `libasound2` at runtime.

## Layout

- `src/main.rs` — window/viewport setup, font + theme + `.env` loading
- `src/app/mod.rs` — `eframe::App` update loop, window title/size management
- `src/app/components/` — `top_bar`, `toolbar`, `editor`, `ai_chat`, `bottom_bar`
- `src/app/audio.rs` — microphone capture + WAV encoding for voice instructions
- `src/app/templates.rs` — starter documents shown in the empty preview
- `src/app/models/notepad.rs` — app state
- `src/app/theme.rs` — palette, panel metrics, shared panel helpers
- `src/app/config.rs` — assistant settings persistence
- `src/rmd_cli.rs` — `rmd` implementation shared by the CLI and `--rmd` mode
- `src/bin/rmd.rs` — the `rmd` console binary
- `scripts/install-rmd.sh` — Unix launcher setup (macOS app / AppImage / dev build)
- `scripts/setup-rmd.ps1` — Windows dev-loop PATH + association setup
- `vendor/egui_commonmark` — upstream 0.14.0 with small preview rendering patches
- `.github/workflows/` — CI and cross-platform release packaging
