use super::snapshot::{new_metadata_save_snapshot, new_processing_snapshot};
use super::state::WorkRuntimeState;
use super::OperationId;

#[test]
fn the_queue_lists_accepted_operations_in_start_order_and_follows_each_change() {
    let mut state = WorkRuntimeState::default();
    let first = OperationId("op-1".to_string());
    state.insert_operation(new_processing_snapshot(
        first.clone(),
        1,
        "Batch encode".to_string(),
        &["/tmp/first.m4b".to_string()],
        None,
    ));
    let mut tags = |id: &str, sequence: u64| {
        let id = OperationId(id.to_string());
        state.insert_operation(new_metadata_save_snapshot(
            id.clone(),
            sequence,
            "Tags".to_string(),
            &["/tmp/a.m4b".to_string()],
        ));
        id
    };
    let third = tags("op-3", 3);
    let second = tags("op-2", 2);
    assert_eq!(
        state.list().queue,
        vec![first.clone(), second.clone(), third.clone()]
    );

    let started = state.mark_running(&first, 150).expect("start");
    assert_eq!(
        state.publish(started).queue,
        vec![second.clone(), third.clone()]
    );

    let cancelled = state
        .cancel(&second, "Cancelled".to_string(), 170)
        .expect("cancel");
    let update = state.publish(cancelled);
    assert_eq!(update.queue, vec![third]);
    assert_eq!(state.list().queue, update.queue);
}
