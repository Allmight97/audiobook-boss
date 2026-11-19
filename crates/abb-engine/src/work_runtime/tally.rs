use super::types::{ChildJobStatus, OperationSnapshot};

pub(super) fn tally(snapshot: &mut OperationSnapshot) {
    let of = snapshot
        .children
        .iter()
        .filter(|child| matches!(child.status, ChildJobStatus::Queued { .. }))
        .count() as u32;
    let mut position = 0;
    for child in &mut snapshot.children {
        if let ChildJobStatus::Queued { .. } = child.status {
            position += 1;
            child.status = ChildJobStatus::Queued { position, of };
        }
    }
}
