//! The engine's host-facing interface and lifetime.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::app_settings::{
    self, AppSettings, AppSettingsPatch, AppSettingsRecoveryPlan, AppSettingsRecoveryResult,
};
use crate::audio::{
    self, AudiobookFormat, EncoderSettingsCapabilities, FileListInfo, SupportedAudioImportMetadata,
    TitleAudioPlan, TitleAudioRequest,
};
use crate::errors::{AppError, Result};
use crate::host::{EventSink, Host};
use crate::metadata::{
    AudiobookMetadata, ChapterPlan, MetadataIntentPatch, MetadataIntentValidationResult,
    NamingMetadata,
};
use crate::metadata_lookup::{MetadataLookupResponse, MetadataSource};
use crate::metadata_save::{MetadataSaveBatchResult, MetadataSaveRequest};
use crate::opened_audio::OpenedAudioFileQueue;
use crate::output_artifact::{
    build_output_path_preview, derive_output_artifact_path, OutputKind, OutputNamingConfig,
};
use crate::power::PowerManager;
use crate::processing::{
    run, JobRegistry, MaxConcurrentJobsCapabilities, ProcessCommandResult, ProcessPayload,
    ProcessingPreflightPlan,
};
use crate::remote_source::{RemoteSourceConfig, RemoteSourceRuntime};
use crate::work_runtime::{
    OperationId, OperationListSnapshot, OperationSnapshot, SubmitProcessingOperationRequest,
    WorkRuntime, WorkSubmissionAccepted,
};
use crate::ManagedJobRegistry;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSettingsCapabilities {
    pub encoder: EncoderSettingsCapabilities,
    pub max_concurrent_jobs: MaxConcurrentJobsCapabilities,
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
    config_dir: PathBuf,
    workspace_root: PathBuf,
    host: Host,
    power: PowerManager,
    jobs: ManagedJobRegistry,
    work: WorkRuntime,
    remote_source: RemoteSourceRuntime,
    opened_audio: OpenedAudioFileQueue,
}

impl Engine {
    /// Starts the engine: clears working files abandoned by a previous run,
    /// then applies the saved settings that have a runtime side.
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

        let jobs: ManagedJobRegistry = Arc::new(JobRegistry::auto());
        log::info!(
            "Job registry initialized: max_concurrent = {}",
            jobs.max_concurrent()
        );
        let engine = Self {
            inner: Arc::new(EngineInner {
                workspace_root: audio::processing_workspace_root(&config.cache_dir),
                config_dir: config.config_dir,
                host: Host::new(config.events, power.clone()),
                power,
                jobs,
                work: WorkRuntime::default(),
                remote_source,
                opened_audio: OpenedAudioFileQueue::default(),
            }),
        };
        match engine.app_settings() {
            Ok(settings) => engine.apply_settings_to_runtime(&settings),
            Err(error) => {
                log::warn!("Startup app settings hydration failed; using runtime defaults: {error}")
            }
        }
        Ok(engine)
    }

    /// Applies the runtime side of accepted settings. Every entry point calls
    /// this only after storage succeeded: start, update, reset, and recovery.
    fn apply_settings_to_runtime(&self, settings: &AppSettings) {
        self.inner
            .power
            .set_enabled(settings.keep_awake_while_working);
    }

    // ---- Settings ----

    pub fn app_settings(&self) -> Result<AppSettings> {
        app_settings::get_app_settings(&self.inner.config_dir)
    }

    pub fn app_settings_recovery(&self) -> Result<Option<AppSettingsRecoveryPlan>> {
        app_settings::get_app_settings_recovery(&self.inner.config_dir)
    }

    pub fn recover_app_settings(
        &self,
        expected: AppSettingsRecoveryPlan,
    ) -> Result<AppSettingsRecoveryResult> {
        let result = app_settings::recover_app_settings(&self.inner.config_dir, expected)?;
        self.apply_settings_to_runtime(&result.settings);
        Ok(result)
    }

    pub fn update_app_settings(&self, patch: AppSettingsPatch) -> Result<AppSettings> {
        let settings = app_settings::update_app_settings(&self.inner.config_dir, patch)?;
        self.apply_settings_to_runtime(&settings);
        Ok(settings)
    }

    /// Resets durable settings and returns concurrency to automatic. Refused
    /// while exports run; a failed reset restores the previous concurrency.
    pub async fn reset_app_settings(&self) -> Result<AppSettings> {
        let settings =
            reset_settings_and_concurrency(&self.inner.config_dir, &self.inner.jobs).await?;
        self.apply_settings_to_runtime(&settings);
        Ok(settings)
    }

    pub async fn runtime_settings_capabilities(&self) -> Result<RuntimeSettingsCapabilities> {
        tokio::task::spawn_blocking(|| RuntimeSettingsCapabilities {
            encoder: audio::encoder_settings_capabilities(),
            max_concurrent_jobs: JobRegistry::max_concurrent_jobs_capabilities(),
        })
        .await
        .map_err(|error| AppError::General(error.to_string()))
    }

    pub fn max_concurrent_jobs(&self) -> usize {
        self.inner.jobs.max_concurrent()
    }

    /// Changes the concurrency limit; `None` selects the automatic default.
    /// Requires that no job is running.
    pub async fn set_max_concurrent_jobs(&self, max_concurrent: Option<usize>) -> Result<usize> {
        let desired = max_concurrent.unwrap_or(JobRegistry::default_max());
        self.inner.jobs.update_max_concurrent(desired).await
    }

    // ---- Import ----

    /// Validates and analyzes local audio files.
    pub fn analyze_audio_files(&self, file_paths: Vec<String>) -> Result<FileListInfo> {
        let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
        audio::get_file_list_info(&paths)
    }

    pub fn supported_audio_import_metadata(&self) -> SupportedAudioImportMetadata {
        audio::supported_audio_import_metadata()
    }

    /// Recursively discovers supported local audio files from files and directories.
    pub async fn discover_audio_import_paths(
        &self,
        input_paths: Vec<String>,
    ) -> Result<Vec<String>> {
        let paths = input_paths
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let discovered =
            tokio::task::spawn_blocking(move || audio::discover_audio_import_paths(&paths))
                .await
                .map_err(|error| {
                    AppError::General(format!("Audio import discovery failed: {error}"))
                })??;
        Ok(discovered
            .into_iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect())
    }

    /// Queues files the operating system asked ABB to open. Unsupported paths
    /// are dropped. Returns whether anything was queued.
    pub fn queue_opened_audio_files(&self, paths: Vec<PathBuf>) -> Result<bool> {
        let supported = crate::opened_audio::supported_opened_audio_paths(paths);
        if supported.is_empty() {
            return Ok(false);
        }
        self.inner.opened_audio.push_paths(supported)?;
        Ok(true)
    }

    /// Drains the files queued by [`Engine::queue_opened_audio_files`].
    pub fn take_opened_audio_files(&self) -> Result<Vec<String>> {
        self.inner.opened_audio.take_paths()
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

    /// Validates and normalizes metadata intent without writing files.
    pub fn validate_metadata_intent_patch(
        &self,
        metadata_patch: &MetadataIntentPatch,
    ) -> MetadataIntentValidationResult {
        crate::metadata::validate_metadata_intent_patch(metadata_patch)
    }

    /// The album sort (TSOA) processing would write for `metadata`.
    pub fn preview_album_sort(&self, metadata: &AudiobookMetadata) -> Option<String> {
        crate::metadata::processing_album_sort(metadata)
    }

    /// Loads a cover image from disk as write-ready JPEG bytes.
    pub async fn load_cover_art_file(&self, file_path: String) -> Result<Vec<u8>> {
        crate::cover_source::load_cover_art_file(file_path).await
    }

    /// Loads a cover image from an HTTPS URL as write-ready JPEG bytes.
    pub async fn load_cover_art_from_url(&self, url: String) -> Result<Vec<u8>> {
        crate::cover_source::load_cover_art_from_url(url).await
    }

    /// Writes each file's metadata intent as one accepted operation and
    /// returns the per-file outcomes.
    pub async fn save_metadata_batch(
        &self,
        items: Vec<MetadataSaveRequest>,
    ) -> Result<MetadataSaveBatchResult> {
        crate::metadata_save::save_metadata_batch(
            &self.inner.host,
            &self.inner.work,
            &self.inner.jobs,
            items,
        )
        .await
    }

    pub async fn search_online_metadata(
        &self,
        query: String,
        sources: Option<Vec<MetadataSource>>,
        limit: Option<u8>,
    ) -> Result<MetadataLookupResponse> {
        crate::metadata_lookup::search_online_metadata(query, sources, limit).await
    }

    // ---- Output and processing ----

    /// Builds an output path preview using naming rules, without collision suffixing.
    pub fn preview_output_path(
        &self,
        output_dir: String,
        metadata: Option<AudiobookMetadata>,
        output_naming: Option<OutputNamingConfig>,
        source_path: Option<String>,
        output_kind: Option<OutputKind>,
        format: AudiobookFormat,
    ) -> Result<String> {
        let base_output_dir = PathBuf::from(output_dir);
        let source_path_buf = source_path.as_deref().map(PathBuf::from);
        let naming = output_naming.unwrap_or_default();
        let draft_naming_metadata = metadata.as_ref().map(NamingMetadata::from_metadata);
        let requested = build_output_path_preview(
            &base_output_dir,
            draft_naming_metadata.as_ref(),
            naming,
            source_path_buf.as_deref(),
        )?;
        let artifact =
            derive_output_artifact_path(&requested, output_kind.unwrap_or(OutputKind::Final))?;
        let artifact = artifact.with_extension(format.extension());
        Ok(artifact.to_string_lossy().to_string())
    }

    pub fn preflight_processing_plan(
        &self,
        payload: ProcessPayload,
        metadata: Option<HashMap<String, MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
    ) -> Result<ProcessingPreflightPlan> {
        run::preflight_payload(payload, metadata, preview_seconds)
    }

    /// Resolves how one title's audio would be handled.
    pub async fn preview_title_audio(
        &self,
        file_paths: Vec<String>,
        request: TitleAudioRequest,
        chapter_plans: Option<HashMap<String, ChapterPlan>>,
    ) -> Result<TitleAudioPlan> {
        let paths = file_paths
            .iter()
            .map(|path| audio::validate_input_audio_path(std::path::Path::new(path)))
            .collect::<Result<Vec<_>>>()?;
        tokio::task::spawn_blocking(move || {
            let mut info = audio::get_file_list_info(&paths)?;
            audio::apply_chapter_plans(&mut info, chapter_plans.as_ref())?;
            audio::resolve_title_audio(&request, &info, false)
        })
        .await
        .map_err(|error| AppError::General(format!("Audio plan failed: {error}")))?
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

fn require_preview_seconds(preview_seconds: Option<f64>) -> Result<f64> {
    preview_seconds.ok_or_else(|| {
        AppError::InvalidInput(
            "Direct processing requires a preview duration; submit final processing through WorkRuntime"
                .to_string(),
        )
    })
}

async fn reset_settings_and_concurrency(
    config_dir: &std::path::Path,
    registry: &ManagedJobRegistry,
) -> Result<AppSettings> {
    let rollback_concurrency = registry.max_concurrent();
    registry.reset_to_auto().await.map_err(|_| {
        AppError::InvalidInput(
            "Settings can't be reset while exports are running. Try again when they finish."
                .to_string(),
        )
    })?;

    let reset = app_settings::reset_app_settings(config_dir);
    if reset.is_err() {
        if let Err(rollback_error) = registry.update_max_concurrent(rollback_concurrency).await {
            log::warn!(
                "Failed to roll back max concurrency after settings reset failed: {}",
                rollback_error
            );
        }
    }
    reset
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn failed_settings_reset_rolls_back_registry_concurrency() {
        let temp = TempDir::new().expect("temp dir");
        std::fs::create_dir(temp.path().join("app-settings.json"))
            .expect("create directory where settings file should be");
        let registry = Arc::new(JobRegistry::new(2));

        let error = reset_settings_and_concurrency(temp.path(), &registry)
            .await
            .expect_err("directory settings path should fail reset");

        assert!(matches!(error, AppError::Io(_)));
        assert_eq!(registry.max_concurrent(), 2);
    }

    #[tokio::test]
    async fn settings_reset_during_an_export_explains_why_and_changes_nothing() {
        let temp = TempDir::new().expect("temp dir");
        let registry = Arc::new(JobRegistry::new(2));
        let (_job_id, _permit) = registry.register_job().await.expect("running export");

        let error = reset_settings_and_concurrency(temp.path(), &registry)
            .await
            .expect_err("reset waits for running exports");

        assert!(
            error
                .to_string()
                .contains("can't be reset while exports are running"),
            "{error}"
        );
        assert_eq!(registry.max_concurrent(), 2);
        assert!(!temp.path().join("app-settings.json").exists());
    }
}
