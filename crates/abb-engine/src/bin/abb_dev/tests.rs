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
                    "lane": "encodeCpu", "progress": progress,
                    "sourceInputIds": [], "cancellable": false, "cancelRequested": false,
                    "warnings": [],
                })
            })
            .collect();
        let operation: OperationSnapshot = serde_json::from_value(serde_json::json!({
            "operationId": "export", "sequence": 1, "revision": 1,
            "kind": "processingBatch", "title": "export",
            "status": if statuses.contains(&"cancelled") { "mixed" } else { "completed" },
            "createdAtMs": 0, "cancellable": false, "cancelRequested": true,
            "lanes": [], "sourceInputIds": [], "progress": progress,
            "children": children, "errors": [], "logTail": [],
        }))
        .expect("export snapshot");
        assert_eq!(
            export_verdict(&operation, Some(1)).is_ok(),
            succeeds,
            "{statuses:?}"
        );
    }
}
