use std::io;
use std::path::{Path, PathBuf};

fn backup_path_for(destination: &Path) -> io::Result<PathBuf> {
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "replacement destination has no parent directory",
        )
    })?;
    let file_name = destination.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "replacement destination has no file name",
        )
    })?;

    Ok(parent.join(format!(
        ".abb_replace_{}_{}",
        uuid::Uuid::new_v4(),
        file_name.to_string_lossy()
    )))
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
            let _ = remove_file(backup_path);
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
}
