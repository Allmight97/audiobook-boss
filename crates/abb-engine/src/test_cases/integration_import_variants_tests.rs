use std::fs;
use std::path::{Path, PathBuf};

use abb_engine::output_artifact::CollisionPolicy;
use abb_engine::session::{InputNotice, MetadataField, SessionIntent, SubmissionStatus};
use abb_engine::work_runtime::{ChildJobStatus, OperationId, WorkOperationStatus};

use super::integration_media_execution_tests::chapters_of;
use super::integration_session_desk::{collision_review, finished, submission, Desk};
use super::wav_fixture::write_wav;

fn imported(desk: &Desk) -> Vec<abb_engine::audio::AudioFile> {
    desk.engine
        .session_snapshot()
        .titles
        .expect("titles part")
        .files
}

pub(super) fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).expect("read folder") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                folders.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

fn name_of(path: &Path) -> String {
    path.file_name()
        .expect("a file name")
        .to_string_lossy()
        .into_owned()
}

async fn finish_submitted_export(desk: &Desk) -> OperationId {
    let Some(SubmissionStatus::Submitted { operation_id, .. }) = submission(desk) else {
        panic!("export not accepted: {:?}", submission(desk));
    };
    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&operation_id))
    })
    .await;
    operation_id
}

async fn export_renaming_any_collision(desk: &Desk) -> OperationId {
    desk.prepare_export().await;
    desk.send(SessionIntent::Submit).await;
    if submission(desk) == Some(SubmissionStatus::ReviewRequired) {
        let review_id = collision_review(desk).review_id;
        desk.send(SessionIntent::ChooseCollisionPolicy {
            review_id,
            policy: CollisionPolicy::RenameNew,
        })
        .await;
    }
    finish_submitted_export(desk).await
}

fn output_seconds(outputs: &[PathBuf]) -> Vec<u32> {
    let mut seconds: Vec<_> = outputs
        .iter()
        .map(|path| {
            abb_engine::audio::get_file_list_info(&[path], abb_engine::audio::AacDecoder::Auto)
                .expect("read an output")
                .files[0]
                .duration
                .expect("duration")
                .round() as u32
        })
        .collect();
    seconds.sort();
    seconds
}

#[cfg(unix)]
#[tokio::test]
async fn a_nested_folder_imports_in_natural_order_and_says_what_it_skipped() {
    let desk = Desk::new();
    let books = desk.root.path().join("books");
    write_wav(&books.join("Part 10/b.wav"), 1, None, None);
    write_wav(&books.join("Part 2/a.wav"), 1, None, None);
    fs::write(books.join("Part 2/notes.txt"), b"not audio").expect("notes");
    let link = desk.root.path().join("link.wav");
    std::os::unix::fs::symlink(books.join("Part 2/a.wav"), &link).expect("symlink");

    desk.send(SessionIntent::Import {
        paths: vec![
            books.to_string_lossy().into_owned(),
            link.to_string_lossy().into_owned(),
        ],
    })
    .await;

    let titles = desk.engine.session_snapshot().titles.expect("titles part");
    let order: Vec<_> = titles
        .files
        .iter()
        .map(|file| {
            let folder = file.path.parent().expect("folder");
            format!("{}/{}", name_of(folder), name_of(&file.path))
        })
        .collect();
    assert_eq!(order, ["Part 2/a.wav", "Part 10/b.wav"]);
    assert!(titles.files.iter().all(|file| file.is_valid));
    let Some(InputNotice::SkippedPaths { skipped }) = titles.notice else {
        panic!("the import says what it skipped: {:?}", titles.notice);
    };
    assert_eq!(skipped.len(), 1, "{skipped:?}");
    assert!(skipped[0].path.ends_with("link.wav"), "{skipped:?}");
    assert!(skipped[0].reason.contains("Symlinks"), "{skipped:?}");
}

#[tokio::test]
async fn a_name_in_either_unicode_form_survives_import_export_and_reimport() {
    let nfc = "Caf\u{e9}";
    let nfd = "Cafe\u{301}";
    assert_ne!(nfc.as_bytes(), nfd.as_bytes());
    let desk = Desk::new();
    let books = desk.root.path().join("books");
    let sources = [
        (books.join(format!("one/{nfc}.wav")), nfc, "Author One"),
        (books.join(format!("two/{nfd}.wav")), nfd, "Author Two"),
    ];
    for (path, title, author) in &sources {
        write_wav(path, 1, Some(title), Some(author));
    }
    desk.send(SessionIntent::Import {
        paths: vec![books.to_string_lossy().into_owned()],
    })
    .await;

    let mut read_titles: Vec<_> = imported(&desk)
        .into_iter()
        .map(|file| file.tag_title.expect("title tag"))
        .collect();
    read_titles.sort();
    assert_eq!(
        read_titles,
        [nfd, nfc],
        "each form is kept as its own bytes"
    );

    desk.export().await;
    desk.wait_until("both books are published", |desk| {
        files_under(&desk.root.path().join("exports")).len() == 2
    })
    .await;
    let outputs = files_under(&desk.root.path().join("exports"));
    let mut output_titles: Vec<_> = outputs
        .iter()
        .map(|path| {
            abb_engine::read_metadata(path)
                .expect("read output tags")
                .title
                .expect("output title")
        })
        .collect();
    output_titles.sort();
    assert_eq!(output_titles, [nfd, nfc], "tag bytes are unchanged");

    let again = Desk::new();
    again
        .send(SessionIntent::Import {
            paths: outputs
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect(),
        })
        .await;
    assert_eq!(imported(&again).len(), 2);
    assert!(imported(&again).iter().all(|file| file.is_valid));
}

#[tokio::test]
async fn odd_names_and_a_wrong_extension_still_import_and_export() {
    let desk = Desk::new();
    let books = desk.root.path().join("books");
    let names = [
        ("UPPER.WAV", "Upper"),
        ("a.final.v2.wav", "Dotted"),
        ("'#%&.wav", "Punctuated"),
        ("really-a-wav.mp3", "Wrong extension"),
    ];
    for (name, title) in names {
        write_wav(&books.join(name), 1, Some(title), Some("Odd Author"));
    }
    desk.send(SessionIntent::Import {
        paths: vec![books.to_string_lossy().into_owned()],
    })
    .await;

    let files = imported(&desk);
    assert_eq!(files.len(), 4, "{files:?}");
    assert!(files.iter().all(|file| file.is_valid), "{files:?}");

    desk.export().await;
    desk.wait_until("every book is published", |desk| {
        files_under(&desk.root.path().join("exports")).len() == 4
    })
    .await;
    let exports = desk.root.path().join("exports");
    let mut published: Vec<_> = files_under(&exports)
        .iter()
        .map(|path| {
            assert!(path.starts_with(&exports), "{path:?}");
            abb_engine::read_metadata(path)
                .expect("read output tags")
                .title
                .expect("output title")
        })
        .collect();
    published.sort();
    assert_eq!(
        published,
        ["Dotted", "Punctuated", "Upper", "Wrong extension"]
    );
    let mut left_in_place: Vec<_> = files_under(&books)
        .iter()
        .map(|path| name_of(path))
        .collect();
    left_in_place.sort();
    let mut expected: Vec<_> = names.iter().map(|(name, _)| name.to_string()).collect();
    expected.sort();
    assert_eq!(left_in_place, expected, "the source folder gains nothing");
}

#[tokio::test]
async fn a_hundred_file_group_exports_in_order_with_a_chapter_each() {
    let desk = Desk::new();
    let parts = desk.root.path().join("parts");
    for number in 1..=100 {
        write_wav(&parts.join(format!("Part {number}.wav")), 1, None, None);
    }
    desk.send(SessionIntent::Import {
        paths: vec![parts.to_string_lossy().into_owned()],
    })
    .await;
    desk.send(SessionIntent::SelectAll).await;
    desk.send(SessionIntent::GroupSelected).await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Hundred Parts".to_string(),
    })
    .await;

    desk.export().await;
    desk.wait_until("the book is published", |desk| {
        !files_under(&desk.root.path().join("exports")).is_empty()
    })
    .await;
    let output = files_under(&desk.root.path().join("exports")).remove(0);

    let chapters = chapters_of(&output);
    assert_eq!(chapters.len(), 100);
    let titles: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.0.clone().unwrap_or_default())
        .collect();
    let expected: Vec<_> = (1..=100).map(|number| format!("Part {number}")).collect();
    assert_eq!(titles, expected);
    let starts: Vec<_> = chapters.iter().map(|chapter| chapter.1).collect();
    assert!(
        starts.windows(2).all(|pair| pair[0] < pair[1]),
        "{starts:?}"
    );
    let last_end = chapters[99].2;
    assert!(
        (last_end - 100_000).abs() < 100,
        "the chapters span 100 s: {last_end}"
    );
}

#[tokio::test]
async fn two_titles_differing_only_by_case_never_overwrite_each_other() {
    let desk = Desk::new();
    let books = desk.root.path().join("books");
    write_wav(&books.join("one.wav"), 1, Some("Dune"), Some("Same Author"));
    write_wav(&books.join("two.wav"), 2, Some("dune"), Some("Same Author"));
    desk.send(SessionIntent::Import {
        paths: vec![books.to_string_lossy().into_owned()],
    })
    .await;

    let operation = export_renaming_any_collision(&desk).await;

    assert_eq!(
        desk.export_status(&operation),
        WorkOperationStatus::Completed
    );
    let outputs = files_under(&desk.root.path().join("exports"));
    assert_eq!(output_seconds(&outputs), [1, 2], "{outputs:?}");
}

/// Where the file system folds Unicode forms together (APFS), the second
/// title's output is the first one's path. It must fail, never replace.
#[tokio::test]
async fn two_titles_differing_only_by_unicode_form_never_overwrite_each_other() {
    let nfc = "Caf\u{e9}";
    let nfd = "Cafe\u{301}";
    let desk = Desk::new();
    let books = desk.root.path().join("books");
    write_wav(&books.join("one.wav"), 1, Some(nfc), Some("Same Author"));
    write_wav(&books.join("two.wav"), 2, Some(nfd), Some("Same Author"));
    desk.send(SessionIntent::Import {
        paths: vec![books.to_string_lossy().into_owned()],
    })
    .await;

    let operation = export_renaming_any_collision(&desk).await;

    let children = desk
        .engine
        .list_work_operations()
        .expect("operations")
        .operations
        .into_iter()
        .find(|snapshot| snapshot.operation_id == operation)
        .expect("listed")
        .children;
    let written = children
        .iter()
        .filter(|child| child.status == ChildJobStatus::Completed)
        .count();
    let refused: Vec<_> = children
        .iter()
        .filter(|child| child.status == ChildJobStatus::Failed)
        .collect();
    assert_eq!(written + refused.len(), 2, "{children:?}");
    for child in &refused {
        assert!(
            child.progress.message.contains("collision"),
            "a refused title says why: {:?}",
            child.progress.message
        );
    }
    let outputs = files_under(&desk.root.path().join("exports"));
    assert_eq!(outputs.len(), written, "{outputs:?}");
    let seconds = output_seconds(&outputs);
    assert!(
        seconds.windows(2).all(|pair| pair[0] != pair[1]),
        "no output holds another title's audio: {seconds:?}"
    );
}
