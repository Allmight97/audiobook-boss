use super::super::{
    AcquisitionHandoff, AcquisitionJob, HandoffRefusal, RemoteAccountStatus, RemoteSourceDiagnostic,
};
use super::{RemoteAuthStatus, RemoteDraftStatus, RemoteLibrarySnapshot, RemoteUiSnapshot};

const STAGED_FILES_REMOVED: &str = "Staged remote files were removed.";

/// `settled_job_line` is the last settled job's line, unless a later library
/// load or sign-in replaced it; only that fact orders a job against them.
pub(super) fn audible_message(
    remote: &RemoteUiSnapshot,
    library: &RemoteLibrarySnapshot,
    settled_job_line: Option<&str>,
) -> String {
    if let RemoteAuthStatus::Failed { error } = &remote.auth {
        return error.message.clone();
    }
    if let RemoteDraftStatus::Failed { error } = &remote.account_status {
        return error.message.clone();
    }
    if let Some(line) = settled_job_line {
        return with_connected_note(line.to_string(), remote);
    }
    if let RemoteDraftStatus::Failed { error } = &remote.library_status {
        return error.message.clone();
    }
    if remote.auth == RemoteAuthStatus::Starting {
        return "Starting Audible authorization.".into();
    }
    if remote.auth == RemoteAuthStatus::Completing {
        return "Completing Audible connection.".into();
    }
    if remote.account_status == RemoteDraftStatus::Running {
        return "Updating connection.".into();
    }
    if remote.library_status == RemoteDraftStatus::Running {
        return "Loading Audible library.".into();
    }
    if remote.auth == RemoteAuthStatus::AwaitingHandoff {
        return "Complete Audible authorization in your browser, then enter the handoff path, or Connect again."
            .into();
    }
    if remote.library_status == RemoteDraftStatus::Succeeded {
        return with_connected_note(library_line(library), remote);
    }
    remote
        .account
        .as_ref()
        .and_then(|account| account.message.clone())
        .unwrap_or_default()
}

pub(super) fn indexer_message(remote: &RemoteUiSnapshot) -> String {
    let status = match &remote.account_status {
        RemoteDraftStatus::Failed { error } => error.message.clone(),
        _ => remote.indexer.message.clone(),
    };
    with_connected_note(status, remote)
}

pub(super) fn settled_job_line(job: &AcquisitionJob) -> String {
    match &job.handoff {
        Some(AcquisitionHandoff::Imported { count }) => {
            let plural = if *count == 1 { "" } else { "s" };
            format!("{count} acquired title{plural} imported.")
        }
        Some(AcquisitionHandoff::Removed {
            reason: HandoffRefusal::ImportFailed { error },
        }) => format!("{} {STAGED_FILES_REMOVED}", error.message),
        Some(AcquisitionHandoff::Removed {
            reason: HandoffRefusal::NothingAdded,
        }) => {
            format!("Acquired titles were not added to the input session. {STAGED_FILES_REMOVED}")
        }
        None => {
            let diagnostics = unique_diagnostic_message(&job.diagnostics);
            if job.progress.terminal && !diagnostics.is_empty() {
                diagnostics
            } else {
                job.progress.message.clone()
            }
        }
    }
}

fn library_line(library: &RemoteLibrarySnapshot) -> String {
    let diagnostics = unique_diagnostic_message(&library.diagnostics);
    if diagnostics.is_empty() {
        format!("{} Audible titles loaded.", library.titles.len())
    } else {
        diagnostics
    }
}

fn unique_diagnostic_message(diagnostics: &[RemoteSourceDiagnostic]) -> String {
    let mut messages: Vec<&str> = Vec::new();
    for diagnostic in diagnostics {
        let message = diagnostic.message.trim();
        if !message.is_empty() && !messages.contains(&message) {
            messages.push(message);
        }
    }
    messages.join(" ")
}

fn with_connected_note(status: String, remote: &RemoteUiSnapshot) -> String {
    let note = remote
        .account
        .as_ref()
        .filter(|account| account.status == RemoteAccountStatus::Connected)
        .and_then(|account| account.message.as_deref())
        .unwrap_or_default();
    if note.is_empty() || status.contains(note) {
        status
    } else if status.is_empty() {
        note.to_string()
    } else {
        format!("{status} {note}")
    }
}

#[cfg(test)]
#[path = "lane_status_tests.rs"]
mod tests;
