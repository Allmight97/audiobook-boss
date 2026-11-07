use crate::{AudiobookMetadata, MetadataIntentPatch, PatchOp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextField {
    Title,
    Artist,
    Album,
    Composer,
    Genre,
    Date,
    Comment,
    Description,
    Series,
    SeriesPart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PositionField {
    Track,
    Disk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagField {
    Text(TextField),
    Position(PositionField),
}

impl TextField {
    pub const ALL: [TextField; 10] = [
        TextField::Title,
        TextField::Artist,
        TextField::Album,
        TextField::Composer,
        TextField::Genre,
        TextField::Date,
        TextField::Comment,
        TextField::Description,
        TextField::Series,
        TextField::SeriesPart,
    ];
}

impl PositionField {
    pub const ALL: [PositionField; 2] = [PositionField::Track, PositionField::Disk];
}

impl TagField {
    pub const ALL: [TagField; 12] = [
        TagField::Text(TextField::Title),
        TagField::Text(TextField::Artist),
        TagField::Text(TextField::Album),
        TagField::Text(TextField::Composer),
        TagField::Text(TextField::Genre),
        TagField::Text(TextField::Date),
        TagField::Text(TextField::Comment),
        TagField::Text(TextField::Description),
        TagField::Text(TextField::Series),
        TagField::Text(TextField::SeriesPart),
        TagField::Position(PositionField::Track),
        TagField::Position(PositionField::Disk),
    ];
}

impl std::fmt::Display for TagField {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TagField::Text(field) => write!(formatter, "{field:?}"),
            TagField::Position(field) => write!(formatter, "{field:?}"),
        }
    }
}

impl AudiobookMetadata {
    /// The field's own value. `Series` and `SeriesPart` give the primary
    /// series and its number only; the tag value joins the sub-series
    /// (`build_series_list`).
    pub fn text(&self, field: TextField) -> Option<&String> {
        match field {
            TextField::Title => self.title.as_ref(),
            TextField::Artist => self.artist.as_ref(),
            TextField::Album => self.album.as_ref(),
            TextField::Composer => self.composer.as_ref(),
            TextField::Genre => self.genre.as_ref(),
            TextField::Date => self.date.as_ref(),
            TextField::Comment => self.comment.as_ref(),
            TextField::Description => self.description.as_ref(),
            TextField::Series => self.series.as_ref(),
            TextField::SeriesPart => self.series_part.as_ref(),
        }
    }

    pub fn text_mut(&mut self, field: TextField) -> &mut Option<String> {
        match field {
            TextField::Title => &mut self.title,
            TextField::Artist => &mut self.artist,
            TextField::Album => &mut self.album,
            TextField::Composer => &mut self.composer,
            TextField::Genre => &mut self.genre,
            TextField::Date => &mut self.date,
            TextField::Comment => &mut self.comment,
            TextField::Description => &mut self.description,
            TextField::Series => &mut self.series,
            TextField::SeriesPart => &mut self.series_part,
        }
    }

    pub fn position(&self, field: PositionField) -> Option<(u32, Option<u32>)> {
        match field {
            PositionField::Track => self.track,
            PositionField::Disk => self.disk,
        }
    }

    pub fn position_mut(&mut self, field: PositionField) -> &mut Option<(u32, Option<u32>)> {
        match field {
            PositionField::Track => &mut self.track,
            PositionField::Disk => &mut self.disk,
        }
    }
}

impl MetadataIntentPatch {
    pub fn text(&self, field: TextField) -> Option<&PatchOp<String>> {
        match field {
            TextField::Title => self.title.as_ref(),
            TextField::Artist => self.artist.as_ref(),
            TextField::Album => self.album.as_ref(),
            TextField::Composer => self.composer.as_ref(),
            TextField::Genre => self.genre.as_ref(),
            TextField::Date => self.date.as_ref(),
            TextField::Comment => self.comment.as_ref(),
            TextField::Description => self.description.as_ref(),
            TextField::Series => self.series.as_ref(),
            TextField::SeriesPart => self.series_part.as_ref(),
        }
    }

    pub fn position(&self, field: PositionField) -> Option<&PatchOp<(u32, Option<u32>)>> {
        match field {
            PositionField::Track => self.track.as_ref(),
            PositionField::Disk => self.disk.as_ref(),
        }
    }
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
