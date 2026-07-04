//! Metadata field sinks for ffmpeg and mp4ameta adapters.

use crate::metadata::field_schema::TagField;
use crate::metadata::metadata_ops::MetadataOp;
use crate::metadata::publication_year_from_date;
use crate::metadata::tag_registry::{
    ITUNES_MEAN, SERIES, SERIES_FREEFORM_NAME, SERIES_PART, SERIES_PART_FREEFORM_NAME,
};
use crate::metadata::AudiobookMetadata;
use ffmpeg_next as ff;
use mp4ameta::{Data, FreeformIdent, MediaType, Tag};

pub struct Mp4ametaSink<'a> {
    tag: &'a mut Tag,
}

impl<'a> Mp4ametaSink<'a> {
    pub fn new(tag: &'a mut Tag) -> Self {
        Self { tag }
    }
}

impl Mp4ametaSink<'_> {
    fn set_string(&mut self, field: TagField, value: &str) {
        match field {
            TagField::Title => self.tag.set_title(value),
            TagField::Artist => {
                self.tag.set_artist(value);
                self.tag.set_album_artist(value);
            }
            TagField::Album => self.tag.set_album(value),
            TagField::Composer => self.tag.set_composer(value),
            TagField::Genre => self.tag.set_genre(value),
            TagField::Date => self.tag.set_year(value.to_string()),
            TagField::Comment => self.tag.set_comment(value),
            TagField::Description => self.tag.set_description(value),
            TagField::Series => {
                let canonical_ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES);
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_FREEFORM_NAME);
                self.tag.remove_data_of(&canonical_ident);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_show_name();
                self.tag.remove_tv_show_name_sort_order();
                self.tag
                    .set_data(canonical_ident, Data::Utf8(value.to_string()));
                self.tag.set_data(ident, Data::Utf8(value.to_string()));
            }
            TagField::SeriesPart => {
                let canonical_ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART);
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART_FREEFORM_NAME);
                self.tag.remove_data_of(&canonical_ident);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_episode();
                self.tag.remove_tv_episode_name();
                self.tag
                    .set_data(canonical_ident, Data::Utf8(value.to_string()));
                self.tag.set_data(ident, Data::Utf8(value.to_string()));
            }
            TagField::Track | TagField::Disk => {}
        }
    }

    fn clear_field(&mut self, field: TagField) {
        match field {
            TagField::Title => self.tag.remove_title(),
            TagField::Artist => {
                self.tag.remove_artists();
                self.tag.remove_album_artists();
            }
            TagField::Album => self.tag.remove_album(),
            TagField::Composer => self.tag.remove_composers(),
            TagField::Genre => self.tag.remove_genres(),
            TagField::Date => self.tag.remove_year(),
            TagField::Comment => self.tag.remove_comments(),
            TagField::Description => self.tag.remove_descriptions(),
            TagField::Series => {
                let canonical_ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES);
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_FREEFORM_NAME);
                self.tag.remove_data_of(&canonical_ident);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_show_name();
                self.tag.remove_tv_show_name_sort_order();
            }
            TagField::SeriesPart => {
                let canonical_ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART);
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART_FREEFORM_NAME);
                self.tag.remove_data_of(&canonical_ident);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_episode();
                self.tag.remove_tv_episode_name();
            }
            TagField::Track => self.tag.remove_track(),
            TagField::Disk => self.tag.remove_disc(),
        }
    }

    fn set_track(&mut self, number: u32, total: Option<u32>) {
        let total = total.unwrap_or(0);
        if number <= u16::MAX as u32 && total <= u16::MAX as u32 {
            self.tag.set_track(number as u16, total as u16);
        }
    }

    fn set_disk(&mut self, number: u32, total: Option<u32>) {
        let total = total.unwrap_or(0);
        if number <= u16::MAX as u32 && total <= u16::MAX as u32 {
            self.tag.set_disc(number as u16, total as u16);
        }
    }

    pub fn set_media_type_audiobook(&mut self) {
        self.tag.set_media_type(MediaType::AudioBook);
    }
}

pub fn apply_metadata_ops(sink: &mut Mp4ametaSink<'_>, ops: &[MetadataOp]) {
    for op in ops {
        match op {
            MetadataOp::SetString { field, value } => sink.set_string(*field, value),
            MetadataOp::Clear(field) => sink.clear_field(*field),
            MetadataOp::SetTrack { number, total } => sink.set_track(*number, *total),
            MetadataOp::SetDisk { number, total } => sink.set_disk(*number, *total),
            MetadataOp::SetMediaTypeAudiobook => sink.set_media_type_audiobook(),
        }
    }
}

pub fn apply_metadata_field_ops_to_ffmpeg_dict(
    dict: &mut ff::Dictionary<'_>,
    metadata: &AudiobookMetadata,
) -> crate::errors::Result<()> {
    let ops = crate::metadata::metadata_ops::plan_metadata_field_ops(metadata);
    apply_ops_to_ffmpeg_dict(dict, &ops)?;
    Ok(())
}

pub fn apply_ops_to_ffmpeg_dict(
    dict: &mut ff::Dictionary<'_>,
    ops: &[MetadataOp],
) -> crate::errors::Result<()> {
    for op in ops {
        match op {
            MetadataOp::SetString { field, value } => {
                for key in field.ffmpeg_write_keys() {
                    dict.set(key, value);
                }
                if *field == TagField::Date {
                    if let Some(year) = publication_year_from_date(Some(value)) {
                        dict.set("year", &year.to_string());
                    }
                }
            }
            MetadataOp::Clear(_) => {}
            MetadataOp::SetTrack { number, total } => {
                dict.set("track", &format_position_field(*number, *total));
            }
            MetadataOp::SetDisk { number, total } => {
                dict.set("disc", &format_position_field(*number, *total));
            }
            MetadataOp::SetMediaTypeAudiobook => {
                dict.set("media_type", "2");
            }
        }
    }
    Ok(())
}

pub fn should_remove_key_for_metadata(metadata: &AudiobookMetadata, key: &str) -> bool {
    TagField::ALL
        .iter()
        .copied()
        .any(|field| field_has_clear_intent(metadata, field) && field.clear_keys().contains(&key))
}

fn field_has_clear_intent(metadata: &AudiobookMetadata, field: TagField) -> bool {
    crate::metadata::metadata_ops::field_has_clear_intent(metadata, field)
}

fn format_position_field(number: u32, total: Option<u32>) -> String {
    match total {
        Some(total) => format!("{number}/{total}"),
        None => number.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_metadata_ops, Mp4ametaSink};
    use crate::metadata::metadata_ops::plan_metadata_field_ops;
    use crate::metadata::AudiobookMetadata;
    use mp4ameta::{FreeformIdent, MediaType, Tag};

    // These tests assert the literal external contract a third-party reader
    // consumes (atom names/values), not the production constants the sink
    // writes — so the test cannot drift in lockstep with a bug in the code it
    // guards. Everything is driven through the real planner + real Mp4ametaSink
    // into an in-memory Tag (no file/ffmpeg I/O; honors the #341 freeze).

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

        // Legacy tv-show/tv-episode compatibility atoms must not be emitted.
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

        // Empty-string intent flows through the planner as a Clear op.
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

        // The atoms are removed entirely, not left as empty values.
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
}
