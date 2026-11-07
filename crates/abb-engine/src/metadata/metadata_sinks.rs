use crate::metadata::field_schema::{position_keys, text_keys, KeySet, ITUNES_MEAN};
use crate::metadata::metadata_ops::MetadataOp;
use crate::metadata::publication_year_from_date;
use crate::metadata::AudiobookMetadata;
use abb_metadata_core::{PositionField, TagField, TextField};
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
    fn set_text(&mut self, field: TextField, value: &str) {
        match field {
            TextField::Title => self.tag.set_title(value),
            TextField::Artist => {
                self.tag.set_artist(value);
                self.tag.set_album_artist(value);
            }
            TextField::Album => self.tag.set_album(value),
            TextField::Composer => self.tag.set_composer(value),
            TextField::Genre => self.tag.set_genre(value),
            TextField::Date => self.tag.set_year(value.to_string()),
            TextField::Comment => self.tag.set_comment(value),
            TextField::Description => self.tag.set_description(value),
            TextField::Series | TextField::SeriesPart => {
                self.remove_series_atoms(field);
                for name in text_keys(field).mp4_freeform {
                    self.tag.set_data(
                        FreeformIdent::new_static(ITUNES_MEAN, name),
                        Data::Utf8(value.to_string()),
                    );
                }
            }
        }
    }

    fn clear_text(&mut self, field: TextField) {
        match field {
            TextField::Title => self.tag.remove_title(),
            TextField::Artist => {
                self.tag.remove_artists();
                self.tag.remove_album_artists();
            }
            TextField::Album => self.tag.remove_album(),
            TextField::Composer => self.tag.remove_composers(),
            TextField::Genre => self.tag.remove_genres(),
            TextField::Date => self.tag.remove_year(),
            TextField::Comment => self.tag.remove_comments(),
            TextField::Description => self.tag.remove_descriptions(),
            TextField::Series | TextField::SeriesPart => self.remove_series_atoms(field),
        }
    }

    fn remove_series_atoms(&mut self, field: TextField) {
        for name in text_keys(field).mp4_freeform {
            self.tag
                .remove_data_of(&FreeformIdent::new_static(ITUNES_MEAN, name));
        }
        if field == TextField::Series {
            self.tag.remove_tv_show_name();
            self.tag.remove_tv_show_name_sort_order();
        } else {
            self.tag.remove_tv_episode();
            self.tag.remove_tv_episode_name();
        }
    }

    fn set_position(&mut self, field: PositionField, number: u32, total: Option<u32>) {
        let total = total.unwrap_or(0);
        if number <= u16::MAX as u32 && total <= u16::MAX as u32 {
            match field {
                PositionField::Track => self.tag.set_track(number as u16, total as u16),
                PositionField::Disk => self.tag.set_disc(number as u16, total as u16),
            }
        }
    }

    fn clear_position(&mut self, field: PositionField) {
        match field {
            PositionField::Track => self.tag.remove_track(),
            PositionField::Disk => self.tag.remove_disc(),
        }
    }

    pub fn set_media_type_audiobook(&mut self) {
        self.tag.set_media_type(MediaType::AudioBook);
    }
}

pub fn apply_metadata_ops(sink: &mut Mp4ametaSink<'_>, ops: &[MetadataOp]) {
    for op in ops {
        match op {
            MetadataOp::SetText { field, value } => sink.set_text(*field, value),
            MetadataOp::Clear(TagField::Text(field)) => sink.clear_text(*field),
            MetadataOp::Clear(TagField::Position(field)) => sink.clear_position(*field),
            MetadataOp::SetPosition {
                field,
                number,
                total,
            } => sink.set_position(*field, *number, *total),
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
            MetadataOp::SetText { field, value } => {
                for key in text_keys(*field).write {
                    dict.set(key, value);
                }
                if *field == TextField::Date {
                    if let Some(year) = publication_year_from_date(Some(value)) {
                        dict.set("year", &year.to_string());
                    }
                }
            }
            MetadataOp::Clear(_) => {}
            MetadataOp::SetPosition {
                field,
                number,
                total,
            } => {
                dict.set(
                    position_keys(*field).write,
                    &format_position_field(*number, *total),
                );
            }
            MetadataOp::SetMediaTypeAudiobook => {
                dict.set("media_type", "2");
            }
        }
    }
    Ok(())
}

/// Existing-dictionary keys a merge drops before this metadata's values are
/// written, from each planned op's row in the field table: a Set drops what
/// its field's `drop_on_set` names, a Clear what `drop_on_clear` names.
pub(crate) struct ExistingKeyRemoval {
    dropped: Vec<KeySet>,
}

impl ExistingKeyRemoval {
    pub(crate) fn for_metadata(metadata: &AudiobookMetadata) -> Self {
        let dropped = crate::metadata::metadata_ops::plan_metadata_field_ops(metadata)
            .into_iter()
            .filter_map(|op| match op {
                MetadataOp::SetText { field, .. } => Some(text_keys(field).drop_on_set),
                MetadataOp::Clear(TagField::Text(field)) => Some(text_keys(field).drop_on_clear),
                MetadataOp::Clear(TagField::Position(field)) => {
                    Some(position_keys(field).drop_on_clear)
                }
                MetadataOp::SetPosition { .. } | MetadataOp::SetMediaTypeAudiobook => None,
            })
            .collect();
        Self { dropped }
    }

    pub(crate) fn removes(&self, key: &str) -> bool {
        self.dropped.iter().any(|set| set.contains(key))
    }
}

fn format_position_field(number: u32, total: Option<u32>) -> String {
    match total {
        Some(total) => format!("{number}/{total}"),
        None => number.to_string(),
    }
}

#[cfg(test)]
#[path = "metadata_sinks_tests.rs"]
mod tests;
