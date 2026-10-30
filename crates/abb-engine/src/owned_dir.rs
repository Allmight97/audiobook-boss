//! Removal of ABB-owned working directories. Each owner names its root; removal
//! refuses anything that resolves outside that root or is itself a symlink.

use std::path::Path;

use crate::errors::{AppError, Result};

#[derive(Clone, Copy)]
pub(crate) struct OwnedRoot<'a> {
    pub(crate) path: &'a Path,
    pub(crate) label: &'a str,
}

impl OwnedRoot<'_> {
    pub(crate) fn remove_child(&self, child: &Path) -> Result<()> {
        if !child.exists() {
            return Ok(());
        }
        self.ensure_contains(child)?;
        self.remove_dir(child)
    }

    pub(crate) fn remove_root(&self) -> Result<()> {
        if !self.path.exists() {
            return Ok(());
        }
        self.remove_dir(self.path)
    }

    pub(crate) fn ensure_root_not_symlink(&self) -> Result<()> {
        self.ensure_not_symlink(self.path)
    }

    fn ensure_not_symlink(&self, path: &Path) -> Result<()> {
        if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
            return Err(AppError::ResourceCleanup(format!(
                "Refusing to follow {} symlink during cleanup",
                self.label
            )));
        }
        Ok(())
    }

    fn ensure_contains(&self, child: &Path) -> Result<()> {
        let invalid = |error: std::io::Error| {
            AppError::ResourceCleanup(format!("Invalid {} path: {error}", self.label))
        };
        let root = self.path.canonicalize().map_err(invalid)?;
        if child.canonicalize().map_err(invalid)?.starts_with(root) {
            return Ok(());
        }
        Err(AppError::ResourceCleanup(format!(
            "Refusing to cleanup path outside ABB {} root",
            self.label
        )))
    }

    fn remove_dir(&self, path: &Path) -> Result<()> {
        self.ensure_not_symlink(path)?;
        std::fs::remove_dir_all(path)?;
        Ok(())
    }
}
