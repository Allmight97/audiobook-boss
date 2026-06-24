//! Metadata field sinks for ffmpeg and mp4ameta adapters.

use crate::metadata::field_schema::TagField;
use crate::metadata::metadata_ops::MetadataOp;
use crate::metadata::publication_year_from_date;
use crate::metadata::tag_registry::{ITUNES_MEAN, SERIES_FREEFORM_NAME, SERIES_PART_FREEFORM_NAME};
use crate::metadata::AudiobookMetadata;
use ffmpeg_next as ff;
use mp4ameta::{Data, FreeformIdent, MediaType, Tag};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub enum MetadataDialect {
    Ffmpeg,
    Mp4ameta,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub enum RecordedOp {
    SetKey {
        key: String,
        value: String,
    },
    ClearKey {
        key: String,
    },
    SetTrack {
        number: u32,
        total: Option<u32>,
    },
    SetDisk {
        number: u32,
        total: Option<u32>,
    },
    SetMediaTypeAudiobook,
    Mp4RemoveSeriesCompat,
    Mp4SetFreeform {
        mean: String,
        name: String,
        value: String,
    },
}

pub trait MetadataFieldSink {
    fn set_string(&mut self, field: TagField, value: &str);
    fn clear_field(&mut self, field: TagField);
    fn set_track(&mut self, number: u32, total: Option<u32>);
    fn set_disk(&mut self, number: u32, total: Option<u32>);
    fn set_media_type_audiobook(&mut self);
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(test)]
pub struct RecordingSink {
    pub ops: Vec<RecordedOp>,
    dialect: MetadataDialect,
}

#[cfg(test)]
impl RecordingSink {
    pub fn new(dialect: MetadataDialect) -> Self {
        Self {
            ops: Vec::new(),
            dialect,
        }
    }
}

#[cfg(test)]
impl MetadataFieldSink for RecordingSink {
    fn set_string(&mut self, field: TagField, value: &str) {
        match self.dialect {
            MetadataDialect::Ffmpeg => {
                for key in field.ffmpeg_write_keys() {
                    self.ops.push(RecordedOp::SetKey {
                        key: (*key).to_string(),
                        value: value.to_string(),
                    });
                }
                if field == TagField::Date {
                    if let Some(year) = publication_year_from_date(Some(value)) {
                        self.ops.push(RecordedOp::SetKey {
                            key: "year".to_string(),
                            value: year.to_string(),
                        });
                    }
                }
            }
            MetadataDialect::Mp4ameta => match field {
                TagField::Series => {
                    self.ops.push(RecordedOp::Mp4RemoveSeriesCompat);
                    self.ops.push(RecordedOp::Mp4SetFreeform {
                        mean: ITUNES_MEAN.to_string(),
                        name: SERIES_FREEFORM_NAME.to_string(),
                        value: value.to_string(),
                    });
                }
                TagField::SeriesPart => {
                    self.ops.push(RecordedOp::Mp4SetFreeform {
                        mean: ITUNES_MEAN.to_string(),
                        name: SERIES_PART_FREEFORM_NAME.to_string(),
                        value: value.to_string(),
                    });
                }
                TagField::Date => self.ops.push(RecordedOp::SetKey {
                    key: "year".to_string(),
                    value: value.to_string(),
                }),
                _ => {
                    if let Some(key) = mp4_string_field_key(field) {
                        self.ops.push(RecordedOp::SetKey {
                            key: key.to_string(),
                            value: value.to_string(),
                        });
                    }
                }
            },
        }
    }

    fn clear_field(&mut self, field: TagField) {
        match self.dialect {
            MetadataDialect::Ffmpeg => {
                for key in field.clear_keys() {
                    self.ops.push(RecordedOp::ClearKey {
                        key: (*key).to_string(),
                    });
                }
            }
            MetadataDialect::Mp4ameta => match field {
                TagField::Series | TagField::SeriesPart => {
                    self.ops.push(RecordedOp::Mp4RemoveSeriesCompat);
                }
                TagField::Date => self.ops.push(RecordedOp::ClearKey {
                    key: "year".to_string(),
                }),
                _ => {
                    if let Some(key) = mp4_string_field_key(field) {
                        self.ops.push(RecordedOp::ClearKey {
                            key: key.to_string(),
                        });
                    }
                }
            },
        }
    }

    fn set_track(&mut self, number: u32, total: Option<u32>) {
        self.ops.push(RecordedOp::SetTrack { number, total });
    }

    fn set_disk(&mut self, number: u32, total: Option<u32>) {
        self.ops.push(RecordedOp::SetDisk { number, total });
    }

    fn set_media_type_audiobook(&mut self) {
        self.ops.push(RecordedOp::SetMediaTypeAudiobook);
    }
}

pub struct Mp4ametaSink<'a> {
    tag: &'a mut Tag,
}

impl<'a> Mp4ametaSink<'a> {
    pub fn new(tag: &'a mut Tag) -> Self {
        Self { tag }
    }
}

impl MetadataFieldSink for Mp4ametaSink<'_> {
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
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_FREEFORM_NAME);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_show_name();
                self.tag.remove_tv_show_name_sort_order();
                self.tag.set_data(ident, Data::Utf8(value.to_string()));
            }
            TagField::SeriesPart => {
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART_FREEFORM_NAME);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_episode();
                self.tag.remove_tv_episode_name();
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
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_FREEFORM_NAME);
                self.tag.remove_data_of(&ident);
                self.tag.remove_tv_show_name();
                self.tag.remove_tv_show_name_sort_order();
            }
            TagField::SeriesPart => {
                let ident = FreeformIdent::new_static(ITUNES_MEAN, SERIES_PART_FREEFORM_NAME);
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

    fn set_media_type_audiobook(&mut self) {
        self.tag.set_media_type(MediaType::AudioBook);
    }
}

pub fn apply_metadata_ops<S: MetadataFieldSink>(sink: &mut S, ops: &[MetadataOp]) {
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
fn mp4_string_field_key(field: TagField) -> Option<&'static str> {
    match field {
        TagField::Title => Some("title"),
        TagField::Artist => Some("artist"),
        TagField::Album => Some("album"),
        TagField::Composer => Some("composer"),
        TagField::Genre => Some("genre"),
        TagField::Comment => Some("comment"),
        TagField::Description => Some("description"),
        TagField::Date
        | TagField::Series
        | TagField::SeriesPart
        | TagField::Track
        | TagField::Disk => None,
    }
}

#[cfg(test)]
pub fn normalize_recorded_ops(ops: &[RecordedOp]) -> Vec<NormalizedRecordedOp> {
    ops.iter().map(NormalizedRecordedOp::from).collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg(test)]
pub enum NormalizedRecordedOp {
    Title(String),
    Artist(String),
    Album(String),
    Composer(String),
    Genre(String),
    Date(String),
    Comment(String),
    Description(String),
    Series(String),
    SeriesPart(String),
    Track { number: u32, total: Option<u32> },
    Disk { number: u32, total: Option<u32> },
    MediaTypeAudiobook,
}

#[cfg(test)]
impl From<&RecordedOp> for NormalizedRecordedOp {
    fn from(op: &RecordedOp) -> Self {
        match op {
            RecordedOp::SetKey { key, value } => normalized_from_key(key, value),
            RecordedOp::Mp4SetFreeform { name, value, .. } => match name.as_str() {
                SERIES_FREEFORM_NAME => NormalizedRecordedOp::Series(value.clone()),
                SERIES_PART_FREEFORM_NAME => NormalizedRecordedOp::SeriesPart(value.clone()),
                _ => NormalizedRecordedOp::Comment(value.clone()),
            },
            RecordedOp::SetTrack { number, total } => NormalizedRecordedOp::Track {
                number: *number,
                total: *total,
            },
            RecordedOp::SetDisk { number, total } => NormalizedRecordedOp::Disk {
                number: *number,
                total: *total,
            },
            RecordedOp::SetMediaTypeAudiobook => NormalizedRecordedOp::MediaTypeAudiobook,
            RecordedOp::ClearKey { .. } | RecordedOp::Mp4RemoveSeriesCompat => {
                NormalizedRecordedOp::Comment(String::new())
            }
        }
    }
}

#[cfg(test)]
fn normalized_from_key(key: &str, value: &str) -> NormalizedRecordedOp {
    match key {
        "title" => NormalizedRecordedOp::Title(value.to_string()),
        "artist" | "album_artist" => NormalizedRecordedOp::Artist(value.to_string()),
        "album" => NormalizedRecordedOp::Album(value.to_string()),
        "composer" => NormalizedRecordedOp::Composer(value.to_string()),
        "genre" => NormalizedRecordedOp::Genre(value.to_string()),
        "date" | "year" => NormalizedRecordedOp::Date(value.to_string()),
        "comment" => NormalizedRecordedOp::Comment(value.to_string()),
        "description" => NormalizedRecordedOp::Description(value.to_string()),
        "series" | "----:com.apple.iTunes:SERIES" => {
            NormalizedRecordedOp::Series(value.to_string())
        }
        "series-part" | "----:com.apple.iTunes:SERIES-PART" => {
            NormalizedRecordedOp::SeriesPart(value.to_string())
        }
        "track" => parse_track_disk_normalized(value, true),
        "disc" => parse_track_disk_normalized(value, false),
        "media_type" => NormalizedRecordedOp::MediaTypeAudiobook,
        _ => NormalizedRecordedOp::Comment(value.to_string()),
    }
}

#[cfg(test)]
fn parse_track_disk_normalized(value: &str, is_track: bool) -> NormalizedRecordedOp {
    let (number, total) = if let Some((number, total)) = value.split_once('/') {
        (number.trim().parse().ok(), total.trim().parse().ok())
    } else {
        (value.trim().parse().ok(), None)
    };

    match (number, is_track) {
        (Some(number), true) => NormalizedRecordedOp::Track { number, total },
        (Some(number), false) => NormalizedRecordedOp::Disk { number, total },
        _ => NormalizedRecordedOp::Comment(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        apply_metadata_ops, normalize_recorded_ops, MetadataDialect, Mp4ametaSink,
        NormalizedRecordedOp, RecordingSink,
    };
    use crate::metadata::metadata_ops::plan_metadata_field_ops;
    use crate::metadata::AudiobookMetadata;
    use mp4ameta::Tag;

    fn normalized_semantic_ops(
        metadata: &AudiobookMetadata,
        dialect: MetadataDialect,
    ) -> Vec<NormalizedRecordedOp> {
        let ops = plan_metadata_field_ops(metadata);
        let mut sink = RecordingSink::new(dialect);
        apply_metadata_ops(&mut sink, &ops);
        let mut normalized = normalize_recorded_ops(&sink.ops);
        normalized.retain(|op| !matches!(op, NormalizedRecordedOp::Comment(_)));
        normalized.sort();
        normalized.dedup();
        normalized
    }

    #[test]
    fn dual_sink_equivalence_for_series_track_and_disk() {
        let metadata = AudiobookMetadata {
            series: Some("Stormlight".to_string()),
            series_part: Some("1".to_string()),
            track: Some((4, Some(32))),
            disk: Some((1, Some(3))),
            title: Some("The Way of Kings".to_string()),
            ..Default::default()
        };

        let ffmpeg = normalized_semantic_ops(&metadata, MetadataDialect::Ffmpeg);
        let mp4 = normalized_semantic_ops(&metadata, MetadataDialect::Mp4ameta);

        assert_eq!(ffmpeg, mp4);
        assert!(ffmpeg.contains(&NormalizedRecordedOp::Series("Stormlight".to_string())));
        assert!(ffmpeg.contains(&NormalizedRecordedOp::SeriesPart("1".to_string())));
        assert!(ffmpeg.contains(&NormalizedRecordedOp::Track {
            number: 4,
            total: Some(32),
        }));
        assert!(ffmpeg.contains(&NormalizedRecordedOp::Disk {
            number: 1,
            total: Some(3),
        }));
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
