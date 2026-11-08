//! `rmd` — create/open a Markdown note in Rusty Notepad.
//!
//! Console binary installed next to `rust_notepad` so `rmd notes.md` works
//! from any terminal on Windows, macOS and Linux.

#[path = "../rmd_cli.rs"]
mod rmd_cli;

fn main() {
    std::process::exit(rmd_cli::run_with(std::env::args().skip(1)));
}
