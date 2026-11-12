use crate::audio::AudioHandling;
use crate::audio::{AacDecoder, AudioExecutionRequest};
use crate::errors::AppError;
use crate::execution::{OutputConfig, ProcessingContext};
use crate::metadata::CoverArtPassthroughPolicy;
use crate::test_cases::support::session::session_with;
use crate::test_cases::support::wav::SAMPLE_RATE;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn cancelling_during_a_keep_audio_join_leaves_no_output_or_workspace_residue() {
    let tmp = tempfile::tempdir().expect("join fixture");
    let sources = [tmp.path().join("part-1.m4a"), tmp.path().join("part-2.m4a")];
    for (source, frequency) in sources.iter().zip([440, 880]) {
        crate::test_cases::media_fixtures::write_complete_frame_aac(source, frequency);
    }
    let original_bytes: Vec<_> = sources
        .iter()
        .map(|source| std::fs::read(source).expect("read source"))
        .collect();
    let destination = tmp.path().join("out").join("Joined.m4b");
    std::fs::create_dir_all(destination.parent().expect("output parent")).expect("output dir");
    let workspace = tmp.path().join("workspace");
    let cancelled = CancellationToken::new();
    let mut context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(session_with(cancelled.clone())),
        SAMPLE_RATE,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    // Joining is announced once before the first packet and again after each
    // source; cancel after the first source's packets are in the partial join.
    let joining_events = Arc::new(AtomicUsize::new(0));
    let listener_events = Arc::clone(&joining_events);
    context.progress_listener = Arc::new(move |event| {
        if event.message.starts_with("Joining original audio")
            && listener_events.fetch_add(1, Ordering::AcqRel) + 1 == 2
        {
            cancelled.cancel();
        }
    });
    let file_info =
        crate::audio::get_file_list_info(&sources, AacDecoder::Auto).expect("probe AAC fixtures");
    assert_eq!(file_info.invalid_count, 0, "fixtures probe as valid AAC");

    let error = crate::audio::execute_audio_engine(AudioExecutionRequest::new(
        context,
        file_info,
        None,
        CoverArtPassthroughPolicy::Preserve,
        AudioHandling::Preserve,
    ))
    .await
    .expect_err("cancelled join must not publish");

    assert!(matches!(error, AppError::Cancellation(_)), "{error:?}");
    assert_eq!(
        joining_events.load(Ordering::Acquire),
        2,
        "cancellation landed after the first source was joined"
    );
    assert!(!destination.exists());
    assert!(
        !workspace.exists()
            || std::fs::read_dir(&workspace)
                .expect("read workspace")
                .next()
                .is_none(),
        "staged copies and the partial join are removed"
    );
    for (source, original) in sources.iter().zip(original_bytes) {
        assert_eq!(std::fs::read(source).expect("source unchanged"), original);
    }
}
