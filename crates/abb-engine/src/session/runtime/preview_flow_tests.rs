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
