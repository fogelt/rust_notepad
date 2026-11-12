//! Assistant infrastructure: prompts, provider transport, and report cleanup.
//!
//! The pure, testable parts of the assistant live here. UI and app-state
//! orchestration stays in [`crate::app::components::ai_chat`].

pub mod http;
pub mod prompt;
pub mod provider;
pub mod report;
pub mod text;
