use super::*;
use tempfile::TempDir;

fn write_file(path: &Path) {
    fs::write(path, b"audio").expect("test file should be written");
}

#[test]
fn supported_audio_import_texts_come_from_canonical_format_table() {
    assert_eq!(
        supported_audio_import_extensions(),
        SUPPORTED_AUDIO_FORMATS
            .iter()
            .map(|format| format.extension)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        supported_audio_import_formats_text(),
        "MKA, MP3, M4A/M4B, AAC, WAV, and FLAC"
    );
    assert_eq!(
        supported_audio_import_support_text(),
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

fn discovered_file_names(names: &[&str]) -> Vec<String> {
    let temp = TempDir::new().expect("temp dir");
    for name in names {
        write_file(&temp.path().join(name));
    }

    let actual = discover_audio_import_paths(&[temp.path().to_path_buf()]);

    actual
        .paths
        .iter()
        .map(|path| {
            path.file_name()
                .expect("file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[test]
fn import_order_ignores_leading_zeros_before_the_rest_of_the_name() {
    assert_eq!(
        discovered_file_names(&["Chapter 1 - Part 2.mp3", "Chapter 01 - Part 1.mp3"]),
        ["Chapter 01 - Part 1.mp3", "Chapter 1 - Part 2.mp3"]
    );
}

#[test]
fn import_order_puts_a_name_before_the_same_name_followed_by_a_number() {
    assert_eq!(
        discovered_file_names(&["Book1.mp3", "Book.mp3"]),
        ["Book.mp3", "Book1.mp3"]
    );
}

#[test]
fn import_order_folds_case_beyond_ascii() {
    assert_eq!(
        discovered_file_names(&["Über 10.mp3", "über 2.mp3"]),
        ["über 2.mp3", "Über 10.mp3"]
    );
}

fn skipped_paths(discovery: &ImportDiscovery) -> Vec<String> {
    discovery
        .skipped
        .iter()
        .map(|(path, _)| path.display().to_string())
        .collect()
}

fn write_pdf(path: &Path, body: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("pdf parent dir");
    }
    fs::write(path, body).expect("pdf file should be written");
}

fn companion_names(discovery: &ImportDiscovery, audio_path: &Path) -> Vec<String> {
    let canonical = audio_path.canonicalize().expect("canonical audio");
    discovery
        .companions
        .get(&canonical)
        .map(|assets| assets.iter().map(|asset| asset.file_name.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn folder_import_attaches_companion_pdfs_by_folder_scope_and_stem() {
    let temp = TempDir::new().expect("temp dir");
    let root = temp.path();

    let strange_loop_audio = root.join("I Am a Strange Loop/IAmaStrangeLoop_ep6.m4a");
    let strange_loop_pdf = root.join("I Am a Strange Loop/BK_HACH_004771.pdf");
    fs::create_dir_all(strange_loop_audio.parent().expect("parent")).expect("dir");
    write_file(&strange_loop_audio);
    write_pdf(&strange_loop_pdf, b"%PDF-1.7\nstrange loop");

    let multi_part_1 = root.join("Multi Part/Part 1.mp3");
    let multi_part_2 = root.join("Multi Part/Part 2.mp3");
    let multi_part_pdf = root.join("Multi Part/Booklet.pdf");
    fs::create_dir_all(multi_part_1.parent().expect("parent")).expect("dir");
    write_file(&multi_part_1);
    write_file(&multi_part_2);
    write_pdf(&multi_part_pdf, b"%PDF-1.7\nbooklet");

    let multi_disc_1 = root.join("Multi Disc/Disc 1/01.mp3");
    let multi_disc_2 = root.join("Multi Disc/Disc 2/02.mp3");
    let multi_disc_pdf = root.join("Multi Disc/Guide.pdf");
    fs::create_dir_all(multi_disc_1.parent().expect("parent")).expect("dir");
    fs::create_dir_all(multi_disc_2.parent().expect("parent")).expect("dir");
    write_file(&multi_disc_1);
    write_file(&multi_disc_2);
    write_pdf(&multi_disc_pdf, b"%PDF-1.7\nguide");

    let extras_audio = root.join("With Extras/book.m4b");
    let extras_pdf = root.join("With Extras/Extras/Illustrations.PDF");
    fs::create_dir_all(extras_audio.parent().expect("parent")).expect("dir");
    write_file(&extras_audio);
    write_pdf(&extras_pdf, b"%PDF-1.7\nextras");

    let flat_a = root.join("Flat/Book A.m4b");
    let flat_b = root.join("Flat/Book B.m4b");
    let flat_a_pdf = root.join("Flat/book a.pdf");
    fs::create_dir_all(flat_a.parent().expect("parent")).expect("dir");
    write_file(&flat_a);
    write_file(&flat_b);
    write_pdf(&flat_a_pdf, b"%PDF-1.7\nbook a");

    let unrelated_pdf = root.join("Receipts/tax.pdf");
    write_pdf(&unrelated_pdf, b"not even a valid pdf");

    let loose_book = root.join("Loose Book.m4b");
    let nested_disc = root.join("Big Book/Disc 1/01.mp3");
    let nested_extras_pdf = root.join("Big Book/Extras/map.pdf");
    fs::create_dir_all(nested_disc.parent().expect("parent")).expect("dir");
    write_file(&loose_book);
    write_file(&nested_disc);
    write_pdf(&nested_extras_pdf, b"%PDF-1.7\nbig book map");

    let actual = discover_audio_import_paths(&[root.to_path_buf()]);

    assert!(actual.skipped.is_empty(), "{:?}", actual.skipped);
    assert_eq!(
        companion_names(&actual, &strange_loop_audio),
        ["BK_HACH_004771.pdf"]
    );
    let strange_asset = &actual.companions[&strange_loop_audio.canonicalize().expect("canon")][0];
    assert_eq!(
        strange_asset.path,
        strange_loop_pdf.canonicalize().expect("canon pdf")
    );
    assert_eq!(
        strange_asset.size_bytes,
        b"%PDF-1.7\nstrange loop".len() as u64
    );
    assert_eq!(
        strange_asset.sha256,
        abb_media_core::sha256_hex(b"%PDF-1.7\nstrange loop")
    );

    assert_eq!(companion_names(&actual, &multi_part_1), ["Booklet.pdf"]);
    assert_eq!(companion_names(&actual, &multi_part_2), ["Booklet.pdf"]);
    assert_eq!(companion_names(&actual, &multi_disc_1), ["Guide.pdf"]);
    assert_eq!(companion_names(&actual, &multi_disc_2), ["Guide.pdf"]);
    assert_eq!(
        companion_names(&actual, &extras_audio),
        ["Illustrations.PDF"]
    );
    assert_eq!(companion_names(&actual, &flat_a), ["book a.pdf"]);
    assert!(companion_names(&actual, &flat_b).is_empty());
    assert!(
        companion_names(&actual, &loose_book).is_empty(),
        "a PDF under Big Book/Extras must not climb past Big Book onto Loose Book"
    );
    assert!(companion_names(&actual, &nested_disc).is_empty());
}

#[test]
fn direct_audio_drop_ignores_unselected_sibling_pdf_but_attaches_explicitly_dropped_pdf() {
    let temp = TempDir::new().expect("temp dir");
    let audio = temp.path().join("book.m4a");
    let pdf = temp.path().join("guide.pdf");
    write_file(&audio);
    write_pdf(&pdf, b"%PDF-1.7\nguide");

    let audio_only = discover_audio_import_paths(std::slice::from_ref(&audio));
    assert!(audio_only.companions.is_empty());
    assert!(audio_only.skipped.is_empty());

    let both = discover_audio_import_paths(&[pdf.clone(), audio.clone()]);
    assert_eq!(companion_names(&both, &audio), ["guide.pdf"]);
    assert!(both.skipped.is_empty());

    let pdf_only = discover_audio_import_paths(std::slice::from_ref(&pdf));
    assert!(pdf_only.paths.is_empty());
    assert!(pdf_only.companions.is_empty());
    assert!(pdf_only.skipped.is_empty());
}

#[test]
fn invalid_companion_pdf_is_skipped_without_blocking_audio_import() {
    let temp = TempDir::new().expect("temp dir");
    let audio = temp.path().join("book.m4a");
    let bogus_pdf = temp.path().join("bogus.pdf");
    write_file(&audio);
    write_pdf(&bogus_pdf, b"not a real pdf");

    let actual = discover_audio_import_paths(&[temp.path().to_path_buf()]);

    assert_eq!(
        actual.paths,
        vec![audio.canonicalize().expect("canonical audio")]
    );
    assert!(actual.companions.is_empty());
    assert_eq!(actual.skipped.len(), 1);
    assert!(actual.skipped[0].0.ends_with("bogus.pdf"));
    assert!(actual.skipped[0]
        .1
        .to_string()
        .contains("magic-byte validation"));
}

#[cfg(unix)]
#[test]
fn nested_symlink_pdf_is_ignored_during_folder_import() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temp dir");
    let folder = temp.path().join("book");
    fs::create_dir(&folder).expect("book dir");
    let audio = folder.join("book.m4a");
    let outside_pdf = temp.path().join("outside.pdf");
    let link_pdf = folder.join("linked.pdf");
    write_file(&audio);
    write_pdf(&outside_pdf, b"%PDF-1.7\noutside");
    symlink(&outside_pdf, &link_pdf).expect("symlink pdf");

    let actual = discover_audio_import_paths(&[folder]);

    assert_eq!(
        actual.paths,
        vec![audio.canonicalize().expect("canonical audio")]
    );
    assert!(actual.companions.is_empty());
    assert!(actual.skipped.is_empty());
}
