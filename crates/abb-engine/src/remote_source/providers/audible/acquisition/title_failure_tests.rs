use super::progress::TitleProgressContext;
use super::*;
use crate::remote_source::tests::{offer_library_title, test_runtime_acquiring};
use crate::remote_source::{AcquireTitles, RemoteUiIntent};
use std::sync::{Arc, Mutex};

struct Pause {
    arrived: tokio::sync::oneshot::Sender<()>,
    release: tokio::sync::oneshot::Receiver<()>,
}

fn acquire_unreadable_titles(count: u32, pause_after_first: Option<Pause>) -> AcquireTitles {
    let pause = Arc::new(Mutex::new(pause_after_first));
    Arc::new(move |request| {
        let pause = pause.clone();
        Box::pin(async move {
            let lifecycle = &request.runtime.inner.lifecycle;
            let mut job = AcquisitionOutcome::default();
            for index in 0..count {
                let title_id = format!("B0{index}");
                let path = request.job_dir.join(format!("{title_id}.m4b"));
                std::fs::write(&path, b"not audio").expect("write unreadable title");
                let ctx = TitleAcquisitionCtx {
                    job_id: &request.job_id,
                    title_id: &title_id,
                    item_dir: &request.job_dir,
                    progress_context: TitleProgressContext {
                        title_id: &title_id,
                        item_index: index + 1,
                        total_items: count,
                    },
                };
                let outcome =
                    validation::validate_materialized_audio(&path, ctx, &mut |progress| {
                        lifecycle.update_job_progress(&request.job_id, progress);
                    })
                    .await
                    .map(|file| AcquiredTitle {
                        file: Some(file),
                        assets: Vec::new(),
                        diagnostics: Vec::new(),
                    })
                    .map_err(AudibleAcquisitionError::validation);
                record_title_outcome(&mut job, &request.job_id, &title_id, outcome)?;
                let pause = pause.lock().expect("pause").take();
                if let (0, Some(Pause { arrived, release })) = (index, pause) {
                    arrived.send(()).expect("test waits for the pause");
                    release.await.expect("test releases the pause");
                }
            }
            Ok(job)
        })
    })
}

fn published_job(
    runtime: &crate::remote_source::RemoteSourceRuntime,
) -> crate::remote_source::AcquisitionSnapshot {
    let mut update = crate::session::SessionUpdate::default();
    runtime.add_ui_parts(&mut update, None);
    *update
        .remote_acquisition
        .expect("acquisition part")
        .job
        .expect("published job")
}

async fn start_acquiring(runtime: &crate::remote_source::RemoteSourceRuntime, titles: u32) {
    for index in 0..titles {
        let title_id = format!("B0{index}");
        offer_library_title(runtime, &title_id);
        runtime
            .ui_begin(RemoteUiIntent::ToggleTitle { title_id })
            .finish()
            .await
            .expect("select title");
    }
    runtime
        .ui_begin(RemoteUiIntent::AcquireSelected)
        .finish()
        .await
        .expect("acquisition accepted");
}

async fn until_the_job_has_ended(runtime: &crate::remote_source::RemoteSourceRuntime) {
    runtime.inner.tasks.close();
    runtime.inner.tasks.wait().await;
}

#[tokio::test]
async fn a_title_that_fails_validation_ends_the_job_with_its_reason_in_the_status_line() {
    let root = tempfile::TempDir::new().expect("temp root");
    let (settings, _, _) = crate::app_settings::SettingsRuntime::start(
        root.path().join("config"),
        crate::power::PowerManager::default(),
    );
    let runtime = test_runtime_acquiring(&root, settings, acquire_unreadable_titles(1, None));
    start_acquiring(&runtime, 1).await;
    until_the_job_has_ended(&runtime).await;

    let job = published_job(&runtime);
    let reason = job.diagnostics[0].message.clone();
    assert!(!reason.is_empty());
    let line = runtime.ui_snapshot().audible_message;
    assert!(line.contains(&reason), "{line:?} should carry {reason:?}");
}

#[tokio::test]
async fn a_title_failing_mid_job_does_not_settle_the_job_while_later_titles_run() {
    let root = tempfile::TempDir::new().expect("temp root");
    let (settings, _, _) = crate::app_settings::SettingsRuntime::start(
        root.path().join("config"),
        crate::power::PowerManager::default(),
    );
    let (arrived, first_title_failed) = tokio::sync::oneshot::channel();
    let (release_job, release) = tokio::sync::oneshot::channel();
    let runtime = test_runtime_acquiring(
        &root,
        settings,
        acquire_unreadable_titles(2, Some(Pause { arrived, release })),
    );

    start_acquiring(&runtime, 2).await;
    first_title_failed.await.expect("the first title failed");

    let running = published_job(&runtime);
    assert!(!running.terminal, "{:?}", running.progress);
    assert!(!running.settled);
    assert!(runtime.ui_snapshot().busy);

    release_job.send(()).expect("resume the job");
    until_the_job_has_ended(&runtime).await;
    let settled = published_job(&runtime);
    assert_eq!(settled.diagnostics.len(), 2);
}
