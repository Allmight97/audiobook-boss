use super::support::encoders::native_encoder_settings;
use crate::audio::{
    execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest, ChannelConfig,
    EncoderSettings, SampleRateConfig,
};
use crate::metadata::{
    extract_passthrough_metadata, AudiobookMetadata, CoverArtPassthroughPolicy,
    MetadataIntentPatch, PassthroughSource,
};
use crate::processing::{AudioHandling, OutputConfig, ProcessingContext, ProcessingSession};
use crate::test_cases::support::session::idle_session;
use crate::AppError;
use abb_test_fixtures::media::decode_pcm_f32;
use abb_test_fixtures::wav::{write_sine_wav, write_stereo_sine_wav};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;

pub(crate) fn picture_codecs(path: &Path) -> Vec<abb_ffmpeg::CodecId> {
    let input = crate::metadata::open_input(path).expect("probe fixture");
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

pub(crate) fn picture_payloads(path: &Path) -> Vec<(abb_ffmpeg::CodecId, Vec<u8>)> {
    let input = crate::metadata::open_input(path).expect("probe fixture");
    let mut pictures: Vec<_> = input
        .streams()
        .iter()
        .filter(|stream| {
            stream
                .disposition
                .contains(abb_ffmpeg::Disposition::ATTACHED_PIC)
        })
        .map(|stream| {
            let payload = input
                .attached_picture(stream.index)
                .expect("picture payload");
            (stream.codec, payload)
        })
        .collect();
    pictures.sort_by_key(|(codec, payload)| (codec.name(), payload.clone()));
    pictures
}

pub(crate) fn duration_ms(path: &Path) -> i64 {
    let info = get_file_list_info(&[path], AacDecoder::Auto).expect("probe fixture length");
    (info.files[0].duration.expect("fixture length") * 1000.0).round() as i64
}

pub(crate) async fn run_planned_title(
    payload: crate::processing::ProcessPayload,
    metadata: Option<std::collections::HashMap<String, MetadataIntentPatch>>,
    workspace: &Path,
) -> Result<(PathBuf, crate::audio::AudioRunOutcome), AppError> {
    let inspected =
        crate::processing::run::inspect_processing_plan(&payload, metadata.as_ref(), false)?
            .into_inspected();
    let job = inspected.plan.jobs.into_iter().next().expect("one title");
    let files: Vec<_> = job
        .source_paths
        .iter()
        .map(|path| {
            inspected
                .file_info
                .files
                .iter()
                .find(|file| &file.path == path)
                .cloned()
                .expect("inspected source")
        })
        .collect();
    let info = crate::audio::FileListInfo {
        total_duration: files.iter().filter_map(|file| file.duration).sum(),
        total_size: 0.0,
        valid_count: files.len(),
        invalid_count: 0,
        files,
    };
    let destination = job.output.resolved_path.clone();
    fs::create_dir_all(destination.parent().expect("output folder")).expect("output folder");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(idle_session()),
        job.audio_plan.sample_rate,
        OutputConfig::from_plan(job.output),
        workspace.to_path_buf(),
    );
    let outcome = execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            info,
            job.metadata,
            job.cover_art_passthrough,
            job.audio_plan.handling,
        )
        .with_metadata_intent(job.metadata_intent),
    )
    .await?;
    Ok((destination, outcome))
}

pub(crate) fn single_title_payload(
    source: &Path,
    output_dir: &Path,
    request: crate::audio::TitleAudioRequest,
) -> crate::processing::ProcessPayload {
    fs::create_dir_all(output_dir).expect("output folder");
    serde_json::from_value(serde_json::json!({
        "inputFiles": [source], "audioRequests": [request], "outputDir": output_dir, "aacDecoder": "auto"
    }))
    .expect("single-title request")
}

pub(crate) fn audio_packet_bytes(path: &Path) -> Vec<Vec<u8>> {
    let mut input = crate::metadata::open_input(path).expect("fixture value");
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

pub(crate) fn preflight_payload(
    payload: crate::processing::ProcessPayload,
    metadata: Option<HashMap<String, MetadataIntentPatch>>,
    preview: bool,
) -> crate::Result<crate::processing::ProcessingPreflightPlan> {
    crate::processing::run::inspect_processing_plan(&payload, metadata.as_ref(), preview)
        .map(|inspected| inspected.inspected().plan.to_public())
}

pub(crate) struct MediaLane {
    pub(crate) tmp: TempDir,
    pub(crate) inputs: Vec<PathBuf>,
    encoder_settings: EncoderSettings,
    sample_rate: SampleRateConfig,
    aac_decoder: AacDecoder,
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
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
            aac_decoder: AacDecoder::Auto,
        }
    }

    pub(crate) fn for_inputs(inputs: Vec<PathBuf>) -> Self {
        let tmp = TempDir::new().expect("create media lane tempdir");
        Self {
            tmp,
            inputs,
            encoder_settings: native_encoder_settings(),
            sample_rate: SampleRateConfig::Auto,
            aac_decoder: AacDecoder::Auto,
        }
    }

    pub(crate) fn with_aac_decoder(mut self, aac_decoder: AacDecoder) -> Self {
        self.aac_decoder = aac_decoder;
        self
    }

    pub(crate) fn with_encoder(mut self, encoder_settings: EncoderSettings) -> Self {
        self.encoder_settings = encoder_settings;
        self
    }

    pub(crate) fn with_sample_rate(mut self, sample_rate: SampleRateConfig) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    fn resolved_sample_rate(&self) -> u32 {
        match self.sample_rate {
            SampleRateConfig::Explicit(rate) => rate,
            SampleRateConfig::Auto => {
                let source = get_file_list_info(&self.inputs[..1], self.aac_decoder)
                    .expect("probe the first source")
                    .files[0]
                    .sample_rate
                    .expect("the first source has a sample rate");
                crate::audio::automatic_sample_rate(
                    self.encoder_settings.encoder_type,
                    self.encoder_settings.faac_profile,
                    source,
                )
            }
        }
    }

    pub(crate) fn output_path(&self) -> PathBuf {
        self.tmp.path().join("out").join("lane-output.m4b")
    }

    pub(crate) fn workspace_root(&self) -> PathBuf {
        self.tmp.path().join("workspace")
    }

    pub(crate) fn handling(&self) -> AudioHandling {
        AudioHandling::Encode {
            settings: self.encoder_settings.clone(),
        }
    }

    pub(crate) fn context(&self, session: ProcessingSession) -> ProcessingContext {
        let output_dir = self.output_path();
        fs::create_dir_all(output_dir.parent().expect("output parent")).expect("create output dir");
        let mut context = ProcessingContext::new_headless_with_workspace_root(
            Arc::new(session),
            self.resolved_sample_rate(),
            OutputConfig::new(self.output_path()),
            self.workspace_root(),
        );
        context.aac_decoder = self.aac_decoder;
        context
    }

    pub(crate) fn execution_request(
        &self,
        session: ProcessingSession,
        metadata: Option<AudiobookMetadata>,
    ) -> AudioExecutionRequest {
        let file_info =
            get_file_list_info(&self.inputs, self.aac_decoder).expect("probe WAV fixtures");
        assert_eq!(
            file_info.invalid_count, 0,
            "generated fixtures must probe as valid audio"
        );
        AudioExecutionRequest::new(
            self.context(session),
            file_info,
            metadata,
            CoverArtPassthroughPolicy::Preserve,
            self.handling(),
        )
    }

    pub(crate) async fn process(&self, metadata: Option<AudiobookMetadata>) -> PathBuf {
        execute_audio_engine(self.execution_request(idle_session(), metadata))
            .await
            .expect("native processing succeeds");
        self.output_path()
    }

    pub(crate) fn residual_workspace_dirs(&self) -> Vec<PathBuf> {
        let root = self.workspace_root();
        if !root.exists() {
            return Vec::new();
        }
        fs::read_dir(root)
            .expect("read workspace root")
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .collect()
    }
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

pub(crate) async fn assert_auto_merge_stereo(settings: EncoderSettings) {
    let fixtures = TempDir::new().expect("mixed channel fixtures");
    let mono = fixtures.path().join("mono.wav");
    let stereo = fixtures.path().join("stereo.wav");
    write_sine_wav(&mono, 0.3, 220.0);
    write_stereo_sine_wav(&stereo, 0.6, 440.0, 660.0);
    for inputs in [
        vec![mono.clone(), stereo.clone()],
        vec![stereo.clone(), mono.clone()],
    ] {
        let lane = MediaLane::for_inputs(inputs).with_encoder(EncoderSettings {
            channels: ChannelConfig::Auto,
            ..settings.clone()
        });
        let output = lane.process(None).await;
        let probe =
            get_file_list_info(&[&output], AacDecoder::Auto).expect("re-probe mixed-channel M4B");
        assert_eq!(probe.files[0].channels, Some(2), "Auto preserves stereo");
        let samples = decode_pcm_f32(&output);
        let frames = samples.as_chunks::<2>().0;
        let count = frames.len() as f64;
        let mut energy = [0.0_f64; 3];
        for frame in frames {
            let left = f64::from(frame[0]);
            let right = f64::from(frame[1]);
            energy[0] += left * left;
            energy[1] += right * right;
            energy[2] += (left - right).powi(2);
        }
        for (signal, energy) in ["left", "right", "left minus right"]
            .into_iter()
            .zip(energy)
        {
            let rms = (energy / count).sqrt();
            assert!(
                rms > 0.05,
                "{signal} RMS {rms}: missing audio or collapsed stereo"
            );
        }
    }
}

pub(crate) fn accepted_chapters(path: &Path) -> Vec<(Option<String>, i64, i64)> {
    get_file_list_info(&[path], AacDecoder::Auto)
        .expect("import fixture")
        .files[0]
        .chapter_plan
        .as_ref()
        .expect("accepted chapter plan")
        .chapters
        .iter()
        .map(|chapter| (chapter.title.clone(), chapter.start_ms, chapter.end_ms))
        .collect()
}

pub(crate) fn title_audio_request(
    intent: crate::audio::AudioIntent,
    settings: EncoderSettings,
) -> crate::audio::TitleAudioRequest {
    crate::audio::TitleAudioRequest {
        output: crate::audio::OutputAudio::M4b { settings },
        intent,
        sample_rate: SampleRateConfig::Auto,
    }
}
