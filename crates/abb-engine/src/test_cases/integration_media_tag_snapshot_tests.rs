//! Tag output pinned for interop. Sources seeded with every tag alias and
//! several pictures go through each way ABB writes tags, and what the file
//! then carries is compared with a committed snapshot. MP4 Saves also pin the
//! file's SHA-256. A refactor of tag writing must leave every snapshot
//! identical; `ABB_BLESS_TAG_SNAPSHOTS=1` rewrites them on purpose.

use super::media_fixtures::{
    native_encoder_settings, preflight_payload, run_planned_title, single_title_payload,
};
use super::tag_interop_dump::{assert_snapshot, dump_tags, sha256_hex};
use super::tag_interop_seed::{
    seed_flac, seed_m4b, seed_mergeable_m4a, seed_mka, seed_mp3, Edit, Pictures,
};
use abb_engine::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, ChannelConfig, EncoderSettings, EncoderType,
    OutputAudio, SampleRateConfig, TitleAudioRequest,
};
use abb_engine::save_metadata_intent;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn opus_settings() -> EncoderSettings {
    EncoderSettings {
        encoder_type: EncoderType::Opus,
        bitrate_mode: BitrateMode::VbrTarget,
        channels: ChannelConfig::Auto,
        ..native_encoder_settings()
    }
}

fn output_audio(format: AudiobookFormat) -> OutputAudio {
    match format {
        AudiobookFormat::M4b => OutputAudio::M4b {
            settings: native_encoder_settings(),
        },
        other => OutputAudio::new(other, opus_settings()),
    }
}

struct Seeds {
    _dir: TempDir,
    m4b: PathBuf,
    mp3: PathBuf,
    flac: PathBuf,
    mka: PathBuf,
}

impl Seeds {
    fn new() -> Self {
        let dir = TempDir::new().expect("seed workspace");
        let root = dir.path();
        Self {
            m4b: seed_m4b(&root.join("m4b")),
            mp3: seed_mp3(&root.join("mp3")),
            flac: seed_flac(&root.join("flac")),
            mka: seed_mka(&root.join("mka"), Pictures::Mixed),
            _dir: dir,
        }
    }

    fn all(&self) -> [(&'static str, &Path); 4] {
        [
            ("m4b", &self.m4b),
            ("mp3", &self.mp3),
            ("flac", &self.flac),
            ("mka", &self.mka),
        ]
    }
}

fn save_snapshot(seed: &Path) -> String {
    let extension = seed
        .extension()
        .and_then(|extension| extension.to_str())
        .expect("seed extension");
    let work = TempDir::new().expect("save workspace");
    let mut out = format!("# seed\n{}", dump_tags(seed));
    for edit in Edit::ALL {
        let copy = work.path().join(format!("{}.{extension}", edit.label()));
        fs::copy(seed, &copy).expect("copy the seed");
        save_metadata_intent(&copy, &edit.patch()).expect("save the edit");
        writeln!(out, "\n# save {}", edit.label()).expect("write to a String");
        if extension == "m4b" {
            let bytes = fs::read(&copy).expect("read the saved file");
            writeln!(out, "sha256: {}", sha256_hex(&bytes)).expect("write to a String");
        }
        out.push_str(&dump_tags(&copy));
    }
    out
}

#[test]
fn saving_an_m4b_leaves_the_snapshotted_atoms_and_bytes() {
    let seeds = Seeds::new();
    assert_snapshot("save-m4b", &save_snapshot(&seeds.m4b));
}

#[test]
fn saving_an_mp3_leaves_the_snapshotted_frames_and_keys() {
    let seeds = Seeds::new();
    assert_snapshot("save-mp3", &save_snapshot(&seeds.mp3));
}

#[test]
fn saving_a_flac_leaves_the_snapshotted_keys() {
    let seeds = Seeds::new();
    assert_snapshot("save-flac", &save_snapshot(&seeds.flac));
}

#[test]
fn saving_an_mka_leaves_the_snapshotted_keys() {
    let seeds = Seeds::new();
    assert_snapshot("save-mka", &save_snapshot(&seeds.mka));
}

fn request(output: OutputAudio, intent: AudioIntent) -> TitleAudioRequest {
    TitleAudioRequest {
        output,
        intent,
        sample_rate: SampleRateConfig::Auto,
    }
}

async fn export_section(
    sources: &[&Path],
    output: OutputAudio,
    intent: AudioIntent,
    edit: Edit,
    work: &Path,
) -> String {
    let label = format!("{}-{}", output.format().extension(), edit.label());
    let anchor = sources[0];
    let out_dir = work.join(format!("out-{label}"));
    let payload = match sources {
        [only] => single_title_payload(only, &out_dir, request(output, intent)),
        _ => {
            fs::create_dir_all(&out_dir).expect("output folder");
            serde_json::from_value(serde_json::json!({
                "inputFiles": [anchor],
                "titleSources": {anchor.to_str().expect("utf8 path"): sources.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
                "audioRequests": [request(output, intent)],
                "outputDir": out_dir, "aacDecoder": "auto"
            }))
            .expect("grouped title request")
        }
    };
    let metadata = (edit != Edit::Untouched)
        .then(|| HashMap::from([(anchor.to_string_lossy().into_owned(), edit.patch())]));
    let plan = preflight_payload(payload.clone(), metadata.clone(), false)
        .unwrap_or_else(|error| panic!("{label}: preflight: {error:?}"));
    let handling = if plan.audio_plans[0].handling.preserves() {
        "preserve"
    } else {
        "encode"
    };
    let originals: Vec<_> = sources
        .iter()
        .map(|source| fs::read(source).expect("read source"))
        .collect();
    let (output, _) = run_planned_title(payload, metadata, &work.join(format!("work-{label}")))
        .await
        .unwrap_or_else(|error| panic!("{label}: {error}"));
    for (source, original) in sources.iter().zip(originals) {
        assert_eq!(
            fs::read(source).expect("reread source"),
            original,
            "{label}: source untouched"
        );
    }
    format!(
        "# {} {label}\nhandling: {handling}\n{}",
        anchor
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default(),
        dump_tags(&output)
    )
}

async fn encode_snapshot(seeds: &Seeds, format: AudiobookFormat) -> String {
    let work = TempDir::new().expect("encode workspace");
    let mut sections = Vec::new();
    for (name, seed) in seeds.all() {
        let source = work
            .path()
            .join(format!("{name}-source"))
            .join(seed.file_name().expect("seed name"));
        fs::create_dir_all(source.parent().expect("source folder")).expect("source folder");
        fs::copy(seed, &source).expect("copy the seed");
        for edit in Edit::ALL {
            sections.push(
                export_section(
                    &[&source],
                    output_audio(format),
                    AudioIntent::Encode,
                    edit,
                    &work.path().join(name),
                )
                .await,
            );
        }
    }
    sections.join("\n")
}

#[tokio::test]
async fn encoding_to_m4b_leaves_the_snapshotted_atoms_and_keys() {
    let seeds = Seeds::new();
    assert_snapshot(
        "encode-m4b",
        &encode_snapshot(&seeds, AudiobookFormat::M4b).await,
    );
}

#[tokio::test]
async fn encoding_to_m4a_opus_leaves_the_snapshotted_atoms_and_keys() {
    let seeds = Seeds::new();
    assert_snapshot(
        "encode-m4a-opus",
        &encode_snapshot(&seeds, AudiobookFormat::M4aOpus).await,
    );
}

#[tokio::test]
async fn encoding_to_mka_opus_leaves_the_snapshotted_keys() {
    let seeds = Seeds::new();
    assert_snapshot(
        "encode-mka-opus",
        &encode_snapshot(&seeds, AudiobookFormat::MkaOpus).await,
    );
}

async fn preserve_snapshot(sources: &[PathBuf], formats: &[AudiobookFormat]) -> String {
    let work = TempDir::new().expect("preserve workspace");
    let refs: Vec<&Path> = sources.iter().map(PathBuf::as_path).collect();
    let mut sections = Vec::new();
    for format in formats {
        for edit in Edit::ALL {
            sections.push(
                export_section(
                    &refs,
                    output_audio(*format),
                    AudioIntent::Preserve,
                    edit,
                    work.path(),
                )
                .await,
            );
        }
    }
    sections.join("\n")
}

#[tokio::test]
async fn a_preserved_m4b_remux_leaves_the_snapshotted_atoms() {
    let seeds = Seeds::new();
    assert_snapshot(
        "preserve-m4b",
        &preserve_snapshot(std::slice::from_ref(&seeds.m4b), &[AudiobookFormat::M4b]).await,
    );
}

#[tokio::test]
async fn a_preserved_opus_file_keeps_its_keys_in_the_same_container() {
    let seeds = Seeds::new();
    assert_snapshot(
        "preserve-mka-opus",
        &preserve_snapshot(
            std::slice::from_ref(&seeds.mka),
            &[AudiobookFormat::MkaOpus],
        )
        .await,
    );
}

#[tokio::test]
async fn a_preserved_opus_file_changing_container_leaves_the_snapshotted_atoms() {
    let dir = TempDir::new().expect("jpeg opus workspace");
    let source = seed_mka(&dir.path().join("mka"), Pictures::JpegOnly);
    assert_snapshot(
        "preserve-mka-opus-to-m4a",
        &preserve_snapshot(&[source], &[AudiobookFormat::M4aOpus]).await,
    );
}

#[tokio::test]
async fn a_preserved_m4b_merge_leaves_the_snapshotted_atoms() {
    let dir = TempDir::new().expect("merge workspace");
    let sources = [
        seed_mergeable_m4a(&dir.path().join("first"), "first"),
        seed_mergeable_m4a(&dir.path().join("second"), "second"),
    ];
    assert_snapshot(
        "merge-m4b",
        &preserve_snapshot(&sources, &[AudiobookFormat::M4b]).await,
    );
}
