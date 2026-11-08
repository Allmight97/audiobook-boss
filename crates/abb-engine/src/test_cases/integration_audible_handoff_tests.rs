use std::path::PathBuf;
use std::sync::Arc;

use tempfile::TempDir;

use super::support::form::field_of;
use super::support::poll::until;
use crate::app_settings::SettingsRuntime;
use crate::audio::{AudioIntent, EncoderType};
use crate::power::PowerManager;
use crate::remote_source::tests::{offer_library_title, recorded_job_ids, test_runtime_acquiring};
use crate::remote_source::{
    AcquireTitles, AcquisitionHandoff, AcquisitionOutcome, AcquisitionSnapshot,
    MaterializedSourceFile, RemoteAcquisitionStatus, RemoteSourceRuntime, RemoteUiIntent,
};
use crate::session::{AudioEdit, MetadataField, Session, SessionDeps, SessionIntent};
use crate::work_runtime::WorkRuntime;

const TITLE_ID: &str = "B0FIXTURE";

fn downloads_one_wav(seconds: u32, title: Option<&'static str>) -> AcquireTitles {
    Arc::new(move |request| {
        Box::pin(async move {
            let path = request.job_dir.join("book.wav");
            super::support::wav::write_wav(&path, seconds, title, None);
            Ok(AcquisitionOutcome {
                files: vec![MaterializedSourceFile {
                    input_id: "remote-1".into(),
                    title_id: TITLE_ID.into(),
                    path,
                    size_bytes: 0,
                    sha256: String::new(),
                }],
                ..AcquisitionOutcome::default()
            })
        })
    })
}

fn session_acquiring(root: &TempDir, acquire: AcquireTitles) -> (Session, RemoteSourceRuntime) {
    let (settings, jobs, _) =
        SettingsRuntime::start(root.path().join("config"), PowerManager::default());
    let remote = test_runtime_acquiring(root, settings.clone(), acquire);
    let tasks = crate::engine_tasks::EngineTasks::default();
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
    (session, remote)
}

async fn acquire(session: &Session) -> Box<AcquisitionSnapshot> {
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
    until("the download to reach the list", || {
        let job = session.snapshot().remote_acquisition?.job?;
        job.job.handoff.is_some().then_some(job)
    })
    .await
}

fn shown(session: &Session, field: MetadataField) -> String {
    let metadata = session.snapshot().metadata.expect("metadata part");
    field_of(&metadata.form, field).value
}

#[tokio::test]
async fn an_acquired_title_reaches_the_list_and_its_download_leaves_with_it() {
    let root = TempDir::new().expect("engine root");
    let (session, remote) = session_acquiring(&root, downloads_one_wav(1, None));

    let job = acquire(&session).await;
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

#[tokio::test]
async fn an_edit_left_unsaved_during_an_export_outlives_the_download_and_reaches_the_output() {
    let root = TempDir::new().expect("engine root");
    let (session, remote) = session_acquiring(&root, downloads_one_wav(900, Some("Book")));
    let job = acquire(&session).await;
    let staged = remote.staging_root().join(&job.job.job_id);
    let exports = root.path().join("exports");
    std::fs::create_dir_all(&exports).expect("export folder");
    session
        .dispatch(SessionIntent::SetOutputDirectory {
            directory: exports.to_string_lossy().into_owned(),
        })
        .await;
    let title_ids: Vec<String> = session
        .snapshot()
        .titles
        .expect("titles")
        .files
        .into_iter()
        .map(|file| file.input_id)
        .collect();
    for edit in [
        AudioEdit::Intent(AudioIntent::Encode),
        AudioEdit::Encoder(EncoderType::NativeAac),
    ] {
        let title_ids = title_ids.clone();
        session
            .dispatch(SessionIntent::SetTitleAudio { title_ids, edit })
            .await;
    }
    session.dispatch(SessionIntent::Submit).await;
    session
        .dispatch(SessionIntent::SetField {
            binding: session.snapshot().metadata.expect("metadata part").binding,
            field: MetadataField::Genre,
            value: "Mystery".into(),
        })
        .await;
    assert!(staged.exists(), "typed while the export reads the download");

    until("the finished export's download to leave", || {
        let titles = session.snapshot().titles?;
        titles.files[0].source_removed.then_some(())
    })
    .await;
    assert_eq!(shown(&session, MetadataField::Genre), "Mystery");
    assert_eq!(shown(&session, MetadataField::Title), "Book");

    session
        .dispatch(SessionIntent::SetField {
            binding: session.snapshot().metadata.expect("metadata part").binding,
            field: MetadataField::Narrator,
            value: "Reader".into(),
        })
        .await;
    session.dispatch(SessionIntent::Save).await;

    let output = exports.join("Unknown Author/Book/Book.m4b");
    let tags = until("the Save to reach the finished output", || {
        let tags = crate::metadata::read_metadata(&output).ok()?;
        tags.composer.is_some().then_some(tags)
    })
    .await;
    assert_eq!(tags.genre.as_deref(), Some("Mystery"));
    assert_eq!(tags.composer.as_deref(), Some("Reader"));
}
