use super::container::Container;
use super::MetadataWritePlan;
use crate::errors::{AppError, Result};
use abb_metadata_core::{PositionField, TagField, TextField};

const RAW_AAC_REFUSAL: &str =
    "Raw AAC (.aac) files can't hold tags. Export the title to M4B to keep them.";

enum Edited {
    Field(TagField),
    AlbumSort,
    Cover,
}

impl Edited {
    fn label(&self) -> &'static str {
        match self {
            Edited::Field(TagField::Text(field)) => match field {
                TextField::Title => "title",
                TextField::Artist => "author",
                TextField::Album => "album",
                TextField::Composer => "narrator",
                TextField::Genre => "genre",
                TextField::Date => "date",
                TextField::Comment => "comment",
                TextField::Description => "description",
                TextField::Series => "series",
                TextField::SeriesPart => "book number",
            },
            Edited::Field(TagField::Position(PositionField::Track)) => "track number",
            Edited::Field(TagField::Position(PositionField::Disk)) => "disc number",
            Edited::AlbumSort => "title sort",
            Edited::Cover => "cover",
        }
    }

    fn wav_holds(&self) -> bool {
        matches!(
            self,
            Edited::Field(
                TagField::Text(
                    TextField::Title
                        | TextField::Artist
                        | TextField::Album
                        | TextField::Genre
                        | TextField::Date
                        | TextField::Comment
                ) | TagField::Position(PositionField::Track)
            )
        )
    }
}

fn edited(plan: &MetadataWritePlan) -> Vec<Edited> {
    let mut edited: Vec<Edited> = plan
        .ops
        .iter()
        .map(|op| Edited::Field(op.field()))
        .collect();
    if plan.album_sort.is_some() {
        edited.push(Edited::AlbumSort);
    }
    if plan.cover.is_some() {
        edited.push(Edited::Cover);
    }
    edited
}

fn join_labels(labels: &[&str]) -> String {
    match labels {
        [] => String::new(),
        [only] => (*only).to_string(),
        [rest @ .., last] => format!("{} or {last}", rest.join(", ")),
    }
}

/// Refuses a Save that edits a tag the file's container cannot store: its
/// muxer would drop the edit and the Save would still report success. Only
/// raw AAC (ADTS, no tags at all) and WAV (a short list of RIFF INFO tags)
/// drop tags; MP4, Matroska, MP3 and FLAC hold every field a plan writes.
pub(super) fn refuse_unholdable_edits(
    container: &Container,
    plan: &MetadataWritePlan,
) -> Result<()> {
    let edited = edited(plan);
    match container.muxer_name() {
        "adts" if !edited.is_empty() => Err(AppError::FileValidation(RAW_AAC_REFUSAL.to_string())),
        "wav" => {
            let lost: Vec<&str> = edited
                .iter()
                .filter(|edit| !edit.wav_holds())
                .map(Edited::label)
                .collect();
            if lost.is_empty() {
                return Ok(());
            }
            let keep = if lost.len() == 1 { "it" } else { "them" };
            Err(AppError::FileValidation(format!(
                "WAV files can't hold {}. Export the title to M4B to keep {keep}.",
                join_labels(&lost)
            )))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
#[path = "tag_support_tests.rs"]
mod tests;
