use std::fs;
use std::path::PathBuf;

use abb_engine::session::{MetadataField, SessionIntent, SessionOutcome};
use abb_engine::work_runtime::WorkOperationStatus;

use super::integration_media_execution_tests::{
    audio_duration_ts, chapters_of, mux_chapters, SAMPLE_RATE,
};
use super::integration_session_desk::Desk;

#[tokio::test]
async fn a_preview_plays_ninety_seconds_with_the_edited_metadata_and_no_chapters() {
    let desk = Desk::new();
    let built = desk
        .audiobook(&desk.root.path().join("build/alpha.m4b"), 95.0)
        .await;
    let book = desk.root.path().join("library/alpha.m4b");
    fs::create_dir_all(book.parent().expect("library folder")).expect("create library");
    mux_chapters(
        &built,
        &[("One", 0, 50_000), ("Two", 50_000, 95_000)],
        &book,
    );
    desk.import(&book).await;
    let library = desk.root.path().join("out");
    fs::create_dir_all(&library).expect("create output folder");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: library.to_string_lossy().into_owned(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Preview Title".to_string(),
    })
    .await;

    desk.send(SessionIntent::Preview).await;
    let preview = desk
        .engine
        .session_snapshot()
        .output
        .expect("output")
        .preview_run
        .expect("preview snapshot");
    assert_eq!(preview.operation.status, WorkOperationStatus::Completed);
    let SessionOutcome::PreviewOutput { path: Some(path) } = desk
        .send(SessionIntent::TakePreviewOutput {
            run_id: preview.operation.operation_id.to_string(),
        })
        .await
    else {
        panic!("a finished preview has an output");
    };
    let path = PathBuf::from(path);

    assert_eq!(
        audio_duration_ts(&path),
        90 * u64::from(SAMPLE_RATE),
        "exactly 90 s"
    );
    assert_eq!(
        abb_engine::read_metadata(&path)
            .expect("read preview tags")
            .title
            .as_deref(),
        Some("Preview Title")
    );
    assert!(chapters_of(&path).is_empty(), "{:?}", chapters_of(&path));
}
