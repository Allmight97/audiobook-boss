use super::types::{ChildCounts, ChildJobStatus, OperationSnapshot};

pub(super) fn tally(snapshot: &mut OperationSnapshot) {
    let mut counts = ChildCounts::default();
    for child in &snapshot.children {
        let count = match child.status {
            ChildJobStatus::Queued { .. } => &mut counts.queued,
            ChildJobStatus::Running => &mut counts.running,
            ChildJobStatus::Completed => &mut counts.completed,
            ChildJobStatus::Skipped => &mut counts.skipped,
            ChildJobStatus::Cancelled => &mut counts.cancelled,
            ChildJobStatus::Failed => &mut counts.failed,
        };
        *count += 1;
    }
    let mut position = 0;
    for child in &mut snapshot.children {
        if let ChildJobStatus::Queued { .. } = child.status {
            position += 1;
            child.status = ChildJobStatus::Queued {
                position,
                of: counts.queued,
            };
        }
    }
    snapshot.counts = counts;
}
