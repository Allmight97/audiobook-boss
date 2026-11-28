use super::super::boxes::encode_box;
use super::*;
use crate::metadata::cover_art::detect_cover_art_format;
use crate::metadata::cover_art::format::CoverFormat;
use std::io::Write;
use tempfile::NamedTempFile;

const JPEG_HEADER: [u8; 4] = [0xFF, 0xD8, 0xFF, 0xD9];

fn apply_real_plan(metadata: &AudiobookMetadata) -> Mp4Tag {
    let mut tag = Mp4Tag::default();
    let plan = MetadataWritePlan::from_metadata(metadata);
    apply_write_plan(&mut tag, &plan).expect("apply metadata write plan");
    tag
}

fn build_data_atom(image: &[u8], datatype: u32) -> Vec<u8> {
    let mut content = vec![
        0,
        ((datatype >> 16) & 0xFF) as u8,
        ((datatype >> 8) & 0xFF) as u8,
        (datatype & 0xFF) as u8,
        0,
        0,
        0,
        0,
    ];
    content.extend_from_slice(image);
    encode_box(DATA_ATOM, &content)
}

fn build_mp4_with_covr_data_atoms(data_atoms: &[Vec<u8>], quicktime_meta: bool) -> Vec<u8> {
    let mut covr_content = Vec::new();
    for data in data_atoms {
        covr_content.extend_from_slice(data);
    }
    let covr = encode_box(COVR_ATOM, &covr_content);
    let ilst = encode_box(*b"ilst", &covr);
    let mut meta_content = if quicktime_meta {
        Vec::new()
    } else {
        vec![0, 0, 0, 0]
    };
    meta_content.extend_from_slice(&ilst);
    let meta = encode_box(*b"meta", &meta_content);
    let udta = encode_box(*b"udta", &meta);
    let moov = encode_box(MOOV_ATOM, &udta);
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0isommp41");
    [ftyp, moov].concat()
}

fn write_bytes(bytes: &[u8]) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("temp mp4");
    file.write_all(bytes).expect("write temp mp4");
    file
}

fn write_fixture(image: &[u8]) -> NamedTempFile {
    write_bytes(&build_mp4_with_covr_data_atoms(
        &[build_data_atom(image, JPEG_TYPE)],
        false,
    ))
}

fn nest_in_udta(mut content: Vec<u8>, levels: usize) -> Vec<u8> {
    for _ in 0..levels {
        content = encode_box(*b"udta", &content);
    }
    content
}

#[test]
fn mp4_plan_writes_string_fields_with_album_artist_fanout() {
    let tag = apply_real_plan(&AudiobookMetadata {
        title: Some("The Way of Kings".to_string()),
        artist: Some("Brandon Sanderson".to_string()),
        album: Some("The Stormlight Archive".to_string()),
        composer: Some("Michael Kramer".to_string()),
        genre: Some("Fantasy".to_string()),
        date: Some("2010".to_string()),
        comment: Some("Unabridged".to_string()),
        description: Some("Book one".to_string()),
        ..Default::default()
    });

    assert_eq!(
        tag.first_fourcc_string(TITLE_ATOM),
        Some("The Way of Kings")
    );
    assert_eq!(
        tag.first_fourcc_string(ARTIST_ATOM),
        Some("Brandon Sanderson")
    );
    assert_eq!(
        tag.first_fourcc_string(ALBUM_ARTIST_ATOM),
        Some("Brandon Sanderson")
    );
    assert_eq!(
        tag.first_fourcc_string(ALBUM_ATOM),
        Some("The Stormlight Archive")
    );
    assert_eq!(
        tag.first_fourcc_string(COMPOSER_ATOM),
        Some("Michael Kramer")
    );
    assert_eq!(tag.first_fourcc_string(GENRE_ATOM), Some("Fantasy"));
    assert_eq!(tag.first_fourcc_string(YEAR_ATOM), Some("2010"));
    assert_eq!(tag.first_fourcc_string(COMMENT_ATOM), Some("Unabridged"));
    assert_eq!(tag.first_fourcc_string(DESCRIPTION_ATOM), Some("Book one"));
}

#[test]
fn mp4_plan_writes_series_and_subseries_to_canonical_and_itunes_freeform_atoms() {
    let tag = apply_real_plan(&AudiobookMetadata {
        series: Some("Primary".to_string()),
        series_part: Some("1".to_string()),
        subseries: Some("Sub".to_string()),
        subseries_part: Some("2".to_string()),
        ..Default::default()
    });

    let canonical_series = AtomIdent::freeform("com.apple.iTunes", "series");
    let canonical_part = AtomIdent::freeform("com.apple.iTunes", "series-part");
    let itunes_series = AtomIdent::freeform("com.apple.iTunes", "SERIES");
    let itunes_part = AtomIdent::freeform("com.apple.iTunes", "SERIES-PART");
    assert_eq!(
        tag.strings_of(&canonical_series).next(),
        Some("Primary; Sub")
    );
    assert_eq!(tag.strings_of(&canonical_part).next(), Some("1; 2"));
    assert_eq!(tag.strings_of(&itunes_series).next(), Some("Primary; Sub"));
    assert_eq!(tag.strings_of(&itunes_part).next(), Some("1; 2"));
    assert_eq!(tag.first_fourcc_string(TV_SHOW_NAME_ATOM), None);
    assert_eq!(tag.first_be_int(TV_EPISODE_ATOM), None);
}

#[test]
fn mp4_plan_clear_removes_string_atoms_rather_than_blanking() {
    let mut tag = apply_real_plan(&AudiobookMetadata {
        title: Some("Temp Title".to_string()),
        artist: Some("Temp Artist".to_string()),
        series: Some("Temp Series".to_string()),
        series_part: Some("9".to_string()),
        ..Default::default()
    });
    assert_eq!(tag.first_fourcc_string(TITLE_ATOM), Some("Temp Title"));
    assert_eq!(tag.first_fourcc_string(ARTIST_ATOM), Some("Temp Artist"));
    assert_eq!(
        tag.first_fourcc_string(ALBUM_ARTIST_ATOM),
        Some("Temp Artist")
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "series"))
            .next(),
        Some("Temp Series")
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "series-part"))
            .next(),
        Some("9")
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "SERIES"))
            .next(),
        Some("Temp Series")
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "SERIES-PART"))
            .next(),
        Some("9")
    );

    let clear_plan = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        title: Some("   ".to_string()),
        artist: Some(String::new()),
        series: Some(String::new()),
        series_part: Some(String::new()),
        ..Default::default()
    });
    apply_write_plan(&mut tag, &clear_plan).expect("apply clear");

    assert_eq!(tag.first_fourcc_string(TITLE_ATOM), None);
    assert_eq!(tag.first_fourcc_string(ARTIST_ATOM), None);
    assert_eq!(tag.first_fourcc_string(ALBUM_ARTIST_ATOM), None);
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "series"))
            .next(),
        None
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "series-part"))
            .next(),
        None
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "SERIES"))
            .next(),
        None
    );
    assert_eq!(
        tag.strings_of(&AtomIdent::freeform("com.apple.iTunes", "SERIES-PART"))
            .next(),
        None
    );
}

#[test]
fn mp4_plan_marks_media_type_audiobook_and_sets_positions() {
    let tag = apply_real_plan(&AudiobookMetadata {
        title: Some("Anything".to_string()),
        track: Some((9, Some(20))),
        disk: Some((2, Some(4))),
        ..Default::default()
    });
    assert_eq!(tag.media_type_code(), Some(2));
    assert_eq!(tag.track(), (Some(9), Some(20)));
    assert_eq!(tag.disc(), (Some(2), Some(4)));
}

#[test]
fn mp4_plan_refuses_position_values_over_u16_max() {
    let mut tag = Mp4Tag::default();
    let plan = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        track: Some((70_000, None)),
        ..Default::default()
    });
    let error = apply_write_plan(&mut tag, &plan).expect_err("position overflow must fail");
    assert!(error.to_string().contains("65535"), "{error}");
}

#[test]
fn reads_normal_mp4_cover_without_full_tag_load() {
    let file = write_fixture(&JPEG_HEADER);
    let cover = read_bounded_cover_art(file.path(), 10 * 1024 * 1024)
        .expect("bounded read")
        .expect("cover");
    assert_eq!(cover, JPEG_HEADER.to_vec());
    assert!(looks_like_mp4_family_file(file.path()).expect("ftyp probe"));
}

#[test]
fn reads_quicktime_meta_cover_without_version_flags_header() {
    let qt = write_bytes(&build_mp4_with_covr_data_atoms(
        &[build_data_atom(&JPEG_HEADER, JPEG_TYPE)],
        true,
    ));
    let cover = read_bounded_cover_art(qt.path(), 10 * 1024 * 1024)
        .expect("qt bounded read")
        .expect("qt cover");
    assert_eq!(cover, JPEG_HEADER.to_vec());
    assert!(detect_cover_art_format(&cover) == Some(CoverFormat::Jpeg));
}

#[test]
fn mp4_probe_accepts_padding_before_ftyp_and_legacy_moov_without_ftyp() {
    let free = encode_box(*b"free", b"padding");
    let fixture = build_mp4_with_covr_data_atoms(&[], false);
    let file = write_bytes(&[free, fixture].concat());
    assert!(looks_like_mp4_family_file(file.path()).expect("bounded top-level probe"));

    let moov = encode_box(MOOV_ATOM, b"");
    let legacy = write_bytes(&moov);
    assert!(looks_like_mp4_family_file(legacy.path()).expect("legacy moov probe"));
}

#[test]
fn rejects_oversized_covr_before_allocating_payload() {
    let file = write_fixture(&vec![0xFF; (10 * 1024 * 1024) + 1]);
    let error = read_bounded_cover_art(file.path(), 10 * 1024 * 1024).expect_err("oversized");
    assert!(error.to_string().contains("thumbnail input limit"));
}

#[test]
fn skips_non_image_data_sibling_and_returns_later_jpeg() {
    let reserved = build_data_atom(b"not-an-image", 0);
    let jpeg = build_data_atom(&JPEG_HEADER, JPEG_TYPE);
    let file = write_bytes(&build_mp4_with_covr_data_atoms(&[reserved, jpeg], false));
    let cover = read_bounded_cover_art(file.path(), 10 * 1024 * 1024)
        .expect("bounded read")
        .expect("later jpeg");
    assert_eq!(cover, JPEG_HEADER.to_vec());
}

#[test]
fn public_thumbnail_path_rejects_oversized_mp4_covr_without_ffmpeg_open() {
    let file = write_fixture(&vec![0xFF; (10 * 1024 * 1024) + 1]);
    let error = crate::metadata::read_audio_cover_thumbnail(file.path()).expect_err("oversized");
    assert!(error.to_string().contains("thumbnail input limit"));
}

#[test]
fn rejects_excessive_container_depth_before_stack_exhaustion() {
    let nested = nest_in_udta(encode_box(*b"free", b"payload"), MAX_CONTAINER_DEPTH + 1);
    let moov = encode_box(MOOV_ATOM, &nested);
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0isommp41");
    let file = write_bytes(&[ftyp, moov].concat());
    let error =
        read_bounded_cover_art(file.path(), 10 * 1024 * 1024).expect_err("deep container tree");
    assert!(error.to_string().contains("container depth limit"));
}

#[test]
fn rejects_excessive_atom_traversal() {
    let mut moov_content = Vec::new();
    for _ in 0..MAX_SCANNED_ATOMS {
        moov_content.extend_from_slice(&encode_box(*b"free", b""));
    }
    let moov = encode_box(MOOV_ATOM, &moov_content);
    let ftyp = encode_box(*b"ftyp", b"isom\0\0\0\0isommp41");
    let file = write_bytes(&[ftyp, moov].concat());
    let error = read_bounded_cover_art(file.path(), 10 * 1024 * 1024).expect_err("too many atoms");
    assert!(error.to_string().contains("atom traversal limit"));
}

#[test]
fn covr_children_share_the_atom_traversal_budget() {
    let data_atoms = (0..MAX_SCANNED_ATOMS)
        .map(|_| build_data_atom(b"not-an-image", 0))
        .collect::<Vec<_>>();
    let file = write_bytes(&build_mp4_with_covr_data_atoms(&data_atoms, false));
    let error =
        read_bounded_cover_art(file.path(), 10 * 1024 * 1024).expect_err("too many covr children");
    assert!(error.to_string().contains("atom traversal limit"));
}

#[test]
fn reads_quicktime_meta_cover_when_first_child_exceeds_64_byte_probe() {
    let mut image = vec![0xAA; 256];
    image[..3].copy_from_slice(&[0xFF, 0xD8, 0xFF]);
    image[254..].copy_from_slice(&[0xFF, 0xD9]);
    let qt = write_bytes(&build_mp4_with_covr_data_atoms(
        &[build_data_atom(&image, JPEG_TYPE)],
        true,
    ));
    let cover = read_bounded_cover_art(qt.path(), 10 * 1024 * 1024)
        .expect("qt bounded read")
        .expect("qt cover over 64 bytes");
    assert_eq!(cover, image);
}
