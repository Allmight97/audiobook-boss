use super::*;

#[test]
fn round_trips_standard_freeform_utf16_and_position_atoms_in_order() {
    let mut tag = Mp4Tag::default();
    tag.set_fourcc_utf8(TITLE_ATOM, "Seed Title");
    tag.set_data(
        AtomIdent::Fourcc(ALBUM_ATOM),
        AtomData::Utf16("UTF16 \u{2014} Album".to_string()),
    );
    tag.set_track(0, 0);
    tag.set_disc(1, 3);
    tag.set_data(
        AtomIdent::freeform("com.apple.iTunes", "ASIN"),
        AtomData::Utf8("B000000001".to_string()),
    );
    tag.set_media_type_audiobook();

    let encoded = tag.encode_ilst();
    let ilst_slices = parse_child_slices(&encoded).expect("parse wrapper");
    let decoded = Mp4Tag::parse_ilst(ilst_slices[0].content(&encoded)).expect("parse ilst");

    assert_eq!(decoded.first_fourcc_string(TITLE_ATOM), Some("Seed Title"));
    assert_eq!(
        decoded.first_fourcc_string(ALBUM_ATOM),
        Some("UTF16 \u{2014} Album")
    );
    assert_eq!(decoded.track(), (None, None));
    assert_eq!(decoded.disc(), (Some(1), Some(3)));
    assert_eq!(decoded.media_type_code(), Some(2));
    assert_eq!(decoded.items.len(), 6);
    assert_eq!(decoded.items[0].ident.to_string(), "\u{a9}nam");
    assert_eq!(
        decoded.items[4].ident.to_string(),
        "----:com.apple.iTunes:ASIN"
    );
}

#[test]
fn set_data_updates_existing_ident_in_place_and_preserves_later_atoms() {
    let mut tag = Mp4Tag::default();
    tag.set_fourcc_utf8(TITLE_ATOM, "Before");
    tag.set_data(
        AtomIdent::freeform("com.apple.iTunes", "ASIN"),
        AtomData::Utf8("ASIN1".to_string()),
    );
    tag.add_data(AtomIdent::Fourcc(COVR_ATOM), AtomData::Png(vec![1, 2, 3]));
    tag.set_fourcc_utf8(TITLE_ATOM, "After");
    tag.set_data(
        AtomIdent::Fourcc(COVR_ATOM),
        AtomData::Jpeg(vec![0xFF, 0xD8]),
    );

    let names: Vec<String> = tag
        .items
        .iter()
        .map(|item| item.ident.to_string())
        .collect();
    assert_eq!(names, ["\u{a9}nam", "----:com.apple.iTunes:ASIN", "covr"]);
    assert_eq!(tag.first_fourcc_string(TITLE_ATOM), Some("After"));
    assert_eq!(tag.first_artwork(), Some([0xFF, 0xD8].as_slice()));
}
