use super::*;
use crate::remote_source::HandoffRefusal;
use abb_remote_source_core::acquisition_progress_for_current_title;

fn done(ending: JobEnding) -> AcquisitionJobState {
    AcquisitionJobState::Done { ending }
}

fn handed_off(handoff: AcquisitionHandoff) -> AcquisitionJobState {
    done(JobEnding::Handoff { handoff })
}

fn show_title_download_tick(job: &mut Registered) {
    job.show_progress(acquisition_progress_for_current_title(
        acquisition_progress(AcquisitionStage::Download, Some(0.5), Some(1), Some(2)),
        "B000000001",
        1,
        2,
    ));
}

fn assert_shows_ending(progress: &AcquisitionProgress, message: &str) {
    assert_eq!(progress.percentage, 100.0);
    assert_eq!(progress.message, message);
    assert_eq!(progress.bytes_downloaded, None);
    assert_eq!(progress.bytes_total, None);
    assert_eq!(progress.current_title_id, None);
    assert_eq!(progress.current_item_index, None);
    assert_eq!(progress.total_items, None);
}

pub(crate) fn titles_job(job_id: &str) -> Registered {
    Registered::new(
        job_id,
        &AcquisitionWork::Titles(crate::remote_source::AcquisitionPlan {
            selections: Vec::new(),
        }),
    )
}

fn one_file(diagnostics: Vec<RemoteSourceDiagnostic>) -> AcquisitionOutcome {
    TitlesOutcome {
        files: vec![MaterializedSourceFile {
            input_id: "input-1".into(),
            title_id: "B000000001".into(),
            path: "book.m4b".into(),
            size_bytes: 1,
            sha256: String::new(),
        }],
        diagnostics,
        ..TitlesOutcome::default()
    }
    .into()
}

fn diagnostic(message: &str) -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind: RemoteAcquisitionFailureKind::ValidationFailed,
        title_id: Some("B000000001".into()),
        message: message.into(),
    }
}

pub(crate) fn validated(job_id: &str, diagnostics: Vec<RemoteSourceDiagnostic>) -> Registered {
    let mut job = titles_job(job_id);
    assert!(job.finish(one_file(diagnostics)));
    job
}

pub(crate) fn imported(
    job_id: &str,
    count: usize,
    diagnostics: Vec<RemoteSourceDiagnostic>,
) -> Registered {
    let mut job = validated(job_id, diagnostics);
    assert!(
        job.handoff_done(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
            count
        }))
    );
    job
}

pub(crate) fn removed(
    job_id: &str,
    reason: HandoffRefusal,
    diagnostics: Vec<RemoteSourceDiagnostic>,
) -> Registered {
    let mut job = validated(job_id, diagnostics);
    assert!(
        job.handoff_done(AcquisitionHandoff::Titles(TitlesHandoff::Removed {
            reason
        }))
    );
    job
}

pub(crate) fn failed(job_id: &str, diagnostics: Vec<RemoteSourceDiagnostic>) -> Registered {
    let mut job = titles_job(job_id);
    assert!(job.finish(
        TitlesOutcome {
            diagnostics,
            ..TitlesOutcome::default()
        }
        .into()
    ));
    job
}

pub(crate) fn running(job_id: &str, stage: AcquisitionStage) -> Registered {
    let mut job = titles_job(job_id);
    job.show_progress(acquisition_progress(stage, Some(1.0), None, None));
    job
}

#[test]
fn progress_is_shown_while_running_and_never_moves_the_state() {
    let mut job = titles_job("job-1");
    assert_eq!(job.snapshot().state, AcquisitionJobState::Running);

    let full_tick = acquisition_progress(AcquisitionStage::ImportHandoff, Some(1.0), None, None);
    assert_eq!(job.show_progress(full_tick.clone()), Some(true));

    let snapshot = job.snapshot();
    assert_eq!(snapshot.job.progress, full_tick);
    assert_eq!(snapshot.state, AcquisitionJobState::Running);
}

#[test]
fn progress_after_the_job_ended_is_ignored() {
    let mut job = validated("job-1", Vec::new());
    let before = job.snapshot();

    let tick = acquisition_progress(AcquisitionStage::Download, Some(0.5), None, None);
    assert_eq!(job.show_progress(tick), None);

    assert_eq!(job.snapshot(), before);
}

#[test]
fn files_validate_the_job_but_leave_it_unsettled_until_the_handoff() {
    let job = validated("job-1", Vec::new());

    assert!(!job.is_running());
    assert!(!job.is_done());
    assert!(job.holds_staged_files());
    assert_eq!(job.snapshot().state, AcquisitionJobState::HandingOver);
    let waiting = job.job_for_handoff().expect("files wait");
    assert!(matches!(
        waiting.content,
        AcquiredContent::Titles { ref files, .. } if files.len() == 1
    ));
}

#[test]
fn an_outcome_with_no_file_settles_the_job_as_failed_with_its_reasons() {
    let mut job = titles_job("job-1");
    show_title_download_tick(&mut job);
    assert!(job.finish(
        TitlesOutcome {
            diagnostics: vec![diagnostic("not audio")],
            ..TitlesOutcome::default()
        }
        .into()
    ));

    let snapshot = job.snapshot();
    assert_eq!(snapshot.state, done(JobEnding::Failed));
    assert_shows_ending(&snapshot.job.progress, "Acquisition failed.");
    assert_eq!(snapshot.job.diagnostics[0].message, "not audio");
    assert!(!job.holds_staged_files());
    assert!(job.job_for_handoff().is_none());
}

#[test]
fn a_finish_after_a_cancel_changes_nothing() {
    let mut job = titles_job("job-1");
    show_title_download_tick(&mut job);
    assert!(job.cancel());
    let cancelled = job.snapshot();

    assert!(!job.finish(one_file(Vec::new())));

    assert_eq!(job.snapshot(), cancelled);
    assert!(job.is_cancelled());
    assert!(job.is_done());
    assert_eq!(cancelled.state, done(JobEnding::Cancelled));
    assert_shows_ending(&cancelled.job.progress, "Acquisition cancelled.");
}

#[test]
fn a_cancel_cannot_undo_a_finished_job() {
    let mut job = validated("job-1", Vec::new());

    assert!(!job.cancel());

    assert_eq!(job.snapshot().state, AcquisitionJobState::HandingOver);
    assert!(job.job_for_handoff().is_some());
}

#[test]
fn a_cancel_names_itself_once() {
    let mut job = titles_job("job-1");
    assert!(job.cancel());
    assert!(!job.cancel());

    let cancelled_reasons = job
        .snapshot()
        .job
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.kind == RemoteAcquisitionFailureKind::Cancelled)
        .count();
    assert_eq!(cancelled_reasons, 1);
}

#[test]
fn the_handoff_settles_a_validated_job_and_only_a_validated_job() {
    let mut running = titles_job("job-1");
    assert!(
        !running.handoff_done(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
            count: 1
        }))
    );
    assert!(running.is_running());

    let job = imported("job-2", 1, Vec::new());
    assert_eq!(
        job.snapshot().state,
        handed_off(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
            count: 1
        }))
    );
    assert!(job.holds_staged_files());

    let mut refused = validated("job-3", Vec::new());
    assert!(
        refused.handoff_done(AcquisitionHandoff::Titles(TitlesHandoff::Removed {
            reason: HandoffRefusal::NothingAdded,
        }))
    );
    assert!(refused.is_done());
    assert_eq!(
        refused.snapshot().state,
        handed_off(AcquisitionHandoff::Titles(TitlesHandoff::Removed {
            reason: HandoffRefusal::NothingAdded,
        }))
    );
    assert!(!refused.holds_staged_files());
    assert!(
        !refused.handoff_done(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
            count: 1
        }))
    );
}
