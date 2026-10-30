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
                #[expect(
                    clippy::disallowed_methods,
                    reason = "rename only after the injected failures end"
                )]
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
                #[expect(
                    clippy::disallowed_methods,
                    reason = "rename only after the injected failures end"
                )]
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
