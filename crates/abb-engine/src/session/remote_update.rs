//! The remote parts of a session update: a full snapshot, or one job's tick.

use serde::Serialize;

/// A download tick for one job. The rest of the remote snapshot is unchanged.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteProgressUpdate {
    pub job_id: String,
    pub progress: crate::remote_source::AcquisitionProgress,
}

impl super::SessionUpdate {
    pub(crate) fn remote(snapshot: crate::remote_source::RemoteUiSnapshot) -> Self {
        Self {
            revision: 0,
            titles: None,
            selection: None,
            metadata: None,
            lookup: None,
            audio: None,
            output: None,
            remote: Some(snapshot),
            remote_library: None,
            remote_progress: None,
        }
    }

    pub(crate) fn remote_progress(
        job_id: &str,
        progress: crate::remote_source::AcquisitionProgress,
    ) -> Self {
        Self {
            revision: 0,
            titles: None,
            selection: None,
            metadata: None,
            lookup: None,
            audio: None,
            output: None,
            remote: None,
            remote_library: None,
            remote_progress: Some(RemoteProgressUpdate {
                job_id: job_id.to_string(),
                progress,
            }),
        }
    }
}
