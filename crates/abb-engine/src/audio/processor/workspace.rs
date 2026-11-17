use std::path::{Path, PathBuf};

use crate::audio::cleanup::CleanupGuard;
use crate::errors::Result;
use crate::execution::ProcessingContext;

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

    pub(super) fn published(&mut self, output: &Path) -> Result<()> {
        self.guard.forget_path(output);
        self.guard.cleanup_now()
    }
}
