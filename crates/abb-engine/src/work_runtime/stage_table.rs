use super::types::{ChildJobStatus, WorkOperationStatus, WorkProgressStage};
use crate::processing::{EventStage, ProcessResultStatus};

pub(super) const CANCELLING_STAGE: WorkProgressStage = WorkProgressStage::Cleaning;

pub(super) struct StageRow {
    pub result: Option<ProcessResultStatus>,
    pub event: EventStage,
    pub child: ChildJobStatus,
    pub finished_operations: &'static [WorkOperationStatus],
    pub stage: WorkProgressStage,
}

const STAGE_TABLE: [StageRow; 7] = [
    StageRow {
        result: Some(ProcessResultStatus::Success),
        event: EventStage::Completed,
        child: ChildJobStatus::Completed,
        finished_operations: &[WorkOperationStatus::Completed, WorkOperationStatus::Mixed],
        stage: WorkProgressStage::Complete,
    },
    StageRow {
        result: Some(ProcessResultStatus::Skipped),
        event: EventStage::Skipped,
        child: ChildJobStatus::Skipped,
        finished_operations: &[],
        stage: WorkProgressStage::Complete,
    },
    StageRow {
        result: Some(ProcessResultStatus::Cancelled),
        event: EventStage::Cancelled,
        child: ChildJobStatus::Cancelled,
        finished_operations: &[WorkOperationStatus::Cancelled],
        stage: WorkProgressStage::Cancelled,
    },
    StageRow {
        result: Some(ProcessResultStatus::Failed),
        event: EventStage::Failed,
        child: ChildJobStatus::Failed,
        finished_operations: &[WorkOperationStatus::Failed],
        stage: WorkProgressStage::Failed,
    },
    StageRow {
        result: None,
        event: EventStage::Analyzing,
        child: ChildJobStatus::Running,
        finished_operations: &[],
        stage: WorkProgressStage::Analyzing,
    },
    StageRow {
        result: None,
        event: EventStage::Converting,
        child: ChildJobStatus::Running,
        finished_operations: &[],
        stage: WorkProgressStage::Converting,
    },
    StageRow {
        result: None,
        event: EventStage::Writing,
        child: ChildJobStatus::Running,
        finished_operations: &[],
        stage: WorkProgressStage::Writing,
    },
];

pub(super) fn for_result(status: ProcessResultStatus) -> &'static StageRow {
    STAGE_TABLE
        .iter()
        .find(|row| row.result == Some(status))
        .expect("every result status has a stage row")
}

pub(super) fn for_event(stage: EventStage) -> &'static StageRow {
    STAGE_TABLE
        .iter()
        .find(|row| row.event == stage)
        .expect("every event stage has a stage row")
}

pub(super) fn for_finished_operation(status: WorkOperationStatus) -> Option<&'static StageRow> {
    STAGE_TABLE
        .iter()
        .find(|row| row.finished_operations.contains(&status))
}
