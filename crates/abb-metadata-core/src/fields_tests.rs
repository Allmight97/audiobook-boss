use super::*;

#[test]
fn tag_fields_list_every_field_in_write_order_under_their_log_names() {
    let names: Vec<String> = TagField::ALL.iter().map(ToString::to_string).collect();
    assert_eq!(
        names,
        [
            "Title",
            "Artist",
            "Album",
            "Composer",
            "Genre",
            "Date",
            "Comment",
            "Description",
            "Series",
            "SeriesPart",
            "Track",
            "Disk"
        ]
    );
    let texts: Vec<_> = TagField::ALL
        .iter()
        .filter_map(|field| match field {
            TagField::Text(text) => Some(*text),
            TagField::Position(_) => None,
        })
        .collect();
    assert_eq!(texts, TextField::ALL);
}

#[test]
fn accessors_reach_the_struct_field_each_name_stands_for() {
    let mut metadata = AudiobookMetadata::default();
    for (index, field) in TextField::ALL.into_iter().enumerate() {
        *metadata.text_mut(field) = Some(format!("value {index}"));
    }
    assert_eq!(metadata.title.as_deref(), Some("value 0"));
    assert_eq!(metadata.comment.as_deref(), Some("value 6"));
    assert_eq!(metadata.series.as_deref(), Some("value 8"));
    assert_eq!(metadata.series_part.as_deref(), Some("value 9"));
    assert_eq!(metadata.subseries, None);
    for (index, field) in TextField::ALL.into_iter().enumerate() {
        assert_eq!(metadata.text(field), Some(&format!("value {index}")));
    }

    *metadata.position_mut(PositionField::Disk) = Some((2, Some(5)));
    assert_eq!(metadata.disk, Some((2, Some(5))));
    assert_eq!(metadata.position(PositionField::Disk), Some((2, Some(5))));
    assert_eq!(metadata.position(PositionField::Track), None);
}

#[test]
fn patch_accessors_reach_the_patch_field_each_name_stands_for() {
    let patch = MetadataIntentPatch {
        description: Some(PatchOp::Clear),
        series_part: Some(PatchOp::Set("3".into())),
        disk: Some(PatchOp::Set((1, None))),
        ..Default::default()
    };
    assert_eq!(
        patch.text(TextField::Description),
        Some(&PatchOp::Clear::<String>)
    );
    assert_eq!(
        patch.text(TextField::SeriesPart),
        Some(&PatchOp::Set("3".into()))
    );
    assert_eq!(patch.text(TextField::Title), None);
    assert_eq!(
        patch.position(PositionField::Disk),
        Some(&PatchOp::Set((1, None)))
    );
    assert_eq!(patch.position(PositionField::Track), None);
}
