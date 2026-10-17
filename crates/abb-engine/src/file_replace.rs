use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// A file ABB writes beside `destination` before installing it there. Removed
/// on drop unless committed, so a failed write leaves nothing behind.
pub(crate) struct SiblingTemp {
    path: PathBuf,
    committed: bool,
}

impl SiblingTemp {
    /// Names `.abb_<tag>_<uuid><extension>` beside `destination`; the writer
    /// creates the file.
    pub(crate) fn beside(destination: &Path, tag: &str, extension: &str) -> io::Result<Self> {
        let parent = destination.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "destination has no parent directory",
            )
        })?;
        Ok(Self {
            path: parent.join(format!(".abb_{tag}_{}{extension}", uuid::Uuid::new_v4())),
            committed: false,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Flushes the temp, gives it the destination's permissions, and installs
    /// it over `destination`.
    pub(crate) fn commit(self, destination: &Path) -> io::Result<()> {
        self.commit_with(destination, replace_file)
    }

    pub(crate) fn commit_with(
        mut self,
        destination: &Path,
        install: impl FnOnce(&Path, &Path) -> io::Result<()>,
    ) -> io::Result<()> {
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)?
            .sync_all()?;
        match std::fs::metadata(destination) {
            Ok(existing) => std::fs::set_permissions(&self.path, existing.permissions())?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        install(&self.path, destination)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for SiblingTemp {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => log::warn!(
                "sibling_temp cleanup failed temp={} error={error}",
                crate::diagnostics::artifact_id(&self.path)
            ),
        }
    }
}

/// Writes `content` to a synced temporary file beside `path`, then moves it
/// into place, so a crash leaves the old file or the new one, never a torn one.
pub(crate) fn write_atomically(path: &Path, content: &[u8]) -> io::Result<()> {
    let temp = SiblingTemp::beside(path, "write", ".tmp")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(temp.path())?;
    file.write_all(content)?;
    drop(file);
    temp.commit(path)
}

fn backup_path_for(destination: &Path) -> io::Result<PathBuf> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "replacement destination has no parent directory",
        )
    })?;
    Ok(parent.join(format!(".abb_replace_{}", uuid::Uuid::new_v4())))
}

fn replace_file_with_ops<R, M>(
    source: &Path,
    destination: &Path,
    backup_path: &Path,
    mut rename: R,
    mut remove_file: M,
) -> io::Result<()>
where
    R: FnMut(&Path, &Path) -> io::Result<()>,
    M: FnMut(&Path) -> io::Result<()>,
{
    match rename(source, destination) {
        Ok(()) => return Ok(()),
        Err(error) if !destination.exists() => return Err(error),
        Err(error) => {
            let destination_type = std::fs::symlink_metadata(destination)?.file_type();
            if !destination_type.is_file() {
                return Err(error);
            }
        }
    }

    rename(destination, backup_path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to move existing destination aside for replacement: {error}"),
        )
    })?;

    match rename(source, destination) {
        Ok(()) => {
            cleanup_backup_after_success(backup_path, &mut remove_file);
            Ok(())
        }
        Err(replace_error) => {
            if let Err(rollback_error) = rename(backup_path, destination) {
                return Err(io::Error::new(
                    replace_error.kind(),
                    format!(
                        "failed to install replacement and failed to restore original destination: {replace_error}; rollback: {rollback_error}"
                    ),
                ));
            }
            Err(replace_error)
        }
    }
}

fn cleanup_backup_after_success<M>(backup_path: &Path, remove_file: &mut M)
where
    M: FnMut(&Path) -> io::Result<()>,
{
    if let Err(error) = remove_file(backup_path) {
        #[cfg(windows)]
        {
            if clear_readonly_backup(backup_path).is_ok() && remove_file(backup_path).is_ok() {
                return;
            }
        }

        log::warn!(
            "Replacement succeeded, but failed to remove backup '{}': {}",
            backup_path.display(),
            error
        );
    }
}

#[cfg(windows)]
fn clear_readonly_backup(backup_path: &Path) -> io::Result<()> {
    let metadata = std::fs::metadata(backup_path)?;
    let mut permissions = metadata.permissions();
    if permissions.readonly() {
        permissions.set_readonly(false);
        std::fs::set_permissions(backup_path, permissions)?;
    }
    Ok(())
}

pub(crate) fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    let backup_path = backup_path_for(destination)?;
    replace_file_with_ops(
        source,
        destination,
        &backup_path,
        |from, to| std::fs::rename(from, to),
        |path| std::fs::remove_file(path),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use tempfile::TempDir;

    #[cfg(unix)]
    #[test]
    fn write_atomically_keeps_the_replaced_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let root = TempDir::new().expect("temp root");
        let path = root.path().join("settings.toml");
        std::fs::write(&path, b"old").expect("write original");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
            .expect("restrict original");

        write_atomically(&path, b"new").expect("write atomically");

        assert_eq!(std::fs::read(&path).expect("read"), b"new");
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn replace_file_overwrites_existing_destination() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.m4b");
        let destination = root.path().join("destination.m4b");

        std::fs::write(&source, b"new").expect("write source");
        std::fs::write(&destination, b"old").expect("write destination");

        replace_file(&source, &destination).expect("replace file");

        assert_eq!(
            std::fs::read(&destination).expect("read destination"),
            b"new"
        );
        assert!(!source.exists(), "source should be consumed");
    }

    #[test]
    fn replace_file_rolls_back_when_install_after_backup_fails() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.m4b");
        let destination = root.path().join("destination.m4b");
        let backup = root.path().join("backup.m4b");
        let calls = Cell::new(0);

        std::fs::write(&source, b"new").expect("write source");
        std::fs::write(&destination, b"old").expect("write destination");

        let result = replace_file_with_ops(
            &source,
            &destination,
            &backup,
            |from, to| {
                calls.set(calls.get() + 1);
                match calls.get() {
                    1 => Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "destination exists",
                    )),
                    3 => Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "replacement blocked",
                    )),
                    _ => std::fs::rename(from, to),
                }
            },
            |path| std::fs::remove_file(path),
        );

        assert!(result.is_err(), "replacement should fail");
        assert_eq!(
            std::fs::read(&destination).expect("read destination"),
            b"old"
        );
        assert_eq!(std::fs::read(&source).expect("read source"), b"new");
        assert!(!backup.exists(), "backup should be consumed by rollback");
    }

    #[test]
    fn replace_file_reports_success_when_backup_cleanup_fails_after_install() {
        let root = TempDir::new().expect("temp root");
        let source = root.path().join("source.m4b");
        let destination = root.path().join("destination.m4b");
        let backup = root.path().join("backup.m4b");
        let calls = Cell::new(0);

        std::fs::write(&source, b"new").expect("write source");
        std::fs::write(&destination, b"old").expect("write destination");

        replace_file_with_ops(
            &source,
            &destination,
            &backup,
            |from, to| {
                calls.set(calls.get() + 1);
                match calls.get() {
                    1 => Err(io::Error::new(
                        io::ErrorKind::AlreadyExists,
                        "destination exists",
                    )),
                    _ => std::fs::rename(from, to),
                }
            },
            |_path| Err(io::Error::new(io::ErrorKind::PermissionDenied, "locked")),
        )
        .expect("successful install should remain successful");

        assert_eq!(
            std::fs::read(&destination).expect("read destination"),
            b"new"
        );
        assert!(!source.exists(), "source should be consumed");
        assert_eq!(
            std::fs::read(&backup).expect("read leftover backup"),
            b"old"
        );
    }
}
