use super::*;
use crate::session::runtime::submission_flow::failed;

#[test]
fn a_cancelled_run_ends_as_cancelled_and_any_other_error_as_failed() {
    assert!(matches!(
        failed(&AppError::cancelled()),
        SubmissionStatus::Cancelled
    ));
    assert!(matches!(
        failed(&AppError::General("disk full".to_string())),
        SubmissionStatus::Failed { .. }
    ));
}
