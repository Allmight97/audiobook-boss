//! ```
//! use abb_engine::remote_source::{RemoteUiSnapshot, ReleaseGrabStatus};
//! fn sent_releases(snapshot: &RemoteUiSnapshot) -> usize {
//!     snapshot.indexer.release_grabs.values()
//!         .filter(|row| row.status == ReleaseGrabStatus::Sent).count()
//! }
//! ```

//! ```compile_fail
//! use abb_engine::remote_source::RemoteSourceRuntime;
//! ```
//! ```compile_fail
//! use abb_engine::metadata::save_metadata_intent;
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

pub mod app_settings;
pub mod audio {
    pub use abb_media_pipeline::audio::{
        supported_audio_import_extensions, supported_audio_import_support_text, AacDecoder,
        AudioIntent, AudiobookFormat, ALLOWED_IMAGE_EXTENSIONS,
    };
}
mod cover_service;
mod cover_source;
mod engine;
mod engine_tasks;
mod host;
pub mod metadata_lookup;
mod metadata_save;
pub mod output_artifact {
    pub use abb_media_pipeline::output_artifact::{CollisionPolicy, NamingPreset};
}
pub mod processing {
    pub use abb_media_pipeline::processing::RunTerminalClass;
}
mod quit_press;
pub mod remote_source;
pub mod session;
pub mod work_runtime;

pub use abb_media_pipeline::diagnostics::media_build_identity;
pub use abb_media_pipeline::errors::{
    AppError, AppErrorCategory, AppErrorCode, AppErrorEnvelope, Result,
};
pub use engine::{Engine, EngineConfig, QuitConsent, RunningWork, ShutdownOutcome};
pub use host::{DiscardEvents, EngineEvent, EventSink};
pub use quit_press::QuitPress;

pub(crate) use abb_media_pipeline::ManagedJobRegistry;

#[cfg(test)]
mod test_cases;
