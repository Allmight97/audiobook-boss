use crate::errors::{AppError, AppErrorCategory, AppErrorEnvelope, Result};
use crate::host::Host;
use crate::processing::plan::{
    prepare_execution_plan, resolve_preflight_plan, resolve_processing_plan, title_file_info,
};
use crate::processing::title_output::{TitleOutput, TitleOutputPlan};
use crate::processing::{ProcessCommandResult, ProcessPayload, ProcessingPreflightPlan};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

mod run_dispatch;
mod run_job;
mod run_options;
mod run_validation;

pub(crate) use run_options::ProcessingRunOptions;
use run_validation::inspect_and_validate_external_processing_contract;

pub(crate) async fn process_payload_with_options(
    host: Host,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    let result = dispatch_payload(
        host,
        registry,
        workspace_root,
        payload,
        metadata,
        preview_seconds,
        options,
    )
    .await;
    if let Some(record) = result
        .as_ref()
        .err()
        .and_then(processing_request_rejected_record)
    {
        log::error!("{record}");
    }
    result
}

/// Stable dev-log diagnostic for processing requests that terminate as an
/// error before any job lifecycle exists (validation, planning, registration).
/// Counted by `scripts/dev-log-analysis.ts` via its generic ERROR counter;
/// cancellations are excluded so a user cancel cannot degrade a session.
fn processing_request_rejected_record(error: &AppError) -> Option<String> {
    let envelope = AppErrorEnvelope::from(error);
    if envelope.category == AppErrorCategory::Cancellation {
        return None;
    }
    Some(format!(
        "processing_request event=rejected code={} category={}",
        envelope.code.log_label(),
        envelope.category.log_label(),
    ))
}

async fn dispatch_payload(
    host: Host,
    registry: crate::ManagedJobRegistry,
    workspace_root: PathBuf,
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
    options: ProcessingRunOptions,
) -> Result<ProcessCommandResult> {
    let file_info = inspect_and_validate_external_processing_contract(&payload)?;

    let execution_plan =
        prepare_execution_plan(&payload, metadata.as_ref(), preview_seconds, file_info)?;
    run_dispatch::dispatch_title_jobs(
        host,
        registry,
        workspace_root,
        &payload,
        execution_plan,
        options,
    )
    .await
}

/// The plan an export of `payload` would follow, read from its sources.
pub fn preflight_payload(
    payload: ProcessPayload,
    metadata: Option<HashMap<String, crate::metadata::MetadataIntentPatch>>,
    preview_seconds: Option<f64>,
) -> Result<ProcessingPreflightPlan> {
    let file_info = inspect_and_validate_external_processing_contract(&payload)?;

    resolve_preflight_plan(&payload, metadata.as_ref(), preview_seconds, &file_info)
}

/// Preflights an export, then gives each title an output record from what
/// the preflight read.
pub(crate) fn preflight_title_outputs(
    payload: &ProcessPayload,
    metadata: Option<&HashMap<String, crate::metadata::MetadataIntentPatch>>,
) -> Result<Vec<Arc<TitleOutput>>> {
    let file_info = inspect_and_validate_external_processing_contract(payload)?;
    let plan = resolve_processing_plan(payload, metadata, None, &file_info)?;
    plan.jobs
        .into_iter()
        .map(|job| {
            let sources = title_file_info(&file_info, &job.source_paths)?.files;
            TitleOutput::new(TitleOutputPlan {
                anchor: job.input_path,
                sources: crate::audio::passthrough_sources_from_audio_files(&sources),
                base: job.source_metadata,
                accepted: metadata
                    .and_then(|map| map.get(&payload.input_files[job.input_index]))
                    .cloned(),
                output_dir: PathBuf::from(&payload.output_dir),
                naming: payload.output_naming.clone().unwrap_or_default(),
                extension: job.audio_plan.format.extension().to_string(),
                requested: job.output.requested_path,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::run_job::{
        commit_supplemental_assets, register_job_and_validate_output,
        supplemental_assets_for_input, title_outcome, ProcessingJobLogContext,
    };
    use crate::audio::{BitrateMode, ChannelConfig, EncoderSettings, EncoderType};
    use crate::output_artifact::OutputKind;
    use crate::processing::terminal_outcomes::ProcessingJobTerminalOutcome;
    use crate::processing::OperationKind;
    use crate::processing::{ProcessPayload, SupplementalProcessingAsset};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn encoder_settings() -> EncoderSettings {
        EncoderSettings {
            encoder_type: EncoderType::Auto,
            bitrate_kbps: 64,
            bitrate_mode: BitrateMode::Vbr(3),
            channels: ChannelConfig::Auto,
            native_aac_speed: 0,
            faac_profile: crate::audio::FaacProfile::Auto,
        }
    }

    fn process_payload(overrides: impl FnOnce(&mut ProcessPayload)) -> ProcessPayload {
        let mut payload = ProcessPayload {
            title_sources: None,
            chapter_plans: None,
            input_files: vec!["/books/input.m4b".to_string()],
            input_ids: None,
            output_dir: "/tmp/out".to_string(),
            audio_requests: vec![crate::audio::TitleAudioRequest {
                format: crate::audio::AudiobookFormat::M4b,
                intent: crate::audio::AudioIntent::Encode,
                settings: Some(encoder_settings()),
                sample_rate: crate::audio::SampleRateConfig::Auto,
            }],
            output_naming: None,
            collision_policy: None,
            preflight_signature: None,
            supplemental_assets_by_input_id: None,
        };
        overrides(&mut payload);
        payload
            .audio_requests
            .resize(payload.input_files.len(), payload.audio_requests[0].clone());
        payload
    }

    #[test]
    fn preflight_rejects_symlink_before_metadata_projection() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source = temp_dir.path().join("source.m4b");
        std::fs::write(&source, b"path validation fixture").expect("write source");
        let symlink = temp_dir.path().join("source-link.m4b");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&source, &symlink).expect("create symlink");
        #[cfg(not(unix))]
        std::os::windows::fs::symlink_file(&source, &symlink).expect("create symlink");

        let payload = process_payload(|payload| {
            payload.input_files = vec![symlink.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        });

        let err = super::preflight_payload(payload, None, None)
            .expect_err("symlink should be rejected before metadata projection");

        assert!(
            err.to_string().contains("Symlinks are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn execution_inspection_rejects_a_source_replaced_after_preflight() {
        let temp_dir = TempDir::new().expect("temp dir");
        let source = temp_dir.path().join("source.wav");
        write_silence_wav(&source, 1);
        let payload = process_payload(|payload| {
            payload.input_files = vec![source.to_string_lossy().to_string()];
            payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        });

        super::preflight_payload(payload.clone(), None, None)
            .expect("valid source should be accepted during preflight");

        let original = temp_dir.path().join("source-original.wav");
        std::fs::rename(&source, &original).expect("move accepted source");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&original, &source).expect("replace source with symlink");
        #[cfg(not(unix))]
        std::os::windows::fs::symlink_file(&original, &source)
            .expect("replace source with symlink");

        let err =
            super::run_validation::inspect_and_validate_external_processing_contract(&payload)
                .expect_err("execution must inspect the replaced source");
        assert!(
            err.to_string().contains("Symlinks are not supported"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn native_target_ceiling_is_checked_before_output_planning() {
        use crate::audio::SampleRateConfig;
        let temp = TempDir::new().expect("temp dir");
        let source = temp.path().join("source.wav");
        write_silence_wav(&source, 1);
        let output = temp.path();
        for (bitrate, channels, rate, accepted) in [
            (600, ChannelConfig::Stereo, SampleRateConfig::Auto, false),
            (529, ChannelConfig::Stereo, SampleRateConfig::Auto, true),
            (300, ChannelConfig::Auto, SampleRateConfig::Auto, false),
            (
                600,
                ChannelConfig::Stereo,
                SampleRateConfig::Explicit(96_000),
                true,
            ),
        ] {
            let payload = process_payload(|payload| {
                payload.input_files = vec![source.to_string_lossy().into_owned()];
                payload.output_dir = output.to_string_lossy().into_owned();
                payload.audio_requests[0]
                    .settings
                    .as_mut()
                    .expect("encode fixture settings")
                    .encoder_type = EncoderType::NativeAac;
                payload.audio_requests[0]
                    .settings
                    .as_mut()
                    .expect("encode fixture settings")
                    .bitrate_mode = BitrateMode::Cbr;
                payload.audio_requests[0]
                    .settings
                    .as_mut()
                    .expect("encode fixture settings")
                    .bitrate_kbps = bitrate;
                payload.audio_requests[0]
                    .settings
                    .as_mut()
                    .expect("encode fixture settings")
                    .channels = channels;
                payload.audio_requests[0].sample_rate = rate;
            });
            let result = super::preflight_payload(payload, None, None);
            if accepted {
                result.expect("target within resolved ceiling should pass");
            } else {
                assert!(result
                    .expect_err("over-ceiling target must fail preflight")
                    .to_string()
                    .contains("Native target bitrate exceeds"));
            }
            assert_eq!(
                std::fs::read_dir(output)
                    .expect("read output directory")
                    .count(),
                1,
                "preflight must not create output directories"
            );
        }
    }

    #[test]
    fn native_target_ceiling_uses_each_separate_title_but_combined_grouped_channels() {
        use crate::processing::types::TitleSource;
        let temp = TempDir::new().expect("temp dir");
        let mono = temp.path().join("mono.wav");
        let stereo = temp.path().join("stereo.wav");
        write_silence_wav(&mono, 1);
        write_silence_wav(&stereo, 2);
        let stereo = stereo.to_string_lossy().into_owned();
        let mono = mono.to_string_lossy().into_owned();
        for grouped in [false, true] {
            let payload = process_payload(|payload| {
                payload.input_files = if grouped {
                    payload.title_sources = Some(HashMap::from([(
                        stereo.clone(),
                        [&stereo, &mono]
                            .map(|path| TitleSource {
                                path: path.clone(),
                                input_id: None,
                            })
                            .to_vec(),
                    )]));
                    vec![stereo.clone()]
                } else {
                    vec![stereo.clone(), mono.clone()]
                };
                payload.output_dir = temp.path().to_string_lossy().into_owned();
                let settings = payload.audio_requests[0]
                    .settings
                    .as_mut()
                    .expect("encode fixture settings");
                settings.encoder_type = EncoderType::NativeAac;
                settings.bitrate_mode = BitrateMode::Cbr;
                settings.bitrate_kbps = 300;
            });
            let result = super::preflight_payload(payload, None, None);
            if grouped {
                result.expect("combined stereo output permits 300 kbps");
            } else {
                assert!(result
                    .expect_err("over-ceiling target must fail preflight")
                    .to_string()
                    .contains("264 kbps"));
            }
        }
    }

    #[test]
    fn execution_rejected_by_review_does_not_create_the_output_root() {
        let temp = TempDir::new().expect("temp dir");
        let source = temp.path().join("source.wav");
        write_silence_wav(&source, 1);
        let missing_root = temp.path().join("deleted-library");
        let payload = process_payload(|payload| {
            payload.input_files = vec![source.to_string_lossy().into_owned()];
            payload.output_dir = missing_root.to_string_lossy().into_owned();
            payload.preflight_signature = Some("stale review".into());
        });
        let inspected =
            super::run_validation::inspect_and_validate_external_processing_contract(&payload)
                .expect("inspect source");

        let Err(error) =
            crate::processing::plan::prepare_execution_plan(&payload, None, None, inspected)
        else {
            panic!("a stale review must reject execution");
        };

        assert!(
            error.to_string().contains("review"),
            "unexpected error: {error}"
        );
        assert!(
            !missing_root.exists(),
            "review rejection must not leave a folder"
        );
    }

    #[test]
    fn stacked_title_plan_keeps_source_order_metadata_and_separate_outputs() {
        use crate::metadata::{MetadataIntentPatch, PatchOp};
        use crate::processing::types::TitleSource;
        let temp = TempDir::new().expect("title plan workspace");
        let paths: Vec<_> = ["one.wav", "two.wav", "other.wav"]
            .iter()
            .map(|name| {
                let path = temp.path().join(name);
                write_silence_wav(&path, 1);
                path.canonicalize().expect("canonical fixture")
            })
            .collect();
        let names: Vec<_> = paths
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let mut payload = process_payload(|payload| {
            payload.input_files = vec![names[0].clone(), names[2].clone()];
            payload.title_sources = Some(HashMap::from([(
                names[0].clone(),
                vec![
                    TitleSource {
                        path: names[1].clone(),
                        input_id: Some("two".into()),
                    },
                    TitleSource {
                        path: names[0].clone(),
                        input_id: Some("one".into()),
                    },
                ],
            )]));
            payload.output_dir = temp.path().to_string_lossy().into_owned();
            let settings = payload.audio_requests[0]
                .settings
                .as_mut()
                .expect("encode settings");
            settings.encoder_type = EncoderType::NativeAac;
            settings.bitrate_mode = BitrateMode::Cbr;
        });
        let metadata = HashMap::from([
            (
                names[0].clone(),
                MetadataIntentPatch {
                    title: Some(PatchOp::Set("Combined title".into())),
                    ..Default::default()
                },
            ),
            (
                names[2].clone(),
                MetadataIntentPatch {
                    title: Some(PatchOp::Set("Separate title".into())),
                    ..Default::default()
                },
            ),
        ]);
        let reviewed = super::preflight_payload(payload.clone(), Some(metadata.clone()), None)
            .expect("review titles");
        payload.preflight_signature = Some(reviewed.plan_signature);
        let inspected =
            super::run_validation::inspect_and_validate_external_processing_contract(&payload)
                .expect("inspect all title sources");
        let execution = crate::processing::plan::prepare_execution_plan(
            &payload,
            Some(&metadata),
            None,
            inspected,
        )
        .expect("prepare reviewed execution");
        let jobs = execution.plan.jobs;
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].input_path, paths[0]);
        assert_eq!(jobs[0].source_paths, [paths[1].clone(), paths[0].clone()]);
        assert_eq!(jobs[1].source_paths, [paths[2].clone()]);
        for (job, title) in jobs.iter().zip(["Combined title", "Separate title"]) {
            assert_eq!(
                job.metadata
                    .as_ref()
                    .and_then(|value| value.title.as_deref()),
                Some(title)
            );
            assert!(job.output.resolved_path.to_string_lossy().contains(title));
            assert!(!job.output.resolved_path.exists());
        }
    }

    fn write_silence_wav(path: &std::path::Path, channels: u16) {
        const SAMPLE_RATE: u32 = 44_100;
        let data_len = SAMPLE_RATE * 2 * u32::from(channels);
        let mut bytes = Vec::with_capacity(44 + data_len as usize);
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&channels.to_le_bytes());
        bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend_from_slice(&(2 * channels).to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.resize(44 + data_len as usize, 0);
        std::fs::write(path, bytes).expect("write WAV fixture");
    }

    // Exercise the production planner's two metadata representations through the
    // real writers. Supplying metadata directly to the engine misses this handoff.
    async fn execute_reviewed_metadata_batch(
        mut payload: ProcessPayload,
        metadata: &HashMap<String, crate::metadata::MetadataIntentPatch>,
        workspace: &std::path::Path,
    ) -> Vec<std::path::PathBuf> {
        use crate::audio::{execute_audio_engine, AudioExecutionRequest, SampleRateConfig};
        use crate::processing::{OutputConfig, ProcessingContext, ProcessingSession};
        let reviewed = super::preflight_payload(payload.clone(), Some(metadata.clone()), None)
            .expect("review metadata batch");
        payload.preflight_signature = Some(reviewed.plan_signature);
        let inspected =
            super::run_validation::inspect_and_validate_external_processing_contract(&payload)
                .expect("inspect metadata batch");
        let execution = super::prepare_execution_plan(&payload, Some(metadata), None, inspected)
            .expect("plan metadata batch");
        let mut outputs = Vec::new();
        for job in execution.plan.jobs {
            let info =
                crate::processing::plan::title_file_info(&execution.file_info, &job.source_paths)
                    .expect("retain title inspection");
            outputs.push(job.output.resolved_path.clone());
            let context = ProcessingContext::new_headless_with_workspace_root(
                std::sync::Arc::new(ProcessingSession::new()),
                job.audio_plan.settings,
                SampleRateConfig::Explicit(job.audio_plan.sample_rate),
                OutputConfig::from_plan(job.output),
                workspace.to_path_buf(),
            );
            execute_audio_engine(
                AudioExecutionRequest::new(context, info, job.metadata, job.cover_art_passthrough)
                    .with_handling(job.audio_plan.handling)
                    .with_metadata_intent(job.metadata_intent),
            )
            .await
            .expect("write planned metadata to audio artifact");
        }
        outputs
    }

    fn metadata_batch_payload(
        paths: &[std::path::PathBuf],
        output: &std::path::Path,
        intent: crate::audio::AudioIntent,
    ) -> ProcessPayload {
        std::fs::create_dir_all(output).expect("create output folder");
        process_payload(|payload| {
            payload.input_files = paths
                .iter()
                .map(|path| path.to_string_lossy().into())
                .collect();
            payload.output_dir = output.to_string_lossy().into();
            payload.audio_requests[0].intent = intent;
            let settings = payload.audio_requests[0]
                .settings
                .as_mut()
                .expect("settings");
            settings.encoder_type = EncoderType::NativeAac;
            settings.bitrate_mode = BitrateMode::Cbr;
            if intent == crate::audio::AudioIntent::Preserve {
                payload.audio_requests[0].settings = None;
            }
        })
    }

    async fn metadata_source_fixtures(
        root: &std::path::Path,
        workspace: &std::path::Path,
    ) -> Vec<std::path::PathBuf> {
        use crate::metadata::{
            save_metadata_intent, AlbumSortPatchOp, MetadataIntentPatch, PatchOp,
        };
        let wavs: Vec<_> = ["alpha.wav", "beta.wav"]
            .map(|name| {
                let path = root.join(name);
                write_silence_wav(&path, 1);
                path
            })
            .into();
        let seeds = execute_reviewed_metadata_batch(
            metadata_batch_payload(
                &wavs,
                &root.join("sources"),
                crate::audio::AudioIntent::Encode,
            ),
            &HashMap::new(),
            workspace,
        )
        .await;
        // External books may carry a stale sort key or none at all.
        for (index, (source, title)) in seeds.iter().zip(["Alpha", "Beta"]).enumerate() {
            save_metadata_intent(
                source,
                &MetadataIntentPatch {
                    title: Some(PatchOp::Set(title.into())),
                    artist: Some(PatchOp::Set("Source Author".into())),
                    series: Some(PatchOp::Set("Saga".into())),
                    series_part: Some(PatchOp::Set("2.5".into())),
                    album_sort: Some(if index == 0 {
                        AlbumSortPatchOp::Set("Stale sort".into())
                    } else {
                        AlbumSortPatchOp::Clear
                    }),
                    ..Default::default()
                },
            )
            .expect("seed external source tags");
        }
        seeds
    }

    async fn metadata_two_pass_workflow(intent: crate::audio::AudioIntent) {
        use crate::metadata::{save_metadata_intent, MetadataIntentPatch, PatchOp};
        let temp = TempDir::new().expect("metadata workflow workspace");
        let workspace = temp.path().join("workspace");
        let seeds = metadata_source_fixtures(temp.path(), &workspace).await;
        let cover = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/minimal.jpg"
        ))
        .expect("existing JPEG fixture");
        let mut edits: HashMap<_, _> = seeds
            .iter()
            .map(|source| {
                (
                    source.to_string_lossy().into_owned(),
                    MetadataIntentPatch {
                        artist: Some(PatchOp::Set("Edited Author".into())),
                        cover_art: Some(PatchOp::Set(cover.clone())),
                        ..Default::default()
                    },
                )
            })
            .collect();
        for (pass, titles) in [["Alpha", "Beta"], ["Retitled Alpha", "Retitled Beta"]]
            .iter()
            .enumerate()
        {
            if pass == 1 {
                // The session's tests prove retention of these earlier edits;
                // this half proves that both accumulated requests reach disk.
                for (source, title) in seeds.iter().zip(titles) {
                    edits
                        .get_mut(&source.to_string_lossy().into_owned())
                        .expect("pending patch")
                        .title = Some(PatchOp::Set((*title).into()));
                }
            }
            let outputs = execute_reviewed_metadata_batch(
                metadata_batch_payload(&seeds, &temp.path().join(format!("pass-{pass}")), intent),
                &edits,
                &workspace,
            )
            .await;
            assert_eq!(outputs.len(), 2);
            for (output, title) in outputs.iter().zip(titles) {
                // Read atoms independently of ABB's read_metadata projection.
                let tags = mp4ameta::Tag::read_from_path(output).expect("read actual output tags");
                assert_eq!(tags.title(), Some(*title));
                assert_eq!(tags.artist(), Some("Edited Author"));
                assert_eq!(
                    tags.album_sort_order(),
                    Some(format!("Saga 02.5 - {title}").as_str())
                );
                assert_eq!(tags.artwork().expect("written cover").data, cover);
            }
        }
        for (index, source) in seeds.iter().enumerate() {
            let tags = mp4ameta::Tag::read_from_path(source).expect("read untouched source");
            assert_eq!(tags.artist(), Some("Source Author"));
            assert_eq!(
                tags.album_sort_order(),
                (index == 0).then_some("Stale sort")
            );
            assert!(tags.artwork().is_none());
            save_metadata_intent(
                source,
                &MetadataIntentPatch {
                    artist: Some(PatchOp::Set("Saved Author".into())),
                    ..Default::default()
                },
            )
            .expect("save source metadata");
            let saved = mp4ameta::Tag::read_from_path(source).expect("read saved source");
            assert_eq!(saved.artist(), Some("Saved Author"));
            assert_eq!(
                saved.album_sort_order(),
                (index == 0).then_some("Stale sort")
            );
        }
    }

    #[tokio::test]
    async fn metadata_workflow_two_pass_encode_writes_planned_tags() {
        metadata_two_pass_workflow(crate::audio::AudioIntent::Encode).await;
    }

    #[tokio::test]
    async fn metadata_workflow_two_pass_preserve_writes_planned_tags() {
        metadata_two_pass_workflow(crate::audio::AudioIntent::Preserve).await;
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

    fn batch_log_context() -> ProcessingJobLogContext {
        ProcessingJobLogContext {
            operation_id: None,
            input_index: 0,
            operation_kind: OperationKind::ProcessingBatch,
        }
    }

    /// Captures lifecycle records emitted through `log` so tests can assert
    /// terminal truth. Installable once per process; safe under Nextest's
    /// process-per-test execution.
    struct CapturingLogger {
        records: std::sync::Mutex<Vec<String>>,
    }

    impl log::Log for CapturingLogger {
        fn enabled(&self, _metadata: &log::Metadata<'_>) -> bool {
            true
        }

        fn log(&self, record: &log::Record<'_>) {
            self.records
                .lock()
                .expect("capturing logger lock")
                .push(record.args().to_string());
        }

        fn flush(&self) {}
    }

    static CAPTURING_LOGGER: CapturingLogger = CapturingLogger {
        records: std::sync::Mutex::new(Vec::new()),
    };

    #[tokio::test]
    async fn failed_output_validation_emits_started_and_failed_terminal_records() {
        log::set_logger(&CAPTURING_LOGGER).expect("install capturing logger");
        log::set_max_level(log::LevelFilter::Info);
        let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(2));
        let temp_dir = TempDir::new().expect("create temp dir");
        let invalid_output = temp_dir.path().join("output.mp3");

        if register_job_and_validate_output(
            &registry,
            &invalid_output,
            None,
            batch_log_context(),
            crate::processing::AudioHandling::Encode,
        )
        .await
        .is_ok()
        {
            panic!("invalid extension should fail validation");
        }

        let records = CAPTURING_LOGGER
            .records
            .lock()
            .expect("capturing logger lock")
            .clone();
        let started: Vec<&String> = records
            .iter()
            .filter(|record| record.contains("processing_job event=started"))
            .collect();
        let terminal: Vec<&String> = records
            .iter()
            .filter(|record| record.contains("processing_job event=terminal"))
            .collect();
        assert_eq!(started.len(), 1, "records: {records:?}");
        assert_eq!(terminal.len(), 1, "records: {records:?}");
        assert!(
            terminal[0].contains("status=failed")
                && terminal[0].contains("elapsed_ms=")
                && terminal[0].contains("code=invalid_input")
                && terminal[0].contains("category=validation"),
            "unexpected terminal record: {}",
            terminal[0]
        );
    }

    #[test]
    fn processing_request_rejected_record_pins_format_and_skips_cancellation() {
        assert_eq!(
            super::processing_request_rejected_record(&crate::errors::AppError::FileValidation(
                "bad output".to_string(),
            ))
            .as_deref(),
            Some(
                "processing_request event=rejected code=file_validation_failed category=validation"
            )
        );
        assert_eq!(
            super::processing_request_rejected_record(&crate::errors::AppError::cancelled()),
            None
        );
    }

    #[tokio::test]
    async fn register_job_and_validate_output_cleans_up_failed_validation() {
        let registry = std::sync::Arc::new(crate::processing::JobRegistry::new(2));
        let temp_dir = TempDir::new().expect("create temp dir");
        let invalid_output = temp_dir.path().join("output.mp3");

        let error = match register_job_and_validate_output(
            &registry,
            &invalid_output,
            None,
            batch_log_context(),
            crate::processing::AudioHandling::Encode,
        )
        .await
        {
            Ok(_) => panic!("invalid extension should fail validation"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("Encoded output must be .m4b, .m4a or .mka, got: .mp3"),
            "unexpected error: {error}"
        );

        let status = registry.get_aggregate_status().await;
        assert_eq!(status.active_jobs, 0, "active jobs should be cleared");
        assert_eq!(status.total_jobs, 0, "tracked jobs should be cleared");
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

        assert!(supplemental_assets_for_input(&payload, 0).is_empty());
        assert_eq!(supplemental_assets_for_input(&payload, 1), vec![asset]);
    }

    #[test]
    fn supplemental_commit_failure_keeps_published_title_successful_with_warning() {
        let root = TempDir::new().expect("temp root");
        let original_bytes = b"%PDF-1.7\nbody";
        let source = root.path().join("source.pdf");
        std::fs::write(&source, original_bytes).expect("write source pdf");
        let asset = supplemental_asset(source.clone(), "current-input-1", original_bytes);
        std::fs::write(&source, b"%PDF-1.7\nBODY").expect("change source pdf");
        let final_audio = root.path().join("Book.m4b");
        std::fs::write(&final_audio, b"published audiobook").expect("published audio");

        let outcome = title_outcome(Ok("Exported Book.m4b".to_string()), None, None, || {
            commit_supplemental_assets(OutputKind::Final, &[asset], &final_audio)
        });

        let ProcessingJobTerminalOutcome::Success {
            message,
            supplemental_warning: Some(warning),
            ..
        } = outcome
        else {
            panic!("a published title with a failed PDF stays successful: {outcome:?}");
        };
        assert_eq!(message, "Exported Book.m4b");
        assert!(
            warning.contains("Audiobook output 'Book.m4b' was created")
                && warning.contains("requested Supplemental PDFs could not be committed")
                && warning.contains("hash changed"),
            "unexpected warning: {warning}"
        );
        assert_eq!(
            std::fs::read(&final_audio).expect("audiobook remains"),
            b"published audiobook"
        );
    }

    #[test]
    fn failed_audio_never_publishes_companions() {
        let outcome = title_outcome(
            Err(crate::errors::AppError::FileValidation(
                "encode failed".into(),
            )),
            None,
            None,
            || panic!("companions must not publish without an audiobook"),
        );

        assert!(matches!(outcome, ProcessingJobTerminalOutcome::Failed(_)));
    }
}
