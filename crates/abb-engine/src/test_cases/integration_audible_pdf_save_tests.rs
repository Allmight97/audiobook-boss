use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use abb_remote_source_core::AcquisitionStage;
use tempfile::TempDir;

use super::integration_audible_handoff_tests::session_and_settings_acquiring;
use super::support::fs_listing::files_under;
use super::support::poll::until;
use crate::app_settings::SettingsRuntime;
use crate::output_artifact::NamingPreset;
use crate::remote_source::tests::{
    offer_library_title, offer_pdf_title, offer_unacquirable_pdf_title,
};
use crate::remote_source::{
    AcquireTitles, AcquisitionHandoff, AcquisitionOutcome, AcquisitionSnapshot, AcquisitionWork,
    PdfHandoff, PdfTitle, RemoteAcquisitionFailureKind, RemoteSourceDiagnostic,
    RemoteSourceRuntime, RemoteUiIntent, StagedPdf,
};
use crate::session::{Session, SessionIntent, SessionOutcome};

const TITLE_ID: &str = "B0PDFONLY";
const PDF_BYTES: &[u8] = b"%PDF-1.7 companion guide";
const FAILURE: &str =
    "Audible Supplemental PDF could not be downloaded. The Supplemental PDF was not saved.";

enum Download {
    Pdf,
    PdfAfter(tokio::sync::oneshot::Receiver<()>),
    Changed,
    Fails,
}

/// Stands in for Audible's companion-file route: it counts its calls and
/// answers a PDF job with a staged PDF, a staged PDF whose bytes no longer
/// match the digest it reports, or the failure a real download reports.
fn audible_pdf(calls: Arc<AtomicUsize>, download: Download) -> AcquireTitles {
    let download = Arc::new(Mutex::new(Some(download)));
    Arc::new(move |request| {
        calls.fetch_add(1, Ordering::SeqCst);
        let download = download.lock().expect("download").take();
        Box::pin(async move {
            assert_eq!(
                request.work,
                AcquisitionWork::Pdf {
                    title_id: TITLE_ID.into()
                }
            );
            let path = request.job_dir.join("companion.pdf");
            std::fs::write(&path, PDF_BYTES).expect("stage the PDF");
            let download = download.unwrap_or(Download::Pdf);
            let sha256 = match download {
                Download::Changed => abb_media_core::sha256_hex(b"other bytes"),
                _ => abb_media_core::sha256_hex(PDF_BYTES),
            };
            match download {
                Download::Fails => {
                    return Ok(AcquisitionOutcome::refused(RemoteSourceDiagnostic {
                        kind: RemoteAcquisitionFailureKind::SupplementalPdfFailed,
                        title_id: Some(TITLE_ID.into()),
                        message: FAILURE.into(),
                    }));
                }
                Download::PdfAfter(release) => release.await.expect("the test releases it"),
                Download::Pdf | Download::Changed => {}
            }
            Ok(AcquisitionOutcome::pdf(
                StagedPdf {
                    path,
                    size_bytes: PDF_BYTES.len() as u64,
                    sha256,
                },
                PdfTitle {
                    title: "Book Two".into(),
                    authors: vec!["Ann Author".into()],
                    series: Some("Saga".into()),
                    series_part: Some("2".into()),
                    release_date: Some("2021-03-04".into()),
                },
            ))
        })
    })
}

struct Rig {
    root: TempDir,
    session: Session,
    remote: RemoteSourceRuntime,
    settings: SettingsRuntime,
    calls: Arc<AtomicUsize>,
}

impl Rig {
    fn new(download: Download) -> Self {
        let root = TempDir::new().expect("engine root");
        let calls = Arc::new(AtomicUsize::new(0));
        let (session, remote, settings) =
            session_and_settings_acquiring(&root, audible_pdf(calls.clone(), download));
        offer_pdf_title(&remote, TITLE_ID);
        Self {
            root,
            session,
            remote,
            settings,
            calls,
        }
    }

    fn output(&self) -> PathBuf {
        self.root.path().join("library")
    }

    async fn choose_output(&self) {
        std::fs::create_dir_all(self.output()).expect("output folder");
        self.set_output(self.output().to_string_lossy().into_owned())
            .await;
    }

    async fn set_output(&self, directory: String) {
        let reply = self
            .session
            .dispatch(SessionIntent::SetOutputDirectory { directory })
            .await;
        assert_eq!(reply.outcome, SessionOutcome::Applied);
    }

    async fn name_with_template(&self, template: &str) {
        for intent in [
            SessionIntent::SetNamingPreset {
                preset: NamingPreset::CustomTemplate,
            },
            SessionIntent::SetNamingTemplate {
                template: template.into(),
            },
        ] {
            assert_eq!(
                self.session.dispatch(intent).await.outcome,
                SessionOutcome::Applied
            );
        }
        until("the settings to hold the template", || {
            let naming = self.settings.output_defaults().output_naming;
            (naming.custom_template.as_deref() == Some(template)).then_some(())
        })
        .await;
    }

    async fn save_pdf(&self) -> SessionOutcome {
        self.session
            .dispatch(SessionIntent::Remote {
                intent: RemoteUiIntent::SavePdf {
                    title_id: TITLE_ID.into(),
                },
            })
            .await
            .outcome
    }

    fn job(&self) -> Option<Box<AcquisitionSnapshot>> {
        self.session.snapshot().remote_acquisition?.job
    }

    async fn settled_after(&self, previous: Option<String>) -> Box<AcquisitionSnapshot> {
        until("the PDF job to settle", || {
            self.job()
                .filter(|job| job.settled && Some(&job.job_id) != previous.as_ref())
        })
        .await
    }

    async fn staging_gone(&self, job_id: &str) {
        let staged = self.remote.staging_root().join(job_id);
        until("the staged PDF to be removed", || {
            (!staged.exists()).then_some(())
        })
        .await;
    }

    /// Waits until the job's download is under way: its PDF is staged and the
    /// fake waits to be released.
    async fn downloading(&self) -> Box<AcquisitionSnapshot> {
        until("the PDF download to start", || {
            self.job().filter(|job| {
                !job.terminal
                    && self
                        .remote
                        .staging_root()
                        .join(&job.job_id)
                        .join("companion.pdf")
                        .exists()
            })
        })
        .await
    }

    fn audible_message(&self) -> String {
        self.session
            .snapshot()
            .remote
            .expect("remote part")
            .audible_message
    }
}

fn refusal(outcome: SessionOutcome) -> String {
    match outcome {
        SessionOutcome::Rejected { error } => error.message,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

fn saved_path(job: &AcquisitionSnapshot) -> &Path {
    match &job.handoff {
        Some(AcquisitionHandoff::Pdf(PdfHandoff::Saved { path })) => path,
        other => panic!("expected a saved PDF, got {other:?}"),
    }
}

#[tokio::test]
async fn a_saved_pdf_lands_where_its_audiobook_would_go_and_a_second_save_takes_the_next_name() {
    let rig = Rig::new(Download::Pdf);
    rig.choose_output().await;

    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let first = rig.settled_after(None).await;
    rig.staging_gone(&first.job_id).await;

    let folder = rig.output().join("Ann Author/Saga/Book 2 - Book Two");
    let expected = folder.join("Book 2 - Book Two - Supplemental PDF.pdf");
    assert_eq!(saved_path(&first), expected);
    assert_eq!(std::fs::read(&expected).expect("saved PDF"), PDF_BYTES);
    let remote = rig.session.snapshot().remote.expect("remote part");
    assert!(!remote.busy);
    assert!(remote.selected_title_ids.is_empty());
    assert_eq!(remote.include_pdf_by_title_id.get(TITLE_ID), Some(&true));
    assert!(
        rig.audible_message()
            .contains(&expected.display().to_string()),
        "{}",
        rig.audible_message()
    );
    let titles = rig.session.snapshot().titles.expect("titles part");
    assert!(titles.files.is_empty(), "a PDF save imports no audiobook");

    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let second = rig.settled_after(Some(first.job_id.clone())).await;
    rig.staging_gone(&second.job_id).await;

    let renamed = folder.join("Book 2 - Book Two - Supplemental PDF (2).pdf");
    assert_eq!(saved_path(&second), renamed);
    let mut saved = vec![expected, renamed];
    saved.sort();
    assert_eq!(files_under(&rig.output()), saved);
}

#[tokio::test]
async fn a_saved_pdf_follows_a_custom_naming_template() {
    let rig = Rig::new(Download::Pdf);
    rig.choose_output().await;
    rig.name_with_template("{series}/{year} - {title}").await;

    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let job = rig.settled_after(None).await;

    let expected = rig
        .output()
        .join("Saga/2021 - Book Two - Supplemental PDF.pdf");
    assert_eq!(saved_path(&job), expected);
    assert_eq!(files_under(&rig.output()), vec![expected]);
}

#[tokio::test]
async fn a_pdf_that_fails_its_publish_check_leaves_no_folder_in_the_library() {
    let rig = Rig::new(Download::Changed);
    rig.choose_output().await;

    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let job = rig.settled_after(None).await;
    rig.staging_gone(&job.job_id).await;

    let Some(AcquisitionHandoff::Pdf(PdfHandoff::NotSaved { error })) = &job.handoff else {
        panic!("expected an unsaved PDF, got {:?}", job.handoff);
    };
    assert!(error.message.contains("hash changed"), "{}", error.message);
    let entries: Vec<_> = std::fs::read_dir(rig.output())
        .expect("library folder")
        .collect();
    assert!(entries.is_empty(), "{entries:?}");
}

#[tokio::test]
async fn a_pdf_save_with_no_output_folder_is_refused_before_anything_downloads() {
    let rig = Rig::new(Download::Pdf);

    let message = refusal(rig.save_pdf().await);

    assert!(message.contains("output folder"), "{message}");
    assert_eq!(rig.calls.load(Ordering::SeqCst), 0);
    assert!(rig.job().is_none());
}

#[tokio::test]
async fn a_title_without_a_pdf_is_refused() {
    let rig = Rig::new(Download::Pdf);
    offer_library_title(&rig.remote, TITLE_ID);
    rig.choose_output().await;

    let message = refusal(rig.save_pdf().await);

    assert!(message.contains("no supplemental PDF"), "{message}");
    assert_eq!(rig.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_pdf_save_is_refused_for_a_title_that_cannot_be_acquired() {
    let rig = Rig::new(Download::Pdf);
    offer_unacquirable_pdf_title(&rig.remote, TITLE_ID);
    rig.choose_output().await;

    let message = refusal(rig.save_pdf().await);

    assert!(
        message.contains("not available for acquisition"),
        "{message}"
    );
    assert_eq!(rig.calls.load(Ordering::SeqCst), 0);
    assert!(rig.job().is_none());
}

#[tokio::test]
async fn a_failed_pdf_download_shows_why_saves_nothing_and_leaves_no_staged_file() {
    let rig = Rig::new(Download::Fails);
    rig.choose_output().await;

    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let job = rig.settled_after(None).await;
    rig.staging_gone(&job.job_id).await;

    assert_eq!(job.progress.stage, AcquisitionStage::Failed);
    assert_eq!(job.handoff, None);
    assert!(rig.audible_message().contains(FAILURE));
    assert!(files_under(&rig.output()).is_empty());
    assert!(!rig.session.snapshot().remote.expect("remote part").busy);
}

#[tokio::test]
async fn a_pdf_whose_output_folder_was_cleared_meanwhile_is_not_saved_and_its_download_goes() {
    let (release, released) = tokio::sync::oneshot::channel();
    let rig = Rig::new(Download::PdfAfter(released));
    rig.choose_output().await;

    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    rig.set_output(String::new()).await;
    release.send(()).expect("the download waits");
    let job = rig.settled_after(None).await;
    rig.staging_gone(&job.job_id).await;

    let Some(AcquisitionHandoff::Pdf(PdfHandoff::NotSaved { error })) = &job.handoff else {
        panic!("expected an unsaved PDF, got {:?}", job.handoff);
    };
    assert!(error.message.contains("output folder"), "{}", error.message);
    assert!(rig.audible_message().contains(&error.message));
    assert!(files_under(&rig.output()).is_empty());
}

#[tokio::test]
async fn a_pdf_save_is_refused_while_the_account_is_changing() {
    let rig = Rig::new(Download::Pdf);
    rig.choose_output().await;
    let _signing_in = rig.session.begin(SessionIntent::Remote {
        intent: RemoteUiIntent::StartAuth,
    });

    let message = refusal(rig.save_pdf().await);

    assert!(message.contains("account change"), "{message}");
    assert_eq!(rig.calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_pdf_save_is_refused_while_an_acquisition_is_running() {
    let (release, released) = tokio::sync::oneshot::channel();
    let rig = Rig::new(Download::PdfAfter(released));
    rig.choose_output().await;
    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let running = rig.downloading().await;

    let message = refusal(rig.save_pdf().await);

    assert!(message.contains("current Audible acquisition"), "{message}");
    assert_eq!(rig.calls.load(Ordering::SeqCst), 1);
    release.send(()).expect("the download waits");
    let settled = rig.settled_after(None).await;
    assert_eq!(settled.job_id, running.job_id);
    assert_eq!(rig.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn a_pdf_save_cancelled_mid_download_saves_nothing_and_leaves_no_staged_file() {
    let (_release, released) = tokio::sync::oneshot::channel();
    let rig = Rig::new(Download::PdfAfter(released));
    rig.choose_output().await;
    assert_eq!(rig.save_pdf().await, SessionOutcome::Applied);
    let running = rig.downloading().await;

    let reply = rig
        .session
        .dispatch(SessionIntent::Remote {
            intent: RemoteUiIntent::CancelAcquisition {
                job_id: running.job_id.clone(),
            },
        })
        .await;
    assert_eq!(reply.outcome, SessionOutcome::Applied);
    let job = until("the cancelled job to settle", || {
        rig.job().filter(|job| job.settled)
    })
    .await;
    rig.staging_gone(&job.job_id).await;

    assert_eq!(job.progress.stage, AcquisitionStage::Cancelled);
    assert_eq!(job.handoff, None);
    assert!(files_under(&rig.output()).is_empty());
    assert!(!rig.session.snapshot().remote.expect("remote part").busy);
}
