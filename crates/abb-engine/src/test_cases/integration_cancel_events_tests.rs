use super::support::encoders::native_encoder_settings;
use super::support::poll::until_true;
use super::support::wav::write_sine_wav;
use crate::audio::{AacDecoder, AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest};
use crate::errors::AppError;
use crate::host::Host;
use crate::power::PowerManager;
use crate::processing::run::{
    inspect_processing_plan, preflight_title_outputs, process_inspected_with_options, ExportTitle,
    ProcessingRun, RunMode,
};
use crate::processing::{EventStage, JobRegistry, ProcessPayload, ProgressEvent};
use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio_util::sync::CancellationToken;

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
    let outputs = preflight_title_outputs(&payload, None, &inspected).expect("title outputs");

    let cancel = CancellationToken::new();
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
        ProcessingRun {
            operation_id: "cancel-events".to_string(),
            listener,
            mode: RunMode::Export(
                outputs
                    .into_iter()
                    .map(|output| ExportTitle {
                        cancel: cancel.clone(),
                        output,
                    })
                    .collect(),
            ),
        },
    );
    tokio::pin!(processing);
    let encode_started = async {
        until_true("an encode holds staged bytes", || {
            holds_staged_bytes(&workspace_root)
        })
        .await;
        cancel.cancel();
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

type TitleEnd = (EventStage, Option<usize>);

fn encode_request() -> TitleAudioRequest {
    TitleAudioRequest {
        output: OutputAudio::M4b {
            settings: native_encoder_settings(),
        },
        intent: AudioIntent::Encode,
        sample_rate: SampleRateConfig::Auto,
    }
}

#[tokio::test]
async fn a_title_that_fails_reports_failed_before_a_later_title_ends() {
    let tmp = TempDir::new().expect("tempdir");
    let failing = tmp.path().join("failing.wav");
    let running = tmp.path().join("running.wav");
    write_sine_wav(&failing, 1.0, 440.0);
    write_sine_wav(&running, 60.0, 440.0);
    let output_dir = tmp.path().join("out");
    fs::create_dir_all(&output_dir).expect("output dir");
    let workspace_root = tmp.path().join("workspace");
    let payload = ProcessPayload {
        input_files: vec![
            failing.to_string_lossy().into_owned(),
            running.to_string_lossy().into_owned(),
        ],
        title_sources: None,
        chapter_plans: None,
        input_ids: None,
        output_dir: output_dir.to_string_lossy().into_owned(),
        audio_requests: vec![encode_request(), encode_request()],
        output_naming: None,
        collision_policy: None,
        preflight_signature: None,
        supplemental_assets_by_input_id: None,
        aac_decoder: Some(AacDecoder::Auto),
    };
    let inspected = inspect_processing_plan(&payload, None, false).expect("inspect");
    let outputs = preflight_title_outputs(&payload, None, &inspected).expect("title outputs");
    fs::remove_file(&failing).expect("remove the first title's source");

    let cancels = [CancellationToken::new(), CancellationToken::new()];
    let ends: Arc<Mutex<Vec<TitleEnd>>> = Arc::default();
    let listener = {
        let ends = Arc::clone(&ends);
        Arc::new(move |event: &ProgressEvent| {
            if matches!(
                event.stage,
                EventStage::Failed | EventStage::Cancelled | EventStage::Completed
            ) {
                ends.lock()
                    .expect("ends")
                    .push((event.stage, event.input_index));
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
        ProcessingRun {
            operation_id: "failed-events".to_string(),
            listener,
            mode: RunMode::Export(
                outputs
                    .into_iter()
                    .zip(cancels.iter().cloned())
                    .map(|(output, cancel)| ExportTitle { cancel, output })
                    .collect(),
            ),
        },
    );
    tokio::pin!(processing);
    let second_started = async {
        until_true("the second title holds staged bytes", || {
            holds_staged_bytes(&workspace_root)
        })
        .await;
        cancels[1].cancel();
    };
    let result = tokio::select! {
        result = &mut processing => result,
        () = second_started => processing.await,
    };

    assert!(result.is_ok(), "{result:?}");
    assert_eq!(
        *ends.lock().expect("ends"),
        vec![
            (EventStage::Failed, Some(0)),
            (EventStage::Cancelled, Some(1))
        ],
        "the first title's Failed event arrives when it ends, before the second title's"
    );
}
