use super::stage_table::{for_event, for_finished_operation, for_result, CANCELLING_STAGE};
use super::{ChildJobStatus, WorkOperationStatus, WorkProgressStage};
use crate::processing::{EventStage, ProcessResultStatus};

#[test]
fn every_result_status_maps_to_its_child_and_stage() {
    use ChildJobStatus as Child;
    use ProcessResultStatus as Result;
    use WorkProgressStage as Stage;
    let rows = [
        (Result::Success, Child::Completed, Stage::Complete),
        (Result::Skipped, Child::Skipped, Stage::Complete),
        (Result::Cancelled, Child::Cancelled, Stage::Cancelled),
        (Result::Failed, Child::Failed, Stage::Failed),
    ];
    for (result, child, stage) in rows {
        let row = for_result(result);
        assert_eq!((row.child, row.stage), (child, stage), "{result:?}");
    }
}

#[test]
fn every_event_stage_maps_to_its_child_and_stage() {
    use ChildJobStatus as Child;
    use EventStage as Event;
    use WorkProgressStage as Stage;
    let rows = [
        (Event::Analyzing, Child::Running, Stage::Analyzing),
        (Event::Converting, Child::Running, Stage::Converting),
        (Event::Writing, Child::Running, Stage::Writing),
        (Event::Completed, Child::Completed, Stage::Complete),
        (Event::Skipped, Child::Skipped, Stage::Complete),
        (Event::Failed, Child::Failed, Stage::Failed),
        (Event::Cancelled, Child::Cancelled, Stage::Cancelled),
    ];
    for (event, child, stage) in rows {
        let row = for_event(event);
        assert_eq!((row.child, row.stage), (child, stage), "{event:?}");
    }
}

#[test]
fn only_finished_operations_have_a_row_and_cancelling_shows_the_cleaning_stage() {
    use WorkOperationStatus as Status;
    let rows = [
        (Status::Accepted, None),
        (Status::Running, None),
        (Status::Cancelling, None),
        (Status::Completed, Some(WorkProgressStage::Complete)),
        (Status::Mixed, Some(WorkProgressStage::Complete)),
        (Status::Cancelled, Some(WorkProgressStage::Cancelled)),
        (Status::Failed, Some(WorkProgressStage::Failed)),
    ];
    for (status, stage) in rows {
        assert_eq!(
            for_finished_operation(status).map(|row| row.stage),
            stage,
            "{status:?}"
        );
    }
    assert_eq!(CANCELLING_STAGE, WorkProgressStage::Cleaning);
}

#[test]
fn an_operation_is_finished_exactly_where_the_stage_table_has_a_finished_row() {
    use super::terminal::phase_of;
    use super::OperationPhase as Phase;
    use WorkOperationStatus as Status;
    let rows = [
        (Status::Accepted, Phase::Accepted),
        (Status::Running, Phase::Active),
        (Status::Cancelling, Phase::Active),
        (Status::Completed, Phase::Finished),
        (Status::Mixed, Phase::Finished),
        (Status::Cancelled, Phase::Finished),
        (Status::Failed, Phase::Finished),
    ];
    for (status, phase) in rows {
        assert_eq!(phase_of(status), phase, "{status:?}");
        assert_eq!(
            for_finished_operation(status).is_some(),
            phase == Phase::Finished,
            "{status:?}"
        );
    }
}
