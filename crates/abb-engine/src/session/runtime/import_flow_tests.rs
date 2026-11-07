use super::*;

#[tokio::test]
async fn importing_a_folder_with_no_audio_explains_what_is_supported() {
    let rig = rig();
    let empty = tempfile::TempDir::new().expect("temp dir");

    rig.send(SessionIntent::Import {
        paths: vec![empty.path().to_string_lossy().into_owned()],
    })
    .await;

    let titles = rig.session.snapshot().titles.expect("titles part");
    assert!(titles.files.is_empty());
    assert!(matches!(
        titles.notice,
        Some(InputNotice::NoSupportedFiles { .. })
    ));
}

#[tokio::test]
async fn importing_lists_what_it_read_and_names_what_it_skipped() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let book = staged_wav(folder.path(), "book");
    let missing = folder.path().join("missing.wav");

    rig.send(SessionIntent::Import {
        paths: vec![
            book.to_string_lossy().into_owned(),
            missing.to_string_lossy().into_owned(),
        ],
    })
    .await;

    let titles = rig.session.snapshot().titles.expect("titles part");
    assert_eq!(
        rig.listed_paths(),
        vec![book.canonicalize().expect("canonical book")]
    );
    let Some(InputNotice::SkippedPaths { skipped }) = titles.notice else {
        panic!("expected the skipped path notice, got {:?}", titles.notice);
    };
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].path, missing.display().to_string());
    assert!(
        skipped[0]
            .reason
            .starts_with("File validation failed: Cannot read it:"),
        "the path is listed beside the reason, not inside it: {}",
        skipped[0].reason
    );
}

#[tokio::test]
async fn importing_while_a_submission_holds_the_list_shows_the_order_locked_notice() {
    let rig = rig();
    rig.load(&["held"]);
    rig.lock_order(true);
    let folder = tempfile::TempDir::new().expect("temp dir");
    let book = staged_wav(folder.path(), "book");

    rig.send(SessionIntent::Import {
        paths: vec![book.to_string_lossy().into_owned()],
    })
    .await;

    assert_eq!(
        rig.session.snapshot().titles.expect("titles").notice,
        Some(InputNotice::OrderLocked)
    );
    assert_eq!(rig.listed_paths(), vec![path("held")]);
}

#[tokio::test]
async fn opened_files_wait_for_unlock_without_a_host_request() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let opened = staged_wav(folder.path(), "opened");
    rig.lock_order(true);
    rig.session
        .import_opened(vec![opened.clone(), opened.clone()])
        .expect("opened files admitted");
    tokio::task::yield_now().await;
    assert!(rig.listed_paths().is_empty());
    rig.lock_order(false);
    rig.session.sources_released();
    rig.wait_for_titles(1).await;
    assert_eq!(
        rig.listed_paths(),
        vec![opened.canonicalize().expect("canonical opened path")]
    );

    rig.session
        .import_opened(vec![opened])
        .expect("opened again");
    rig.wait_for_notice(InputNotice::DuplicatesOnly).await;
    assert_eq!(rig.listed_paths().len(), 1);
}

#[tokio::test]
async fn closing_refuses_opened_files() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let opened = staged_wav(folder.path(), "opened");
    rig.session.inner.deps.tasks.close();

    assert!(rig.session.import_opened(vec![opened]).is_err());
}
