//! Real AAX/AAXC decrypt through the bundled FFmpeg.
//!
//! Fixtures are synthesized at test time from a lavfi sine book. Compare the
//! decrypt output against that generated source (packets, duration, tags,
//! chapters, cover), never against fixed hashes. A wrong AAX activation fails
//! at open. A wrong AAXC key opens and copies noise; materialization must
//! reject that output.

use std::path::{Path, PathBuf};

use secrecy::SecretBox;
use tempfile::TempDir;

use super::super::decrypt::{stream_copy, CopyControl};
use super::{
    materialize_protected_download, staged_materialized_path, AudibleAcquisitionError,
    TitleAcquisitionCtx, TitleProgressContext,
};
use crate::errors::AppError;
use crate::remote_source::{RemoteAcquisitionFailureKind, RemoteSourceDiagnostic};
use crate::test_cases::encrypted_audible_books::{
    books, edit_list_books, hex_16, hex_bytes, AAXC_IV_HEX, AAXC_KEY_HEX, AAX_ACTIVATION_HEX,
    AAX_WRONG_ACTIVATION_HEX, EDIT_LIST_PLAYED_SAMPLES, WRONG_AAXC_KEY_HEX,
};
use crate::test_cases::media_fixtures::{audio_packet_bytes, ffmpeg, ffprobe, ffprobe_tag_ci};
use abb_audible_core::{AudibleDecryptionMaterial, DrmBoxes};

fn ffprobe_json(path: &Path, extra: &[&str]) -> serde_json::Value {
    let output = ffprobe(
        ["-v", "quiet", "-print_format", "json"]
            .into_iter()
            .chain(extra.iter().copied())
            .chain([path.to_str().expect("utf8 path")]),
    );
    serde_json::from_slice(&output).expect("ffprobe json")
}

fn cover_bytes(path: &Path) -> Vec<u8> {
    let mut input = ffmpeg_next::format::input(path).expect("open book");
    let index = input
        .streams()
        .find(|stream| {
            stream
                .disposition()
                .contains(ffmpeg_next::format::stream::Disposition::ATTACHED_PIC)
        })
        .expect("cover stream")
        .index();
    input
        .packets()
        .find(|(stream, _)| stream.index() == index)
        .and_then(|(_, packet)| packet.data().map(<[u8]>::to_vec))
        .expect("cover packet")
}

fn assert_matches_source(output: &Path, source: &Path) {
    assert_eq!(
        audio_packet_bytes(output),
        audio_packet_bytes(source),
        "audio packets must match the generated source"
    );
    assert_eq!(
        cover_bytes(output),
        cover_bytes(source),
        "cover picture must match the generated source"
    );

    let out_format = ffprobe_json(output, &["-show_format", "-show_chapters"]);
    let src_format = ffprobe_json(source, &["-show_format", "-show_chapters"]);
    let out_duration: f64 = out_format["format"]["duration"]
        .as_str()
        .expect("output duration")
        .parse()
        .expect("output duration f64");
    let src_duration: f64 = src_format["format"]["duration"]
        .as_str()
        .expect("source duration")
        .parse()
        .expect("source duration f64");
    assert!(
        (out_duration - src_duration).abs() < 0.05,
        "duration {out_duration} vs {src_duration}"
    );

    let out_tags = out_format["format"]["tags"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let src_tags = src_format["format"]["tags"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for key in ["title", "artist", "album", "genre"] {
        assert_eq!(
            ffprobe_tag_ci(&out_tags, key),
            ffprobe_tag_ci(&src_tags, key),
            "tag {key}"
        );
    }

    let out_chapters = out_format["chapters"].as_array().expect("output chapters");
    let src_chapters = src_format["chapters"].as_array().expect("source chapters");
    assert_eq!(out_chapters.len(), src_chapters.len(), "chapter count");
    for (out_chapter, src_chapter) in out_chapters.iter().zip(src_chapters) {
        assert_eq!(
            ffprobe_tag_ci(
                out_chapter["tags"]
                    .as_object()
                    .unwrap_or(&serde_json::Map::new()),
                "title"
            ),
            ffprobe_tag_ci(
                src_chapter["tags"]
                    .as_object()
                    .unwrap_or(&serde_json::Map::new()),
                "title"
            )
        );
        assert_eq!(out_chapter["start_time"], src_chapter["start_time"]);
        assert_eq!(out_chapter["end_time"], src_chapter["end_time"]);
    }
}

fn played_samples(path: &Path) -> usize {
    let pcm = ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-i",
        path.to_str().expect("utf8 path"),
        "-map",
        "0:a",
        "-f",
        "s16le",
        "-",
    ]);
    pcm.len() / 4
}

fn aax_material(activation_bytes_hex: &str) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aax {
        activation_bytes: SecretBox::new(Box::new(
            hex_bytes(activation_bytes_hex)
                .try_into()
                .expect("4-byte hex"),
        )),
    }
}

fn aaxc_material(key_hex: &str) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aaxc {
        key: SecretBox::new(Box::new(hex_16(key_hex))),
        iv: SecretBox::new(Box::new(hex_16(AAXC_IV_HEX))),
    }
}

#[derive(Debug)]
struct ProtectedMaterializationRun {
    pub output: PathBuf,
    pub partial: PathBuf,
}

type MaterializationFailure = Box<(RemoteSourceDiagnostic, TempDir, ProtectedMaterializationRun)>;

async fn materialize_protected(
    source: &Path,
    download_name: &str,
    material: AudibleDecryptionMaterial,
) -> Result<(TempDir, ProtectedMaterializationRun), MaterializationFailure> {
    let root = TempDir::new().expect("protected download root");
    let input = root.path().join(download_name);
    std::fs::copy(source, &input).expect("copy protected download");
    match run_protected_materialization(&input, root.path(), &material, &|| false).await {
        (Ok(()), run) => Ok((root, run)),
        (Err(error), run) => Err(Box::new((
            AudibleAcquisitionError::materialization(error)
                .into_diagnostic(Some(TITLE_ID.to_string())),
            root,
            run,
        ))),
    }
}

const TITLE_ID: &str = "B0DECRYPT";

async fn run_protected_materialization(
    downloaded_path: &Path,
    item_dir: &Path,
    material: &AudibleDecryptionMaterial,
    is_cancelled: &impl Fn() -> bool,
) -> (Result<(), AppError>, ProtectedMaterializationRun) {
    let title_name = "Synthetic Book";
    let output = staged_materialized_path(item_dir, Some(title_name), TITLE_ID);
    let run = ProtectedMaterializationRun {
        partial: crate::remote_source::scoped_output::partial_sibling(&output),
        output: output.clone(),
    };
    let ctx = TitleAcquisitionCtx {
        job_id: "decrypt-job",
        title_id: TITLE_ID,
        item_dir,
        progress_context: TitleProgressContext {
            title_id: TITLE_ID,
            item_index: 1,
            total_items: 1,
        },
    };
    let result = materialize_protected_download(
        downloaded_path,
        Some(title_name),
        material,
        ctx,
        &mut |_| {},
        is_cancelled,
    )
    .await
    .map(|source| assert_eq!(source.path, output, "materialized path"));
    (result, run)
}

fn assert_message_holds_no_secret(message: &str, secrets: &[&str]) {
    let message = message.to_ascii_lowercase();
    for secret in secrets {
        assert!(!message.contains(secret), "message must not hold a secret");
    }
    assert!(!message.contains('/'), "message must not include a path");
}

#[test]
fn the_probe_reads_each_lane_brand_and_drm_box_from_the_encrypted_books() {
    let books = books();
    let read =
        crate::remote_source::providers::audible::protected_file_probe::read_protected_file_facts;

    let aax = read(&books.aax).expect("aax facts");
    let aaxc = read(&books.aaxc).expect("aaxc facts");

    assert_eq!(
        (aax.major_brand.as_str(), aax.drm_boxes),
        (
            "aax ",
            Some(DrmBoxes {
                adrm_present: true,
                aavd_entry_present: true
            })
        )
    );
    assert_eq!(
        (aaxc.major_brand.as_str(), aaxc.drm_boxes),
        (
            "aaxc",
            Some(DrmBoxes {
                adrm_present: false,
                aavd_entry_present: true
            })
        )
    );
}

#[tokio::test]
async fn aax_decrypts_to_the_generated_source() {
    let books = books();
    let (_root, run) =
        materialize_protected(&books.aax, "source.aax", aax_material(AAX_ACTIVATION_HEX))
            .await
            .unwrap_or_else(|failure| panic!("aax decrypt failed: {}", failure.0.message));
    assert_matches_source(&run.output, &books.source);
}

#[tokio::test]
async fn aaxc_decrypts_to_the_generated_source() {
    let books = books();
    let (_root, run) =
        materialize_protected(&books.aaxc, "source.aaxc", aaxc_material(AAXC_KEY_HEX))
            .await
            .unwrap_or_else(|failure| panic!("aaxc decrypt failed: {}", failure.0.message));
    assert_matches_source(&run.output, &books.source);
}

#[tokio::test]
async fn decrypt_plays_exactly_the_samples_the_source_edit_list_plays() {
    let books = edit_list_books();
    assert_eq!(
        played_samples(&books.source),
        EDIT_LIST_PLAYED_SAMPLES,
        "the source must play its length, not its whole frames"
    );
    let lanes = [
        (&books.aax, "source.aax", aax_material(AAX_ACTIVATION_HEX)),
        (&books.aaxc, "source.aaxc", aaxc_material(AAXC_KEY_HEX)),
    ];
    for (book, name, material) in lanes {
        let (_root, run) = materialize_protected(book, name, material)
            .await
            .unwrap_or_else(|failure| panic!("decrypt failed: {}", failure.0.message));
        assert_eq!(
            played_samples(&run.output),
            EDIT_LIST_PLAYED_SAMPLES,
            "{name}: decrypt must keep the source's played length"
        );
    }
}

#[tokio::test]
async fn wrong_aax_activation_bytes_fail_and_leave_no_output() {
    let books = books();
    let (diagnostic, _root, run) = *materialize_protected(
        &books.aax,
        "source.aax",
        aax_material(AAX_WRONG_ACTIVATION_HEX),
    )
    .await
    .expect_err("wrong activation bytes must fail");
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::MaterializationFailed
    );
    assert!(
        diagnostic.message.contains("opening the download"),
        "wrong activation bytes must be refused when the file opens: {}",
        diagnostic.message
    );
    assert_message_holds_no_secret(
        &diagnostic.message,
        &[AAX_WRONG_ACTIVATION_HEX, AAX_ACTIVATION_HEX],
    );
    assert!(!run.output.exists(), "no committed output");
    assert!(!run.partial.exists(), "no partial output");
}

#[tokio::test]
async fn wrong_aaxc_key_is_rejected_by_the_decodes_as_audio_gate() {
    let books = books();
    let (diagnostic, _root, run) = *materialize_protected(
        &books.aaxc,
        "source.aaxc",
        aaxc_material(WRONG_AAXC_KEY_HEX),
    )
    .await
    .expect_err("wrong AAXC key must fail at materialization, not return Ok for later validation");
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::MaterializationFailed
    );
    assert!(
        diagnostic.message.contains("decrypt"),
        "message must name the decrypt: {}",
        diagnostic.message
    );
    assert!(
        diagnostic.message.contains("decodable audio"),
        "message must say the decrypt output is not audio: {}",
        diagnostic.message
    );
    assert_message_holds_no_secret(
        &diagnostic.message,
        &[WRONG_AAXC_KEY_HEX, AAXC_KEY_HEX, AAXC_IV_HEX],
    );
    assert!(!run.output.exists(), "no committed m4b");
    assert!(!run.partial.exists(), "no partial");
}

#[tokio::test]
async fn a_download_that_does_not_match_the_license_lane_is_refused() {
    let books = books();
    let cases = [
        (&books.aax, "source.aax", aaxc_material(AAXC_KEY_HEX)),
        (&books.aaxc, "source.aaxc", aax_material(AAX_ACTIVATION_HEX)),
        (&books.source, "source.aaxc", aaxc_material(AAXC_KEY_HEX)),
    ];
    for (book, name, material) in cases {
        let (diagnostic, _root, run) = *materialize_protected(book, name, material)
            .await
            .expect_err("a lane mismatch must fail");
        assert_eq!(
            diagnostic.kind,
            RemoteAcquisitionFailureKind::MaterializationFailed
        );
        assert!(
            diagnostic.message.contains("does not match"),
            "message must say the lane does not match: {}",
            diagnostic.message
        );
        assert_message_holds_no_secret(
            &diagnostic.message,
            &[AAXC_KEY_HEX, AAXC_IV_HEX, AAX_ACTIVATION_HEX],
        );
        assert!(!run.output.exists(), "no committed output");
        assert!(!run.partial.exists(), "no partial output");
    }
}

#[tokio::test]
async fn cancel_leaves_no_output_and_no_partial() {
    let books = books();
    let root = TempDir::new().expect("cancel root");
    let input = root.path().join("source.aaxc");
    std::fs::copy(&books.aaxc, &input).expect("copy protected download");
    let polls = std::cell::Cell::new(0_u32);
    let cancel_after_start = || {
        polls.set(polls.get() + 1);
        polls.get() > 1
    };

    let (result, run) = run_protected_materialization(
        &input,
        root.path(),
        &aaxc_material(AAXC_KEY_HEX),
        &cancel_after_start,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "cancel should stop decrypt, got {result:?}"
    );
    assert!(!run.output.exists(), "no committed output");
    assert!(!run.partial.exists(), "no partial output");
}

#[test]
fn the_stream_copy_stops_at_the_next_packet_when_told_to() {
    let books = books();
    let dir = TempDir::new().expect("stop dir");
    let output = dir.path().join("out.m4b");
    let control = CopyControl::default();
    control.request_stop();

    let result = stream_copy(&books.source, &output, &[], &control);

    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "{result:?}"
    );
    assert_eq!(control.fraction(), 0.0, "no packet was copied");
    assert!(!output.exists(), "the stopped copy removes its own partial");
}

const LEAK_CHILD_ENV: &str = "ABB_DECRYPT_LEAK_CHILD";

struct StderrLogger;

impl log::Log for StderrLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        eprintln!("{} {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

static STDERR_LOGGER: StderrLogger = StderrLogger;

fn leak_child_test_name() -> String {
    let module = module_path!();
    let module = module.split_once("::").map_or(module, |(_, rest)| rest);
    format!("{module}::decrypt_cases_run_for_the_leak_check")
}

/// The child half of `decrypting_never_prints_a_secret_or_the_file_checksum`.
/// It does nothing unless that test started it, so the parent can read all
/// the output of this process, FFmpeg's and every `log` level included.
#[tokio::test]
async fn decrypt_cases_run_for_the_leak_check() {
    if std::env::var_os(LEAK_CHILD_ENV).is_none() {
        return;
    }
    crate::audio::limit_ffmpeg_logging();
    log::set_logger(&STDERR_LOGGER).expect("install the leak check logger");
    log::set_max_level(log::LevelFilter::Trace);
    let books = books();
    let junk = TempDir::new().expect("junk dir");
    let not_mp4 = junk.path().join("not-mp4.bin");
    std::fs::write(&not_mp4, vec![0x5a_u8; 4096]).expect("write junk");
    let lanes = [
        (
            &books.aax,
            aax_material(AAX_ACTIVATION_HEX),
            aax_material(AAX_WRONG_ACTIVATION_HEX),
        ),
        (
            &books.aaxc,
            aaxc_material(AAXC_KEY_HEX),
            aaxc_material(WRONG_AAXC_KEY_HEX),
        ),
    ];
    for (book, right, wrong) in lanes {
        let accepted = |outcome: Result<_, MaterializationFailure>| outcome.is_ok();
        assert!(accepted(
            materialize_protected(book, "book.enc", right).await
        ));
        assert!(!accepted(
            materialize_protected(book, "book.enc", wrong).await
        ));
    }
    for material in [
        aax_material(AAX_ACTIVATION_HEX),
        aaxc_material(AAXC_KEY_HEX),
    ] {
        let outcome = materialize_protected(&not_mp4, "book.enc", material).await;
        assert!(outcome.is_err());
    }
}

#[test]
fn decrypting_never_prints_a_secret_or_the_file_checksum() {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            &leak_child_test_name(),
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LEAK_CHILD_ENV, "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run the decrypt child");
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();

    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "the child ran its decrypt cases: {stdout} {stderr}"
    );
    assert!(
        stderr.contains("stage=protected_file_facts"),
        "the child's log records reach stderr: {stderr}"
    );
    let printed = format!("{stdout}{stderr}");
    for secret in [
        AAXC_KEY_HEX,
        AAXC_IV_HEX,
        WRONG_AAXC_KEY_HEX,
        AAX_ACTIVATION_HEX,
        AAX_WRONG_ACTIVATION_HEX,
        "file checksum",
    ] {
        assert!(
            !printed.contains(&secret.to_ascii_lowercase()),
            "the child printed a secret or the AAX file checksum"
        );
    }
}
