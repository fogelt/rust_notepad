# Rusty Notepad

A lightweight Markdown text editor built in Rust on [eframe](https://github.com/emilk/egui)/egui.

## Run

```bash
cargo run
```

Open a file directly with `cargo run -- <file.md>`. A missing path opens as a
new buffer that saves there on first Save.

## `rmd` command (Windows)

`scripts/rmd.cmd` is a self-locating shim: placed next to `rust_notepad.exe`
it forwards any file argument, so `rmd notes.md` works from any cmd/PowerShell
window (paths with spaces included). For local dev wiring:

```powershell
.\scripts\setup-rmd.ps1                                        # uses target\{debug,release} build
.\scripts\setup-rmd.ps1 -ExePath 'C:\...\rust_notepad.exe'     # packaged install
.\scripts\setup-rmd.ps1 -AssociateMd                           # also double-click .md to open (current user, no admin)
.\scripts\setup-rmd.ps1 -Uninstall                             # remove shim, PATH entry, association
```

This adds the exe folder to the *user* PATH (new terminals pick it up) and,
with `-AssociateMd`, registers `.md` under `HKCU\Software\Classes`.

## Sharing (single setup.exe)

`cargo packager --release` produces `target/release/rust_notepad_0.1.0_x64-setup.exe`,
which needs nothing else. Its `[package.metadata.packager]` config wires in:

* `resources` — ships `rmd.cmd` next to the exe (auto-removed on uninstall).
* `fileAssociations` — double-click `.md`/`.markdown` opens the app (registered
  and unregistered natively, `RustyNotepad.Markdown` ProgID).
* `nsis.preinstall-section` — appends the install dir to PATH (idempotent,
  re-registers the floor per DPI context, broadcasts the env change) with a
  matching `un.onUninstSuccess` cleanup. It also explicitly unregisters the
  `.md`/`.markdown` associations there, because the packager template's own
  uninstall loop reads a nonexistent `association.ext` key and emits nothing.
  Default install mode is `currentUser`, so no admin prompt and HKCU-only writes.

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
* `scripts/rmd.cmd` — `rmd` shim shipped next to the exe by the installer
* `scripts/setup-rmd.ps1` — dev-loop wiring (PATH + shim + `.md` association, `-Uninstall` reverts)
