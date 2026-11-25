use super::*;
use abb_media_pipeline::metadata::PatchOp;

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
    let cancellation = CancellationChecker::new(tokio_util::sync::CancellationToken::new());

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
    let cancellation =
        CancellationChecker::new(crate::test_cases::support::session::cancelled_token());
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

struct LockProbe {
    registry: crate::ManagedJobRegistry,
    locked: std::sync::Mutex<Vec<bool>>,
}

impl crate::EventSink for LockProbe {
    fn emit(&self, _event: crate::EngineEvent) {
        self.locked
            .lock()
            .expect("probe lock")
            .push(self.registry.concurrency_locked());
    }
}

fn missing_file(title: &str) -> Vec<MetadataSaveRequest> {
    vec![MetadataSaveRequest {
        file_path: "/definitely/missing.m4b".to_string(),
        metadata_patch: title_patch(title),
    }]
}

#[tokio::test]
async fn a_save_runs_while_every_export_slot_is_held_and_stays_a_tracked_job() {
    let registry: crate::ManagedJobRegistry =
        std::sync::Arc::new(abb_media_pipeline::processing::JobRegistry::new(1));
    let runtime = WorkRuntime::new(crate::engine_tasks::EngineTasks::default());
    let quiet = Host::new(
        std::sync::Arc::new(crate::DiscardEvents),
        abb_media_pipeline::power::PowerManager::default(),
    );
    let export =
        crate::test_cases::support::blocked_export::BlockedExport::start(&registry, 1).await;

    let saved = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        save_metadata_batch(&quiet, &runtime, &registry, missing_file("A")),
    )
    .await
    .expect("a save does not wait for an export slot")
    .expect("the save reports its files");
    assert_eq!(saved.summary.total, 1);
    assert_eq!(saved.summary.failed, 1);
    export.finish().await;

    let probe = std::sync::Arc::new(LockProbe {
        registry: registry.clone(),
        locked: Default::default(),
    });
    let host = Host::new(
        probe.clone(),
        abb_media_pipeline::power::PowerManager::default(),
    );
    save_metadata_batch(&host, &runtime, &registry, missing_file("B"))
        .await
        .expect("the save reports its files");
    let seen = probe.locked.lock().expect("probe lock").clone();
    assert!(
        seen.contains(&true),
        "the running save is a tracked job: {seen:?}"
    );
    assert_eq!(
        registry
            .update_max_concurrent(2)
            .expect("the finished save is no longer tracked"),
        2
    );
}
