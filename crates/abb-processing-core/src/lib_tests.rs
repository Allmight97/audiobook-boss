use super::*;

#[test]
fn summary_counts_each_terminal_status() {
    let summary = summarize_result_statuses([
        ProcessResultStatus::Success,
        ProcessResultStatus::Skipped,
        ProcessResultStatus::Cancelled,
        ProcessResultStatus::Failed,
        ProcessResultStatus::Failed,
    ]);

    assert_eq!(
        summary,
        OperationResultSummary {
            total: 5,
            succeeded: 1,
            skipped: 1,
            cancelled: 1,
            failed: 2,
        }
    );
}

#[test]
fn terminal_classification_preserves_empty_single_and_mixed_classes() {
    assert_eq!(
        classify_run_terminal(&summarize_result_statuses([])),
        RunTerminalClass::Empty
    );
    assert_eq!(
        classify_run_terminal(&summarize_result_statuses([ProcessResultStatus::Success])),
        RunTerminalClass::Success
    );
    assert_eq!(
        classify_run_terminal(&summarize_result_statuses([ProcessResultStatus::Cancelled])),
        RunTerminalClass::Cancelled
    );
    assert_eq!(
        classify_run_terminal(&summarize_result_statuses([
            ProcessResultStatus::Success,
            ProcessResultStatus::Failed,
            ProcessResultStatus::Skipped,
        ])),
        RunTerminalClass::Mixed
    );
}

#[test]
fn classify_run_terminal_treats_success_plus_skipped_as_mixed() {
    let summary =
        summarize_result_statuses([ProcessResultStatus::Success, ProcessResultStatus::Skipped]);
    assert_eq!(classify_run_terminal(&summary), RunTerminalClass::Mixed);
}

#[test]
fn classify_run_terminal_treats_skipped_only_as_skipped() {
    let summary = summarize_result_statuses([ProcessResultStatus::Skipped]);
    assert_eq!(classify_run_terminal(&summary), RunTerminalClass::Skipped);
}

#[test]
fn whole_operation_summary_constructors_are_consistent() {
    let failed = OperationResultSummary::all_failed(3);
    assert_eq!(failed.total, 3);
    assert_eq!(failed.failed, 3);
    assert_eq!(classify_run_terminal(&failed), RunTerminalClass::Failed);

    let cancelled = OperationResultSummary::all_cancelled(0);
    assert_eq!(cancelled.total, 1);
    assert_eq!(cancelled.cancelled, 1);
    assert_eq!(
        classify_run_terminal(&cancelled),
        RunTerminalClass::Cancelled
    );
}

#[test]
fn classifier_treats_all_errors_cancelled_only_when_no_success_skip_or_failure() {
    let mut classifier = RunTerminalClassifier::default();
    classifier.observe_cancelled();
    assert!(classifier.is_fully_cancelled());

    classifier.observe_status(ProcessResultStatus::Success);
    assert!(!classifier.is_fully_cancelled());
    assert_eq!(classifier.class(), RunTerminalClass::Mixed);
}
