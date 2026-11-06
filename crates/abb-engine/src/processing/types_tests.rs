use super::*;
use abb_processing_core::RunTerminalClass;

fn entry(input_index: usize, status: ProcessResultStatus) -> ProcessResultEntry {
    ProcessResultEntry {
        input_index,
        status,
        message: String::new(),
        error: None,
        output_path: None,
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id: None,
    }
}

#[test]
fn a_run_with_a_success_and_a_failure_is_classified_mixed() {
    let result = ProcessCommandResult::new(vec![
        entry(0, ProcessResultStatus::Success),
        entry(1, ProcessResultStatus::Failed),
    ]);

    assert_eq!(result.terminal_class, RunTerminalClass::Mixed);
    assert_eq!((result.summary.succeeded, result.summary.failed), (1, 1));
}

#[test]
fn a_run_with_no_results_and_a_run_of_successes_are_classified_apart() {
    assert_eq!(
        ProcessCommandResult::new(Vec::new()).terminal_class,
        RunTerminalClass::Empty
    );
    assert_eq!(
        ProcessCommandResult::new(vec![entry(0, ProcessResultStatus::Success)]).terminal_class,
        RunTerminalClass::Success
    );
}
