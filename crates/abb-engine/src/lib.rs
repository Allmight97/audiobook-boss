//! ABB's engine: every rule and workflow that decides what happens to a
//! user's audiobooks, with no dependency on a UI toolkit.
//!
//! A host constructs one [`Engine`] and drives it through that type's
//! methods. The engine publishes [`EngineEvent`]s through the host's
//! [`EventSink`].

//! Hosts can read typed remote outcomes without reaching private workflow code.
//!
//! ```
//! use abb_engine::remote_source::{RemoteUiSnapshot, ReleaseGrabStatus};
//! fn sent_releases(snapshot: &RemoteUiSnapshot) -> usize {
//!     snapshot.indexer.release_grabs.values()
//!         .filter(|row| row.status == ReleaseGrabStatus::Sent).count()
//! }
//! ```

//! Hosts cannot bypass accepted remote lifecycle work or write tags/start
//! processing outside the session's safety rules.
//!
//! ```compile_fail
//! fn disconnect(engine: &abb_engine::Engine) {
//!     let _ = engine.remote_source().logout(abb_engine::remote_source::ProviderId::Audible);
//! }
//! ```
//!
//! ```compile_fail
//! use abb_engine::save_metadata_intent;
//! ```
//! ```compile_fail
//! use abb_engine::audio::execute_audio_engine;
//! ```
//! ```compile_fail
//! use abb_engine::processing::ProcessingContext;
//! ```
//! ```compile_fail
//! use abb_engine::work_runtime::WorkRuntime;
//! ```

#![deny(clippy::unwrap_used)]
#![warn(clippy::too_many_lines)]

pub mod app_settings;
pub mod audio;
mod cover_source;
mod diagnostics;
mod engine;
mod errors;
mod file_replace;
mod host;
mod metadata;
pub mod metadata_lookup;
mod metadata_save;
mod opened_audio;
pub mod output_artifact;
mod owned_dir;
mod power;
pub mod processing;
pub mod remote_source;
pub mod session;
pub mod work_runtime;

pub use diagnostics::ffmpeg_build_identity;
pub use engine::{Engine, EngineConfig, RunningWork};
pub use errors::{
    sanitize_path_for_display, sanitize_path_str_for_display, AppError, AppErrorCategory,
    AppErrorCode, AppErrorEnvelope, Result,
};
pub use host::{DiscardEvents, EngineEvent, EventSink};
pub use metadata::{
    AlbumSortPatchOp, AudiobookMetadata, ChapterPlan, CoverArtPassthroughPolicy,
    MetadataIntentPatch, MetadataIntentValidationResult, NamingMetadata, PassthroughSource,
    PatchOp,
};

/// Shared handle to the job scheduler.
pub(crate) type ManagedJobRegistry = std::sync::Arc<processing::JobRegistry>;

// Real-file proofs are crate-local so tests cannot enlarge the host interface.
#[cfg(test)]
extern crate self as abb_engine;
#[cfg(test)]
mod test_cases;
#[cfg(test)]
pub(crate) use metadata::{
    extract_passthrough_metadata, finalize_artifact_metadata, read_audio_cover_thumbnail,
    read_metadata, save_metadata_intent,
};
