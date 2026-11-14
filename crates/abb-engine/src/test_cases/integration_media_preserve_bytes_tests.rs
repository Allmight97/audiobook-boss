//! The two ways a single M4B is preserved, pinned down to the output file's
//! bytes: the copy (chapters already match, so the staged copy only gets its
//! tags written) and the remux (chapters differ, so FFmpeg rewrites the
//! container). Sources are the committed untagged M4B plus tags and chapters
//! written by mp4ameta, so the bytes do not depend on the FFmpeg that is
//! installed. `ABB_BLESS_TAG_SNAPSHOTS=1` rewrites the snapshots on purpose.
//! An encode is not snapshotted, because its bytes follow the linked FFmpeg;
//! two encodes of one source must still agree.

use super::media_fixtures::{
    chapters_of, preflight_payload, run_planned_title, single_title_payload,
};
use super::support::encoders::native_encoder_settings;
use super::tag_interop_dump::{assert_snapshot, dump_tags, sha256_hex};
use super::tag_interop_seed::{seed_m4b, Edit};
use crate::audio::{
    get_file_list_info, AacDecoder, AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest,
};
use mp4ameta::{Chapter, Tag, WriteConfig};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;

fn chaptered_seed(dir: &Path, chapters: &[(u64, &str)]) -> PathBuf {
    let path = seed_m4b(dir);
    let mut tag = Tag::read_from_path(&path).expect("read the seed tags");
    *tag.chapter_list_mut() = chapters
        .iter()
        .map(|(start_ms, title)| Chapter::new(Duration::from_millis(*start_ms), *title))
        .collect();
    tag.write_with_path(
        &path,
        &WriteConfig {
            write_meta_items: true,
            write_chapter_list: true,
            ..WriteConfig::NONE
        },
    )
    .expect("write the seed chapters");
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

async fn encoded_bytes(source: &Path, work: &Path) -> Vec<u8> {
    let payload = single_title_payload(
        source,
        &work.join("out"),
        TitleAudioRequest {
            output: OutputAudio::M4b {
                settings: native_encoder_settings(),
            },
            intent: AudioIntent::Encode,
            sample_rate: SampleRateConfig::Auto,
        },
    );
    let (output, _) = run_planned_title(payload, None, &work.join("work"))
        .await
        .expect("encode");
    fs::read(output).expect("read output")
}

#[tokio::test]
async fn the_same_encode_twice_writes_the_same_bytes() {
    let dir = TempDir::new().expect("seed workspace");
    let source = chaptered_seed(dir.path(), &[(0, "One"), (250, "Two")]);
    let first = encoded_bytes(&source, &dir.path().join("first")).await;
    std::thread::sleep(Duration::from_millis(1100));
    let second = encoded_bytes(&source, &dir.path().join("second")).await;
    assert!(
        first == second,
        "two encodes of one source differ: sha256 {} and {}",
        sha256_hex(&first),
        sha256_hex(&second)
    );
}
