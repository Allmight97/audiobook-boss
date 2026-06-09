use crate::audio::validate_encoder_settings;
use crate::errors::Result;
use crate::processing::plan::{prepare_execution_plan, resolve_preflight_plan};
use crate::processing::{JobType, ProcessCommandResult, ProcessPayload, ProcessingPreflightPlan};
use std::collections::HashMap;
use std::path::PathBuf;

mod run_dispatch;
mod run_job;
mod run_options;
mod run_validation;

pub(crate) use run_options::ProcessingRunOptions;
use run_validation::{log_encoder_summary, validate_external_processing_contract};

pub(crate) struct ProcessingRun;

pub(crate) async fn process_payload(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessCommandResult> {
    process_payload_with_options(
        window,
        registry,
        workspace_root,
        payload,
        metadata,
        preview_seconds,
        ProcessingRunOptions::default(),
    )
    .await
}

pub(crate) async fn process_payload_with_options(
    window: tauri::Window,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    ProcessingRun::execute(
        window,
        registry,
        workspace_root,
        payload,
        metadata,
        preview_seconds,
        options,
    )
    .await
}

pub(crate) fn preflight_payload(
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessingPreflightPlan> {
    ProcessingRun::preflight(payload, metadata, preview_seconds)
}

impl ProcessingRun {
    pub(crate) async fn execute(
        window: tauri::Window,
        registry: crate::ManagedJobRegistry,
        workspace_root: PathBuf,
        payload: ProcessPayload,
        metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
        options: ProcessingRunOptions,
    ) -> Result<ProcessCommandResult> {
        validate_encoder_settings(&payload.settings)?;
        validate_external_processing_contract(&payload)?;
        log_encoder_summary(&payload);

        let execution_plan = prepare_execution_plan(&payload, metadata.as_ref(), preview_seconds)?;
        let job_type = execution_plan.plan.job_type;

        match job_type {
            JobType::Merge => {
                run_dispatch::dispatch_merge_job(
                    window,
                    registry,
                    workspace_root,
                    &payload,
                    execution_plan,
                    options,
                )
                .await
            }
            JobType::Batch => {
                run_dispatch::dispatch_batch_jobs(
                    window,
                    registry,
                    workspace_root,
                    &payload,
                    execution_plan,
                    options,
                )
                .await
            }
        }
    }

    pub(crate) fn preflight(
        payload: ProcessPayload,
        metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
        preview_seconds: Option<f64>,
    ) -> Result<ProcessingPreflightPlan> {
        validate_encoder_settings(&payload.settings)?;
        validate_external_processing_contract(&payload)?;

        resolve_preflight_plan(&payload, metadata.as_ref(), preview_seconds)
    }
}

#[cfg(test)]
mod tests {
    use super::run_job::{
        commit_supplemental_assets, register_job_and_validate_output, supplemental_assets_for_input,
    };
    use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting};
    use crate::output_artifact::OutputKind;
    use crate::processing::terminal_outcomes::{
        classify_processing_error, ProcessingJobTerminalOutcome,
    };
    use crate::processing::{JobType, ProcessPayload, SupplementalProcessingAsset};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::Auto,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: ChannelConfig::Auto,
            afterburner: true,
            threads: ThreadSetting::Auto,
            twoloop: true,
        }
    }

    fn process_payload(overrides: impl FnOnce(&mut ProcessPayload)) -> ProcessPayload {
        let mut payload = ProcessPayload {
            input_files: vec!["/books/input.m4b".to_string()],
            input_ids: None,
            output_dir: "/tmp/out".to_string(),
            settings: encoder_settings(),
            sample_rate: None,
            job_type: Some(JobType::Batch),
            output_naming: None,
            collision_policy: None,
            preflight_signature: None,
            supplemental_assets_by_input_id: None,
        };
        overrides(&mut payload);
        payload
    }

    fn supplemental_asset(
        path: std::path::PathBuf,
        input_id: &str,
        bytes: &[u8],
    ) -> SupplementalProcessingAsset {
        SupplementalProcessingAsset {
            asset_id: "asset-1".to_string(),
            input_id: input_id.to_string(),
            title_id: "B000000001".to_string(),
            path,
            file_name: "Supplemental PDF.pdf".to_string(),
            size_bytes: bytes.len() as u64,
            sha256: abb_media_core::sha256_hex(bytes),
        }
    }

    #[tokio::test]
    async fn register_job_and_validate_output_cleans_up_failed_validation() {
        let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(2));
        let temp_dir = TempDir::new().expect("create temp dir");
        let invalid_output = temp_dir.path().join("output.mp3");

        let error = match register_job_and_validate_output(&registry, &invalid_output, None).await {
            Ok(_) => panic!("invalid extension should fail validation"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("Output must be .m4b file, got: .mp3"),
            "unexpected error: {error}"
        );

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.active_jobs, 0, "active jobs should be cleared");
        assert_eq!(status.total_jobs, 0, "tracked jobs should be cleared");
        assert!(
            registry.list_active_jobs().await.is_empty(),
            "active job list should be empty after failed validation"
        );
        assert_eq!(
            registry
                .update_max_concurrent(1)
                .await
                .expect("idle registry should allow concurrency updates"),
            1
        );
    }

    #[test]
    fn supplemental_assets_for_input_selects_by_file_list_input_id() {
        let asset = SupplementalProcessingAsset {
            asset_id: "asset-1".to_string(),
            input_id: "current-input-2".to_string(),
            title_id: "B000000001".to_string(),
            path: "/staged/book.pdf".into(),
            file_name: "Supplemental PDF.pdf".to_string(),
            size_bytes: 128,
            sha256: "hash".to_string(),
        };
        let mut assets = HashMap::new();
        assets.insert("current-input-2".to_string(), vec![asset.clone()]);
        let payload = process_payload(|payload| {
            payload.input_files = vec![
                "/books/first.m4b".to_string(),
                "/books/second.m4b".to_string(),
            ];
            payload.input_ids = Some(vec![
                Some("current-input-1".to_string()),
                Some("current-input-2".to_string()),
            ]);
            payload.supplemental_assets_by_input_id = Some(assets);
        });

        assert!(supplemental_assets_for_input(&payload, None).is_empty());
        assert!(supplemental_assets_for_input(&payload, Some(0)).is_empty());
        assert_eq!(
            supplemental_assets_for_input(&payload, Some(1)),
            vec![asset]
        );
    }

    #[test]
    fn supplemental_commit_failure_classifies_processing_as_terminal_failed() {
        let root = TempDir::new().expect("temp root");
        let original_bytes = b"%PDF-1.7\nbody";
        let changed_bytes = b"%PDF-1.7\nBODY";
        let source = root.path().join("source.pdf");
        std::fs::write(&source, original_bytes).expect("write source pdf");
        let asset = supplemental_asset(source.clone(), "current-input-1", original_bytes);
        std::fs::write(&source, changed_bytes).expect("change source pdf");
        let final_audio = root.path().join("Book.m4b");

        let error = commit_supplemental_assets(OutputKind::Final, &[asset], &final_audio)
            .expect_err("stale output-artifact supplemental commit should fail");
        let outcome = classify_processing_error(error);

        let ProcessingJobTerminalOutcome::Failed(envelope) = outcome else {
            panic!("Supplemental PDF commit failure should produce failed terminal outcome");
        };
        assert!(
            envelope
                .message
                .contains("Audiobook output 'Book.m4b' was created"),
            "unexpected envelope: {envelope:?}"
        );
        assert!(
            envelope
                .message
                .contains("requested Supplemental PDFs could not be committed"),
            "unexpected envelope: {envelope:?}"
        );
        assert!(
            envelope.message.contains("hash changed"),
            "unexpected envelope: {envelope:?}"
        );
    }
}
