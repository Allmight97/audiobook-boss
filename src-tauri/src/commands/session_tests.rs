use super::*;
use abb_engine::session::SelectionSnapshot;

fn reply(outcome: SessionOutcome) -> SessionReply {
    SessionReply {
        outcome,
        update: SessionUpdate {
            revision: 42,
            titles: None,
            selection: Some(SelectionSnapshot {
                revision: 41,
                selected_indices: vec![0],
                selected_anchor: Some(0),
            }),
            metadata: None,
            lookup: None,
            audio: None,
            ..SessionUpdate::default()
        },
    }
}

#[test]
fn open_preview_output_opens_the_engine_granted_path() {
    let granted = SessionOutcome::PreviewOutput {
        path: Some("/engine/preview.m4b".into()),
    };
    let mut opened = Vec::new();

    let result = open_preview_reply(reply(granted.clone()), |path| {
        opened.push(path.to_string());
        Ok::<_, std::io::Error>(())
    });

    assert_eq!(opened, ["/engine/preview.m4b"]);
    assert_eq!(result.outcome, granted);
    assert_eq!(result.update.revision, 42);
}

#[test]
fn open_preview_output_opens_nothing_when_the_engine_refuses() {
    let refusal = SessionOutcome::Rejected {
        error: AppErrorEnvelope::from(&AppError::InvalidInput("preview unavailable".into())),
    };
    for outcome in [refusal, SessionOutcome::PreviewOutput { path: None }] {
        let result = open_preview_reply(reply(outcome.clone()), |_| -> std::io::Result<()> {
            panic!("a refused request must not reach the opener");
        });

        assert_eq!(result.outcome, outcome);
        assert_eq!(result.update.revision, 42);
    }
}

#[test]
fn open_preview_output_keeps_the_update_when_the_opener_fails() {
    let mut opened = Vec::new();
    let result = open_preview_reply(
        reply(SessionOutcome::PreviewOutput {
            path: Some("/engine/preview.m4b".into()),
        }),
        |path| {
            opened.push(path.to_string());
            Err(std::io::Error::other("native opener failed"))
        },
    );

    assert_eq!(opened, ["/engine/preview.m4b"]);
    assert_eq!(result.update.revision, 42);
    let selection = result.update.selection.expect("session update retained");
    assert_eq!(selection.revision, 41);
    assert_eq!(selection.selected_indices, [0]);
    assert_eq!(selection.selected_anchor, Some(0));
    assert_eq!(
        result.outcome,
        SessionOutcome::Rejected {
            error: AppErrorEnvelope::new(
                AppErrorCode::InternalError,
                AppErrorCategory::Internal,
                "native opener failed",
                None,
            ),
        }
    );
}
