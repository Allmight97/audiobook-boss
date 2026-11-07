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
