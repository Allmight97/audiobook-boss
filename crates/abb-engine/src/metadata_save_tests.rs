use super::*;
use crate::metadata::PatchOp;

fn title_patch(title: &str) -> MetadataIntentPatch {
    MetadataIntentPatch {
        title: Some(PatchOp::Set(title.to_string())),
        ..Default::default()
    }
}

#[tokio::test]
async fn metadata_batch_returns_ordered_per_file_failures_without_aborting() {
    let items = vec![
        MetadataSaveRequest {
            file_path: "/definitely/missing-a.m4b".to_string(),
            metadata_patch: title_patch("A"),
        },
        MetadataSaveRequest {
            file_path: "/definitely/missing-b.m4b".to_string(),
            metadata_patch: title_patch("B"),
        },
    ];
    let mut progress = Vec::new();
    let cancellation = CancellationChecker::new(None);

    let batch = save_metadata_batch_impl(items, cancellation, |event| progress.push(event))
        .await
        .expect("batch should report per-file failures");
    let result = &batch.result;

    assert_eq!(result.summary.total, 2);
    assert_eq!(result.summary.succeeded, 0);
    assert_eq!(result.summary.failed, 2);
    assert_eq!(result.results[0].input_index, 0);
    assert_eq!(result.results[1].input_index, 1);
    assert_eq!(result.results[0].status, MetadataSaveResultStatus::Failed);
    assert_eq!(result.results[1].status, MetadataSaveResultStatus::Failed);
    assert_eq!(progress.len(), 4);
    assert_eq!(progress[0].stage, EventStage::Writing);
    assert_eq!(progress[1].stage, EventStage::Failed);
    assert_eq!(progress[2].stage, EventStage::Writing);
    assert_eq!(progress[3].stage, EventStage::Failed);
    for (child, name) in batch
        .children
        .iter()
        .zip(["missing-a.m4b", "missing-b.m4b"])
    {
        assert_eq!(child.1, ProcessResultStatus::Failed);
        assert!(
            child.2.contains(name) && !child.2.starts_with("Failed metadata save"),
            "child reason should explain the failure: {}",
            child.2
        );
    }
}

#[tokio::test]
async fn metadata_batch_cancels_remaining_items_between_writes() {
    let items = vec![
        MetadataSaveRequest {
            file_path: "/definitely/missing-a.m4b".to_string(),
            metadata_patch: title_patch("A"),
        },
        MetadataSaveRequest {
            file_path: "/definitely/missing-b.m4b".to_string(),
            metadata_patch: title_patch("B"),
        },
    ];
    let cancellation = CancellationChecker::new(Some(std::sync::Arc::new(
        std::sync::atomic::AtomicBool::new(true),
    )));
    let mut progress = Vec::new();

    let result = save_metadata_batch_impl(items, cancellation, |event| progress.push(event))
        .await
        .expect("cancelled batch should return terminal item results")
        .result;

    assert_eq!(result.summary.total, 2);
    assert_eq!(result.summary.succeeded, 0);
    assert_eq!(result.summary.failed, 0);
    assert_eq!(result.summary.cancelled, 2);
    assert!(result
        .results
        .iter()
        .all(|entry| entry.status == MetadataSaveResultStatus::Cancelled));
    assert!(progress
        .iter()
        .all(|event| event.stage == EventStage::Cancelled));
}

#[test]
fn cancelled_metadata_save_batch_returns_per_file_cancelled_results() {
    let result = cancelled_metadata_save_batch(vec![
        MetadataSaveRequest {
            file_path: "/books/a.m4b".to_string(),
            metadata_patch: title_patch("A"),
        },
        MetadataSaveRequest {
            file_path: "/books/b.m4b".to_string(),
            metadata_patch: title_patch("B"),
        },
    ]);

    assert_eq!(result.summary.total, 2);
    assert_eq!(result.summary.succeeded, 0);
    assert_eq!(result.summary.failed, 0);
    assert_eq!(result.summary.cancelled, 2);
    assert_eq!(result.results[0].input_index, 0);
    assert_eq!(result.results[0].file_path, "/books/a.m4b");
    assert_eq!(result.results[1].input_index, 1);
    assert_eq!(result.results[1].file_path, "/books/b.m4b");
    assert!(result
        .results
        .iter()
        .all(|entry| entry.status == MetadataSaveResultStatus::Cancelled));
}
