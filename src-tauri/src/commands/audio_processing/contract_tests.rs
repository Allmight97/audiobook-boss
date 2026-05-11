use crate::audio::output_path::{CollisionPolicy, OutputKind};
use crate::audio::settings_encoder::{
    BitrateMode, ChannelConfig, EncoderSettings, EncoderType, ThreadSetting,
};
use crate::commands::audio_processing::plan::{prepare_execution_plan, resolve_preflight_plan};
use crate::commands::audio_types::{JobType, NamingPreset, OutputNamingConfig, ProcessPayload};
use crate::metadata::{MetadataIntentPatch, PatchOp};
use std::collections::HashMap;
use std::path::PathBuf;
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

fn copy_audio_fixture(temp_dir: &TempDir, name: &str) -> PathBuf {
    let source_fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("media")
        .join("media_20sec.mp3");
    let input_path = temp_dir.path().join(name);
    std::fs::copy(&source_fixture, &input_path).expect("copy fixture");
    input_path
}

fn process_payload(overrides: impl FnOnce(&mut ProcessPayload)) -> ProcessPayload {
    let mut payload = ProcessPayload {
        input_files: vec!["/books/input.m4b".to_string()],
        output_dir: "/tmp/out".to_string(),
        settings: encoder_settings(),
        external_toolchain: None,
        sample_rate: None,
        job_type: Some(JobType::Batch),
        output_naming: None,
        collision_policy: None,
        preflight_signature: None,
    };
    overrides(&mut payload);
    payload
}

#[test]
fn processing_plan_contract_preflight_is_side_effect_free() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
    let planned_parent = temp_dir.path().join("Nested");
    let mut metadata = HashMap::new();
    metadata.insert(
        input_path.to_string_lossy().to_string(),
        MetadataIntentPatch {
            title: PatchOp::Set("Contract Book".to_string()),
            ..Default::default()
        },
    );
    let payload = process_payload(|payload| {
        payload.input_files = vec![input_path.to_string_lossy().to_string()];
        payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        payload.output_naming = Some(OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("Nested/{title}".to_string()),
        });
    });

    let plan =
        resolve_preflight_plan(&payload, Some(&metadata), Some(30.0)).expect("preflight plan");

    assert_eq!(plan.job_type, JobType::Batch);
    assert_eq!(plan.preview_seconds, Some(30.0));
    assert_eq!(plan.outputs.len(), 1);
    assert_eq!(plan.outputs[0].kind, OutputKind::Preview);
    assert!(
        !planned_parent.exists(),
        "preflight must not create output parent directories"
    );
}

#[test]
fn processing_plan_contract_execution_consumes_review_and_creates_parent_dirs() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
    let planned_parent = temp_dir.path().join("Nested");
    let mut metadata = HashMap::new();
    metadata.insert(
        input_path.to_string_lossy().to_string(),
        MetadataIntentPatch {
            title: PatchOp::Set("Contract Book".to_string()),
            ..Default::default()
        },
    );
    let payload = process_payload(|payload| {
        payload.input_files = vec![input_path.to_string_lossy().to_string()];
        payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        payload.output_naming = Some(OutputNamingConfig {
            preset: NamingPreset::CustomTemplate,
            include_year: false,
            custom_template: Some("Nested/{title}".to_string()),
        });
    });

    let plan = prepare_execution_plan(&payload, Some(&metadata), None).expect("execution plan");

    assert_eq!(plan.jobs.len(), 1);
    assert_eq!(plan.collision_policy, CollisionPolicy::Fail);
    assert!(planned_parent.exists());
    assert!(plan.jobs[0]
        .output
        .resolved_path
        .ends_with("Nested/Contract Book.m4b"));
}

#[test]
fn processing_plan_contract_rejects_stale_preflight_signature() {
    let temp_dir = TempDir::new().expect("temp dir");
    let input_path = copy_audio_fixture(&temp_dir, "input.m4b");
    let payload = process_payload(|payload| {
        payload.input_files = vec![input_path.to_string_lossy().to_string()];
        payload.output_dir = temp_dir.path().to_string_lossy().to_string();
        payload.preflight_signature = Some("stale".to_string());
    });

    let err = prepare_execution_plan(&payload, None, None)
        .expect_err("stale preflight signature should fail");

    assert!(err.to_string().contains("collision state changed"));
}
