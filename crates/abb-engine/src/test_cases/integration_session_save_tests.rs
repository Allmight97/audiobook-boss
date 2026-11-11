use std::fs;
use std::path::Path;

use crate::metadata::{read_metadata, AudiobookMetadata};
use crate::session::{
    FieldAction, MetadataField, MetadataStatus, OutputEdits, SelectionModifiers, SessionIntent,
    SessionOutcome, TitlePlan,
};
use crate::work_runtime::WorkOperationStatus;
use crate::ShutdownOutcome;

use super::integration_session_desk::Desk;
use super::media_fixtures::ffmpeg;

fn id_of(desk: &Desk, name: &str) -> String {
    desk.engine
        .session_snapshot()
        .titles
        .expect("titles")
        .files
        .into_iter()
        .find(|file| file.path.file_name().is_some_and(|found| found == name))
        .unwrap_or_else(|| panic!("{name} is listed"))
        .input_id
}

async fn only(desk: &Desk, name: &str) {
    let outcome = desk
        .send(SessionIntent::SelectFile {
            title_id: id_of(desk, name),
            modifiers: SelectionModifiers::default(),
        })
        .await;
    assert_eq!(outcome, SessionOutcome::Applied);
}

fn listed_cover(desk: &Desk) -> Option<String> {
    let titles = desk.engine.session_snapshot().titles.expect("titles");
    titles.files.into_iter().next().and_then(|file| file.cover)
}

fn tags(path: &Path) -> AudiobookMetadata {
    read_metadata(path.to_string_lossy().as_ref()).expect("read tags")
}

#[tokio::test]
async fn one_save_blanks_author_on_two_titles_and_writes_each_titles_own_edit() {
    let desk = Desk::new();
    let alpha = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 0.2)
        .await;
    let beta = desk.root.path().join("library/beta.m4b");
    fs::copy(&alpha, &beta).expect("copy second book");
    desk.import(&alpha).await;
    desk.import(&beta).await;

    only(&desk, "alpha.m4b").await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Genre,
        value: "Mystery".to_string(),
    })
    .await;
    only(&desk, "beta.m4b").await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "New".to_string(),
    })
    .await;
    desk.send(SessionIntent::SelectAll).await;
    desk.send(SessionIntent::SetFieldAction {
        binding: desk.metadata().binding,
        field: MetadataField::Author,
        action: FieldAction::Blank,
    })
    .await;

    assert_eq!(
        desk.send(SessionIntent::Save).await,
        SessionOutcome::Applied
    );

    let (alpha_tags, beta_tags) = (tags(&alpha), tags(&beta));
    assert_eq!(alpha_tags.artist, None, "alpha author is blanked");
    assert_eq!(beta_tags.artist, None, "beta author is blanked");
    assert_eq!(alpha_tags.genre.as_deref(), Some("Mystery"));
    assert_eq!(
        beta_tags.genre.as_deref(),
        Some("Fantasy"),
        "alpha's genre edit stays on alpha"
    );
    assert_eq!(beta_tags.title.as_deref(), Some("New"));
    assert_eq!(beta_tags.album.as_deref(), Some("New"));
    assert_eq!(alpha_tags.title.as_deref(), Some("Alpha"));

    let metadata = desk.metadata();
    assert_eq!(
        metadata.status,
        Some(MetadataStatus::SaveComplete {
            succeeded: 2,
            failed: 0,
            cancelled: 0,
            waiting: 0,
            held: 0,
            outputs: Default::default(),
        })
    );
    assert!(!metadata.save_in_progress && !metadata.has_pending_edits);
    assert!(alpha.exists() && beta.exists(), "both files stay in place");

    let operations = desk.engine.list_work_operations().expect("operations");
    let save = operations.operations.first().expect("the save is listed");
    assert_eq!(save.status, WorkOperationStatus::Completed);
    assert_eq!(save.children.len(), 2, "the save reports each title");
}

#[tokio::test]
async fn a_save_changes_the_title_and_author_the_list_shows() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 0.2)
        .await;
    desk.import(&book).await;
    let listed = |desk: &Desk| {
        let file = desk
            .engine
            .session_snapshot()
            .titles
            .expect("titles")
            .files
            .remove(0);
        (file.label, file.artist)
    };
    assert_eq!(
        listed(&desk),
        ("Alpha".to_string(), Some("Source Author".to_string()))
    );

    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "Renamed".to_string(),
    })
    .await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Author,
        value: "New Author".to_string(),
    })
    .await;
    assert_eq!(
        desk.send(SessionIntent::Save).await,
        SessionOutcome::Applied
    );

    assert_eq!(
        listed(&desk),
        ("Renamed".to_string(), Some("New Author".to_string()))
    );
}

#[tokio::test]
async fn a_save_lists_the_new_file_size_and_keeps_the_resolved_plan() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 0.2)
        .await;
    desk.import(&book).await;
    desk.wait_until("the plan resolves", |desk| {
        matches!(desk.plan(), TitlePlan::Resolved { .. })
    })
    .await;
    let imported = fs::metadata(&book).expect("source").len();

    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "A much longer title than the one the file had ".repeat(40),
    })
    .await;
    assert_eq!(
        desk.send(SessionIntent::Save).await,
        SessionOutcome::Applied
    );

    let saved = fs::metadata(&book).expect("source").len();
    assert_ne!(saved, imported);
    let titles = desk.engine.session_snapshot().titles.expect("titles");
    assert_eq!(titles.files[0].size, Some(saved as f64));
    assert_eq!(titles.total_size_bytes, saved as f64);
    assert!(matches!(desk.plan(), TitlePlan::Resolved { .. }));
}

#[tokio::test]
async fn a_second_save_on_a_finished_output_keeps_the_first_saves_tag() {
    let desk = Desk::new();
    let source = desk
        .audiobook(&desk.root.path().join("source.m4b"), 0.2)
        .await;
    desk.import(&source).await;
    let export = desk.export_and_finish().await;
    assert_eq!(desk.export_status(&export), WorkOperationStatus::Completed);
    let output = desk.output_of(&export);

    desk.edit_genre_and_save().await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Narrator,
        value: "Reader".to_string(),
    })
    .await;
    assert_eq!(
        desk.send(SessionIntent::Save).await,
        SessionOutcome::Applied
    );

    let output_tags = tags(&output);
    assert_eq!(
        output_tags.genre.as_deref(),
        Some("Mystery"),
        "the second save keeps the first save's genre"
    );
    assert_eq!(output_tags.composer.as_deref(), Some("Reader"));
    assert_eq!(output_tags.artist.as_deref(), Some("Source Author"));
    assert!(matches!(
        desk.metadata().status,
        Some(MetadataStatus::SaveComplete {
            outputs: OutputEdits {
                updated: 1,
                failed: 0,
                ..
            },
            ..
        })
    ));
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}

#[tokio::test]
async fn a_cover_loaded_and_then_cleared_through_the_session_is_on_and_then_off_the_file() {
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 0.2)
        .await;
    desk.import(&book).await;
    assert!(tags(&book).cover_art.is_none());
    let before_cover = listed_cover(&desk);
    assert!(before_cover.is_some());

    let custom = desk.root.path().join("custom.png");
    image::RgbImage::from_pixel(1, 1, image::Rgb([180, 20, 90]))
        .save(&custom)
        .expect("custom cover");
    assert_eq!(
        desk.send(SessionIntent::LoadCoverFromFile {
            path: custom.to_string_lossy().into_owned(),
        })
        .await,
        SessionOutcome::Applied
    );
    assert!(desk.metadata().cover.custom);
    desk.edit_genre_and_save().await;
    assert!(
        tags(&book).cover_art.is_some(),
        "the chosen cover is in the file"
    );
    let after_cover = listed_cover(&desk);
    assert_ne!(
        after_cover, before_cover,
        "a saved cover moves the cover address"
    );

    desk.send(SessionIntent::ClearCover).await;
    assert_eq!(
        desk.send(SessionIntent::Save).await,
        SessionOutcome::Applied
    );

    let cleared = tags(&book);
    assert!(
        cleared.cover_art.is_none(),
        "the cleared cover is off the file"
    );
    assert_eq!(cleared.genre.as_deref(), Some("Mystery"));
    assert!(desk.metadata().cover.image.is_none());
    assert_ne!(
        listed_cover(&desk),
        after_cover,
        "a cleared cover moves the cover address"
    );
}

#[tokio::test]
async fn a_save_on_raw_aac_fails_with_its_reason_and_keeps_the_edit_pending() {
    let desk = Desk::new();
    let book = desk.root.path().join("library/raw.aac");
    fs::create_dir_all(book.parent().expect("library")).expect("library dir");
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "sine=d=1",
        "-c:a",
        "aac",
        "-f",
        "adts",
        "-y",
        book.to_str().expect("utf8 path"),
    ]);
    let bytes = fs::read(&book).expect("read source");
    desk.import(&book).await;
    desk.send(SessionIntent::SetField {
        binding: desk.metadata().binding,
        field: MetadataField::Title,
        value: "NewTitle".to_string(),
    })
    .await;

    assert_eq!(
        desk.send(SessionIntent::Save).await,
        SessionOutcome::Applied
    );

    let metadata = desk.metadata();
    assert!(
        matches!(
            metadata.status,
            Some(MetadataStatus::SaveComplete {
                succeeded: 0,
                failed: 1,
                ..
            })
        ),
        "{:?}",
        metadata.status
    );
    assert!(metadata.has_pending_edits, "the edit stays pending");
    assert_eq!(fs::read(&book).expect("reread"), bytes);
    let operations = desk.engine.list_work_operations().expect("operations");
    let child = &operations.operations[0].children[0];
    assert!(
        child
            .message
            .as_deref()
            .is_some_and(|message| message.contains("Raw AAC (.aac) files can't hold tags")),
        "{:?}",
        child.message
    );
}
