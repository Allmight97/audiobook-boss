//! A title that fails after its workspace exists, for a reason other than a
//! cancel, publishes nothing and leaves no workspace and no temp file, and a
//! file already at the destination keeps its bytes.

use super::media_fixtures::MediaLane;
use super::support::fs_listing::names_in;
use super::support::wav::SAMPLE_RATE;
use crate::test_cases::support::session::idle_session;

use crate::audio::{execute_audio_engine, get_file_list_info, AacDecoder, AudioExecutionRequest};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};
use crate::processing::{AudioHandling, EventStage, OutputConfig, ProcessingContext};
use crate::AppError;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn staged_files(workspace_root: &Path) -> Vec<PathBuf> {
    let Ok(sessions) = fs::read_dir(workspace_root) else {
        return Vec::new();
    };
    sessions
        .flat_map(|session| {
            fs::read_dir(session.expect("session folder").path())
                .expect("read session folder")
                .map(|entry| entry.expect("staged entry").path())
        })
        .collect()
}

#[tokio::test]
async fn an_encode_that_fails_mid_pipeline_leaves_no_workspace_and_no_output() {
    let lane = MediaLane::with_fixtures(&[3.0]);
    let info = get_file_list_info(&lane.inputs, AacDecoder::Auto).expect("probe fixture");
    let mut context = lane.context(idle_session());
    let workspace_root = lane.workspace_root();
    let sabotaged = Arc::new(AtomicBool::new(false));
    let stages = Arc::new(Mutex::new(Vec::new()));
    let (flag, seen, root) = (
        Arc::clone(&sabotaged),
        Arc::clone(&stages),
        workspace_root.clone(),
    );
    context.progress_listener = Arc::new(move |event| {
        seen.lock().expect("stages").push(event.stage);
        if event.stage != EventStage::Writing || flag.load(Ordering::SeqCst) {
            return;
        }
        if let Some(encoded) = staged_files(&root)
            .into_iter()
            .find(|path| path.extension().is_some_and(|extension| extension == "m4b"))
        {
            fs::remove_file(&encoded).expect("take the encoder output away");
            fs::create_dir(&encoded).expect("a folder where the encoder output was");
            flag.store(true, Ordering::SeqCst);
        }
    });

    let result = execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        Some(AudiobookMetadata {
            title: Some("Never Published".into()),
            ..Default::default()
        }),
        CoverArtPassthroughPolicy::Preserve,
        lane.handling(),
    ))
    .await;

    assert!(
        sabotaged.load(Ordering::SeqCst),
        "the encoded file existed when the metadata write began"
    );
    let error = result.expect_err("a title whose encoder output is gone cannot publish");
    assert!(
        !matches!(error, AppError::Cancellation(_)),
        "a failure is not a cancel: {error:?}"
    );
    assert!(!stages
        .lock()
        .expect("stages")
        .contains(&EventStage::Completed));
    assert!(!lane.output_path().exists());
    assert!(
        names_in(lane.output_path().parent().expect("output folder")).is_empty(),
        "no partial file or temp file beside the output"
    );
    assert!(lane.residual_workspace_dirs().is_empty());
}

#[tokio::test]
async fn an_encode_refused_at_publish_keeps_the_file_already_at_the_destination() {
    let lane = MediaLane::with_fixtures(&[1.0]);
    let info = get_file_list_info(&lane.inputs, AacDecoder::Auto).expect("probe fixture");
    let context = lane.context(idle_session());
    fs::write(lane.output_path(), b"a book the user already had").expect("existing book");
    let sources: Vec<_> = lane
        .inputs
        .iter()
        .map(|source| fs::read(source).expect("read source"))
        .collect();

    let error = execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        lane.handling(),
    ))
    .await
    .expect_err("a write plan never replaces what is there");

    assert!(
        error
            .to_string()
            .contains("Review collisions and try again"),
        "{error}"
    );
    assert_eq!(
        fs::read(lane.output_path()).expect("read destination"),
        b"a book the user already had"
    );
    assert_eq!(
        names_in(lane.output_path().parent().expect("output folder")),
        ["lane-output.m4b"]
    );
    assert!(lane.residual_workspace_dirs().is_empty());
    for (path, bytes) in lane.inputs.iter().zip(sources) {
        assert_eq!(fs::read(path).expect("reread source"), bytes);
    }
}

#[tokio::test]
async fn a_preserve_that_fails_after_its_workspace_exists_leaves_the_source_and_no_residue() {
    let encoded = MediaLane::with_fixtures(&[1.0]);
    let source = encoded.process(None).await;
    let lane = MediaLane::for_inputs(vec![source.clone()]);
    let info = get_file_list_info(&lane.inputs, AacDecoder::Auto).expect("probe source");
    let destination = lane.tmp.path().join("preserved.m4b");
    let context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(idle_session()),
        SAMPLE_RATE,
        OutputConfig::new(&destination),
        lane.workspace_root(),
    );
    let mut grown = fs::read(&source).expect("read source");
    grown.extend_from_slice(b"appended after inspection");
    fs::write(&source, &grown).expect("the source changes after inspection");

    let result = execute_audio_engine(AudioExecutionRequest::new(
        context,
        info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        AudioHandling::Preserve,
    ))
    .await;

    let error = result.expect_err("a source that changed since inspection is refused");
    assert!(!matches!(error, AppError::Cancellation(_)), "{error:?}");
    assert!(!destination.exists());
    assert_eq!(fs::read(&source).expect("reread source"), grown);
    assert!(lane.residual_workspace_dirs().is_empty());
}
