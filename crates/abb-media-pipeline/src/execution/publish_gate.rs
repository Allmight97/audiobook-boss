use std::path::{Path, PathBuf};

use crate::errors::Result;

pub(crate) struct PublishedOutput {
    pub(crate) path: PathBuf,
    pub(crate) cancelled: bool,
    pub(crate) cleanup_warning: Option<String>,
}

pub(crate) trait PublishGate: Send + Sync {
    fn publish(
        &self,
        staged: &Path,
        final_path: &Path,
        commit: &mut dyn FnMut() -> Result<PublishedOutput>,
    ) -> Result<PublishedOutput>;
}
