use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::errors::Result;
use crate::processing::ProcessingContext;

/// The one temp folder of a job. Every staged file lives under it, and it is
/// removed when the job ends: by `published`, or on drop for any other end. A
/// failed removal stays owned and retries on drop.
pub(super) struct Workspace {
    dir: PathBuf,
    guard: CleanupGuard,
}

impl Workspace {
    pub(super) fn open(context: &ProcessingContext) -> Result<Self> {
        let dir = super::staging::create_processing_workspace_dir(
            context.session.uuid(),
            context.processing_workspace_root(),
        )?;
        let mut guard = CleanupGuard::new(context.session.id());
        guard.add_path(&dir);
        Ok(Self { dir, guard })
    }

    pub(super) fn dir(&self) -> &Path {
        &self.dir
    }

    /// Removes the workspace after `output` is published. `output` is never
    /// removed, even if it sits under the workspace.
    pub(super) fn published(&mut self, output: &Path) -> Result<()> {
        self.guard.forget_path(output);
        self.guard.cleanup_now()
    }
}
