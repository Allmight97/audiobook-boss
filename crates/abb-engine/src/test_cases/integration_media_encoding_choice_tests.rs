use std::fs;
use std::path::{Path, PathBuf};

use abb_engine::audio::{
    AudioIntent, AudiobookFormat, ChannelConfig, EncoderType, SampleRateConfig,
};
use abb_engine::session::{AudioEdit, MetadataField, SessionIntent, SubmissionStatus, TitlePlan};
use abb_engine::work_runtime::WorkOperationStatus;

use super::integration_session_desk::{submission, Desk};
use super::media_fixtures::{audio_duration_ts, decode_pcm_f32, MediaLane};
use super::media_fixtures::{audio_packet_bytes, ffmpeg, ffprobe};

fn stream_fact(path: &Path, entry: &str) -> String {
    let out = ffprobe([
        "-v",
        "error",
        "-select_streams",
        "a:0",
        "-show_entries",
        entry,
        "-of",
        "default=nw=1:nk=1",
        path.to_str().expect("utf8"),
    ]);
    String::from_utf8(out).expect("utf8").trim().to_string()
}

fn exact_wav(path: &Path, rate: u32, samples: u64) {
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        &format!("sine=frequency=440:sample_rate={rate}"),
        "-af",
        &format!("atrim=end_sample={samples}"),
        "-c:a",
        "pcm_s16le",
        "-y",
        path.to_str().expect("utf8"),
    ]);
}

#[tokio::test]
async fn a_native_aac_upsample_keeps_every_sample_of_the_book() {
    let dir = tempfile::TempDir::new().expect("dir");
    for (sources, rate, expected) in [
        (vec![22_117_u64], 44_100_u32, 44_234_u64),
        (vec![22_117, 11_003], 44_100, 66_240),
        (vec![22_117], 48_000, 48_146),
    ] {
        let inputs: Vec<_> = sources
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let p = dir.path().join(format!("{rate}-{}-{i}.wav", sources.len()));
                exact_wav(&p, 22_050, *n);
                p
            })
            .collect();
        let lane = MediaLane::for_inputs(inputs).with_sample_rate(SampleRateConfig::Explicit(rate));
        let output = lane.process(None).await;
        assert_eq!(audio_duration_ts(&output), expected, "declared");
        assert_eq!(decode_pcm_f32(&output).len() as u64, expected, "decoded");
        assert!(lane.residual_workspace_dirs().is_empty());
    }
}

fn stereo_pink_noise(path: &Path) {
    let channel = |seed: u32| {
        format!(
            "anoisesrc=color=pink:sample_rate=44100:amplitude=0.3:seed={seed}:duration={NOISE_SECONDS}"
        )
    };
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        &channel(7),
        "-f",
        "lavfi",
        "-i",
        &channel(9),
        "-filter_complex",
        "[0][1]join=inputs=2:channel_layout=stereo",
        "-c:a",
        "pcm_s16le",
        "-y",
        path.to_str().expect("utf8"),
    ]);
}

async fn choose(desk: &Desk, choice: &Choice) {
    let title_ids: Vec<_> = desk
        .engine
        .session_snapshot()
        .titles
        .expect("titles")
        .files
        .into_iter()
        .map(|file| file.input_id)
        .collect();
    let mut edits = vec![
        AudioEdit::Format(choice.format),
        AudioEdit::Intent(AudioIntent::Encode),
    ];
    if choice.format == AudiobookFormat::M4b {
        edits.push(AudioEdit::Encoder(EncoderType::NativeAac));
    }
    edits.extend([
        AudioEdit::Bitrate(choice.kbps),
        AudioEdit::SampleRate(SampleRateConfig::Explicit(choice.sample_rate)),
        AudioEdit::Channels(choice.channels),
    ]);
    for edit in edits {
        desk.send(SessionIntent::SetTitleAudio {
            title_ids: title_ids.clone(),
            edit,
        })
        .await;
    }
}

async fn export_when_planned(desk: &Desk) -> PathBuf {
    let out = desk.root.path().join("exports");
    fs::create_dir_all(&out).expect("out");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: out.to_string_lossy().into_owned(),
    })
    .await;
    desk.wait_until("plan", |d| d.plan() != TitlePlan::Pending)
        .await;
    desk.send(SessionIntent::Submit).await;
    let Some(SubmissionStatus::Submitted { operation_id, .. }) = submission(desk) else {
        panic!("{:?}", submission(desk))
    };
    desk.wait_until("export", |d| d.export_finished(&operation_id))
        .await;
    assert_eq!(
        desk.export_status(&operation_id),
        WorkOperationStatus::Completed
    );
    desk.output_of(&operation_id)
}

fn packet_kbps(output: &Path) -> f64 {
    let bytes: usize = audio_packet_bytes(output).iter().map(Vec::len).sum();
    bytes as f64 * 8.0 / NOISE_SECONDS / 1000.0
}

struct Choice {
    format: AudiobookFormat,
    sample_rate: u32,
    channels: ChannelConfig,
    kbps: u16,
    codec: &'static str,
    declared_rate: &'static str,
    declared_channels: &'static str,
    extension: &'static str,
    container: &'static str,
}

const OPUS_DECLARED_RATE: &str = "48000";
const MP4_CONTAINER: &str = "mov,mp4,m4a,3gp,3g2,mj2";
const MATROSKA_CONTAINER: &str = "matroska,webm";
const NOISE_SECONDS: f64 = 2.0;
const VARIABLE_BITRATE_TOLERANCE: f64 = 0.15;

#[tokio::test]
async fn encoding_choices_reach_the_file() {
    let choices = [
        Choice {
            format: AudiobookFormat::M4b,
            sample_rate: 22_050,
            channels: ChannelConfig::Mono,
            kbps: 48,
            codec: "aac",
            declared_rate: "22050",
            declared_channels: "1",
            extension: "m4b",
            container: MP4_CONTAINER,
        },
        Choice {
            format: AudiobookFormat::M4b,
            sample_rate: 32_000,
            channels: ChannelConfig::Stereo,
            kbps: 96,
            codec: "aac",
            declared_rate: "32000",
            declared_channels: "2",
            extension: "m4b",
            container: MP4_CONTAINER,
        },
        Choice {
            format: AudiobookFormat::M4aOpus,
            sample_rate: 24_000,
            channels: ChannelConfig::Mono,
            kbps: 48,
            codec: "opus",
            declared_rate: OPUS_DECLARED_RATE,
            declared_channels: "1",
            extension: "m4a",
            container: MP4_CONTAINER,
        },
        Choice {
            format: AudiobookFormat::MkaOpus,
            sample_rate: 24_000,
            channels: ChannelConfig::Mono,
            kbps: 48,
            codec: "opus",
            declared_rate: OPUS_DECLARED_RATE,
            declared_channels: "1",
            extension: "mka",
            container: MATROSKA_CONTAINER,
        },
        Choice {
            format: AudiobookFormat::MkaOpus,
            sample_rate: 48_000,
            channels: ChannelConfig::Stereo,
            kbps: 96,
            codec: "opus",
            declared_rate: OPUS_DECLARED_RATE,
            declared_channels: "2",
            extension: "mka",
            container: MATROSKA_CONTAINER,
        },
    ];
    for choice in choices {
        let desk = Desk::new();
        let book = desk.root.path().join("noise.wav");
        stereo_pink_noise(&book);
        desk.import(&book).await;
        desk.send(SessionIntent::SetField {
            binding: desk.metadata().binding,
            field: MetadataField::Title,
            value: "Noise".to_string(),
        })
        .await;
        choose(&desk, &choice).await;
        let output = export_when_planned(&desk).await;

        assert_eq!(
            output.extension().and_then(|e| e.to_str()),
            Some(choice.extension)
        );
        assert_eq!(stream_fact(&output, "format=format_name"), choice.container);
        assert_eq!(stream_fact(&output, "stream=codec_name"), choice.codec);
        assert_eq!(
            stream_fact(&output, "stream=sample_rate"),
            choice.declared_rate
        );
        assert_eq!(
            stream_fact(&output, "stream=channels"),
            choice.declared_channels
        );
        let wanted = f64::from(choice.kbps);
        let got = packet_kbps(&output);
        assert!(
            (got - wanted).abs() <= wanted * VARIABLE_BITRATE_TOLERANCE,
            "{:?} wanted {wanted} kbps, got {got}",
            choice.format
        );
    }
}
