use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use abb_engine::session::{
    CollisionReview, MetadataField, SessionIntent, SessionOutcome, SubmissionStatus,
};
use abb_engine::work_runtime::WorkOperationStatus;
use abb_engine::ShutdownOutcome;

use super::integration_session_desk::{
    collision_review, finished, genre_on_disk, submission, Desk,
};

#[tokio::test]
async fn submit_exports_the_session_reviews_a_collision_and_previews() {
    use abb_engine::output_artifact::CollisionPolicy;

    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 2.0)
        .await;
    desk.import(&book).await;
    let library = desk.root.path().join("out");
    fs::create_dir_all(&library).expect("create output folder");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: library.to_string_lossy().into_owned(),
    })
    .await;

    desk.send(SessionIntent::Submit).await;
    let Some(SubmissionStatus::Submitted {
        operation_id,
        title,
    }) = submission(&desk)
    else {
        panic!("submitted: {:?}", submission(&desk));
    };
    assert_eq!(title, "Alpha");
    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&operation_id))
    })
    .await;
    assert_eq!(
        desk.export_status(&operation_id),
        WorkOperationStatus::Completed
    );

    desk.send(SessionIntent::Submit).await;
    assert!(
        matches!(submission(&desk), Some(SubmissionStatus::ReviewRequired)),
        "{:?}",
        submission(&desk)
    );
    let review_id = collision_review(&desk).review_id;
    desk.send(SessionIntent::ChooseCollisionPolicy {
        review_id,
        policy: CollisionPolicy::RenameNew,
    })
    .await;
    assert!(
        matches!(submission(&desk), Some(SubmissionStatus::Submitted { .. })),
        "{:?}",
        submission(&desk)
    );

    desk.send(SessionIntent::Preview { seconds: 1.0 }).await;
    let preview = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .preview_run
        .expect("preview snapshot");
    assert_eq!(preview.operation.status, WorkOperationStatus::Completed);
    assert!(preview.open_ready);
    let id = preview.operation.operation_id.to_string();
    assert!(matches!(
        desk.send(SessionIntent::TakePreviewOutput { run_id: id.clone() })
            .await,
        SessionOutcome::PreviewOutput { path: Some(_) }
    ));
    assert_eq!(
        desk.send(SessionIntent::TakePreviewOutput { run_id: id })
            .await,
        SessionOutcome::PreviewOutput { path: None }
    );
    assert!(
        matches!(
            submission(&desk),
            Some(SubmissionStatus::PreviewFinished { .. })
        ),
        "{:?}",
        submission(&desk)
    );
    assert!(
        !desk
            .engine
            .session_snapshot()
            .titles
            .expect("titles")
            .order_locked
    );
}

#[tokio::test]
async fn shutdown_answers_a_pending_collision_review_so_a_waiting_save_lands() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;
    desk.import(&book).await;
    let first = desk.export().await;
    desk.wait_until("the first export finishes", |desk| {
        finished(desk.export_status(&first))
    })
    .await;
    desk.send(SessionIntent::Submit).await;
    assert!(
        matches!(submission(&desk), Some(SubmissionStatus::ReviewRequired)),
        "{:?}",
        submission(&desk)
    );
    let metadata = desk.edit_genre_and_save().await;
    assert_eq!(metadata.waiting_writes.len(), 1);

    let outcome = tokio::time::timeout(Duration::from_secs(30), desk.engine.shutdown())
        .await
        .expect("shutdown does not wait on an unanswered review");
    assert_eq!(outcome, ShutdownOutcome::Settled);

    assert_eq!(genre_on_disk(&book).as_deref(), Some("Mystery"));
}

#[tokio::test]
async fn a_collision_that_appears_during_review_is_reviewed_before_any_policy_applies() {
    use abb_engine::output_artifact::CollisionPolicy;

    let desk = Desk::new();
    let library = desk.root.path().join("library");
    let alpha = desk.audiobook(&library.join("alpha.m4b"), 1.0).await;
    let beta = desk.audiobook(&library.join("beta.m4b"), 1.0).await;
    desk.send(SessionIntent::Import {
        paths: vec![
            alpha.to_string_lossy().into_owned(),
            beta.to_string_lossy().into_owned(),
        ],
    })
    .await;
    let beta_id = desk.engine.session_snapshot().titles.expect("titles").files[1]
        .input_id
        .clone();
    desk.send(SessionIntent::SelectFile {
        title_id: beta_id,
        modifiers: abb_engine::session::SelectionModifiers::default(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Beta".to_string(),
    })
    .await;
    let first = desk.export().await;
    desk.wait_until("the first export finishes", |desk| {
        finished(desk.export_status(&first))
    })
    .await;
    let outputs = |desk: &Desk| -> Vec<PathBuf> { walk(&desk.root.path().join("exports")) };
    let exported = outputs(&desk);
    assert_eq!(exported.len(), 2, "{exported:?}");
    let existing_bytes: Vec<_> = exported
        .iter()
        .map(|path| fs::read(path).expect("read existing audiobook"))
        .collect();

    let moved = desk.root.path().join("set-aside.m4b");
    crate::test_cases::move_test_file(&exported[1], &moved);
    desk.send(SessionIntent::Submit).await;
    assert_eq!(submission(&desk), Some(SubmissionStatus::ReviewRequired));
    let CollisionReview {
        review_id,
        outputs: seen,
        ..
    } = collision_review(&desk);
    assert_eq!(seen.len(), 1);

    crate::test_cases::move_test_file(&moved, &exported[1]);
    desk.send(SessionIntent::ChooseCollisionPolicy {
        review_id,
        policy: CollisionPolicy::ReplaceExisting,
    })
    .await;
    assert_eq!(submission(&desk), Some(SubmissionStatus::ReviewRequired));
    let CollisionReview {
        review_id: next_review,
        outputs: now,
        ..
    } = collision_review(&desk);
    assert_eq!(now.len(), 2);
    assert_ne!(next_review, review_id);
    for intent in [
        SessionIntent::ChooseCollisionPolicy {
            review_id,
            policy: CollisionPolicy::ReplaceExisting,
        },
        SessionIntent::CancelCollisionReview { review_id },
    ] {
        assert_eq!(desk.send(intent).await, SessionOutcome::Superseded);
        assert_eq!(collision_review(&desk).review_id, next_review);
        assert!(
            desk.engine
                .session_snapshot()
                .output
                .expect("output")
                .submission_in_progress
        );
    }
    assert_eq!(outputs(&desk), exported);
    for (path, original) in exported.iter().zip(existing_bytes) {
        assert!(
            fs::read(path).expect("read preserved audiobook") == original,
            "held collision review changed {path:?}"
        );
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).expect("read folder").flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files.sort();
    files
}
