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
    let items: Vec<&IlstItem> = decoded.items().collect();
    assert_eq!(items.len(), 6);
    assert_eq!(items[0].ident.to_string(), "\u{a9}nam");
    assert_eq!(items[4].ident.to_string(), "----:com.apple.iTunes:ASIN");
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

    let names: Vec<String> = tag.items().map(|item| item.ident.to_string()).collect();
    assert_eq!(names, ["\u{a9}nam", "----:com.apple.iTunes:ASIN", "covr"]);
    assert_eq!(tag.first_fourcc_string(TITLE_ATOM), Some("After"));
    assert_eq!(tag.first_artwork(), Some([0xFF, 0xD8].as_slice()));
}

#[test]
fn parse_ilst_strict_refuses_malformed_child_sizes() {
    let title = {
        let data = encode_box(*b"data", &[0, 0, 0, 1, 0, 0, 0, 0, b'O', b'K']);
        encode_box(TITLE_ATOM, &data)
    };
    let mut malformed_cmt = Vec::new();
    malformed_cmt.extend_from_slice(&40u32.to_be_bytes());
    malformed_cmt.extend_from_slice(&COMMENT_ATOM);
    malformed_cmt.extend_from_slice(b"short");
    let asin = {
        let mut children = Vec::new();
        children.extend_from_slice(&encode_full_box(*b"mean", 0, [0; 3], b"com.apple.iTunes"));
        children.extend_from_slice(&encode_full_box(*b"name", 0, [0; 3], b"ASIN"));
        children.extend_from_slice(&encode_box(*b"data", &[0, 0, 0, 1, 0, 0, 0, 0, b'B', b'1']));
        encode_box(*b"----", &children)
    };
    let covr = {
        let data = encode_box(*b"data", &[0, 0, 0, 13, 0, 0, 0, 0, 0xFF, 0xD8]);
        encode_box(COVR_ATOM, &data)
    };
    let ilst_body = [title, malformed_cmt, asin, covr].concat();

    let err = Mp4Tag::parse_ilst_strict(&ilst_body).expect_err("malformed ilst must fail strict");
    assert!(err.to_string().contains("Malformed MP4 container"), "{err}");
}

#[test]
fn round_trip_preserves_nameless_freeform_nonzero_version_data_and_non_data_children() {
    let nameless_freeform = encode_box(
        *b"----",
        &[
            encode_full_box(*b"mean", 0, [0; 3], b"com.vendor"),
            encode_box(*b"data", &[0, 0, 0, 1, 0, 0, 0, 0, b'v', b'a', b'l']),
        ]
        .concat(),
    );
    let versioned_data = encode_box(*b"data", &[1, 0, 0, 1, 0, 0, 0, 0, b'v', b'1']);
    let extra_child = encode_box(*b"meta", b"custom-sibling");
    let mixed_item = encode_box(*b"cust", &[versioned_data, extra_child].concat());
    let title_item = encode_box(
        TITLE_ATOM,
        &encode_box(*b"data", &[0, 0, 0, 1, 0, 0, 0, 0, b'A']),
    );
    let ilst_body = [title_item, nameless_freeform.clone(), mixed_item.clone()].concat();

    let mut tag = Mp4Tag::parse_ilst_strict(&ilst_body).expect("strict parse");
    tag.set_fourcc_utf8(TITLE_ATOM, "Edited");
    let rebuilt = tag.encode_ilst();
    let ilst_slices = parse_child_slices_strict(&rebuilt).expect("wrapper");
    let children = parse_child_slices_strict(ilst_slices[0].content(&rebuilt)).expect("children");
    assert_eq!(children.len(), 3);
    let rebuilt_body = ilst_slices[0].content(&rebuilt);
    assert_eq!(
        &rebuilt_body[children[1].start..children[1].end],
        nameless_freeform.as_slice()
    );
    assert_eq!(
        &rebuilt_body[children[2].start..children[2].end],
        mixed_item.as_slice()
    );
}
