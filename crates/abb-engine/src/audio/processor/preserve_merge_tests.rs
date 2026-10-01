use crate::audio::{AudioExecutionRequest, SampleRateConfig};
use crate::errors::AppError;
use crate::metadata::CoverArtPassthroughPolicy;
use crate::processing::{
    AudioHandling, CancellationChecker, JobRegistry, OutputConfig, ProcessingContext,
    ProcessingSession,
};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Writes `frames` silent MPEG-1 Layer III mono frames at 128 kbps / 44.1 kHz.
/// Zero side information means no bit-reservoir dependency and no priming, so
/// the sources are joinable by stream copy.
fn write_silent_mp3(path: &Path, frames: usize) {
    const FRAME_BYTES: usize = 417;
    let mut frame = vec![0_u8; FRAME_BYTES];
    frame[..4].copy_from_slice(&[0xFF, 0xFB, 0x90, 0xC0]);
    std::fs::write(path, frame.repeat(frames)).expect("write MP3 fixture");
}

#[tokio::test]
async fn cancelling_during_a_keep_audio_join_leaves_no_output_or_workspace_residue() {
    let tmp = tempfile::tempdir().expect("join fixture");
    let sources = [tmp.path().join("part-1.mp3"), tmp.path().join("part-2.mp3")];
    for source in &sources {
        write_silent_mp3(source, 40);
    }
    let original_bytes: Vec<_> = sources
        .iter()
        .map(|source| std::fs::read(source).expect("read source"))
        .collect();
    let destination = tmp.path().join("out").join("Joined.mp3");
    std::fs::create_dir_all(destination.parent().expect("output parent")).expect("output dir");
    let workspace = tmp.path().join("workspace");
    let registry = JobRegistry::new(1);
    let (job_id, _permit) = registry.register_job().await.expect("register join job");
    let cancelled = Arc::new(AtomicBool::new(false));
    let checker = CancellationChecker::new(Some(Arc::clone(&cancelled)));
    let mut context = ProcessingContext::new_headless_with_workspace_root(
        Arc::new(ProcessingSession::with_cancellation(job_id.0, checker)),
        None,
        SampleRateConfig::Auto,
        OutputConfig::new(&destination),
        workspace.clone(),
    );
    // Joining is announced once before the first packet and again after each
    // source; cancel after the first source's packets are in the partial join.
    let joining_events = Arc::new(AtomicUsize::new(0));
    let listener_events = Arc::clone(&joining_events);
    context.progress_listener = Some(Arc::new(move |event| {
        if event.message.starts_with("Joining original audio")
            && listener_events.fetch_add(1, Ordering::AcqRel) + 1 == 2
        {
            cancelled.store(true, Ordering::Release);
        }
    }));
    let file_info = crate::audio::get_file_list_info(&sources).expect("probe MP3 fixtures");
    assert_eq!(file_info.invalid_count, 0, "fixtures probe as valid MP3");

    let error = crate::audio::execute_audio_engine(
        AudioExecutionRequest::new(
            context,
            file_info,
            None,
            CoverArtPassthroughPolicy::Preserve,
        )
        .with_handling(AudioHandling::Preserve),
    )
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
