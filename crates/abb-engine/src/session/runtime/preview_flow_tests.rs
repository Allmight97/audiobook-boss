use super::*;

async fn wait_preview(
    rig: &Rig,
    status: crate::work_runtime::WorkOperationStatus,
) -> crate::session::preview::PreviewSnapshot {
    crate::test_cases::support::poll::until(&format!("the preview reaches {status:?}"), || {
        output(rig)
            .preview_run
            .filter(|preview| preview.operation.status == status)
    })
    .await
}

#[tokio::test]
async fn preview_cancel_stops_a_scheduler_wait_and_survives_a_dropped_host_reply() {
    let rig = rig();
    let lane = crate::test_cases::media_fixtures::MediaLane::with_fixtures(&[0.2]);
    let source = lane
        .process(Some(AudiobookMetadata {
            title: Some("Preview".to_string()),
            ..Default::default()
        }))
        .await;
    std::fs::create_dir_all(rig._config.path().join("previews")).expect("preview folder");
    rig.send(SessionIntent::Import {
        paths: vec![source.to_string_lossy().into_owned()],
    })
    .await;
    rig.send(SessionIntent::SetOutputDirectory {
        directory: rig
            ._config
            .path()
            .join("previews")
            .to_string_lossy()
            .into_owned(),
    })
    .await;
    let custom = rig._config.path().join("custom.png");
    image::RgbImage::from_pixel(1, 1, image::Rgb([180, 20, 90]))
        .save(&custom)
        .expect("custom cover");
    rig.send(SessionIntent::LoadCoverFromFile {
        path: custom.to_string_lossy().into_owned(),
    })
    .await;
    let cover = rig.cover_shown().await.expect("accepted normalized cover");
    let jobs = &rig.session.inner.deps.jobs;
    let (first, first_permit) = jobs.register_job().await.expect("hold first slot");
    let (second, second_permit) = jobs.register_job().await.expect("hold second slot");
    let run = rig.session.begin(SessionIntent::Preview);
    drop(run);
    let id = output(&rig)
        .preview_run
        .expect("accepted preview")
        .operation
        .operation_id;
    wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Running).await;
    let offered = output(&rig)
        .preview_run
        .and_then(|run| run.cover)
        .expect("the running preview offers its artwork");
    let shown = rig
        .session
        .cover(&host_request(&offered))
        .await
        .expect("the running preview's artwork")
        .expect("a custom cover");
    assert_eq!(
        shown.to_vec(),
        crate::metadata::render_display_thumbnail(&cover).expect("thumbnail")
    );
    assert_eq!(
        rig.send(SessionIntent::CancelPreview {
            run_id: id.to_string(),
            child_job_id: None
        })
        .await,
        SessionOutcome::Applied
    );
    let preview = wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Cancelled).await;
    assert!(preview.operation.cancel_requested);
    assert!(!preview.open_ready);
    assert!(preview
        .operation
        .children
        .iter()
        .all(|child| child.output_path.is_none()
            && child.progress.stage == crate::work_runtime::WorkProgressStage::Cancelled));
    assert_eq!(
        rig.send(SessionIntent::TakePreviewOutput {
            run_id: id.to_string()
        })
        .await,
        SessionOutcome::PreviewOutput { path: None }
    );
    assert!(!rig.session.snapshot().titles.expect("titles").order_locked);
    jobs.complete_job(first);
    jobs.complete_job(second);
    drop((first_permit, second_permit));
}

async fn imported_preview_rig(
    lengths: &[f64],
) -> (Rig, crate::test_cases::media_fixtures::MediaLane) {
    let rig = rig();
    let lane = crate::test_cases::media_fixtures::MediaLane::with_fixtures(lengths);
    let source = lane
        .process(Some(AudiobookMetadata {
            title: Some("Preview".to_string()),
            ..Default::default()
        }))
        .await;
    let previews = rig._config.path().join("previews");
    std::fs::create_dir_all(&previews).expect("preview folder");
    rig.send(SessionIntent::Import {
        paths: vec![source.to_string_lossy().into_owned()],
    })
    .await;
    rig.send(SessionIntent::SetOutputDirectory {
        directory: previews.to_string_lossy().into_owned(),
    })
    .await;
    (rig, lane)
}

fn accepted_preview(rig: &Rig) -> (String, String) {
    drop(rig.session.begin(SessionIntent::Preview));
    let preview = output(rig).preview_run.expect("accepted preview");
    let child = preview.operation.children[0].child_job_id.clone();
    (preview.operation.operation_id.to_string(), child)
}

#[tokio::test]
async fn a_preview_cancelled_before_it_starts_ends_cancelled() {
    let (rig, _lane) = imported_preview_rig(&[0.2]).await;
    let (id, _) = accepted_preview(&rig);

    assert_eq!(
        rig.send(SessionIntent::CancelPreview {
            run_id: id,
            child_job_id: None
        })
        .await,
        SessionOutcome::Applied
    );

    let preview = wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Cancelled).await;
    assert!(preview.operation.cancel_requested);
    assert!(!preview.open_ready);
    assert!(!rig.session.snapshot().titles.expect("titles").order_locked);
}

#[tokio::test]
async fn one_title_of_a_preview_cancelled_before_it_starts_ends_cancelled() {
    let (rig, _lane) = imported_preview_rig(&[0.2]).await;
    let (id, child) = accepted_preview(&rig);

    assert_eq!(
        rig.send(SessionIntent::CancelPreview {
            run_id: id,
            child_job_id: Some(child)
        })
        .await,
        SessionOutcome::Applied
    );

    let preview = wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Cancelled).await;
    assert!(!preview.open_ready);
    assert!(preview
        .operation
        .children
        .iter()
        .all(|child| child.output_path.is_none()));
}

#[tokio::test]
async fn one_title_of_a_running_preview_stops_on_its_own_cancel() {
    let (rig, _lane) = imported_preview_rig(&[0.2]).await;
    let jobs = &rig.session.inner.deps.jobs;
    let (first, first_permit) = jobs.register_job().await.expect("hold first slot");
    let (second, second_permit) = jobs.register_job().await.expect("hold second slot");
    let (id, child) = accepted_preview(&rig);
    wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Running).await;

    assert_eq!(
        rig.send(SessionIntent::CancelPreview {
            run_id: id,
            child_job_id: Some(child)
        })
        .await,
        SessionOutcome::Applied
    );

    let preview = wait_preview(&rig, crate::work_runtime::WorkOperationStatus::Cancelled).await;
    assert!(!preview.open_ready);
    jobs.complete_job(first);
    jobs.complete_job(second);
    drop((first_permit, second_permit));
}
