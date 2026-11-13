use super::*;
use abb_engine::remote_source::ProviderId;
use abb_engine::remote_source::{
    AcquisitionJob, AcquisitionJobState, AcquisitionSnapshot, JobEnding, RemoteTitleAvailability,
    RemoteTitleAvailabilityStatus,
};
use abb_remote_source_core::{acquisition_progress, AcquisitionStage};

fn job(
    handoff: Option<AcquisitionHandoff>,
    diagnostics: Vec<RemoteSourceDiagnostic>,
) -> AcquisitionSnapshot {
    AcquisitionSnapshot {
        job: AcquisitionJob {
            job_id: "job".into(),
            provider_id: ProviderId::Audible,
            progress: acquisition_progress(AcquisitionStage::Download, Some(1.0), None, None),
            diagnostics,
        },
        state: AcquisitionJobState::Done {
            ending: handoff.map_or(JobEnding::Failed, |handoff| JobEnding::Handoff { handoff }),
        },
    }
}

fn diagnostic(
    kind: RemoteAcquisitionFailureKind,
    title_id: Option<&str>,
    message: &str,
) -> RemoteSourceDiagnostic {
    RemoteSourceDiagnostic {
        kind,
        title_id: title_id.map(str::to_string),
        message: message.to_string(),
    }
}

fn path(name: &str) -> Vec<PathBuf> {
    vec![PathBuf::from(name)]
}

#[test]
fn an_imported_title_lists_its_files_and_keeps_non_blocking_diagnostics_as_warnings() {
    let job = job(
        Some(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
            count: 1,
        })),
        vec![diagnostic(
            RemoteAcquisitionFailureKind::SupplementalPdfUnavailable,
            Some("B0A"),
            "no PDF offered",
        )],
    );
    let outcome = title_outcome("B0A", &job, path("/state/book.m4b"));
    assert_eq!(outcome.status, OutcomeStatus::Imported);
    assert_eq!(
        outcome_text(&outcome),
        "B0A  imported\n  /state/book.m4b\n  warning: SupplementalPdfUnavailable: no PDF offered"
    );
}

#[test]
fn a_failed_title_reports_the_first_blocking_diagnostic_for_it_or_for_the_whole_job() {
    let job = job(
        None,
        vec![
            diagnostic(
                RemoteAcquisitionFailureKind::LicenseDenied,
                Some("B0OTHER"),
                "other",
            ),
            diagnostic(
                RemoteAcquisitionFailureKind::ProtectedSourcePurgeFailed,
                Some("B0A"),
                "purge",
            ),
            diagnostic(
                RemoteAcquisitionFailureKind::MaterializationFailed,
                Some("B0A"),
                "decrypt failed",
            ),
        ],
    );
    let outcome = title_outcome("B0A", &job, Vec::new());
    assert_eq!(outcome.status, OutcomeStatus::Failed);
    assert_eq!(
        outcome_text(&outcome),
        "B0A  failed: MaterializationFailed\n  decrypt failed"
    );

    let job_wide = self::job(
        None,
        vec![diagnostic(
            RemoteAcquisitionFailureKind::AuthRequired,
            None,
            "sign in again",
        )],
    );
    assert_eq!(
        outcome_text(&title_outcome("B0A", &job_wide, Vec::new())),
        "B0A  failed: AuthRequired\n  sign in again"
    );
}

#[test]
fn a_title_with_no_files_and_no_diagnostic_still_fails_with_a_reason() {
    let removed = job(
        Some(AcquisitionHandoff::Titles(TitlesHandoff::Removed {
            reason: HandoffRefusal::NothingAdded,
        })),
        Vec::new(),
    );
    assert_eq!(
        outcome_text(&title_outcome("B0A", &removed, Vec::new())),
        "B0A  failed\n  the session already listed these files"
    );
    let imported_nothing = job(
        Some(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
            count: 0,
        })),
        Vec::new(),
    );
    assert_eq!(
        outcome_text(&title_outcome("B0A", &imported_nothing, Vec::new())),
        "B0A  failed\n  the acquisition ended with no imported file"
    );
}

#[test]
fn json_outcomes_name_the_title_by_id_and_by_the_reference_the_logs_use() {
    let outcome = title_outcome(
        "B0A",
        &job(
            Some(AcquisitionHandoff::Titles(TitlesHandoff::Imported {
                count: 1,
            })),
            Vec::new(),
        ),
        path("/state/book.m4b"),
    );
    let json = serde_json::to_value(&outcome).expect("outcome json");
    assert_eq!(json["titleId"], "B0A");
    assert_eq!(json["titleRef"], title_ref("B0A"));
    assert_eq!(json["status"], "imported");
    assert_eq!(
        json["importedPaths"],
        serde_json::json!(["/state/book.m4b"])
    );
    assert_eq!(json["failureKind"], serde_json::Value::Null);
}

#[test]
fn library_rows_show_what_the_engine_reports_and_flag_a_title_that_cannot_be_acquired() {
    let mut title = RemoteTitle {
        provider_id: ProviderId::Audible,
        title_id: "B0A".into(),
        title: "A Book".into(),
        authors: vec!["Ann".into(), "Bo".into()],
        narrators: vec!["Cy".into()],
        cover: None,
        supplemental_pdf_available: true,
        availability: RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::Available,
            acquirable: true,
            label: "Available".into(),
            detail: None,
        },
    };
    assert_eq!(
        library_text(&LibraryRow::from(&title)),
        "B0A  A Book / Ann, Bo (read by Cy)  [pdf]"
    );
    title.availability.acquirable = false;
    title.availability.label = "Catalog only".into();
    assert!(library_text(&LibraryRow::from(&title)).ends_with("[not acquirable: Catalog only]"));
}

#[test]
fn a_pdf_save_reports_where_the_pdf_landed_or_the_reason_it_did_not() {
    let saved = job(
        Some(AcquisitionHandoff::Pdf(PdfHandoff::Saved {
            path: PathBuf::from("/library/Book - Supplemental PDF.pdf"),
        })),
        Vec::new(),
    );
    assert_eq!(
        pdf_outcome("B0A", &saved),
        PdfOutcome::Saved {
            title_id: "B0A".into(),
            path: PathBuf::from("/library/Book - Supplemental PDF.pdf"),
        }
    );
    let download_failed = job(
        None,
        vec![diagnostic(
            RemoteAcquisitionFailureKind::SupplementalPdfFailed,
            Some("B0A"),
            "The Supplemental PDF was not saved.",
        )],
    );
    assert_eq!(
        pdf_outcome("B0A", &download_failed),
        PdfOutcome::Failed {
            title_id: "B0A".into(),
            failure_kind: Some(RemoteAcquisitionFailureKind::SupplementalPdfFailed),
            message: "The Supplemental PDF was not saved.".into(),
        }
    );
    let json = serde_json::to_value(pdf_outcome("B0A", &saved)).expect("json");
    assert_eq!(json["status"], "saved");
    assert_eq!(json["titleId"], "B0A");
}
