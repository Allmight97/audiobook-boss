use super::super::{
    AcquisitionHandoff, AcquisitionJob, HandoffRefusal, RemoteAccountStatus, RemoteSourceDiagnostic,
};
use super::audible_lane::AudibleLane;
use super::audible_lane::AudibleSignIn;
use super::{RemoteDraftStatus, RemoteLibrarySnapshot, RemoteUiSnapshot};

const STAGED_FILES_REMOVED: &str = "Staged remote files were removed.";

/// `settled_job_line` is the last settled job's line, unless a later library
/// load or sign-in replaced it; only that fact orders a job against them.
pub(super) fn audible_message(
    remote: &RemoteUiSnapshot,
    audible: &AudibleLane,
    library: &RemoteLibrarySnapshot,
) -> String {
    if let AudibleSignIn::Failed(error) = &audible.sign_in {
        return error.message.clone();
    }
    if let RemoteDraftStatus::Failed { error } = &remote.account_status {
        return error.message.clone();
    }
    if let Some(line) = &audible.settled_job_line {
        return with_connected_note(line.clone(), remote);
    }
    if let RemoteDraftStatus::Failed { error } = &audible.library_status {
        return error.message.clone();
    }
    if matches!(audible.sign_in, AudibleSignIn::Starting) {
        return "Starting Audible authorization.".into();
    }
    if matches!(audible.sign_in, AudibleSignIn::Completing) {
        return "Completing Audible connection.".into();
    }
    if remote.account_status == RemoteDraftStatus::Running {
        return "Updating connection.".into();
    }
    if audible.library_status == RemoteDraftStatus::Running {
        return "Loading Audible library.".into();
    }
    if matches!(audible.sign_in, AudibleSignIn::AwaitingHandoff(_)) {
        return "Complete Audible authorization in your browser, then enter the handoff path, or Connect again."
            .into();
    }
    if audible.library_status == RemoteDraftStatus::Succeeded {
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

pub(super) fn grab_batch_line(sent: usize, failed: usize, unconfirmed: usize) -> String {
    let mut line = format!("{sent} releases sent.");
    if failed > 0 {
        line.push_str(&format!(" {failed} failed; retry their rows."));
    }
    if unconfirmed > 0 {
        line.push_str(&format!(
            " {unconfirmed} not confirmed; check your downloader before you retry."
        ));
    }
    line
}

pub(super) fn settled_job_line(job: &AcquisitionJob) -> String {
    let diagnostics = unique_diagnostic_message(&job.diagnostics);
    let outcome = match &job.handoff {
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
        None if !diagnostics.is_empty() => return diagnostics,
        None => return job.progress.message.clone(),
    };
    if diagnostics.is_empty() || outcome.contains(&diagnostics) {
        outcome
    } else {
        format!("{outcome} {diagnostics}")
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
