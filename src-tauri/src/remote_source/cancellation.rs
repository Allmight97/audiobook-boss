use crate::errors::{AppError, Result};

pub(in crate::remote_source) fn remote_acquisition_cancelled() -> AppError {
    AppError::Cancellation("Remote source acquisition was cancelled.".to_string())
}

pub(in crate::remote_source) fn ensure_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<()> {
    if is_cancelled() {
        return Err(remote_acquisition_cancelled());
    }
    Ok(())
}
