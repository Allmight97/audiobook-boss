//! legacy.rs
//!
//! TODO (Roadmap P2.1.1): Feature-gate or remove.
//!
//! Legacy / deprecated adapter functions for audio processor.
//!
//! Phase 4 legacy isolation as part of processor split (Task P1.1.1).
//! See: docs/planning/processor_split_plan.md
//!
//! Contents (future migration in Phase 4):
//!   - process_audiobook (deprecated)
//!   - process_audiobook_with_events (deprecated)
//!   - merge_audio_files_with_events (deprecated)
//!   - execute_with_progress_events (deprecated)
//!   - create_temp_directory (deprecated; adapter)
//!   - cleanup_temp_directory (deprecated; adapter)
//!   - create_session_from_legacy_state
//!
//! Rationale:
//!   These functions are retained temporarily to maintain the public API surface
//!   and prevent breaking existing call sites. They will delegate into the new
//!   staged implementation (prepare / execute / finalize) so that no business
//!   logic diverges or drifts.
//!
//! Roadmap:
//!   TODO(P2.1.1): Introduce feature gating (e.g. `legacy-adapters`) and/or
//!   remove these functions once downstream callers are migrated.
//!
//! Guidelines:
//!   - Do not add new logic here; only delegate to non‑deprecated functions.
//!   - Keep per‑function bodies minimal (<60 LOC remains trivially satisfied).
//!   - Maintain existing `#[deprecated]` attributes when moving code.
//!
//! During Phase 0 this module intentionally contains no executable code to avoid
//! duplication with the still‑present monolithic `audio/processor.rs`.
//!
//! After migration:
//!   - This file should contain ONLY deprecated adapter layer code.
//!   - All core logic lives in: prepare.rs, execute.rs, finalize.rs.
//!
//! Clippy / lint considerations:
//!   - `allow(dead_code)` is temporary until migration completes.
//!   - `allow(deprecated)` is required because this module references
//!     deprecated functions (by design).
//!
//! Size expectation: << 400 LOC (will shrink further post-removal).
//
#![allow(dead_code)]
#![allow(deprecated)]
//
// Phase 1 NOTE:
// Bringing in a subset of legacy adapter functions earlier than the
// original Phase 4 plan to keep build green while incremental
// migration proceeds.
//
// Imports (scoped to legacy adapter responsibilities only)
use std::process::Command;
use super::process_audiobook_with_context;
use crate::audio::context::ProcessingContext;
use crate::audio::session::ProcessingSession;
use crate::audio::{AudioFile, AudioSettings};
use crate::errors::{AppError, Result};
use crate::metadata::AudiobookMetadata;

/// Creates processing session from legacy state
///
/// ADAPTER FUNCTION: Bridges old global state to new session model.
/// Maintains backward compatibility for legacy command handlers.
pub fn create_session_from_legacy_state(
    state: &tauri::State<'_, crate::ProcessingState>,
) -> Result<std::sync::Arc<ProcessingSession>> {
    use std::sync::Arc;
    // Share the same underlying Arcs so cancellation and processing flags
    // are observed consistently across command handlers and processing code.
    let mut session = ProcessingSession::new();
    session.state_mut().is_processing = state.is_processing.clone();
    session.state_mut().is_cancelled = state.is_cancelled.clone();
    session.state_mut().progress = state.progress.clone();
    Ok(Arc::new(session))
}

/// Main function to process audiobook with event emission for progress tracking
///
/// DEPRECATED ADAPTER: Maintains legacy call pattern.
/// New code should use `process_audiobook_with_context` directly.
#[deprecated = "Use process_audiobook_with_context for new code - this adapter maintains compatibility"]
pub async fn process_audiobook_with_events(
    window: tauri::Window,
    state: tauri::State<'_, crate::ProcessingState>,
    files: Vec<AudioFile>,
    settings: AudioSettings,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let session = create_session_from_legacy_state(&state)?;
    let context = ProcessingContext::new(window, session, settings);
    process_audiobook_with_context(context, files, metadata).await
}

/// Execute with progress events (deprecated adapter)
///
/// DEPRECATED ADAPTER: Maintains legacy call pattern for media pipeline execution.
/// New code should use execute_ffmpeg_with_progress_context directly.
#[deprecated = "Use execute_ffmpeg_with_progress_context for new code - this adapter maintains compatibility"]
pub async fn execute_with_progress_events(
    cmd: Command,
    window: &tauri::Window,
    state: &tauri::State<'_, crate::ProcessingState>,
    total_duration: f64,
) -> Result<()> {
    // Convert legacy parameters to context-based approach
    let session = create_session_from_legacy_state(state)?;
    let context = ProcessingContext::new(window.clone(), session, AudioSettings::default());
    // Note: We use default settings here since they're not available in the legacy adapter

    crate::audio::media_pipeline::execute_ffmpeg_with_progress_context(cmd, &context, total_duration).await
}

/// Create temporary directory (deprecated adapter)
///
/// DEPRECATED ADAPTER: Maintains legacy call pattern.
/// New code should use create_temp_directory_with_session directly.
#[deprecated = "Use create_temp_directory_with_session for new code - this adapter maintains compatibility"]
pub fn create_temp_directory() -> Result<std::path::PathBuf> {
    use uuid::Uuid;
    let session_id = Uuid::new_v4().to_string();
    super::prepare::create_temp_directory_with_session(&session_id)
}

/// Clean up temporary directory (deprecated adapter)
///
/// DEPRECATED ADAPTER: Maintains legacy call pattern.
/// New code should use cleanup_temp_directory_with_session directly.
#[deprecated = "Use cleanup_temp_directory_with_session for new code - this adapter maintains compatibility"]
pub fn cleanup_temp_directory(temp_dir: std::path::PathBuf) -> Result<()> {
    use uuid::Uuid;
    let session_id = Uuid::new_v4().to_string();
    super::finalize::cleanup_temp_directory_with_session(&session_id, temp_dir)
}

// NOTE: The following functions were listed in the plan but do not exist in the current codebase:
// - process_audiobook: Not found in current codebase
// - merge_audio_files_with_events: Not found in current codebase
// These may have been removed in earlier phases or never existed.

// Re-exports for internal (crate) visibility until final public API reconciliation (Phase 5)
// Removed unused re-export aliases (_legacy_*), no longer needed.
