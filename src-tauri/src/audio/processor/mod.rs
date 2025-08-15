//! Audio processor module (split refactor - Phase 5 complete).
//!
//! Current staged modules:
//!   - prepare.rs   : validation, workspace setup, sample rate detection
//!   - execute.rs   : merge / ffmpeg execution logic
//!   - finalize.rs  : metadata writing, move, cleanup
//!   - selection.rs : engine selection type aliases (P1.3)
//!   - legacy.rs    : deprecated adapters (to be gated/removed in P2.1.1)
//!   - mod.rs       : orchestrator and public API re-exports
//!
//! Plan Reference: docs/planning/processor_split_plan.md (Task P1.1.1, Phase 5)
//!
//! Status:
//!   Phase 5 complete: Orchestrator consolidated in mod.rs, calling staged functions:
//!   - prepare::validate_and_prepare
//!   - execute::execute_processing  
//!   - finalize::finalize_processing
//!
//! Public Surface (current):
//!   - processor::detect_input_sample_rate
//!   - processor::process_audiobook_with_context
//!   - Deprecated adapters re-exported from legacy.rs
//!
//! Note: Monolithic file removal / full legacy isolation continues in subsequent phases.



// Imports for orchestrator function
use std::time::Duration;
use crate::audio::context::ProcessingContext;
use crate::audio::metrics::ProcessingMetrics;
use crate::audio::{AudioFile, ProcessingStage, ProgressReporter};
use crate::errors::Result;
use crate::metadata::AudiobookMetadata;

// Submodules
pub mod execute;
pub mod finalize;
#[cfg(feature = "legacy-adapters")]
pub mod legacy;
pub mod prepare;
pub mod selection;

// Re-exports (current public / crate API)
// Underlying items are currently pub(crate); visibility can be expanded if needed
// Note: process_audiobook_with_context now implemented in this module (Phase 5)
#[cfg(feature = "legacy-adapters")]
#[allow(deprecated)]
pub use legacy::process_audiobook_with_events;
pub use prepare::detect_input_sample_rate;

// Legacy function re-exports (Phase 4 additions)
#[cfg(feature = "legacy-adapters")]
#[allow(deprecated)]
pub use legacy::execute_with_progress_events;
#[cfg(feature = "legacy-adapters")]
#[allow(deprecated)]
pub use legacy::create_temp_directory;
#[cfg(feature = "legacy-adapters")]
#[allow(deprecated)]
pub use legacy::cleanup_temp_directory;
#[cfg(feature = "legacy-adapters")]
pub use legacy::create_session_from_legacy_state;
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
    #[allow(dead_code)] // Internal accessor kept for near-term staged use
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

/// Orchestrator: Main processing entrypoint (Phase 5: moved from finalize.rs)
/// 
/// Coordinates the three-stage processing pipeline:
/// 1. Validate & Prepare
/// 2. Execute Processing  
/// 3. Finalize Processing
pub async fn process_audiobook_with_context(
    context: ProcessingContext,
    files: Vec<AudioFile>,
    metadata: Option<AudiobookMetadata>,
) -> Result<String> {
    let mut reporter = ProgressReporter::new(files.len());
    let mut metrics = ProcessingMetrics::new();

    // Stage 1: Validate + Prepare (from prepare module)
    reporter.set_stage(ProcessingStage::Analyzing);
    let workflow = prepare::validate_and_prepare(&context, &files)?;

    // Metrics accumulation (estimates)
    for file in &files {
        if file.is_valid {
            if let Some(duration) = file.duration {
                let estimated_bytes = (duration * context.settings.bitrate as f64 * 125.0) as usize;
                metrics.update_file_processed(Duration::from_secs_f64(duration), estimated_bytes);
            }
        }
    }

    // Stage 2: Execute (execute module)
    let merged_output =
        execute::execute_processing(&context, &workflow, &files, &mut reporter).await?;

    // Stage 3: Finalize
    let result =
        finalize::finalize_processing(&context, workflow, merged_output, metadata, &mut reporter).await?;

    log::info!("{}", metrics.format_summary());
    Ok(result)
}

// NOTE (Phase 5 Complete):
// - Preparation + validation + workflow construction migrated (prepare.rs)
// - Execution layer extracted (execute.rs) with feature-gated processor selection
// - Finalization logic migrated (finalize.rs) 
// - Legacy / deprecated adapters isolated (legacy.rs) - TODO P2.1.1 for gating/removal
// - Orchestrator consolidated in mod.rs calling staged functions
// Next Steps:
//   Phase 6+: Compile & lint validation, test verification
// Next Steps:
//   Phase 2+: Refine execution module & ensure function size limits remain enforced
//   Phase 4: Move deprecated adapters into legacy.rs with TODO gating
//   Phase 5: Centralize re-exports here (including deprecated) and finalize public surface
