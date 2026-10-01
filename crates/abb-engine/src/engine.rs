//! The engine's host-facing interface and lifetime.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::app_settings::{
    SettingsIntent, SettingsOutcome, SettingsReply, SettingsRuntime, SettingsSnapshot,
};
use crate::audio::{self, SupportedAudioImportMetadata};
use crate::errors::{AppError, Result};
use crate::host::{EventSink, Host};
use crate::metadata::{AudiobookMetadata, MetadataIntentPatch};
use crate::opened_audio::OpenedAudioFileQueue;
use crate::power::PowerManager;
use crate::processing::{run, ProcessCommandResult, ProcessPayload, ProcessingPreflightPlan};
use crate::remote_source::{RemoteSourceConfig, RemoteSourceRuntime};
use crate::session::{
    Session, SessionDeps, SessionIntent, SessionReply, SessionRun, SessionUpdate,
};
use crate::work_runtime::{
    OperationId, OperationListSnapshot, OperationSnapshot, SubmitProcessingOperationRequest,
    WorkRuntime, WorkSubmissionAccepted,
};
use crate::ManagedJobRegistry;

/// What a host supplies to start the engine.
pub struct EngineConfig {
    /// Root for engine-owned working files: processing workspaces and staged
    /// remote downloads. The engine clears abandoned contents at start.
    pub cache_dir: PathBuf,
    /// Root for durable settings.
    pub config_dir: PathBuf,
    /// Scopes stored credentials. Two identifiers never share a vault.
    pub app_identifier: String,
    /// Where the engine publishes events.
    pub events: Arc<dyn EventSink>,
    /// Location of the Audible helper binary. `None` resolves it beside the
    /// running executable.
    pub aaxclean_helper: Option<PathBuf>,
}

/// One running ABB engine. Cloning shares the same engine.
///
/// One engine owns one storage namespace (`cache_dir`, `config_dir`,
/// `app_identifier`): starting it removes working files a previous run left
/// behind, so two engines must not share those roots.
#[derive(Clone)]
pub struct Engine {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    workspace_root: PathBuf,
    host: Host,
    settings: SettingsRuntime,
    jobs: ManagedJobRegistry,
    work: WorkRuntime,
    remote_source: RemoteSourceRuntime,
    opened_audio: Arc<OpenedAudioFileQueue>,
    session: Session,
    /// Preview runs in flight.
    previews: Arc<AtomicUsize>,
}

impl Engine {
    /// Starts the engine: clears working files abandoned by a previous run,
    /// then puts the saved settings in effect.
    pub fn start(config: EngineConfig) -> Result<Self> {
        let power = PowerManager::default();
        audio::cleanup_abandoned_processing_workspaces(&config.cache_dir)?;
        let remote_source = RemoteSourceRuntime::new(RemoteSourceConfig {
            cache_dir: config.cache_dir.clone(),
            config_dir: config.config_dir.clone(),
            app_identifier: config.app_identifier,
            power: power.clone(),
            aaxclean_helper: config.aaxclean_helper,
        })?;
        remote_source.cleanup_abandoned_sessions()?;

        let (settings, jobs, startup) = SettingsRuntime::start(config.config_dir, power.clone());
        let host = Host::new(config.events, power);
        let work = WorkRuntime::default();
        let opened_audio = Arc::new(OpenedAudioFileQueue::default());
        let previews = Arc::new(AtomicUsize::new(0));
        let session = Session::new(SessionDeps {
            host: host.clone(),
            work: work.clone(),
            jobs: Arc::clone(&jobs),
            temporary_root: remote_source.staging_root(),
            opened_audio: Arc::clone(&opened_audio),
            previews: Arc::clone(&previews),
            settings: settings.clone(),
        });
        session.start_from_defaults(
            startup.as_ref(),
            Some(audio::encoder_settings_capabilities()),
        );
        Ok(Self {
            inner: Arc::new(EngineInner {
                workspace_root: audio::processing_workspace_root(&config.cache_dir),
                host,
                settings,
                jobs,
                work,
                remote_source,
                opened_audio,
                session,
                previews,
            }),
        })
    }

    // ---- Settings ----

    /// Applies one intent to the settings and returns the settings in effect.
    pub async fn settings_dispatch(&self, intent: SettingsIntent) -> SettingsReply {
        let reset = matches!(intent, SettingsIntent::Reset);
        let reply = self.inner.settings.dispatch(intent).await;
        // A reset returns the session's defaults to the reset settings; loaded
        // titles keep their own choices.
        if reset && reply.outcome == SettingsOutcome::Applied {
            if let Some(defaults) = &reply.snapshot.startup_defaults {
                self.inner.session.replace_defaults(defaults);
            }
        }
        reply
    }

    /// The settings in effect and whether they are saved.
    pub async fn settings_snapshot(&self) -> SettingsSnapshot {
        self.inner.settings.snapshot().await
    }

    // ---- Working session ----

    /// Applies one intent to the working session and returns what changed.
    /// Changes the engine makes on its own arrive as [`crate::EngineEvent::Session`].
    pub async fn session_dispatch(&self, intent: SessionIntent) -> SessionReply {
        self.inner.session.dispatch(intent).await
    }

    /// Applies an intent's immediate effect and returns the rest of its work.
    /// A host whose transport may reorder or overlap requests begins intents
    /// in the order the user made them and finishes each one afterward.
    pub fn session_begin(&self, intent: SessionIntent) -> SessionRun {
        self.inner.session.begin(intent)
    }

    /// The whole working session, for a host that is starting or resyncing.
    pub fn session_snapshot(&self) -> SessionUpdate {
        self.inner.session.snapshot()
    }

    /// The cover image the session currently shows.
    pub fn session_cover_art(&self) -> Option<Vec<u8>> {
        self.inner.session.cover_art()
    }

    /// The pending metadata edits for `file_paths`, as processing takes them.
    pub fn session_metadata_intents(
        &self,
        file_paths: &[String],
    ) -> HashMap<String, MetadataIntentPatch> {
        let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
        self.inner.session.pending_intents(&paths)
    }

    /// Source files with a Save accepted and not yet written because an
    /// export is still reading them. A host warns before quitting while this
    /// is non-empty.
    pub fn waiting_metadata_writes(&self) -> Vec<PathBuf> {
        self.inner.session.waiting_write_paths()
    }

    // ---- Import ----

    pub fn supported_audio_import_metadata(&self) -> SupportedAudioImportMetadata {
        audio::supported_audio_import_metadata()
    }

    /// Queues files the operating system asked ABB to open. Unsupported paths
    /// are dropped. Returns whether anything was queued. The session's
    /// `ImportOpened` intent imports them.
    pub fn queue_opened_audio_files(&self, paths: Vec<PathBuf>) -> Result<bool> {
        let supported = crate::opened_audio::supported_opened_audio_paths(paths);
        if supported.is_empty() {
            return Ok(false);
        }
        self.inner.opened_audio.push_paths(supported)?;
        Ok(true)
    }

    // ---- Metadata ----

    pub async fn read_audio_metadata(&self, file_path: String) -> Result<AudiobookMetadata> {
        tokio::task::spawn_blocking(move || {
            let validated_path = audio::validate_input_audio_path(&PathBuf::from(&file_path))?;
            crate::metadata::read_metadata(validated_path.to_string_lossy().as_ref())
        })
        .await
        .map_err(|e| AppError::General(format!("Metadata read task failed: {e}")))?
    }

    /// Reads an audio file's embedded cover as a bounded JPEG thumbnail.
    pub async fn read_audio_cover_thumbnail(&self, file_path: String) -> Result<Option<Vec<u8>>> {
        tokio::task::spawn_blocking(move || {
            let validated_path = audio::validate_input_audio_path(&PathBuf::from(&file_path))?;
            crate::metadata::read_audio_cover_thumbnail(&validated_path)
        })
        .await
        .map_err(|error| AppError::General(format!("Cover thumbnail read task failed: {error}")))?
    }

    /// Loads a cover image from an HTTPS URL as write-ready JPEG bytes.
    pub async fn load_cover_art_from_url(&self, url: String) -> Result<Vec<u8>> {
        crate::cover_source::load_cover_art_from_url(url).await
    }

    // ---- Output and processing ----

    pub fn preflight_processing_plan(
        &self,
        payload: ProcessPayload,
        metadata: Option<HashMap<String, MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
    ) -> Result<ProcessingPreflightPlan> {
        run::preflight_payload(payload, metadata, preview_seconds)
    }

    /// Runs a direct preview. Final processing enters through
    /// [`Engine::submit_processing_operation`] so it has operation identity,
    /// snapshots, and operation and title cancellation.
    pub async fn process_preview(
        &self,
        payload: ProcessPayload,
        metadata: Option<HashMap<String, MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
    ) -> Result<ProcessCommandResult> {
        let preview_seconds = require_preview_seconds(preview_seconds)?;
        let _previewing = PreviewInFlight::begin(&self.inner.previews);
        run::process_payload(
            self.inner.host.clone(),
            self.inner.jobs.clone(),
            self.inner.workspace_root.clone(),
            payload,
            metadata,
            Some(preview_seconds),
        )
        .await
    }

    pub async fn submit_processing_operation(
        &self,
        request: SubmitProcessingOperationRequest,
    ) -> Result<WorkSubmissionAccepted> {
        self.inner
            .work
            .submit_processing_operation(
                self.inner.host.clone(),
                self.inner.jobs.clone(),
                self.inner.workspace_root.clone(),
                request,
            )
            .await
    }

    pub fn list_work_operations(&self) -> Result<OperationListSnapshot> {
        self.inner.work.list_operations()
    }

    /// Cancels a whole operation, or one of its titles when `child_job_id` is given.
    pub fn cancel_work_operation(
        &self,
        operation_id: OperationId,
        child_job_id: Option<String>,
    ) -> Result<OperationSnapshot> {
        self.inner
            .work
            .cancel_operation(&self.inner.host, operation_id, child_job_id)
    }

    // ---- Remote sources ----

    pub fn remote_source(&self) -> &RemoteSourceRuntime {
        &self.inner.remote_source
    }
}

/// Counts a preview run for as long as it lives.
struct PreviewInFlight(Arc<AtomicUsize>);

impl PreviewInFlight {
    fn begin(previews: &Arc<AtomicUsize>) -> Self {
        previews.fetch_add(1, Ordering::SeqCst);
        Self(Arc::clone(previews))
    }
}

impl Drop for PreviewInFlight {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn require_preview_seconds(preview_seconds: Option<f64>) -> Result<f64> {
    preview_seconds.ok_or_else(|| {
        AppError::InvalidInput(
            "Direct processing requires a preview duration; submit final processing through WorkRuntime"
                .to_string(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_processing_requires_preview_duration() {
        match require_preview_seconds(None) {
            Err(AppError::InvalidInput(message)) => assert_eq!(
                message,
                "Direct processing requires a preview duration; submit final processing through WorkRuntime"
            ),
            result => panic!("expected invalid-input error, got {result:?}"),
        }
    }
}
