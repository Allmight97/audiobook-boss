use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use abb_media_pipeline::audio::{
    get_file_list_info, AacDecoder, AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest,
};
use abb_media_pipeline::metadata::{
    extract_passthrough_metadata, MetadataIntentPatch, PassthroughSource,
};
use abb_media_pipeline::processing::run::{inspect_processing_plan, InspectedRun, ProcessingRoots};
use abb_media_pipeline::processing::{JobRegistry, ProcessPayload, ProcessingPreflightPlan};
use abb_test_fixtures::wav::write_sine_wav;
use tempfile::TempDir;

use super::support::encoders::native_encoder_settings;
use crate::work_runtime::{
    OperationId, OperationSnapshot, SubmitProcessingOperationRequest, WorkOperationStatus,
    WorkRuntime,
};

fn open_input(path: &Path) -> abb_ffmpeg::Input {
    abb_ffmpeg::Input::open(path, &abb_ffmpeg::SecretOptions::default())
        .expect("probe fixture")
        .input
}

pub(crate) fn picture_codecs(path: &Path) -> Vec<abb_ffmpeg::CodecId> {
    let input = open_input(path);
    let mut codecs: Vec<_> = input
        .streams()
        .iter()
        .filter(|stream| {
            stream.medium == abb_ffmpeg::Medium::Attachment
                || stream
                    .disposition
                    .contains(abb_ffmpeg::Disposition::ATTACHED_PIC)
        })
        .map(|stream| stream.codec)
        .collect();
    codecs.sort_by_key(|id| id.name());
    codecs
}

pub(crate) fn duration_ms(path: &Path) -> i64 {
    let info = get_file_list_info(&[path], AacDecoder::Auto).expect("probe fixture length");
    (info.files[0].duration.expect("fixture length") * 1000.0).round() as i64
}

pub(crate) fn audio_packet_bytes(path: &Path) -> Vec<Vec<u8>> {
    let mut input = open_input(path);
    let index = input.best_audio_stream().expect("fixture value");
    let mut packet = abb_ffmpeg::Packet::new().expect("packet");
    let mut payloads = Vec::new();
    while input.read_packet(&mut packet).expect("read packet") {
        if packet.stream_index() == index {
            payloads.push(packet.data().to_vec());
        }
    }
    payloads
}

pub(crate) fn chapters_of(path: &Path) -> Vec<(Option<String>, i64, i64)> {
    let passthrough = extract_passthrough_metadata(&[PassthroughSource {
        chapters: None,
        path: path.to_path_buf(),
        duration: None,
        is_valid: true,
    }]);
    passthrough
        .chapters
        .into_iter()
        .map(|chapter| (chapter.title, chapter.start_ms, chapter.end_ms))
        .collect()
}

pub(crate) fn single_title_payload(
    source: &Path,
    output_dir: &Path,
    request: TitleAudioRequest,
) -> ProcessPayload {
    fs::create_dir_all(output_dir).expect("output folder");
    serde_json::from_value(serde_json::json!({
        "inputFiles": [source], "audioRequests": [request], "outputDir": output_dir, "aacDecoder": "auto"
    }))
    .expect("single-title request")
}

pub(crate) fn preflight_payload(
    payload: ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview: bool,
) -> crate::Result<ProcessingPreflightPlan> {
    inspect_processing_plan(&payload, metadata.as_ref(), preview)
        .map(|run| run.inspected().plan.to_public())
}

pub(crate) struct MediaLane {
    pub(crate) tmp: TempDir,
    pub(crate) inputs: Vec<PathBuf>,
}

impl MediaLane {
    pub(crate) fn with_fixtures(durations_secs: &[f64]) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        let inputs = durations_secs
            .iter()
            .enumerate()
            .map(|(index, seconds)| {
                let path = tmp.path().join(format!("fixture-{index}.wav"));
                write_sine_wav(&path, *seconds, 440.0 + 110.0 * index as f64);
                path
            })
            .collect();
        Self { tmp, inputs }
    }

    pub(crate) async fn export(&self, tags: MetadataIntentPatch) -> PathBuf {
        let [source] = &self.inputs[..] else {
            panic!("an exported book has one source");
        };
        let output_dir = self.tmp.path().join("out");
        let payload = single_title_payload(
            source,
            &output_dir,
            TitleAudioRequest {
                output: OutputAudio::M4b {
                    settings: native_encoder_settings(),
                },
                intent: AudioIntent::Encode,
                sample_rate: SampleRateConfig::Auto,
            },
        );
        let metadata = HashMap::from([(source.to_string_lossy().into_owned(), tags)]);
        let InspectedRun::Export(plan) =
            inspect_processing_plan(&payload, Some(&metadata), false).expect("inspect the export")
        else {
            panic!("expected an export plan");
        };
        let settled = Arc::new(Mutex::new(None));
        let on_finished = {
            let settled = Arc::clone(&settled);
            Box::new(move |snapshot: OperationSnapshot| {
                *settled.lock().expect("settled") = Some(snapshot.status);
            })
        };
        let runtime = WorkRuntime::new(crate::engine_tasks::EngineTasks::default());
        let host = crate::host::Host::new(
            Arc::new(crate::DiscardEvents),
            abb_media_pipeline::power::PowerManager::default(),
        );
        runtime
            .submit_processing_operation(
                host,
                Arc::new(JobRegistry::new(1)),
                ProcessingRoots::for_cache(&self.tmp.path().join("cache")),
                SubmitProcessingOperationRequest {
                    operation_id: OperationId("fixture-book".to_string()),
                    payload,
                    metadata: Some(metadata),
                    title: "Book".to_string(),
                },
                Some(on_finished),
                plan,
            )
            .await
            .expect("accept the export");
        abb_test_fixtures::poll::until_true("the fixture export settled", || {
            settled.lock().expect("settled").is_some()
        })
        .await;
        assert_eq!(
            *settled.lock().expect("settled"),
            Some(WorkOperationStatus::Completed)
        );
        abb_test_fixtures::fs_listing::files_under(&output_dir)
            .into_iter()
            .find(|path| path.extension().is_some_and(|ext| ext == "m4b"))
            .expect("the exported book")
    }
}
