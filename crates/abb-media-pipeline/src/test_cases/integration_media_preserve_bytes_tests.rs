use super::media_fixtures::{
    chapters_of, preflight_payload, run_planned_title, single_title_payload,
};
use super::support::encoders::{native_encoder_settings, opus_encoder_settings};
use super::tag_interop_dump::{assert_snapshot, dump_tags, sha256_hex};
use super::tag_interop_seed::{seed_m4b, Edit};
use crate::audio::{
    get_file_list_info, AacDecoder, AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest,
};
use abb_test_fixtures::media::decode_pcm_f32;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

fn chaptered_seed(dir: &Path, chapters: &[(u64, &str)]) -> PathBuf {
    let path = seed_m4b(dir);
    crate::metadata::mp4::write_chapters(&path, chapters).expect("write the seed chapters");
    path
}

fn accepted_chapters(path: &Path) -> Vec<(Option<String>, i64)> {
    get_file_list_info(&[path], AacDecoder::Auto)
        .expect("import the seed")
        .files[0]
        .chapter_plan
        .clone()
        .expect("accepted chapter plan")
        .chapters
        .into_iter()
        .map(|chapter| (chapter.title, chapter.start_ms))
        .collect()
}

fn chapters_in_file(path: &Path) -> Vec<(Option<String>, i64)> {
    chapters_of(path)
        .into_iter()
        .map(|(title, start, _)| (title, start))
        .collect()
}

async fn preserved_section(source: &Path, edit: Edit, work: &Path) -> String {
    let label = edit.label();
    let out_dir = work.join(format!("out-{label}"));
    let payload = single_title_payload(
        source,
        &out_dir,
        TitleAudioRequest {
            output: OutputAudio::M4b {
                settings: native_encoder_settings(),
            },
            intent: AudioIntent::Preserve,
            sample_rate: SampleRateConfig::Auto,
        },
    );
    let metadata = (edit != Edit::Untouched)
        .then(|| HashMap::from([(source.to_string_lossy().into_owned(), edit.patch())]));
    let plan = preflight_payload(payload.clone(), metadata.clone(), false)
        .unwrap_or_else(|error| panic!("{label}: preflight: {error:?}"));
    assert!(plan.audio_plans[0].handling.preserves(), "{label}");
    let original = fs::read(source).expect("read source");
    let (output, _) = run_planned_title(payload, metadata, &work.join(format!("work-{label}")))
        .await
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    assert_eq!(
        fs::read(source).expect("reread source"),
        original,
        "{label}"
    );
    let bytes = fs::read(&output).expect("read output");
    let mut section = format!("# {label}\nsha256: {}\n", sha256_hex(&bytes));
    writeln!(section, "chapters: {:?}", chapters_of(&output)).expect("write to a String");
    section.push_str(&dump_tags(&output));
    section
}

async fn snapshot_of(source: &Path) -> String {
    let work = TempDir::new().expect("export workspace");
    let mut sections = Vec::new();
    for edit in [Edit::Untouched, Edit::SetAll] {
        sections.push(preserved_section(source, edit, work.path()).await);
    }
    sections.join("\n")
}

#[tokio::test]
async fn a_preserved_m4b_whose_chapters_match_is_copied_with_the_snapshotted_bytes() {
    let dir = TempDir::new().expect("seed workspace");
    let source = chaptered_seed(dir.path(), &[(0, "One"), (250, "Two")]);
    assert_eq!(
        accepted_chapters(&source),
        chapters_in_file(&source),
        "copy premise: the accepted chapters are the file's"
    );
    assert_snapshot("preserve-bytes-m4b-copy", &snapshot_of(&source).await);
}

#[tokio::test]
async fn a_preserved_m4b_whose_chapters_differ_is_remuxed_with_the_snapshotted_bytes() {
    let dir = TempDir::new().expect("seed workspace");
    let source = chaptered_seed(dir.path(), &[(0, "One"), (250, "Two"), (900, "Late")]);
    assert_ne!(
        accepted_chapters(&source),
        chapters_in_file(&source),
        "remux premise: a chapter past the audio end is dropped from the accepted plan"
    );
    assert_snapshot("preserve-bytes-m4b-remux", &snapshot_of(&source).await);
}

#[tokio::test]
async fn the_same_preserve_twice_writes_the_same_bytes() {
    let dir = TempDir::new().expect("seed workspace");
    let remux = chaptered_seed(
        &dir.path().join("remux"),
        &[(0, "One"), (250, "Two"), (900, "Late")],
    );
    let copy = chaptered_seed(&dir.path().join("copy"), &[(0, "One"), (250, "Two")]);
    let first = [snapshot_of(&remux).await, snapshot_of(&copy).await];
    std::thread::sleep(Duration::from_millis(1100));
    let second = [snapshot_of(&remux).await, snapshot_of(&copy).await];
    assert_eq!(first, second, "a clock reading reached the output bytes");
}

async fn encoded(source: &Path, output: OutputAudio, edit: Edit, work: &Path) -> PathBuf {
    let payload = single_title_payload(
        source,
        &work.join("out"),
        TitleAudioRequest {
            output,
            intent: AudioIntent::Encode,
            sample_rate: SampleRateConfig::Auto,
        },
    );
    let metadata = (edit != Edit::Untouched)
        .then(|| HashMap::from([(source.to_string_lossy().into_owned(), edit.patch())]));
    let (output, _) = run_planned_title(payload, metadata, &work.join("work"))
        .await
        .expect("encode");
    output
}

#[tokio::test]
async fn the_same_encode_twice_writes_the_same_bytes() {
    let dir = TempDir::new().expect("seed workspace");
    let source = chaptered_seed(dir.path(), &[(0, "One"), (250, "Two")]);
    let m4b = || OutputAudio::M4b {
        settings: native_encoder_settings(),
    };
    let first = fs::read(encoded(&source, m4b(), Edit::Untouched, &dir.path().join("first")).await)
        .expect("read output");
    std::thread::sleep(Duration::from_millis(1100));
    let second =
        fs::read(encoded(&source, m4b(), Edit::Untouched, &dir.path().join("second")).await)
            .expect("read output");
    assert!(
        first == second,
        "two encodes of one source differ: sha256 {} and {}",
        sha256_hex(&first),
        sha256_hex(&second)
    );
}

fn held_content(path: &Path) -> String {
    let audio: Vec<u8> = decode_pcm_f32(path)
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    format!(
        "decoded audio: {} samples, sha256 {}\nchapters: {:?}\n{}",
        audio.len() / 4,
        sha256_hex(&audio),
        chapters_of(path),
        dump_tags(path)
    )
}

#[tokio::test]
async fn the_same_mka_opus_encode_twice_holds_the_same_content() {
    let dir = TempDir::new().expect("seed workspace");
    let source = chaptered_seed(dir.path(), &[(0, "One"), (250, "Two")]);
    let mka = || OutputAudio::MkaOpus {
        settings: opus_encoder_settings(),
    };
    let first = encoded(&source, mka(), Edit::SetAll, &dir.path().join("first")).await;
    std::thread::sleep(Duration::from_millis(1100));
    let second = encoded(&source, mka(), Edit::SetAll, &dir.path().join("second")).await;
    let first_content = held_content(&first);
    assert!(
        first_content.contains(r#"title="Set Title""#) && !chapters_of(&first).is_empty(),
        "premise: the export carries the set tags and the chapters"
    );
    assert_eq!(
        first_content,
        held_content(&second),
        "two Matroska Opus encodes of one source hold different content"
    );
}
