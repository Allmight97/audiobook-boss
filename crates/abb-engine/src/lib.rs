//! ABB's engine: every rule and workflow that decides what happens to a
//! user's audiobooks, with no dependency on a UI toolkit.
//!
//! A host constructs one [`Engine`] and drives it through that type's
//! methods. The engine publishes [`EngineEvent`]s through the host's
//! [`EventSink`].

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
    extract_passthrough_metadata, finalize_artifact_metadata, read_audio_cover_thumbnail,
    read_metadata, save_metadata_intent, AlbumSortPatchOp, AudiobookMetadata, ChapterPlan,
    CoverArtPassthroughPolicy, MetadataIntentPatch, MetadataIntentValidationResult, NamingMetadata,
    PassthroughSource, PatchOp,
};
pub use metadata_save::{
    MetadataSaveBatchResult, MetadataSaveRequest, MetadataSaveResultEntry,
    MetadataSaveResultStatus, MetadataSaveSummary,
};

/// Shared handle to the job scheduler.
pub(crate) type ManagedJobRegistry = std::sync::Arc<processing::JobRegistry>;
