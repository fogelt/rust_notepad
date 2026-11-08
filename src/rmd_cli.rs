//! Shared implementation of the `rmd` command.
//!
//! Two binaries link this module:
//!
//! * `rmd` (a console program) — the command users type in a shell.
//! * `rust_notepad --rmd` — used by the AppImage wrapper, where the real
//!   `rmd` binary lives inside the (unmounted) AppImage.
//!
//! The command creates the target file when it does not exist, prints what it
//! is doing, and launches the GUI detached.

use std::env;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const HELP: &str = "\
rmd - open (or create) a Markdown note in Rusty Notepad

Usage:
  rmd [FILE]

With no FILE, rmd asks for a name. If the name has no extension, .md is added.
A missing file is created (empty) before Rusty Notepad opens it.

Options:
  -h, --help       Show this help
  -V, --version    Show the version";

/// Entry point used by `rust_notepad --rmd` (args must not include `--rmd`).
pub fn run_with<I: IntoIterator<Item = String>>(args: I) -> i32 {
    let mut files = Vec::new();
    let mut literal = false;
    for arg in args {
        if literal {
            files.push(arg);
            continue;
        }
        match arg.as_str() {
            "--" => literal = true,
            "-h" | "--help" => {
                println!("{HELP}");
                return 0;
            }
            "-V" | "--version" => {
                println!("rmd {}", env!("CARGO_PKG_VERSION"));
                return 0;
            }
            other if other.starts_with('-') => {
                eprintln!("rmd: unknown option: {other}\n\n{HELP}");
                return 2;
            }
            _ => files.push(arg),
        }
    }

    if files.len() > 1 {
        eprintln!("rmd: expected one file, got {}.\n\n{HELP}", files.len());
        return 2;
    }

    let requested = match files.pop() {
        Some(name) => name,
        None => match prompt_for_name() {
            Ok(name) => name,
            Err(err) => {
                eprintln!("rmd: {err}");
                return 1;
            }
        },
    };

    match open_note(&requested) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("rmd: {err}");
            1
        }
    }
}

fn prompt_for_name() -> Result<String, String> {
    print!("Name for the new note: ");
    io::stdout()
        .flush()
        .map_err(|e| format!("could not write to the console: {e}"))?;

    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("could not read from the console: {e}"))?;

    let name = unquote(line.trim());
    if name.is_empty() {
        return Err("no filename given.".to_string());
    }
    Ok(name.to_string())
}

fn unquote(s: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = s
            .strip_prefix(quote)
            .and_then(|s| s.strip_suffix(quote))
            .map(str::trim)
        {
            return inner;
        }
    }
    s
}

fn open_note(requested: &str) -> Result<(), String> {
    let path = absolute(Path::new(requested))?;

    if path.is_dir() {
        return Err(format!("{} is a directory.", path.display()));
    }
    // `rmd some-folder/` should not create `some-folder/.md`.
    if requested.ends_with('/') || requested.ends_with('\\') {
        return Err(format!("{} is not a file.", path.display()));
    }

    let path = resolve_target(path);
    if path.is_dir() {
        return Err(format!("{} is a directory.", path.display()));
    }

    let created = if path.exists() {
        false
    } else {
        create_empty(&path)?
    };

    let exe = find_gui_binary().ok_or_else(|| {
        "could not find the Rusty Notepad executable. Reinstall the app, or run rmd from its install folder."
            .to_string()
    })?;

    let mut command = Command::new(&exe);
    command.arg(&path);
    if let Ok(cwd) = env::current_dir() {
        command.current_dir(cwd);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command
        .spawn()
        .map_err(|e| format!("could not launch {}: {e}", exe.display()))?;

    if created {
        println!("Created {}", path.display());
    }
    println!("Opening {} in Rusty Notepad…", path.display());
    Ok(())
}

/// Add `.md` when the user typed a bare name (`rmd notes` -> `notes.md`).
fn with_markdown_extension(path: PathBuf) -> PathBuf {
    if path.extension().is_some() || path.file_name().is_none() {
        return path;
    }
    let mut name = path.into_os_string();
    name.push(".md");
    PathBuf::from(name)
}

/// Existing files keep their name (including extensionless ones like
/// `README`); missing names get the Markdown extension.
fn resolve_target(path: PathBuf) -> PathBuf {
    if path.exists() {
        path
    } else {
        with_markdown_extension(path)
    }
}

fn absolute(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let cwd = env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;
    Ok(cwd.join(path))
}

fn create_empty(path: &Path) -> Result<bool, String> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(_) => Ok(true),
        // Lost a race with another process: the file exists now, which is fine.
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(err) => Err(format!("could not create {}: {err}", path.display())),
    }
}

/// Locate the GUI binary. Sibling first (normal installs), then the AppImage
/// runtime, then the macOS bundle locations, then `PATH`.
fn find_gui_binary() -> Option<PathBuf> {
    if let Some(appimage) = env::var_os("APPIMAGE").map(PathBuf::from)
        && appimage.is_file()
    {
        return Some(appimage);
    }

    let current = env::current_exe().ok()?;
    let current = std::fs::canonicalize(&current).unwrap_or(current);
    let dir = current.parent()?;
    let sibling = dir.join(gui_binary_name());
    if sibling.is_file() {
        return Some(sibling);
    }

    #[cfg(target_os = "macos")]
    {
        let mut candidates = vec![PathBuf::from(
            "/Applications/Rusty Notepad.app/Contents/MacOS/rust_notepad",
        )];
        if let Some(home) = env::var_os("HOME") {
            candidates.push(
                PathBuf::from(home)
                    .join("Applications/Rusty Notepad.app/Contents/MacOS/rust_notepad"),
            );
        }
        if let Some(found) = candidates.into_iter().find(|p| p.is_file()) {
            return Some(found);
        }
    }

    find_on_path(gui_binary_name())
}

fn gui_binary_name() -> &'static str {
    if cfg!(windows) {
        "rust_notepad.exe"
    } else {
        "rust_notepad"
    }
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        if Path::new(name).extension().is_none() {
            for ext in ["exe", "cmd", "bat"] {
                let candidate = dir.join(format!("{name}.{ext}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_markdown_extension_to_bare_names() {
        assert_eq!(
            with_markdown_extension(PathBuf::from("notes")),
            PathBuf::from("notes.md")
        );
    }

    #[test]
    fn keeps_existing_extensions() {
        assert_eq!(
            with_markdown_extension(PathBuf::from("notes.markdown")),
            PathBuf::from("notes.markdown")
        );
        assert_eq!(
            with_markdown_extension(PathBuf::from("notes.txt")),
            PathBuf::from("notes.txt")
        );
    }

    #[test]
    fn does_not_extend_folder_like_paths() {
        assert_eq!(
            with_markdown_extension(PathBuf::from(".")),
            PathBuf::from(".")
        );
    }

    #[test]
    fn strips_matching_quotes() {
        assert_eq!(unquote("\"my notes.md\""), "my notes.md");
        assert_eq!(unquote("'my notes.md'"), "my notes.md");
        assert_eq!(unquote("plain.md"), "plain.md");
    }

    #[test]
    fn existing_extensionless_file_is_kept() {
        let dir = std::env::temp_dir().join(format!("rmd_keep_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("README");
        std::fs::write(&file, "hello").unwrap();
        assert_eq!(resolve_target(file.clone()), file);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_bare_name_gets_markdown_extension() {
        let dir = std::env::temp_dir().join(format!("rmd_missing_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("notes");
        let expected = dir.join("notes.md");
        assert_eq!(resolve_target(file), expected);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
