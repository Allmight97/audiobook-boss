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

/// Existing-dictionary keys a merge drops before this metadata's values are
/// written: every alias of a field the planner clears, and every user-comment
/// alias when the comment is set or cleared.
pub(crate) struct ExistingKeyRemoval {
    cleared: Vec<TagField>,
    comment_touched: bool,
}

impl ExistingKeyRemoval {
    pub(crate) fn for_metadata(metadata: &AudiobookMetadata) -> Self {
        let cleared = crate::metadata::metadata_ops::plan_metadata_field_ops(metadata)
            .into_iter()
            .filter_map(|op| match op {
                MetadataOp::Clear(field) => Some(field),
                _ => None,
            })
            .collect();
        Self {
            cleared,
            comment_touched: metadata.comment.is_some(),
        }
    }

    pub(crate) fn removes(&self, key: &str) -> bool {
        (self.comment_touched && super::field_schema::comment_key_rank(key).is_some())
            || self
                .cleared
                .iter()
                .any(|field| field.clear_keys().contains(&key))
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
