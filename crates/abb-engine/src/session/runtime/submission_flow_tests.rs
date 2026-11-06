use super::*;

#[tokio::test(flavor = "current_thread")]
async fn reset_during_preflight_drops_the_review_and_unlocks_the_list() {
    let rig = rig();
    let staging = tempfile::TempDir::new().expect("staging");
    let out = tempfile::TempDir::new().expect("output");
    let audio = staged_wav(staging.path(), "book");
    rig.session.import_acquired(acquired("job-1", &audio)).await;
    rig.send(SessionIntent::SetOutputDirectory {
        directory: out.path().to_string_lossy().into_owned(),
    })
    .await;
    rig.type_into(MetadataField::Title, "Book").await;
    let dest = preview_path(&rig);
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        std::fs::create_dir_all(parent).expect("output parent");
    }
    std::fs::write(&dest, b"already there").expect("colliding output");

    rig.send(SessionIntent::Submit).await;
    assert!(
        matches!(
            output(&rig).submission,
            Some(SubmissionStatus::ReviewRequired)
        ),
        "this output must collide so hold_for_review is reached: {:?}",
        output(&rig).submission
    );
    let review_id = output(&rig)
        .collision_review
        .expect("held review")
        .review_id;
    rig.send(SessionIntent::CancelCollisionReview { review_id })
        .await;
    assert!(!rig.session.snapshot().titles.expect("titles").order_locked);

    let submitting = rig.session.begin(SessionIntent::Submit);
    let _reset = rig.session.begin(SessionIntent::Reset);
    submitting.finish().await;

    let titles = rig.session.snapshot().titles.expect("titles");
    assert!(titles.files.is_empty());
    assert!(
        !titles.order_locked,
        "a reset during preflight unlocks the list"
    );
    assert!(
        output(&rig).collision_review.is_none(),
        "a reset during preflight does not hold a review: {:?}",
        output(&rig).collision_review
    );
    assert!(!output(&rig).submission_in_progress);
}
