use super::*;

#[test]
fn export_verdict_requires_the_requested_title_to_be_cancelled() {
    for (statuses, succeeds) in [
        (["completed", "completed"], false),
        (["cancelled", "completed"], true),
        (["completed", "cancelled"], false),
    ] {
        let progress = serde_json::json!({
            "stage": "complete", "percentage": 100.0, "message": "done",
        });
        let children: Vec<_> = statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                serde_json::json!({
                    "childJobId": index.to_string(), "operationId": "export",
                    "label": index.to_string(), "status": status,
                    "progress": progress,
                    "cancellable": false, "cancelRequested": false,
                    "warnings": [],
                })
            })
            .collect();
        let operation: OperationSnapshot = serde_json::from_value(serde_json::json!({
            "operationId": "export", "sequence": 1, "revision": 1,
            "kind": "processingBatch", "title": "export",
            "status": if statuses.contains(&"cancelled") { "mixed" } else { "completed" },
            "phase": "finished",
            "cancellable": false, "cancelRequested": true,
            "progress": progress,
            "children": children, "logTail": [],
        }))
        .expect("export snapshot");
        assert_eq!(
            export_verdict(&operation, Some(1)).is_ok(),
            succeeds,
            "{statuses:?}"
        );
    }
}

#[test]
fn during_export_steps_keep_the_order_given() {
    let args = [
        "book.m4b",
        "--during-export-set",
        "genre=Mystery",
        "--during-export-reimport",
        "--during-export-set",
        "author=A=B",
    ];
    let options = parse(args.iter().map(|arg| arg.to_string())).expect("options");
    let steps: Vec<_> = options
        .during_export
        .iter()
        .map(|step| match step {
            DuringExport::Set(field, value) => format!("{field:?}={value}"),
            DuringExport::Reimport => "reimport".to_string(),
        })
        .collect();
    assert_eq!(steps, ["Genre=Mystery", "reimport", "Author=A=B"]);
}

#[test]
fn a_during_export_edit_needs_a_known_field_and_a_value() {
    for args in [
        ["book.m4b", "--during-export-set", "genre"],
        ["book.m4b", "--during-export-set", "mood=Calm"],
    ] {
        assert!(parse(args.iter().map(|arg| arg.to_string())).is_err());
    }
}

fn snapshot(revision: u64, status: &str, phase: &str, child: &str) -> OperationSnapshot {
    let progress = serde_json::json!({
        "stage": "converting", "percentage": 10.0, "message": "working",
    });
    serde_json::from_value(serde_json::json!({
        "operationId": "export", "sequence": 1, "revision": revision,
        "kind": "processingBatch", "title": "export",
        "status": status, "phase": phase,
        "cancellable": false, "cancelRequested": false,
        "progress": progress,
        "children": [{
            "childJobId": "0", "operationId": "export", "label": "0",
            "status": child, "progress": progress,
            "cancellable": false, "cancelRequested": false, "warnings": [],
        }],
        "logTail": [],
    }))
    .expect("export snapshot")
}

fn report(progress: &PrintProgress, operation: OperationSnapshot) {
    progress.emit(EngineEvent::WorkOperations(
        abb_engine::work_runtime::WorkOperationsUpdate {
            revision: operation.revision,
            order: vec![operation.operation_id.clone()],
            changed: operation,
        },
    ));
}

#[tokio::test]
async fn the_wait_ends_on_the_finished_snapshot_reported_while_it_waits() {
    let progress = Arc::new(PrintProgress::default());
    let mut feed = progress.feed();
    let id = OperationId("export".into());
    let reporter = async {
        for (revision, status, phase, child) in [
            (1, "accepted", "accepted", "queued"),
            (2, "running", "active", "running"),
            (3, "completed", "finished", "completed"),
        ] {
            report(&progress, snapshot(revision, status, phase, child));
            tokio::task::yield_now().await;
        }
    };
    let mut phases = Vec::new();
    let (finished, ()) = tokio::join!(
        await_finished(&mut feed, &id, |operation| {
            phases.push(operation.phase);
            Ok(())
        }),
        reporter
    );
    let finished = finished.expect("finished");
    assert_eq!(finished.revision, 3);
    assert_eq!(phases.last(), Some(&OperationPhase::Finished));
}

#[tokio::test]
async fn a_snapshot_reported_before_the_wait_began_still_ends_it() {
    let progress = PrintProgress::default();
    report(&progress, snapshot(2, "completed", "finished", "completed"));
    let mut feed = progress.feed();
    let finished = await_finished(&mut feed, &OperationId("export".into()), |_| Ok(()))
        .await
        .expect("finished");
    assert_eq!(finished.revision, 2);
}

#[test]
fn an_older_snapshot_never_replaces_a_newer_one() {
    let progress = PrintProgress::default();
    let feed = progress.feed();
    report(&progress, snapshot(3, "completed", "finished", "completed"));
    report(&progress, snapshot(2, "running", "active", "running"));
    let operations = feed.borrow();
    let known = operations
        .get(&OperationId("export".into()))
        .expect("known");
    assert_eq!(known.revision, 3);
}
