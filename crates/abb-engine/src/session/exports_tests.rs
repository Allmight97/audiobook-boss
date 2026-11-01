use super::*;
use crate::metadata::PatchOp;
use crate::processing::title_output::TitleOutputPlan;

fn output() -> Arc<TitleOutput> {
    TitleOutput::with_writer(
        TitleOutputPlan {
            anchor: PathBuf::from("/books/alpha.m4b"),
            sources: Vec::new(),
            base: None,
            accepted: None,
            output_dir: PathBuf::from("/library"),
            naming: OutputNamingConfig::default(),
            extension: "m4b".to_string(),
            requested: PathBuf::from("/library/alpha.m4b"),
        },
        Box::new(|_, _| Ok(())),
    )
    .expect("output")
}

fn set(field: &str, value: &str) -> MetadataIntentPatch {
    let op = Some(PatchOp::Set(value.to_string()));
    match field {
        "genre" => MetadataIntentPatch {
            genre: op,
            ..MetadataIntentPatch::default()
        },
        _ => MetadataIntentPatch {
            composer: op,
            ..MetadataIntentPatch::default()
        },
    }
}

fn linked() -> Exports {
    let mut exports = Exports::default();
    exports.link(
        &OperationId("operation-1".to_string()),
        [("alpha".to_string(), PathBuf::from("/books/alpha.m4b"))],
        &[output()],
    );
    exports
}

#[test]
fn an_edit_already_written_to_the_source_stays_in_the_output_edit() {
    let mut exports = linked();
    exports.acknowledge(Path::new("/books/alpha.m4b"), &set("genre", "Mystery"));

    let edits = exports.edits(|_| Some(set("narrator", "Reader")));

    let intent = &edits[0].intent;
    assert_eq!(intent.genre, Some(PatchOp::Set("Mystery".to_string())));
    assert_eq!(intent.composer, Some(PatchOp::Set("Reader".to_string())));
}

#[test]
fn an_offer_is_taken_only_as_shown_and_a_kept_location_is_not_offered_again() {
    let mut exports = linked();
    let naming = OutputNamingConfig::default();
    let directory = Some("/library".to_string());
    let edit = exports.edits(|_| None).remove(0);
    let (from, to) = (Path::new("/library/a.m4b"), Path::new("/library/b.m4b"));
    assert!(exports.offer(&edit, from, to, directory.clone(), naming.clone()));

    assert!(exports
        .offered("alpha", edit.revision + 1, directory.as_ref(), &naming)
        .is_err());
    assert!(exports
        .offered(
            "alpha",
            edit.revision,
            Some(&"/elsewhere".to_string()),
            &naming
        )
        .is_err());

    assert!(exports
        .offered("alpha", edit.revision, directory.as_ref(), &naming)
        .is_ok());
    assert!(exports.decline("alpha", edit.revision));
    assert!(exports.offers().is_empty());
    let later = exports.edits(|_| None).remove(0);
    assert!(!exports.offer(&later, from, to, directory, naming));
}

#[test]
fn an_older_reply_cannot_remove_a_newer_offer_or_a_replacement_export() {
    let mut exports = linked();
    let old = exports.edits(|_| None).remove(0);
    let new = exports.edits(|_| None).remove(0);
    exports.offer(
        &new,
        Path::new("/old"),
        Path::new("/new"),
        None,
        OutputNamingConfig::default(),
    );
    assert!(!exports.accepts_reply(&old));
    assert!(exports.accepts_reply(&new));
    exports.link(
        &OperationId("replacement".into()),
        [("alpha".into(), PathBuf::from("/books/alpha.m4b"))],
        &[output()],
    );
    assert!(!exports.accepts_reply(&new));
}
