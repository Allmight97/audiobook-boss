use crate::remote_source::job_state::Registered;
use crate::remote_source::tests::test_runtime;
use crate::remote_source::types::{
    AcquisitionPlan, AcquisitionSelection, RemoteAcquisitionFailureKind,
};
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
