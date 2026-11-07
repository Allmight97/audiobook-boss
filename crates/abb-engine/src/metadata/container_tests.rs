use super::Container;
use crate::audio::Muxer;

#[test]
fn names_a_container_from_the_demuxer_aliases() {
    assert_eq!(
        Container::from_format_name("mov,mp4,m4a,3gp,3g2,mj2"),
        Container::Mp4
    );
    assert_eq!(
        Container::from_format_name("matroska,webm"),
        Container::Matroska
    );
    assert_eq!(Container::from_format_name("mp3"), Container::Mp3);
}

#[test]
fn names_a_container_from_a_muxer_name() {
    assert_eq!(Container::from_format_name("mp4"), Container::Mp4);
    assert_eq!(Container::from_format_name("matroska"), Container::Matroska);
    assert_eq!(
        Container::from_format_name("webm"),
        Container::Other("webm".to_string())
    );
}

#[test]
fn maps_other_demuxer_names_to_the_muxer_that_writes_them() {
    for (demuxer, muxer) in [
        ("aac", "adts"),
        ("wav", "wav"),
        ("flac", "flac"),
        ("ogg", "ogg"),
    ] {
        assert_eq!(
            Container::from_format_name(demuxer),
            Container::Other(muxer.to_string())
        );
    }
}

#[test]
fn muxer_names_round_trip_for_the_containers_abb_writes() {
    for name in ["mp4", "matroska", "mp3", "adts", "flac"] {
        assert_eq!(Container::from_format_name(name).muxer_name(), name);
    }
    assert_eq!(Container::from(Muxer::Mp4), Container::Mp4);
    assert_eq!(Container::from(Muxer::Matroska), Container::Matroska);
}

#[test]
fn extensions_name_the_container_the_format_table_writes() {
    for (extension, container) in [
        ("m4b", Some(Container::Mp4)),
        ("m4a", Some(Container::Mp4)),
        ("mka", Some(Container::Matroska)),
        ("xyz", None),
    ] {
        assert_eq!(
            Container::from_extension(extension),
            container,
            "{extension}"
        );
    }
}

#[test]
fn only_mp4_derives_chapter_ends_and_only_mp3_renames_the_album_sort() {
    assert!(!Container::Mp4.stores_chapter_ends());
    assert!(Container::Matroska.stores_chapter_ends());
    assert!(Container::Other("flac".to_string()).stores_chapter_ends());
    assert!(Container::Mp4.is_mp4_family());
    assert!(!Container::Mp3.is_mp4_family());

    assert_eq!(Container::Mp3.album_sort_key(), "album-sort");
    for container in [
        Container::Mp4,
        Container::Matroska,
        Container::Other("flac".to_string()),
    ] {
        assert_eq!(container.album_sort_key(), "sort_album");
    }
}
