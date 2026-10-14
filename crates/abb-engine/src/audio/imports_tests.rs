use super::*;
use tempfile::TempDir;

fn write_file(path: &Path) {
    fs::write(path, b"audio").expect("test file should be written");
}

#[test]
fn supported_audio_import_metadata_comes_from_canonical_format_table() {
    let metadata = supported_audio_import_metadata();

    assert_eq!(
        metadata.extensions,
        SUPPORTED_AUDIO_FORMATS
            .iter()
            .map(|format| format.extension.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        metadata.formats_text,
        "MKA, MP3, M4A/M4B, AAC, WAV, and FLAC"
    );
    assert_eq!(
        metadata.support_text,
        "Supports MKA, MP3, M4A/M4B, AAC, WAV, and FLAC audio files"
    );
}

#[test]
fn discover_audio_import_paths_recurses_filters_dedupes_and_sorts() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();
    let nested = root.join("Series 10");
    let lower = root.join("Series 2");
    fs::create_dir_all(&nested).expect("nested dir");
    fs::create_dir_all(&lower).expect("lower dir");

    let book_10 = nested.join("Book 10.m4b");
    let book_2 = lower.join("Book 2.MP3");
    let unsupported = lower.join("cover.jpg");
    write_file(&book_10);
    write_file(&book_2);
    write_file(&unsupported);

    let actual = discover_audio_import_paths(&[root.to_path_buf(), book_2.clone()]);

    assert!(actual.skipped.is_empty());
    assert_eq!(
        actual.paths,
        vec![
            book_2.canonicalize().expect("canonical book 2"),
            book_10.canonicalize().expect("canonical book 10"),
        ]
    );
}

#[test]
fn discover_audio_import_paths_skips_direct_unsupported_files() {
    let temp = TempDir::new().expect("temp dir");
    let unsupported = temp.path().join("notes.txt");
    write_file(&unsupported);

    let actual = discover_audio_import_paths(&[unsupported]);

    assert!(actual.paths.is_empty());
    assert!(actual.skipped.is_empty());
}

#[cfg(unix)]
#[test]
fn discover_audio_import_paths_skips_nested_symlinks() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temp dir");
    let real = temp.path().join("Book 1.m4b");
    let link = temp.path().join("Book 2.m4b");
    write_file(&real);
    symlink(&real, &link).expect("symlink");

    let actual = discover_audio_import_paths(&[temp.path().to_path_buf()]);

    assert_eq!(
        actual.paths,
        vec![real.canonicalize().expect("canonical real")]
    );
    assert!(actual.skipped.is_empty());
}

#[cfg(unix)]
#[test]
fn discover_audio_import_paths_continues_after_rejected_direct_inputs() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temp dir");
    let supported = temp.path().join("Book 1.m4b");
    let symlink_target = temp.path().join("Book 2.m4b");
    let symlink_path = temp.path().join("Linked Book.m4b");
    let missing = temp.path().join("Missing Book.m4b");
    write_file(&supported);
    write_file(&symlink_target);
    symlink(&symlink_target, &symlink_path).expect("symlink");

    let actual =
        discover_audio_import_paths(&[symlink_path.clone(), missing.clone(), supported.clone()]);

    assert_eq!(
        actual.paths,
        vec![supported.canonicalize().expect("canonical supported")]
    );
    assert_eq!(
        skipped_paths(&actual),
        vec![
            symlink_path.display().to_string(),
            missing.display().to_string()
        ]
    );
    assert!(actual.skipped[0]
        .1
        .to_string()
        .contains("Symlinks are not supported"));
}

#[cfg(unix)]
#[test]
fn discover_audio_import_paths_keeps_a_folder_past_a_file_it_cannot_take() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path().canonicalize().expect("canonical root");
    let bad_name = root.join("Bad\nName.mp3");
    write_file(&bad_name);
    let mut books = Vec::new();
    for disc in ["Disc 1", "Disc 2"] {
        let folder = root.join(disc);
        fs::create_dir(&folder).expect("disc dir");
        let book = folder.join(format!("{disc}.m4b"));
        write_file(&book);
        books.push(book);
    }

    let actual = discover_audio_import_paths(&[root]);

    assert_eq!(actual.paths, books);
    assert_eq!(skipped_paths(&actual), vec![bad_name.display().to_string()]);
}

fn skipped_paths(discovery: &ImportDiscovery) -> Vec<String> {
    discovery
        .skipped
        .iter()
        .map(|(path, _)| path.display().to_string())
        .collect()
}
