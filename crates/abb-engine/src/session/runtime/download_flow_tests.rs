use super::*;

impl Rig {
    async fn removed_jobs(&self) -> Vec<String> {
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                let removed = self.removed.lock().expect("removed").clone();
                if !removed.is_empty() && !self.session.lock().sweeping.held() {
                    return removed;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("a download is removed")
    }

    /// Waits until `count` removal attempts have finished and the sweep has
    /// parked, so a paused clock can advance without racing the spawn.
    async fn settled_removal_attempts(&self, count: usize) {
        let limit = crate::session::staged::RETRY_DELAY * 4;
        tokio::time::timeout(limit, async {
            loop {
                let attempts = self.removed.lock().expect("removed").len();
                if attempts >= count && !self.session.lock().sweeping.held() {
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {count} removal attempts"))
    }
}

#[tokio::test]
async fn an_acquired_title_is_listed_with_its_companions_and_its_download_goes_when_it_leaves() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");

    let handoff = rig.session.import_acquired(acquired("job-1", &audio)).await;

    assert_eq!(
        handoff,
        crate::remote_source::AcquisitionHandoff::Imported { count: 1 }
    );
    let titles = rig.session.snapshot().titles.expect("titles");
    let input_id = titles.files[0].input_id.clone();
    assert_eq!(titles.companions[&input_id], ["Guide.pdf"]);
    assert!(rig.removed.lock().expect("removed").is_empty());

    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    assert!(rig
        .session
        .snapshot()
        .titles
        .expect("titles")
        .companions
        .is_empty());
}

#[tokio::test]
async fn a_download_with_an_unreadable_file_is_not_listed_at_all() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    let mut job = acquired("job-1", &audio);
    let mut missing = job.materialized_files[0].clone();
    missing.path = staging.path().join("part 2.wav");
    job.materialized_files.push(missing);

    let handoff = rig.session.import_acquired(job).await;

    assert!(
        matches!(
            handoff,
            crate::remote_source::AcquisitionHandoff::Removed {
                reason: crate::remote_source::HandoffRefusal::ImportFailed { .. }
            }
        ),
        "{handoff:?}"
    );
    assert!(rig.listed_paths().is_empty());
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
}

#[tokio::test(start_paused = true)]
async fn a_change_before_the_retry_delay_has_passed_does_not_retry_a_failed_removal() {
    let rig = rig();
    rig.failing_removals
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    let attempts = || rig.removed.lock().expect("removed").len();

    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    rig.send(SessionIntent::SetOutputDirectory {
        directory: "/library".to_string(),
    })
    .await;
    tokio::time::sleep(crate::session::staged::RETRY_DELAY / 2).await;
    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(attempts(), 1);

    tokio::time::sleep(crate::session::staged::RETRY_DELAY).await;
    rig.send(SessionIntent::ClearAll).await;
    crate::test_cases::support::poll::until_true("the removal is retried", || attempts() == 2)
        .await;
}

#[tokio::test(start_paused = true)]
async fn a_failed_removal_is_retried_after_the_delay_with_no_other_change() {
    let rig = rig();
    rig.failing_removals
        .store(1, std::sync::atomic::Ordering::SeqCst);
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    let attempts = || rig.removed.lock().expect("removed").len();

    rig.send(SessionIntent::ClearAll).await;
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    assert_eq!(attempts(), 1);

    tokio::time::sleep(crate::session::staged::RETRY_DELAY * 2).await;
    crate::test_cases::support::poll::until_true("the removal is retried", || attempts() == 2)
        .await;
}

#[tokio::test(start_paused = true)]
async fn each_failed_removal_waits_twice_as_long_before_the_next_retry() {
    let rig = rig();
    rig.failing_removals
        .store(2, std::sync::atomic::Ordering::SeqCst);
    let staging = tempfile::TempDir::new().expect("staging");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    let attempts = || rig.removed.lock().expect("removed").len();
    let delay = crate::session::staged::RETRY_DELAY;

    rig.send(SessionIntent::ClearAll).await;
    rig.settled_removal_attempts(1).await;
    assert_eq!(attempts(), 1);

    tokio::time::sleep(delay / 2).await;
    assert_eq!(attempts(), 1, "the first failure has not been retried yet");
    rig.settled_removal_attempts(2).await;

    tokio::time::sleep(delay).await;
    assert_eq!(
        attempts(),
        2,
        "the second failure waits longer than the first"
    );
    rig.settled_removal_attempts(3).await;
    assert_eq!(attempts(), 3);
}

#[tokio::test]
async fn a_download_goes_once_its_title_is_exported() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let output = tempfile::TempDir::new().expect("output");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    assert!(rig.removed.lock().expect("removed").is_empty());

    rig.send(SessionIntent::SetOutputDirectory {
        directory: output.path().to_string_lossy().into_owned(),
    })
    .await;
    rig.type_into(MetadataField::Title, "Book").await;
    rig.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            rig.session.snapshot().output.expect("output").submission,
            Some(SubmissionStatus::Submitted { .. })
        ),
        "{:?}",
        rig.session.snapshot().output.expect("output").submission
    );
    assert_eq!(rig.removed_jobs().await, ["job-1"]);
    assert_eq!(
        rig.session.snapshot().titles.expect("titles").files.len(),
        1
    );
}

#[tokio::test]
async fn a_download_that_finishes_while_the_list_is_locked_is_imported_once_it_unlocks() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    let draft = rig
        .session
        .transition(|state| {
            state.output.set_directory("/library".to_string());
            state.begin_submission(false)
        })
        .expect("a submission locks the list");

    let audio = staged_wav(staging.path(), "book");
    let session = rig.session.clone();
    #[expect(
        clippy::disallowed_methods,
        reason = "joined by a_download_that_finishes_while_the_list_is_locked_is_imported_once_it_unlocks"
    )]
    let handoff =
        tokio::spawn(async move { session.import_acquired(acquired("job-1", &audio)).await });
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert!(!handoff.is_finished(), "the download waits for the list");

    rig.session
        .end_submission(&draft, SubmissionStatus::Cancelled);
    assert_eq!(
        handoff.await.expect("handoff"),
        crate::remote_source::AcquisitionHandoff::Imported { count: 1 }
    );
    assert!(rig.removed.lock().expect("removed").is_empty());
}
