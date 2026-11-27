use super::boxes::{encode_box, encode_full_box, parse_child_slices};
use crate::errors::{AppError, Result};
use std::fmt;

pub(crate) const TITLE_ATOM: [u8; 4] = *b"\xa9nam";
pub(crate) const ARTIST_ATOM: [u8; 4] = *b"\xa9ART";
pub(crate) const ALBUM_ARTIST_ATOM: [u8; 4] = *b"aART";
pub(crate) const ALBUM_ATOM: [u8; 4] = *b"\xa9alb";
pub(crate) const COMPOSER_ATOM: [u8; 4] = *b"\xa9wrt";
pub(crate) const GENRE_ATOM: [u8; 4] = *b"\xa9gen";
pub(crate) const YEAR_ATOM: [u8; 4] = *b"\xa9day";
pub(crate) const COMMENT_ATOM: [u8; 4] = *b"\xa9cmt";
pub(crate) const DESCRIPTION_ATOM: [u8; 4] = *b"desc";
pub(crate) const ALBUM_SORT_ATOM: [u8; 4] = *b"soal";
pub(crate) const TRACK_ATOM: [u8; 4] = *b"trkn";
pub(crate) const DISC_ATOM: [u8; 4] = *b"disk";
pub(crate) const COVR_ATOM: [u8; 4] = *b"covr";
pub(crate) const MEDIA_TYPE_ATOM: [u8; 4] = *b"stik";
pub(crate) const TV_SHOW_NAME_ATOM: [u8; 4] = *b"tvsh";
pub(crate) const TV_SHOW_SORT_ATOM: [u8; 4] = *b"sosn";
pub(crate) const TV_EPISODE_ATOM: [u8; 4] = *b"tves";
pub(crate) const TV_EPISODE_NAME_ATOM: [u8; 4] = *b"tven";
pub(crate) const MOVEMENT_ATOM: [u8; 4] = *b"\xa9mvn";
pub(crate) const MOVEMENT_INDEX_ATOM: [u8; 4] = *b"\xa9mvi";
pub(crate) const SHOW_MOVEMENT_ATOM: [u8; 4] = *b"shwm";

const RESERVED_TYPE: u32 = 0;
const UTF8_TYPE: u32 = 1;
const UTF16_TYPE: u32 = 2;
pub(crate) const JPEG_TYPE: u32 = 13;
pub(crate) const PNG_TYPE: u32 = 14;
const BE_SIGNED_TYPE: u32 = 21;
pub(crate) const BMP_TYPE: u32 = 27;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomIdent {
    Fourcc([u8; 4]),
    Freeform { mean: String, name: String },
}

impl AtomIdent {
    pub fn freeform(mean: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Freeform {
            mean: mean.into(),
            name: name.into(),
        }
    }
}

impl fmt::Display for AtomIdent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fourcc(fourcc) => {
                for &byte in fourcc {
                    formatter.write_str(&char::from(byte).to_string())?;
                }
                Ok(())
            }
            Self::Freeform { mean, name } => write!(formatter, "----:{mean}:{name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AtomData {
    Reserved(Vec<u8>),
    Utf8(String),
    Utf16(String),
    Jpeg(Vec<u8>),
    Png(Vec<u8>),
    BeSigned(Vec<u8>),
    Bmp(Vec<u8>),
    Unknown { code: u32, data: Vec<u8> },
}

impl AtomData {
    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::Utf8(value) | Self::Utf16(value) => Some(value.as_str()),
            _ => None,
        }
    }

    pub(crate) fn image_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Jpeg(bytes) | Self::Png(bytes) | Self::Bmp(bytes) => Some(bytes.as_slice()),
            _ => None,
        }
    }

    fn type_code(&self) -> u32 {
        match self {
            Self::Reserved(_) => RESERVED_TYPE,
            Self::Utf8(_) => UTF8_TYPE,
            Self::Utf16(_) => UTF16_TYPE,
            Self::Jpeg(_) => JPEG_TYPE,
            Self::Png(_) => PNG_TYPE,
            Self::BeSigned(_) => BE_SIGNED_TYPE,
            Self::Bmp(_) => BMP_TYPE,
            Self::Unknown { code, .. } => *code,
        }
    }

    fn encode(&self) -> Vec<u8> {
        let code = self.type_code();
        let payload = match self {
            Self::Reserved(bytes)
            | Self::Jpeg(bytes)
            | Self::Png(bytes)
            | Self::BeSigned(bytes)
            | Self::Bmp(bytes)
            | Self::Unknown { data: bytes, .. } => bytes.clone(),
            Self::Utf8(text) => text.as_bytes().to_vec(),
            Self::Utf16(text) => text
                .encode_utf16()
                .flat_map(|unit| unit.to_be_bytes())
                .collect(),
        };
        let mut content = Vec::with_capacity(8 + payload.len());
        content.push(0);
        content.push(((code >> 16) & 0xFF) as u8);
        content.push(((code >> 8) & 0xFF) as u8);
        content.push((code & 0xFF) as u8);
        content.extend_from_slice(&[0, 0, 0, 0]);
        content.extend_from_slice(&payload);
        encode_box(*b"data", &content)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IlstItem {
    pub ident: AtomIdent,
    pub data: Vec<AtomData>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mp4Tag {
    pub items: Vec<IlstItem>,
}

impl Mp4Tag {
    pub fn parse_ilst(ilst_content: &[u8]) -> Result<Self> {
        let mut items = Vec::new();
        for slice in parse_child_slices(ilst_content)? {
            if &slice.fourcc == b"free" || &slice.fourcc == b"skip" {
                continue;
            }
            let body = slice.content(ilst_content);
            let Some(item) = parse_ilst_item(slice.fourcc, body)? else {
                continue;
            };
            items.push(item);
        }
        Ok(Self { items })
    }

    pub fn encode_ilst(&self) -> Vec<u8> {
        let mut body = Vec::new();
        for item in &self.items {
            if item.data.is_empty() {
                continue;
            }
            let mut children = Vec::new();
            match &item.ident {
                AtomIdent::Fourcc(fourcc) => {
                    for datum in &item.data {
                        children.extend_from_slice(&datum.encode());
                    }
                    body.extend_from_slice(&encode_box(*fourcc, &children));
                }
                AtomIdent::Freeform { mean, name } => {
                    children.extend_from_slice(&encode_full_box(
                        *b"mean",
                        0,
                        [0; 3],
                        mean.as_bytes(),
                    ));
                    children.extend_from_slice(&encode_full_box(
                        *b"name",
                        0,
                        [0; 3],
                        name.as_bytes(),
                    ));
                    for datum in &item.data {
                        children.extend_from_slice(&datum.encode());
                    }
                    body.extend_from_slice(&encode_box(*b"----", &children));
                }
            }
        }
        encode_box(*b"ilst", &body)
    }

    pub fn set_data(&mut self, ident: AtomIdent, value: AtomData) {
        self.set_all(ident, vec![value]);
    }

    pub fn set_all(&mut self, ident: AtomIdent, values: Vec<AtomData>) {
        if let Some(first_idx) = self.items.iter().position(|item| item.ident == ident) {
            self.items[first_idx].data = values;
            let mut idx = first_idx + 1;
            while idx < self.items.len() {
                if self.items[idx].ident == ident {
                    self.items.remove(idx);
                } else {
                    idx += 1;
                }
            }
        } else {
            self.items.push(IlstItem {
                ident,
                data: values,
            });
        }
    }

    pub fn add_data(&mut self, ident: AtomIdent, value: AtomData) {
        if let Some(existing) = self.items.iter_mut().find(|item| item.ident == ident) {
            existing.data.push(value);
        } else {
            self.items.push(IlstItem {
                ident,
                data: vec![value],
            });
        }
    }

    pub fn remove(&mut self, ident: &AtomIdent) {
        self.items.retain(|item| &item.ident != ident);
    }

    pub fn remove_fourcc(&mut self, fourcc: [u8; 4]) {
        self.remove(&AtomIdent::Fourcc(fourcc));
    }

    pub fn set_fourcc_utf8(&mut self, fourcc: [u8; 4], value: impl Into<String>) {
        self.set_data(AtomIdent::Fourcc(fourcc), AtomData::Utf8(value.into()));
    }

    pub fn strings_of<'a>(&'a self, ident: &'a AtomIdent) -> impl Iterator<Item = &'a str> {
        self.items
            .iter()
            .filter(move |item| &item.ident == ident)
            .flat_map(|item| item.data.iter().filter_map(AtomData::as_str))
    }

    pub fn fourcc_strings(&self, fourcc: [u8; 4]) -> impl Iterator<Item = &str> {
        self.items
            .iter()
            .filter(move |item| item.ident == AtomIdent::Fourcc(fourcc))
            .flat_map(|item| item.data.iter().filter_map(AtomData::as_str))
    }

    pub fn first_fourcc_string(&self, fourcc: [u8; 4]) -> Option<&str> {
        self.fourcc_strings(fourcc).next()
    }

    pub fn data_pairs(&self) -> impl Iterator<Item = (&AtomIdent, &AtomData)> {
        self.items
            .iter()
            .flat_map(|item| item.data.iter().map(move |datum| (&item.ident, datum)))
    }

    pub fn first_be_int(&self, fourcc: [u8; 4]) -> Option<i64> {
        self.items
            .iter()
            .filter(|item| item.ident == AtomIdent::Fourcc(fourcc))
            .flat_map(|item| item.data.iter())
            .find_map(|datum| match datum {
                AtomData::BeSigned(bytes) if !bytes.is_empty() && bytes.len() <= 8 => {
                    Some(decode_be_signed(bytes))
                }
                _ => None,
            })
    }

    pub fn set_track(&mut self, number: u16, total: u16) {
        self.set_position_atom(TRACK_ATOM, number, total);
    }

    pub fn set_disc(&mut self, number: u16, total: u16) {
        self.set_position_atom(DISC_ATOM, number, total);
    }

    pub fn track(&self) -> (Option<u16>, Option<u16>) {
        self.read_position_atom(TRACK_ATOM)
    }

    pub fn disc(&self) -> (Option<u16>, Option<u16>) {
        self.read_position_atom(DISC_ATOM)
    }

    fn set_position_atom(&mut self, fourcc: [u8; 4], number: u16, total: u16) {
        let num = number.to_be_bytes();
        let tot = total.to_be_bytes();
        self.set_data(
            AtomIdent::Fourcc(fourcc),
            AtomData::Reserved(vec![0, 0, num[0], num[1], tot[0], tot[1], 0, 0]),
        );
    }

    fn read_position_atom(&self, fourcc: [u8; 4]) -> (Option<u16>, Option<u16>) {
        let Some(bytes) = self
            .items
            .iter()
            .filter(|item| item.ident == AtomIdent::Fourcc(fourcc))
            .flat_map(|item| item.data.iter())
            .find_map(|datum| match datum {
                AtomData::Reserved(bytes) if bytes.len() >= 6 => Some(bytes.as_slice()),
                _ => None,
            })
        else {
            return (None, None);
        };
        let number = u16::from_be_bytes([bytes[2], bytes[3]]);
        let total = u16::from_be_bytes([bytes[4], bytes[5]]);
        ((number > 0).then_some(number), (total > 0).then_some(total))
    }

    pub fn first_artwork(&self) -> Option<&[u8]> {
        self.items
            .iter()
            .filter(|item| item.ident == AtomIdent::Fourcc(COVR_ATOM))
            .flat_map(|item| item.data.iter())
            .find_map(AtomData::image_bytes)
    }

    pub fn set_media_type_audiobook(&mut self) {
        self.set_data(
            AtomIdent::Fourcc(MEDIA_TYPE_ATOM),
            AtomData::BeSigned(vec![2]),
        );
    }

    pub fn media_type_code(&self) -> Option<i64> {
        self.first_be_int(MEDIA_TYPE_ATOM)
    }
}

fn parse_ilst_item(fourcc: [u8; 4], body: &[u8]) -> Result<Option<IlstItem>> {
    if &fourcc == b"----" {
        return parse_freeform_item(body);
    }
    let mut data = Vec::new();
    for slice in parse_child_slices(body)? {
        if &slice.fourcc == b"data" {
            if let Some(datum) = parse_data_atom(slice.content(body))? {
                data.push(datum);
            }
        }
    }
    Ok(Some(IlstItem {
        ident: AtomIdent::Fourcc(fourcc),
        data,
    }))
}

fn parse_freeform_item(body: &[u8]) -> Result<Option<IlstItem>> {
    let mut mean: Option<String> = None;
    let mut name: Option<String> = None;
    let mut data = Vec::new();
    for slice in parse_child_slices(body)? {
        let content = slice.content(body);
        match &slice.fourcc {
            b"mean" if content.len() >= 4 => {
                mean = Some(String::from_utf8_lossy(&content[4..]).into_owned());
            }
            b"name" if content.len() >= 4 => {
                name = Some(String::from_utf8_lossy(&content[4..]).into_owned());
            }
            b"data" => {
                if let Some(datum) = parse_data_atom(content)? {
                    data.push(datum);
                }
            }
            _ => {}
        }
    }
    let Some(name) = name else {
        return Ok(None);
    };
    Ok(Some(IlstItem {
        ident: AtomIdent::Freeform {
            mean: mean.unwrap_or_default(),
            name,
        },
        data,
    }))
}

pub(crate) fn parse_data_atom(content: &[u8]) -> Result<Option<AtomData>> {
    if content.len() < 8 {
        return Ok(None);
    }
    if content[0] != 0 {
        return Ok(None);
    }
    let code = u32::from_be_bytes([0, content[1], content[2], content[3]]);
    let payload = &content[8..];
    let parsed = match code {
        RESERVED_TYPE => AtomData::Reserved(payload.to_vec()),
        UTF8_TYPE => AtomData::Utf8(
            String::from_utf8(payload.to_vec())
                .map_err(|error| AppError::General(format!("Invalid UTF-8 MP4 atom: {error}")))?,
        ),
        UTF16_TYPE => AtomData::Utf16(decode_utf16_be(payload)?),
        JPEG_TYPE => AtomData::Jpeg(payload.to_vec()),
        PNG_TYPE => AtomData::Png(payload.to_vec()),
        BE_SIGNED_TYPE => AtomData::BeSigned(payload.to_vec()),
        BMP_TYPE => AtomData::Bmp(payload.to_vec()),
        other => AtomData::Unknown {
            code: other,
            data: payload.to_vec(),
        },
    };
    Ok(Some(parsed))
}

fn decode_utf16_be(bytes: &[u8]) -> Result<String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(AppError::General(
            "Odd byte length in UTF-16 MP4 atom".to_string(),
        ));
    }
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_be_bytes(pair))
        .collect();
    String::from_utf16(&units)
        .map_err(|error| AppError::General(format!("Invalid UTF-16 MP4 atom: {error}")))
}

fn decode_be_signed(bytes: &[u8]) -> i64 {
    let mut acc = if (bytes[0] & 0x80) != 0 { -1i64 } else { 0i64 };
    for &byte in bytes {
        acc = (acc << 8) | i64::from(byte);
    }
    acc
}

#[cfg(test)]
#[path = "ilst_tests.rs"]
mod tests;
