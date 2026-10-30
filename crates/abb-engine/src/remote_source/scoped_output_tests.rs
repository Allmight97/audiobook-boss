use super::*;
use tempfile::TempDir;

#[test]
fn drop_removes_the_partial() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("book.pdf");
    let partial_path;
    {
        let staged = StagedTempFile::new(&final_path);
        partial_path = staged.partial_path().to_path_buf();
        std::fs::write(staged.partial_path(), b"partial").expect("write partial");
        assert_eq!(partial_path, root.path().join("book.pdf.partial"));
    }
    assert!(!partial_path.exists());
}

#[test]
fn with_partial_uses_explicit_temp_path() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("out.m4b");
    let temp_path = root.path().join("out.m4b.partial");
    std::fs::write(&temp_path, b"temp").expect("write temp");
    {
        let _staged = StagedTempFile::with_partial(&final_path, &temp_path);
    }
    assert!(!temp_path.exists());
}

#[test]
fn no_extension_partial_is_named_partial() {
    let staged = StagedTempFile::new(Path::new("/tmp/source"));
    assert_eq!(staged.partial_path(), Path::new("/tmp/source.partial"));
}

#[test]
fn prepare_removes_stale_partial_before_write() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("book.m4b");
    let partial_path = partial_sibling(&final_path);
    std::fs::write(&partial_path, b"stale").expect("write stale partial");
    let mut staged = StagedTempFile::new(&final_path);
    staged.prepare().expect("prepare");
    assert!(!partial_path.exists());
}

#[tokio::test]
async fn rename_and_commit_keeps_final_and_drops_partial() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("book.m4b");
    let partial_path = partial_sibling(&final_path);
    let mut staged = StagedTempFile::new(&final_path);
    staged.prepare().expect("prepare");
    std::fs::write(staged.partial_path(), b"payload").expect("write partial");
    staged
        .rename_and_commit(|| false)
        .await
        .expect("rename and commit");
    assert!(final_path.exists());
    assert!(!partial_path.exists());
}

#[tokio::test]
async fn rename_and_commit_cancelled_removes_both_paths() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("book.m4b");
    let partial_path = partial_sibling(&final_path);
    let mut staged = StagedTempFile::new(&final_path);
    staged.prepare().expect("prepare");
    std::fs::write(staged.partial_path(), b"payload").expect("write partial");
    let error = staged
        .rename_and_commit(|| true)
        .await
        .expect_err("cancelled");
    assert!(matches!(error, AppError::Cancellation(_)));
    assert!(!final_path.exists());
    assert!(!partial_path.exists());
}

#[tokio::test]
async fn rename_and_commit_errors_when_partial_missing_before_commit() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("book.m4b");
    let mut staged = StagedTempFile::new(&final_path);
    staged.prepare().expect("prepare");
    std::fs::write(staged.partial_path(), b"payload").expect("write partial");
    std::fs::remove_file(staged.partial_path()).expect("remove partial");

    let error = staged
        .rename_and_commit(|| false)
        .await
        .expect_err("a missing partial must fail the commit");

    assert!(
        matches!(&error, AppError::General(message)
            if message == "Staged partial output is missing before commit."),
        "unexpected error: {error:?}"
    );
    assert!(
        !final_path.exists(),
        "no final file may appear on a failed commit"
    );
}

#[tokio::test]
async fn rename_and_commit_rejects_cross_parent_partial_and_final() {
    let root = TempDir::new().expect("temp root");
    let final_dir = root.path().join("final");
    let partial_dir = root.path().join("staging");
    std::fs::create_dir_all(&final_dir).expect("final dir");
    std::fs::create_dir_all(&partial_dir).expect("partial dir");
    let final_path = final_dir.join("book.m4b");
    let partial_path = partial_dir.join("book.m4b.partial");

    let staged = StagedTempFile::with_partial(&final_path, &partial_path);
    let error = staged
        .rename_and_commit(|| false)
        .await
        .expect_err("a cross-parent staged write must be rejected");

    assert!(
        matches!(&error, AppError::General(message)
            if message == "Staged partial and final paths must share the same parent directory."),
        "unexpected error: {error:?}"
    );
    assert!(
        !final_path.exists(),
        "no final file may appear on a rejected commit"
    );
}

#[test]
fn provisional_committed_file_drops_unless_permanent() {
    let root = TempDir::new().expect("temp root");
    let path = root.path().join("book.m4b");
    std::fs::write(&path, b"final").expect("write");
    {
        let guard = ProvisionalCommittedFile::new(path.clone());
        assert!(guard.path().exists());
    }
    assert!(!path.exists());

    std::fs::write(&path, b"final").expect("rewrite");
    {
        let guard = ProvisionalCommittedFile::new(path.clone());
        guard.permanent();
    }
    assert!(path.exists());
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "StagedTempFile final and partial paths must be distinct")]
fn with_partial_rejects_matching_paths_in_debug() {
    let root = TempDir::new().expect("temp root");
    let final_path = root.path().join("out.m4b");

    let _staged = StagedTempFile::with_partial(&final_path, &final_path);
}

#[cfg(unix)]
#[test]
fn non_utf8_extension_still_gets_partial_suffix() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let root = TempDir::new().expect("temp root");
    let file_name = OsString::from_vec(b"book.\xFFpdf".to_vec());
    let final_path = root.path().join(file_name);
    let staged = StagedTempFile::new(&final_path);

    assert!(staged
        .partial_path()
        .to_string_lossy()
        .ends_with(".partial"));
}
