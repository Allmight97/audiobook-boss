use super::*;

#[tokio::test]
async fn ending_a_submission_wakes_a_save_waiting_on_its_sources() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    let draft = rig
        .session
        .transition(|state| {
            state.output.set_directory("/library".to_string());
            state.begin_submission(false)
        })
        .expect("the submission is prepared");
    rig.type_into(MetadataField::Genre, "Mystery").await;
    rig.send(SessionIntent::Save).await;
    assert_eq!(rig.session.waiting_write_paths(), [path("alpha")]);

    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    rig.session
        .end_submission(&draft, SubmissionStatus::Cancelled);

    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !rig.session.waiting_write_paths().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the waiting write is attempted once its source is free");
}
