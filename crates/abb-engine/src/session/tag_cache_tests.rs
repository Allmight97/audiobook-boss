use std::collections::HashSet;
use std::path::Path;

use super::TagCache;
use crate::metadata::{AudiobookMetadata, MetadataIntentPatch, PatchOp};

const PATH: &str = "/books/a.m4b";

fn path() -> &'static Path {
    Path::new(PATH)
}

fn author() -> MetadataIntentPatch {
    MetadataIntentPatch {
        artist: Some(PatchOp::Set("Edited Author".to_string())),
        ..Default::default()
    }
}

fn title() -> MetadataIntentPatch {
    MetadataIntentPatch {
        title: Some(PatchOp::Set("NMR 64k".to_string())),
        ..Default::default()
    }
}

fn clear_author() -> MetadataIntentPatch {
    MetadataIntentPatch {
        artist: Some(PatchOp::Clear),
        ..Default::default()
    }
}

fn tags(title: &str, artist: Option<&str>) -> AudiobookMetadata {
    AudiobookMetadata {
        title: Some(title.to_string()),
        artist: artist.map(str::to_string),
        ..Default::default()
    }
}

fn read(cache: &mut TagCache, metadata: AudiobookMetadata) -> bool {
    let ticket = cache.begin_read(path()).expect("read is needed");
    cache.complete_read(&ticket, metadata)
}

fn save_pending(cache: &mut TagCache) {
    let pending = cache.pending(path()).expect("pending intent").clone();
    cache.commit_saved(path(), &pending.patch, pending.revision);
}

#[test]
fn an_explicit_clear_is_kept_unless_a_source_read_proves_it_changes_nothing() {
    let mut cache = TagCache::default();
    cache.stage(path(), &clear_author());
    assert_eq!(
        cache.pending(path()).map(|pending| &pending.patch),
        Some(&clear_author())
    );

    cache.clear();
    assert!(read(&mut cache, tags("Known coverless book", None)));
    cache.stage(path(), &clear_author());
    assert!(cache.pending(path()).is_none());
}

#[test]
fn a_saved_patch_folds_into_the_known_tags_and_stops_being_pending() {
    let mut cache = TagCache::default();
    read(&mut cache, tags("Old", Some("Source Author")));
    cache.stage(path(), &author());

    save_pending(&mut cache);

    assert!(cache.pending(path()).is_none());
    assert_eq!(
        cache.effective(path()),
        Some(tags("Old", Some("Edited Author")))
    );
}

#[test]
fn an_edit_staged_while_a_save_ran_keeps_the_whole_pending_patch() {
    let mut cache = TagCache::default();
    read(&mut cache, tags("Old", Some("Source Author")));
    cache.stage(path(), &author());
    let submitted = cache.pending(path()).expect("pending").clone();
    cache.stage(path(), &title());

    cache.commit_saved(path(), &submitted.patch, submitted.revision);

    let mut expected = author();
    expected.merge(&title());
    assert_eq!(
        cache.pending(path()).map(|pending| &pending.patch),
        Some(&expected)
    );
    assert_eq!(
        cache.effective(path()),
        Some(tags("NMR 64k", Some("Edited Author")))
    );
}

#[test]
fn saved_values_without_a_read_are_partial_knowledge() {
    let mut cache = TagCache::default();
    let mut patch = author();
    patch.cover_art = Some(PatchOp::Set(vec![9, 9, 9]));
    cache.stage(path(), &patch);

    save_pending(&mut cache);

    assert_eq!(
        cache.effective(path()),
        Some(AudiobookMetadata {
            artist: Some("Edited Author".to_string()),
            cover_art: Some(vec![9, 9, 9]),
            ..Default::default()
        })
    );
    assert!(!cache.has_source_read(path()));
    // The source value is unknown, so a later Blank still has to reach the file.
    cache.stage(path(), &clear_author());
    assert_eq!(
        cache.pending(path()).map(|pending| &pending.patch),
        Some(&clear_author())
    );
}

#[test]
fn a_first_read_after_an_unread_save_adds_unknown_tags_and_keeps_newer_edits() {
    let mut cache = TagCache::default();
    cache.stage(path(), &author());
    save_pending(&mut cache);
    cache.stage(path(), &title());

    read(
        &mut cache,
        AudiobookMetadata {
            genre: Some("Fantasy".to_string()),
            ..tags("Old", Some("Edited Author"))
        },
    );

    assert!(cache.has_source_read(path()));
    assert_eq!(
        cache.effective(path()),
        Some(AudiobookMetadata {
            genre: Some("Fantasy".to_string()),
            ..tags("NMR 64k", Some("Edited Author"))
        })
    );
}

#[test]
fn a_read_begun_before_a_save_cannot_replace_the_saved_value() {
    let mut cache = TagCache::default();
    let ticket = cache.begin_read(path()).expect("read is needed");
    cache.stage(path(), &author());
    save_pending(&mut cache);

    assert!(!cache.complete_read(&ticket, tags("Old", Some("Source Author"))));

    assert_eq!(
        cache.effective(path()).and_then(|tags| tags.artist),
        Some("Edited Author".to_string())
    );
}

#[test]
fn a_late_read_cannot_revive_a_removed_or_reset_file_and_reimport_reads_again() {
    for reset in [false, true] {
        let mut cache = TagCache::default();
        let ticket = cache.begin_read(path()).expect("read is needed");
        if reset {
            cache.clear();
        } else {
            cache.retain_paths(&HashSet::new());
        }

        assert!(!cache.complete_read(&ticket, tags("Removed", None)));
        assert!(cache.effective(path()).is_none());
        assert!(!cache.has_source_read(path()));

        assert!(read(&mut cache, tags("Reimported", None)));
        assert_eq!(cache.effective(path()), Some(tags("Reimported", None)));
    }
}

#[test]
fn a_cover_only_read_is_not_a_complete_baseline() {
    let mut cache = TagCache::default();
    read(
        &mut cache,
        AudiobookMetadata {
            cover_art: Some(vec![1, 2, 3]),
            ..Default::default()
        },
    );

    assert!(!cache.has_source_read(path()));
    assert_eq!(cache.effective_cover(path()), Some(vec![1, 2, 3]));
    assert!(cache.begin_read(path()).is_some());
}
