use super::*;

fn field_of(rig: &Rig, field: MetadataField) -> crate::session::FieldSnapshot {
    rig.metadata()
        .form
        .fields
        .into_iter()
        .find(|snapshot| snapshot.field == field)
        .expect("field")
}

#[tokio::test]
async fn a_keystroke_typed_before_an_import_selected_a_title_does_not_dirty_it() {
    let rig = rig();
    let folder = tempfile::TempDir::new().expect("temp dir");
    let book = staged_wav(folder.path(), "book");
    let blank_form = rig.metadata().binding;
    rig.send(SessionIntent::Import {
        paths: vec![book.to_string_lossy().into_owned()],
    })
    .await;
    assert_ne!(rig.metadata().binding, blank_form);

    let outcome = rig
        .send(SessionIntent::SetField {
            binding: blank_form,
            field: MetadataField::Genre,
            value: "stray".to_string(),
        })
        .await;

    assert_eq!(outcome, SessionOutcome::EditLost);
    let genre = field_of(&rig, MetadataField::Genre);
    assert!(!genre.dirty && genre.value != "stray");
    assert_eq!(rig.pending("book"), None);
}

#[tokio::test]
async fn a_field_action_aimed_at_a_form_that_was_replaced_changes_nothing() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);
    rig.select(&[0]).await;
    let alpha_form = rig.metadata().binding;
    rig.select(&[1]).await;

    let outcome = rig
        .send(SessionIntent::SetFieldAction {
            binding: alpha_form,
            field: MetadataField::Genre,
            action: crate::session::FieldAction::Blank,
        })
        .await;

    assert_eq!(outcome, SessionOutcome::EditLost);
    assert!(!field_of(&rig, MetadataField::Genre).dirty);
}
