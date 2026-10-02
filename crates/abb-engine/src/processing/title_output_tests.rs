//! When an edit accepted for an export title reaches its output file.

use std::sync::{Arc, Mutex};

use super::*;

type Writes = Arc<Mutex<Vec<(PathBuf, MetadataIntentPatch)>>>;

fn base() -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some("Alpha".to_string()),
        artist: Some("Author".to_string()),
        album: Some("Alpha".to_string()),
        genre: Some("Fantasy".to_string()),
        cover_art: Some(jpeg()),
        ..AudiobookMetadata::default()
    }
}

fn jpeg() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(16, 16)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Jpeg,
        )
        .expect("encode JPEG");
    bytes
}

fn set(value: &str) -> Option<PatchOp<String>> {
    Some(PatchOp::Set(value.to_string()))
}

/// A title accepted with no edit, writing through a recorder.
fn title(folder: &Path) -> (Arc<TitleOutput>, Writes) {
    let mut plan = TitleOutputPlan {
        anchor: PathBuf::from("/books/alpha.m4b"),
        sources: Vec::new(),
        base: Some(base()),
        accepted: None,
        output_dir: folder.to_path_buf(),
        naming: OutputNamingConfig::default(),
        extension: "m4b".to_string(),
        requested: PathBuf::new(),
    };
    plan.requested = plan.tags(None).expect("plan").1;
    let writes: Writes = Arc::default();
    let recorder = Arc::clone(&writes);
    let title = TitleOutput::with_writer(
        plan,
        Box::new(move |path, patch| {
            recorder
                .lock()
                .expect("writes")
                .push((path.to_path_buf(), patch.clone()));
            Ok(())
        }),
    )
    .expect("title output");
    (title, writes)
}

/// Publishes `title` by writing a file at its requested path.
fn publish(title: &TitleOutput, staged: &Path) -> PathBuf {
    let path = title.plan.requested.clone();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
    title
        .publish(staged, &path, || {
            std::fs::write(&path, b"audiobook").expect("publish");
            Ok(((), path.clone()))
        })
        .expect("published");
    path
}

fn genre(genre: &str) -> MetadataIntentPatch {
    MetadataIntentPatch {
        genre: set(genre),
        ..MetadataIntentPatch::default()
    }
}

#[test]
fn an_edit_accepted_before_publication_is_written_to_the_staged_file() {
    let folder = tempfile::TempDir::new().expect("folder");
    let (title, writes) = title(folder.path());

    assert_eq!(
        title.update(1, &genre("Mystery")).expect("update"),
        UpdateReply::Accepted {
            published: false,
            elsewhere: false
        }
    );
    // The same edit again changes nothing.
    assert_eq!(
        title.update(2, &genre("Mystery")).expect("update"),
        UpdateReply::Unchanged
    );
    let staged = folder.path().join("staged.m4b");
    publish(&title, &staged);

    let writes = writes.lock().expect("writes");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, staged);
    assert_eq!(writes[0].1, genre("Mystery"));
    assert_eq!(
        title.update_state().map(|update| update.status),
        Some(OutputUpdateStatus::Applied)
    );
}

#[test]
fn an_edit_accepted_during_publication_reaches_the_published_file() {
    let folder = tempfile::TempDir::new().expect("folder");
    let (title, writes) = title(folder.path());
    let path = title.plan.requested.clone();
    std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");

    title
        .publish(&folder.path().join("staged.m4b"), &path, || {
            std::fs::write(&path, b"audiobook").expect("publish");
            // Accepted while the file is being published.
            title.update(1, &genre("Mystery")).expect("update");
            Ok(((), path.clone()))
        })
        .expect("published");

    let writes = writes.lock().expect("writes");
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, path);
}

#[test]
fn a_naming_edit_moves_an_unpublished_output_and_only_retags_a_published_one() {
    let folder = tempfile::TempDir::new().expect("folder");
    let (title, writes) = title(folder.path());
    let renamed = MetadataIntentPatch {
        title: set("Beta"),
        ..MetadataIntentPatch::default()
    };

    let UpdateReply::MovesOutput { from, to } = title.update(1, &renamed).expect("update") else {
        panic!("an unpublished output moves");
    };
    assert_eq!(from, title.plan.requested);
    assert_ne!(to, from);
    assert!(title.update_state().is_none(), "nothing from it is applied");

    let published = publish(&title, &folder.path().join("staged.m4b"));
    assert!(writes.lock().expect("writes").is_empty());
    assert_eq!(
        title.update(2, &renamed).expect("update"),
        UpdateReply::Accepted {
            published: true,
            elsewhere: true
        }
    );
    title.apply_published();
    assert_eq!(writes.lock().expect("writes")[0].0, published);
}

#[test]
fn a_published_file_changed_by_someone_else_is_not_written_and_the_next_save_retries() {
    let folder = tempfile::TempDir::new().expect("folder");
    let (title, writes) = title(folder.path());
    let published = publish(&title, &folder.path().join("staged.m4b"));
    std::fs::write(&published, b"replaced by another program").expect("replace");

    title.update(1, &genre("Mystery")).expect("update");
    title.apply_published();
    assert!(writes.lock().expect("writes").is_empty());
    assert!(matches!(
        title.update_state().map(|update| update.status),
        Some(OutputUpdateStatus::Failed { .. })
    ));

    // The output still carries the old tags, so the same edit is accepted again.
    assert!(matches!(
        title.update(2, &genre("Mystery")).expect("update"),
        UpdateReply::Accepted { .. }
    ));
}

#[test]
fn a_cover_clear_is_written_even_when_the_planned_tags_already_lack_a_cover() {
    let folder = tempfile::TempDir::new().expect("folder");
    let (title, writes) = title(folder.path());
    let clear = MetadataIntentPatch {
        cover_art: Some(PatchOp::Clear),
        ..MetadataIntentPatch::default()
    };
    title.update(1, &clear).expect("update");
    publish(&title, &folder.path().join("staged.m4b"));
    assert_eq!(
        writes.lock().expect("writes")[0].1.cover_art,
        Some(PatchOp::Clear)
    );

    // Leaving the cover to the sources again restores the anchor's picture.
    title
        .update(2, &MetadataIntentPatch::default())
        .expect("update");
    title.apply_published();
    assert!(matches!(
        &writes.lock().expect("writes")[1].1.cover_art,
        Some(PatchOp::Set(bytes)) if !bytes.is_empty()
    ));
}

#[tokio::test]
async fn a_title_ending_without_an_output_drops_its_waiting_edit() {
    let folder = tempfile::TempDir::new().expect("folder");
    let (title, writes) = title(folder.path());
    title.update(1, &genre("Mystery")).expect("update");

    title.end();

    assert!(!title.settled().await);
    assert_eq!(
        title.update_state().map(|update| update.status),
        Some(OutputUpdateStatus::NotApplied)
    );
    assert_eq!(
        title.update(2, &genre("Horror")).expect("update"),
        UpdateReply::NoOutput
    );
    assert!(writes.lock().expect("writes").is_empty());
}
