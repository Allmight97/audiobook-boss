use super::*;

#[test]
fn an_update_carries_only_the_parts_that_changed() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let before = desk.state.revision();

    desk.type_into(MetadataField::Genre, "Mystery");
    let update = desk.state.update_since(Some(before));
    assert!(update.metadata.is_some());
    assert!(update.titles.is_none() && update.selection.is_none() && update.lookup.is_none());

    let before = desk.state.revision();
    desk.state.settle();
    assert_eq!(
        desk.state.revision(),
        before,
        "nothing changed, nothing stamped"
    );

    desk.select(&[1]).expect("select beta");
    let update = desk.state.update_since(Some(before));
    assert!(update.selection.is_some() && update.metadata.is_some());
    assert!(
        update.titles.is_none(),
        "selecting does not resend the titles"
    );
}

#[test]
fn the_tag_preview_and_output_path_follow_every_value_on_screen() {
    let mut desk = Desk::open(&[("alpha", Some(AudiobookMetadata::default()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    for (field, value) in [
        (MetadataField::Title, "  The Book  "),
        (MetadataField::Author, " An Author "),
        (MetadataField::Narrator, " A Reader "),
        (MetadataField::Series, " Saga "),
        (MetadataField::SeriesPart, " 2 "),
        (MetadataField::Subseries, " Arc "),
        (MetadataField::SubseriesPart, " 3 "),
        (MetadataField::Date, " 2021-05 "),
        (MetadataField::Genre, " Fantasy "),
        (MetadataField::Description, " Not previewed "),
    ] {
        desk.type_into(field, value);
    }

    let tags = desk.metadata().tags;

    assert_eq!(
        tags,
        TagPreview {
            title: "The Book".to_string(),
            album: "The Book".to_string(),
            artist: "An Author".to_string(),
            album_artist: "An Author".to_string(),
            composer: "A Reader".to_string(),
            series: "Saga".to_string(),
            series_part: "2".to_string(),
            subseries: "Arc".to_string(),
            subseries_part: "3".to_string(),
            album_sort: tags.album_sort.clone(),
            year: "2021-05".to_string(),
            genre: "Fantasy".to_string(),
        }
    );
    assert!(
        tags.album_sort.contains("Saga") && tags.album_sort.contains("The Book"),
        "{}",
        tags.album_sort
    );
    desk.state.settle();
    let path = match desk
        .state
        .update_since(None)
        .output
        .expect("output")
        .preview
    {
        crate::session::output::OutputPreview::Path { path } => path,
        other => panic!("expected a preview path, got {other:?}"),
    };
    assert!(
        path.contains("An Author") && path.contains("The Book"),
        "{path}"
    );
}
