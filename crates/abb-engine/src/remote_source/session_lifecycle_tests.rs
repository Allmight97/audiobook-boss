use crate::remote_source::job_state::Registered;
use crate::remote_source::tests::{recorded_job_ids, test_runtime, test_runtime_acquiring};
use crate::remote_source::types::{
    AcquisitionPlan, AcquisitionSelection, RemoteAcquisitionFailureKind,
};
use crate::remote_source::AcquireTitles;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

async fn settled_diagnostics(
    runtime: &crate::remote_source::RemoteSourceRuntime,
) -> Vec<RemoteAcquisitionFailureKind> {
    for _ in 0..500 {
        {
            let jobs = runtime.inner.lifecycle.jobs.lock().expect("jobs");
            if let Some(job) = jobs.values().next().filter(|job| Registered::settled(job)) {
                return job
                    .snapshot()
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.kind.clone())
                    .collect();
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the acquisition never settled");
}

#[tokio::test]
async fn an_acquisition_with_no_stored_sign_in_fails_as_auth_required() {
    let root = TempDir::new().expect("temp root");
    let runtime = test_runtime(&root);

    runtime
        .start_acquisition(AcquisitionPlan {
            selections: vec![AcquisitionSelection {
                title_id: "B000000001".to_string(),
                include_supplemental_pdf: false,
            }],
        })
        .await
        .expect("the job starts");

    assert_eq!(
        settled_diagnostics(&runtime).await,
        [RemoteAcquisitionFailureKind::AuthRequired]
    );
}

#[expect(
    clippy::disallowed_methods,
    reason = "moves a test's own staging folder, never a user file"
)]
fn move_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::rename(from, to).expect("move the folder");
}

fn acquire_until_cancelled(arrived: tokio::sync::oneshot::Sender<()>) -> AcquireTitles {
    let arrived = Arc::new(Mutex::new(Some(arrived)));
    Arc::new(move |request| {
        let arrived = arrived.clone();
        Box::pin(async move {
            std::fs::write(request.job_dir.join("book.m4b.partial"), b"partial").expect("partial");
            if let Some(arrived) = arrived.lock().expect("arrived").take() {
                arrived.send(()).expect("test waits for the job");
            }
            std::future::pending().await
        })
    })
}

#[tokio::test(start_paused = true)]
async fn a_cancel_removes_the_staging_folder_even_when_the_first_purges_are_refused() {
    let root = TempDir::new().expect("temp root");
    let (settings, _, _) = crate::app_settings::SettingsRuntime::start(
        root.path().join("config"),
        crate::power::PowerManager::default(),
    );
    let (arrived, at_work) = tokio::sync::oneshot::channel();
    let runtime = test_runtime_acquiring(&root, settings, acquire_until_cancelled(arrived));
    runtime
        .start_acquisition(AcquisitionPlan {
            selections: vec![AcquisitionSelection {
                title_id: "B000000001".to_string(),
                include_supplemental_pdf: false,
            }],
        })
        .await
        .expect("the job starts");
    at_work.await.expect("the job is running");
    let job_id = recorded_job_ids(&runtime).remove(0);
    let job_dir = runtime.inner.lifecycle.staging.session_root().join(&job_id);
    let held = job_dir.with_extension("held");
    move_dir(&job_dir, &held);
    std::os::unix::fs::symlink(&held, &job_dir).expect("the purge refuses a symlink");

    runtime.cancel_acquisition(&job_id).expect("cancel");
    tokio::task::yield_now().await;
    assert!(
        job_dir.symlink_metadata().is_ok(),
        "the first purge attempt is refused"
    );

    runtime.inner.tasks.spawn(async move {
        tokio::time::sleep(Duration::from_millis(250)).await;
        std::fs::remove_file(&job_dir).expect("remove the symlink");
        move_dir(&held, &job_dir);
    });
    runtime.inner.tasks.close();
    runtime.inner.tasks.wait().await;

    let job_dir = runtime.inner.lifecycle.staging.session_root().join(&job_id);
    assert!(
        !job_dir.exists(),
        "a retry removes the folder once the refusal clears"
    );
}
