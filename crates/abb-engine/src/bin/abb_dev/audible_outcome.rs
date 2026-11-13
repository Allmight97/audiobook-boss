use std::path::PathBuf;

use abb_audible_core::title_ref;
use abb_engine::remote_source::{
    AcquisitionHandoff, AcquisitionSnapshot, HandoffRefusal, PdfHandoff,
    RemoteAcquisitionFailureKind, RemoteSourceDiagnostic, RemoteTitle, TitlesHandoff,
};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum OutcomeStatus {
    Imported,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Warning {
    kind: RemoteAcquisitionFailureKind,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TitleOutcome {
    title_id: String,
    title_ref: String,
    pub(super) status: OutcomeStatus,
    imported_paths: Vec<PathBuf>,
    failure_kind: Option<RemoteAcquisitionFailureKind>,
    message: Option<String>,
    warnings: Vec<Warning>,
}

impl TitleOutcome {
    pub(super) fn failed(title_id: &str, message: impl Into<String>) -> Self {
        Self::new(
            title_id,
            OutcomeStatus::Failed,
            Vec::new(),
            None,
            Some(message.into()),
        )
    }

    fn new(
        title_id: &str,
        status: OutcomeStatus,
        imported_paths: Vec<PathBuf>,
        failure_kind: Option<RemoteAcquisitionFailureKind>,
        message: Option<String>,
    ) -> Self {
        Self {
            title_id: title_id.to_string(),
            title_ref: title_ref(title_id),
            status,
            imported_paths,
            failure_kind,
            message,
            warnings: Vec::new(),
        }
    }
}

/// What one title's job left behind. A title is imported when the session
/// gained its files; the job's diagnostics explain it either way.
pub(super) fn title_outcome(
    title_id: &str,
    job: &AcquisitionSnapshot,
    imported_paths: Vec<PathBuf>,
) -> TitleOutcome {
    let own: Vec<&RemoteSourceDiagnostic> = job
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .title_id
                .as_deref()
                .is_none_or(|id| id == title_id)
        })
        .collect();
    let imported = matches!(
        job.state.handoff(),
        Some(AcquisitionHandoff::Titles(TitlesHandoff::Imported { .. }))
    ) && !imported_paths.is_empty();
    if imported {
        let mut outcome = TitleOutcome::new(
            title_id,
            OutcomeStatus::Imported,
            imported_paths,
            None,
            None,
        );
        outcome.warnings = own.into_iter().map(warning).collect();
        return outcome;
    }
    let blocking = own.iter().find(|diagnostic| diagnostic.kind.blocks_title());
    let (kind, message) = match (blocking, job.state.handoff()) {
        (Some(diagnostic), _) => (Some(diagnostic.kind.clone()), diagnostic.message.clone()),
        (None, Some(AcquisitionHandoff::Titles(TitlesHandoff::Removed { reason }))) => {
            (None, refusal_message(reason))
        }
        (None, _) => (
            None,
            "the acquisition ended with no imported file".to_string(),
        ),
    };
    TitleOutcome::new(
        title_id,
        OutcomeStatus::Failed,
        Vec::new(),
        kind,
        Some(message),
    )
}

fn warning(diagnostic: &RemoteSourceDiagnostic) -> Warning {
    Warning {
        kind: diagnostic.kind.clone(),
        message: diagnostic.message.clone(),
    }
}

fn refusal_message(reason: &HandoffRefusal) -> String {
    match reason {
        HandoffRefusal::ImportFailed { error } => error.message.clone(),
        HandoffRefusal::NothingAdded => "the session already listed these files".to_string(),
    }
}

/// Where one title's Supplemental PDF landed, or why it did not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub(super) enum PdfOutcome {
    #[serde(rename_all = "camelCase")]
    Saved { title_id: String, path: PathBuf },
    #[serde(rename_all = "camelCase")]
    Failed {
        title_id: String,
        failure_kind: Option<RemoteAcquisitionFailureKind>,
        message: String,
    },
}

pub(super) fn pdf_outcome(title_id: &str, job: &AcquisitionSnapshot) -> PdfOutcome {
    let failed = |failure_kind, message| PdfOutcome::Failed {
        title_id: title_id.to_string(),
        failure_kind,
        message,
    };
    if let Some(AcquisitionHandoff::Pdf(handoff)) = job.state.handoff() {
        return match handoff {
            PdfHandoff::Saved { path } => PdfOutcome::Saved {
                title_id: title_id.to_string(),
                path: path.clone(),
            },
            PdfHandoff::NotSaved { error } => failed(None, error.message.clone()),
        };
    }
    match job
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.kind.blocks_title())
    {
        Some(diagnostic) => failed(Some(diagnostic.kind.clone()), diagnostic.message.clone()),
        None => failed(None, "the PDF job ended without saving a PDF".to_string()),
    }
}

pub(super) fn print_pdf_outcome(outcome: &PdfOutcome, json: bool) -> Result<(), String> {
    if json {
        let text = serde_json::to_string_pretty(outcome)
            .map_err(|error| format!("could not print the outcome: {error}"))?;
        println!("{text}");
        return Ok(());
    }
    match outcome {
        PdfOutcome::Saved { title_id, path } => {
            println!("{title_id}  saved\n  {}", path.display());
        }
        PdfOutcome::Failed {
            title_id,
            failure_kind,
            message,
        } => match failure_kind {
            Some(kind) => println!("{title_id}  failed: {kind:?}\n  {message}"),
            None => println!("{title_id}  failed\n  {message}"),
        },
    }
    Ok(())
}

pub(super) fn print_outcomes(outcomes: &[TitleOutcome], json: bool) -> Result<(), String> {
    if json {
        let report = serde_json::json!({ "titles": outcomes });
        let text = serde_json::to_string_pretty(&report)
            .map_err(|error| format!("could not print the outcomes: {error}"))?;
        println!("{text}");
    } else {
        outcomes
            .iter()
            .for_each(|outcome| println!("{}", outcome_text(outcome)));
    }
    Ok(())
}

pub(super) fn outcome_text(outcome: &TitleOutcome) -> String {
    let mut lines = vec![match (&outcome.status, &outcome.failure_kind) {
        (OutcomeStatus::Imported, _) => format!("{}  imported", outcome.title_id),
        (OutcomeStatus::Failed, Some(kind)) => format!("{}  failed: {kind:?}", outcome.title_id),
        (OutcomeStatus::Failed, None) => format!("{}  failed", outcome.title_id),
    }];
    lines.extend(
        outcome
            .imported_paths
            .iter()
            .map(|path| format!("  {}", path.display())),
    );
    lines.extend(outcome.message.iter().map(|message| format!("  {message}")));
    lines.extend(
        outcome
            .warnings
            .iter()
            .map(|warning| format!("  warning: {:?}: {}", warning.kind, warning.message)),
    );
    lines.join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LibraryRow {
    title_id: String,
    title: String,
    authors: Vec<String>,
    narrators: Vec<String>,
    acquirable: bool,
    availability: String,
    pdf_available: bool,
}

impl From<&RemoteTitle> for LibraryRow {
    fn from(title: &RemoteTitle) -> Self {
        Self {
            title_id: title.title_id.clone(),
            title: title.title.clone(),
            authors: title.authors.clone(),
            narrators: title.narrators.clone(),
            acquirable: title.availability.acquirable,
            availability: title.availability.label.clone(),
            pdf_available: title.supplemental_pdf_available,
        }
    }
}

pub(super) fn library_text(row: &LibraryRow) -> String {
    let mut line = format!(
        "{}  {} / {}",
        row.title_id,
        row.title,
        row.authors.join(", ")
    );
    if !row.narrators.is_empty() {
        line.push_str(&format!(" (read by {})", row.narrators.join(", ")));
    }
    if row.pdf_available {
        line.push_str("  [pdf]");
    }
    if !row.acquirable {
        line.push_str(&format!("  [not acquirable: {}]", row.availability));
    }
    line
}

pub(super) fn print_library(rows: &[LibraryRow], json: bool) -> Result<(), String> {
    if json {
        let text = serde_json::to_string_pretty(&serde_json::json!({ "titles": rows }))
            .map_err(|error| format!("could not print the library: {error}"))?;
        println!("{text}");
    } else {
        println!("Titles: {}", rows.len());
        rows.iter()
            .for_each(|row| println!("{}", library_text(row)));
    }
    Ok(())
}

#[cfg(test)]
#[path = "audible_outcome_tests.rs"]
mod tests;
