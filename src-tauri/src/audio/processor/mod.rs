//! Audio processor module (split refactor - Phase 1 state).
//!
//! Current staged modules:
//!   - prepare.rs   : validation, workspace setup, sample rate detection
//!   - execute.rs   : merge / ffmpeg execution logic
//!   - finalize.rs  : metadata writing, move, cleanup, orchestrator (temporary placement)
//!   - legacy.rs    : deprecated adapters (to be isolated fully in later phase; TODO P2.1.1 gating/removal)
//!
//! Plan Reference: docs/planning/processor_split_plan.md (Task P1.1.1, Phase 1)
//!
//! Status:
//!   Phase 1 complete: preparation + finalize/orchestrator logic migrated. Execution
//!   stage & legacy adapters have initial extraction; further consolidation / legacy
//!   isolation continues in later phases.
//!
//! Public Surface (current):
//!   - processor::detect_input_sample_rate
//!   - processor::process_audiobook_with_context
//!   (Deprecated adapters will be re-exported once moved into legacy.rs in Phase 4.)
//!
//! Note: Monolithic file removal / full legacy isolation continues in subsequent phases.

#![allow(dead_code)] // Transitional allowance until all phases complete (will tighten later).

// Submodules
pub mod execute;
pub mod finalize;
pub mod legacy;
pub mod prepare;

// Re-exports (current public / crate API)
// Underlying items are currently pub(crate); visibility can be expanded if needed
pub use finalize::process_audiobook_with_context;
#[allow(deprecated)]
pub use legacy::process_audiobook_with_events;
pub use prepare::detect_input_sample_rate;
// TODO (Phase 5): Re-export deprecated adapters from legacy.rs maintaining original signatures.

/// Internal workflow state passed between staged phases of processing.
///
/// This replaces ad-hoc tuples and keeps intermediate artifacts cohesive.
/// Fields are intentionally minimal; additional items should only be added if
/// required across stage boundaries to avoid hidden coupling.
pub(crate) struct ProcessingWorkflow {
    /// Session-scoped temporary working directory
    pub(crate) temp_dir: std::path::PathBuf,
    /// FFmpeg concat list file path
    pub(crate) concat_file: std::path::PathBuf,
    /// Total duration (seconds) of all valid input files (pre‑computed)
    pub(crate) total_duration: f64,
}

impl ProcessingWorkflow {
    /// Constructor helper to keep instantiation explicit at call sites.
    pub(crate) fn new(
        temp_dir: std::path::PathBuf,
        concat_file: std::path::PathBuf,
        total_duration: f64,
    ) -> Self {
        Self {
            temp_dir,
            concat_file,
            total_duration,
        }
    }

    /// Accessor helpers (kept small / inline for clarity).
    pub(crate) fn temp_dir(&self) -> &std::path::PathBuf {
        &self.temp_dir
    }
    pub(crate) fn concat_file(&self) -> &std::path::PathBuf {
        &self.concat_file
    }
    pub(crate) fn total_duration(&self) -> f64 {
        self.total_duration
    }
}

// NOTE (Phase 1):
// - Preparation + validation + workflow construction migrated (prepare.rs)
// - Orchestrator + finalize logic migrated (finalize.rs)
// - Execution layer extracted (execute.rs) with feature-gated processor selection
// - Legacy / deprecated adapters scheduled for isolation (legacy.rs future phase)
// Next Steps:
//   Phase 2+: Refine execution module & ensure function size limits remain enforced
//   Phase 4: Move deprecated adapters into legacy.rs with TODO gating
//   Phase 5: Centralize re-exports here (including deprecated) and finalize public surface
