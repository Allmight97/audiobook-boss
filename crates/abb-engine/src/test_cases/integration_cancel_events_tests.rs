use super::media_fixtures::{native_encoder_settings, write_sine_wav};
use crate::audio::{AacDecoder, AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest};
use crate::errors::AppError;
use crate::host::Host;
use crate::power::PowerManager;
use crate::processing::run::{
    inspect_processing_plan, process_inspected_with_options, ProcessingRunOptions,
};
use crate::processing::{EventStage, JobRegistry, ProcessPayload, ProgressEvent};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

fn holds_staged_bytes(folder: &Path) -> bool {
    fs::read_dir(folder).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                holds_staged_bytes(&path)
            } else {
                entry.metadata().is_ok_and(|meta| meta.len() > 0)
            }
        })
    })
}

#[tokio::test]
async fn a_cancel_during_the_encode_reports_one_cancelled_event_after_the_staged_file_is_gone() {
    let tmp = TempDir::new().expect("tempdir");
    let source = tmp.path().join("source.wav");
    write_sine_wav(&source, 300.0, 440.0);
    let output_dir = tmp.path().join("out");
    fs::create_dir_all(&output_dir).expect("output dir");
    let workspace_root = tmp.path().join("workspace");
    let payload = ProcessPayload {
        input_files: vec![source.to_string_lossy().into_owned()],
        title_sources: None,
        chapter_plans: None,
        input_ids: None,
        output_dir: output_dir.to_string_lossy().into_owned(),
        audio_requests: vec![TitleAudioRequest {
            output: OutputAudio::M4b {
                settings: native_encoder_settings(),
            },
            intent: AudioIntent::Encode,
            sample_rate: SampleRateConfig::Auto,
        }],
        output_naming: None,
        collision_policy: None,
        preflight_signature: None,
        supplemental_assets_by_input_id: None,
        aac_decoder: Some(AacDecoder::Auto),
    };
    let inspected = inspect_processing_plan(&payload, None, false).expect("inspect");

    let cancel = Arc::new(AtomicBool::new(false));
    let cancelled_events: Arc<Mutex<Vec<bool>>> = Arc::default();
    let listener = {
        let cancelled_events = Arc::clone(&cancelled_events);
        let workspace_root = workspace_root.clone();
        Arc::new(move |event: &ProgressEvent| {
            if event.stage == EventStage::Cancelled {
                cancelled_events
                    .lock()
                    .expect("events")
                    .push(!holds_staged_bytes(&workspace_root));
            }
        })
    };

    let registry = Arc::new(JobRegistry::new(1));
    let run = registry.hold_run();
    let host = Host::new(Arc::new(crate::DiscardEvents), PowerManager::default());
    let processing = process_inspected_with_options(
        &run,
        host,
        Arc::clone(&registry),
        workspace_root.clone(),
        payload,
        inspected,
        ProcessingRunOptions {
            title_cancels: vec![Arc::clone(&cancel)],
            progress_listener: Some(listener),
            ..ProcessingRunOptions::default()
        },
    );
    tokio::pin!(processing);
    let encode_started = async {
        while !holds_staged_bytes(&workspace_root) {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        cancel.store(true, Ordering::SeqCst);
    };
    let result = tokio::select! {
        result = &mut processing => result,
        () = encode_started => processing.await,
    };

    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "a run whose only title was cancelled ends as a cancellation: {result:?}"
    );
    assert_eq!(
        *cancelled_events.lock().expect("events"),
        vec![true],
        "one Cancelled event, sent once the workspace is clean"
    );
}
