use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;

use crate::app_settings::SettingsRuntime;
use crate::power::PowerManager;
use crate::remote_source::tests::{offer_library_title, recorded_job_ids, test_runtime_acquiring};
use crate::remote_source::{
    AcquireTitles, AcquisitionHandoff, AcquisitionJob, MaterializedSourceFile, ProviderId,
    RemoteAcquisitionStatus, RemoteUiIntent,
};
use crate::session::{Session, SessionDeps, SessionIntent};
use crate::work_runtime::WorkRuntime;

const TITLE_ID: &str = "B0FIXTURE";

fn downloads_one_wav() -> AcquireTitles {
    Arc::new(|request| {
        Box::pin(async move {
            let path = request.job_dir.join("book.wav");
            super::wav_fixture::write_wav(&path, 1, None, None);
            Ok(AcquisitionJob {
                job_id: request.job_id,
                provider_id: ProviderId::Audible,
                status: RemoteAcquisitionStatus::Validated,
                progress: abb_remote_source_core::acquisition_progress(
                    abb_remote_source_core::AcquisitionStage::Complete,
                    Some(1.0),
                    None,
                    None,
                ),
                materialized_files: vec![MaterializedSourceFile {
                    input_id: "remote-1".into(),
                    title_id: TITLE_ID.into(),
                    path,
                    size_bytes: 0,
                    sha256: String::new(),
                }],
                supplemental_assets: Vec::new(),
                diagnostics: Vec::new(),
                handoff: None,
            })
        })
    })
}

async fn until<T>(what: &str, mut ready: impl FnMut() -> Option<T>) -> T {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(found) = ready() {
                return found;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

#[tokio::test]
async fn an_acquired_title_reaches_the_list_and_its_download_leaves_with_it() {
    let root = TempDir::new().expect("engine root");
    let (settings, jobs, _) =
        SettingsRuntime::start(root.path().join("config"), PowerManager::default());
    let remote = test_runtime_acquiring(&root, settings.clone(), downloads_one_wav());
    let tasks = crate::engine::EngineTasks::default();
    let session = Session::new(SessionDeps {
        host: crate::host::Host::new(Arc::new(crate::DiscardEvents), PowerManager::default()),
        remote: remote.clone(),
        work: WorkRuntime::new(tasks.clone()),
        jobs,
        temporary_root: remote.staging_root(),
        settings,
        tasks,
        workspace_root: crate::audio::processing_workspace_root(root.path()),
        remove_staged: {
            let remote = remote.clone();
            Arc::new(move |job_id| remote.purge_session(job_id))
        },
    });
    remote.set_handoff(session.handoff());
    session.start_from_defaults(None, Some(crate::audio::encoder_settings_capabilities()));
    offer_library_title(&remote, TITLE_ID);

    session
        .dispatch(SessionIntent::Remote {
            intent: RemoteUiIntent::ToggleTitle {
                title_id: TITLE_ID.into(),
            },
        })
        .await;
    session
        .dispatch(SessionIntent::Remote {
            intent: RemoteUiIntent::AcquireSelected,
        })
        .await;

    let job = until("the download to reach the list", || {
        let job = session.snapshot().remote_acquisition?.job?;
        job.job.handoff.is_some().then_some(job)
    })
    .await;
    assert_eq!(job.job.status, RemoteAcquisitionStatus::ImportedToFileList);
    assert_eq!(
        job.job.handoff,
        Some(AcquisitionHandoff::Imported { count: 1 })
    );
    let titles = session.snapshot().titles.expect("titles");
    assert_eq!(titles.files.len(), 1);
    assert!(titles.files[0].is_valid);
    let staged: PathBuf = remote.staging_root().join(&job.job.job_id);
    assert_eq!(
        std::fs::canonicalize(&titles.files[0].path).expect("listed file exists"),
        std::fs::canonicalize(staged.join("book.wav")).expect("downloaded file exists")
    );

    session.dispatch(SessionIntent::ClearAll).await;

    until("the download folder to leave with the title", || {
        (!staged.exists()).then_some(())
    })
    .await;
    until("the job record to be dropped", || {
        recorded_job_ids(&remote).is_empty().then_some(())
    })
    .await;
}
