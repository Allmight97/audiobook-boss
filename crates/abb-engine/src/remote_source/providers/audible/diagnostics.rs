use crate::errors::AppError;
use crate::remote_source::{RemoteAcquisitionFailureKind, RemoteSourceDiagnostic};

#[derive(Debug)]
pub(super) struct AudibleAcquisitionError {
    kind: RemoteAcquisitionFailureKind,
    error: AppError,
}

impl AudibleAcquisitionError {
    fn new(kind: RemoteAcquisitionFailureKind, error: AppError) -> Self {
        let kind = if matches!(error, AppError::Cancellation(_)) {
            RemoteAcquisitionFailureKind::Cancelled
        } else {
            kind
        };
        Self { kind, error }
    }

    pub(super) fn provider_protocol(error: AppError) -> Self {
        Self::new(
            RemoteAcquisitionFailureKind::ProviderPrivateProtocolFailed,
            error,
        )
    }

    pub(super) fn download(error: AppError) -> Self {
        Self::new(RemoteAcquisitionFailureKind::DownloadFailed, error)
    }

    pub(super) fn materialization(error: AppError) -> Self {
        Self::new(RemoteAcquisitionFailureKind::MaterializationFailed, error)
    }

    pub(super) fn validation(error: AppError) -> Self {
        Self::new(RemoteAcquisitionFailureKind::ValidationFailed, error)
    }

    pub(super) fn supplemental_pdf(error: AppError) -> Self {
        Self::new(RemoteAcquisitionFailureKind::SupplementalPdfFailed, error)
    }

    pub(super) fn cancellation(error: AppError) -> Self {
        Self::new(RemoteAcquisitionFailureKind::Cancelled, error)
    }

    #[cfg(test)]
    pub(super) fn kind(&self) -> RemoteAcquisitionFailureKind {
        self.kind.clone()
    }

    pub(super) fn is_cancellation(&self) -> bool {
        self.kind == RemoteAcquisitionFailureKind::Cancelled
            || matches!(&self.error, AppError::Cancellation(_))
    }

    pub(super) fn into_app_error(self) -> AppError {
        self.error
    }

    pub(super) fn into_diagnostic(self, title_id: Option<String>) -> RemoteSourceDiagnostic {
        RemoteSourceDiagnostic {
            kind: self.kind,
            title_id,
            message: self.error.to_string(),
        }
    }
}
