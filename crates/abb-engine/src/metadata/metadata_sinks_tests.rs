use super::{apply_metadata_ops, Mp4ametaSink};
use crate::metadata::metadata_ops::plan_metadata_field_ops;
use crate::metadata::AudiobookMetadata;
use mp4ameta::{FreeformIdent, MediaType, Tag};

fn apply_real_sink(metadata: &AudiobookMetadata) -> Tag {
    let mut tag = Tag::default();
    let ops = plan_metadata_field_ops(metadata);
    {
        let mut sink = Mp4ametaSink::new(&mut tag);
        apply_metadata_ops(&mut sink, &ops);
    }
    tag
}

#[test]
fn mp4ameta_sink_writes_string_fields_with_album_artist_fanout() {
    let tag = apply_real_sink(&AudiobookMetadata {
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

    assert_eq!(tag.title(), Some("The Way of Kings"));
    assert_eq!(tag.artist(), Some("Brandon Sanderson"));
    // Artist fans out to album-artist so ABS/Plex group by author.
    assert_eq!(tag.album_artist(), Some("Brandon Sanderson"));
    assert_eq!(tag.album(), Some("The Stormlight Archive"));
    assert_eq!(tag.composer(), Some("Michael Kramer"));
    assert_eq!(tag.genre(), Some("Fantasy"));
    assert_eq!(tag.year(), Some("2010"));
    assert_eq!(tag.comment(), Some("Unabridged"));
    assert_eq!(tag.description(), Some("Book one"));
}

#[test]
fn mp4ameta_sink_writes_series_and_subseries_to_canonical_and_itunes_freeform_atoms() {
    let tag = apply_real_sink(&AudiobookMetadata {
        series: Some("Primary".to_string()),
        series_part: Some("1".to_string()),
        subseries: Some("Sub".to_string()),
        subseries_part: Some("2".to_string()),
        ..Default::default()
    });

    let canonical_series = FreeformIdent::new_static("com.apple.iTunes", "series");
    let canonical_series_part = FreeformIdent::new_static("com.apple.iTunes", "series-part");
    let itunes_series = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let itunes_series_part = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");
    assert_eq!(
        tag.strings_of(&canonical_series).next(),
        Some("Primary; Sub")
    );
    assert_eq!(tag.strings_of(&canonical_series_part).next(), Some("1; 2"));
    assert_eq!(tag.strings_of(&itunes_series).next(), Some("Primary; Sub"));
    assert_eq!(tag.strings_of(&itunes_series_part).next(), Some("1; 2"));

    assert_eq!(tag.tv_show_name(), None);
    assert_eq!(tag.tv_episode(), None);
}

#[test]
fn mp4ameta_sink_clear_removes_string_atoms_rather_than_blanking() {
    let mut tag = apply_real_sink(&AudiobookMetadata {
        title: Some("Temp Title".to_string()),
        artist: Some("Temp Artist".to_string()),
        series: Some("Temp Series".to_string()),
        series_part: Some("9".to_string()),
        ..Default::default()
    });
    assert_eq!(tag.title(), Some("Temp Title"));
    assert_eq!(tag.album_artist(), Some("Temp Artist"));
    let canonical_series = FreeformIdent::new_static("com.apple.iTunes", "series");
    let canonical_series_part = FreeformIdent::new_static("com.apple.iTunes", "series-part");
    let itunes_series = FreeformIdent::new_static("com.apple.iTunes", "SERIES");
    let itunes_series_part = FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART");
    assert_eq!(
        tag.strings_of(&canonical_series).next(),
        Some("Temp Series")
    );
    assert_eq!(tag.strings_of(&canonical_series_part).next(), Some("9"));
    assert_eq!(tag.strings_of(&itunes_series).next(), Some("Temp Series"));
    assert_eq!(tag.strings_of(&itunes_series_part).next(), Some("9"));

    let clear_ops = plan_metadata_field_ops(&AudiobookMetadata {
        title: Some("   ".to_string()),
        artist: Some(String::new()),
        series: Some(String::new()),
        series_part: Some(String::new()),
        ..Default::default()
    });
    {
        let mut sink = Mp4ametaSink::new(&mut tag);
        apply_metadata_ops(&mut sink, &clear_ops);
    }

    assert_eq!(tag.title(), None);
    assert_eq!(tag.artist(), None);
    assert_eq!(tag.album_artist(), None);
    assert_eq!(tag.strings_of(&canonical_series).next(), None);
    assert_eq!(tag.strings_of(&canonical_series_part).next(), None);
    assert_eq!(tag.strings_of(&itunes_series).next(), None);
    assert_eq!(tag.strings_of(&itunes_series_part).next(), None);
}

#[test]
fn mp4ameta_sink_marks_media_type_audiobook() {
    let tag = apply_real_sink(&AudiobookMetadata {
        title: Some("Anything".to_string()),
        ..Default::default()
    });

    assert_eq!(tag.media_type(), Some(MediaType::AudioBook));
}

#[test]
fn mp4ameta_sink_sets_and_reads_back_track_and_disk_in_memory() {
    let mut tag = Tag::default();
    let metadata = AudiobookMetadata {
        track: Some((9, Some(20))),
        disk: Some((2, Some(4))),
        ..Default::default()
    };
    let ops = plan_metadata_field_ops(&metadata);

    {
        let mut sink = Mp4ametaSink::new(&mut tag);
        apply_metadata_ops(&mut sink, &ops);
    }

    assert_eq!(tag.track(), (Some(9), Some(20)));
    assert_eq!(tag.disc(), (Some(2), Some(4)));
}
