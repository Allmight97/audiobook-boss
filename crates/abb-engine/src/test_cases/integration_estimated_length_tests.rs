use super::integration_faac_execution_tests::faac_encoder_settings;
use super::integration_media_execution_tests::{
    audio_duration_ts, chapters_of, decode_pcm_f32, mux_chapters, write_sine_mp3_with,
    write_sine_wav, MediaLane, SAMPLE_RATE,
};
use abb_engine::audio::{execute_audio_engine, get_file_list_info, AacDecoder};
use abb_engine::processing::ProcessingSession;
use abb_engine::{read_metadata, save_metadata_intent, MetadataIntentPatch, PatchOp};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[tokio::test]
async fn a_merge_after_an_estimated_length_file_starts_the_next_chapter_at_the_real_join() {
    let tmp = TempDir::new().expect("estimated merge workspace");
    let (first, second) = write_mp3_parts(tmp.path());
    let joined = tmp.path().join("joined.mp3");
    let mut bytes = fs::read(&first).expect("read first part");
    bytes.extend(fs::read(&second).expect("read second part"));
    fs::write(&joined, bytes).expect("join parts");
    let vbr = tmp.path().join("vbr-headerless.mp3");
    write_vbr_mp3_without_header(&vbr);
    let durationless = tmp.path().join("durationless.mka");
    remux_without_duration(&vbr, &durationless);
    let he_adts = tmp.path().join("he.aac");
    write_he_adts(&he_adts).await;

    for (estimated, decoder) in [
        (joined, AacDecoder::Auto),
        (vbr, AacDecoder::Auto),
        (durationless, AacDecoder::Auto),
        (he_adts, AacDecoder::Faad),
    ] {
        assert!(
            crate::metadata::length_is_estimated(
                &ffmpeg_next::format::input(&estimated).expect("open estimated input")
            ),
            "fixture premise: FFmpeg only estimates {estimated:?}"
        );
        let tone = tmp.path().join("tone.wav");
        write_sine_wav(&tone, 8.0, 330.0);
        let lane = MediaLane::for_inputs(vec![estimated.clone(), tone]).with_aac_decoder(decoder);
        execute_audio_engine(lane.execution_request(ProcessingSession::new(), None))
            .await
            .expect("export a merge after an estimated-length file");

        let chapters = chapters_of(&lane.output_path());
        assert_eq!(chapters.len(), 2, "{estimated:?}: {chapters:?}");
        let join_ms = (audio_duration_ts(&lane.output_path()) as f64 * 1000.0
            / f64::from(SAMPLE_RATE))
        .round() as i64
            - 8_000;
        assert!(
            (chapters[1].1 - join_ms).abs() <= 2,
            "{estimated:?}: chapter 2 starts where the output's audio joins ({join_ms} ms): {chapters:?}"
        );
        if decoder == AacDecoder::Auto {
            let source_ms = decoded_ms(&estimated);
            assert!(
                (join_ms - source_ms).abs() <= 2,
                "{estimated:?}: the join falls after the whole source ({source_ms} ms)"
            );
        }
    }
}

async fn write_he_adts(path: &Path) {
    let lane = MediaLane::with_fixtures(&[6.0]).with_encoder(faac_encoder_settings());
    let encoded = lane.process(None).await;
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let output = std::process::Command::new(binary)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(&encoded)
        .args(["-map", "0:a:0", "-c:a", "copy", "-f", "adts", "-y"])
        .arg(path)
        .output()
        .expect("remux HE-AAC to ADTS");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn decoded_ms(path: &Path) -> i64 {
    (decode_pcm_f32(path).len() as f64 * 1000.0 / f64::from(SAMPLE_RATE)).round() as i64
}

fn write_vbr_mp3_without_header(path: &Path) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let quiet_then_loud =
        format!("aevalsrc='if(lt(t,2),0,0.8*sin(2*PI*440*t)*random(0))':s={SAMPLE_RATE}:d=6");
    let output = std::process::Command::new(binary)
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i"])
        .arg(quiet_then_loud)
        .args([
            "-c:a",
            "libmp3lame",
            "-q:a",
            "0",
            "-write_xing",
            "0",
            "-id3v2_version",
            "0",
            "-y",
        ])
        .arg(path)
        .output()
        .expect("encode headerless VBR MP3");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn remux_without_duration(source: &Path, path: &Path) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let output = std::process::Command::new(binary)
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(source)
        .args(["-c", "copy", "-f", "matroska", "pipe:1"])
        .output()
        .expect("remux to streamed Matroska");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::write(path, output.stdout).expect("write durationless Matroska");
}

/// A 1 s MP3 with its length header and a 2 s raw MP3 to append to it, which
/// is what joining tools leave.
fn write_mp3_parts(dir: &Path) -> (PathBuf, PathBuf) {
    let first = dir.join("part-1.mp3");
    let second = dir.join("part-2.mp3");
    write_sine_mp3_with(&first, 1.0, 440.0, &["-ar", "44100", "-b:a", "32k"]);
    write_sine_mp3_with(
        &second,
        2.0,
        660.0,
        &[
            "-ar",
            "44100",
            "-b:a",
            "160k",
            "-write_xing",
            "0",
            "-id3v2_version",
            "0",
        ],
    );
    (first, second)
}

#[test]
fn a_joined_mp3_imports_with_its_chapters_and_saves_its_tags() {
    let tmp = TempDir::new().expect("joined MP3 workspace");
    let (first, second) = write_mp3_parts(tmp.path());
    let chaptered = tmp.path().join("part-1-chapters.mp3");
    mux_chapters(
        &first,
        &[("Opening", 0, 1_000), ("Closing", 1_000, 3_000)],
        &chaptered,
    );
    let joined = tmp.path().join("joined.mp3");
    let mut bytes = fs::read(&chaptered).expect("read first part");
    bytes.extend(fs::read(&second).expect("read second part"));
    fs::write(&joined, bytes).expect("join parts");
    let imported = get_file_list_info(&[&joined], AacDecoder::Auto).expect("probe joined MP3");
    let file = &imported.files[0];
    assert!(file.is_valid, "{:?}", file.error);
    let joined_input = ffmpeg_next::format::input(&joined).expect("open joined MP3");
    assert!(
        crate::metadata::length_is_estimated(&joined_input),
        "fixture premise: FFmpeg can only estimate the joined length"
    );
    let measured = file.duration.expect("joined length");
    let decoded = decoded_ms(&joined) as f64 / 1000.0;
    assert!(
        (measured - decoded).abs() <= 0.002,
        "import measures the decoded length ({decoded} s), not FFmpeg's estimate: {measured} s"
    );
    let accepted: Vec<_> = file
        .chapter_plan
        .as_ref()
        .expect("accepted chapter plan")
        .chapters
        .iter()
        .map(|chapter| (chapter.title.clone(), chapter.start_ms, chapter.end_ms))
        .collect();
    assert_eq!(
        accepted,
        vec![
            (Some("Opening".to_string()), 0, 1_000),
            (
                Some("Closing".to_string()),
                1_000,
                (measured * 1000.0).round() as i64
            ),
        ],
        "the last chapter ends where the measured audio ends"
    );

    save_metadata_intent(
        &joined,
        &MetadataIntentPatch {
            title: Some(PatchOp::Set("Joined".into())),
            ..Default::default()
        },
    )
    .expect("joined MP3 save");
    assert_eq!(
        read_metadata(&joined)
            .expect("read saved MP3")
            .title
            .as_deref(),
        Some("Joined")
    );
}
