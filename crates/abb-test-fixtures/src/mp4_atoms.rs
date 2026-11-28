use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawAtomData {
    Reserved(Vec<u8>),
    Utf8(String),
    Utf16(String),
    Jpeg(Vec<u8>),
    Png(Vec<u8>),
    BeSigned(Vec<u8>),
    Bmp(Vec<u8>),
    Unknown { code: u32, data: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawAtomIdent {
    Fourcc([u8; 4]),
    Freeform { mean: String, name: String },
}

impl RawAtomIdent {
    pub fn display_name(&self) -> String {
        match self {
            Self::Fourcc(fourcc) => fourcc.iter().map(|&b| char::from(b)).collect(),
            Self::Freeform { mean, name } => format!("----:{mean}:{name}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIlstEntry {
    pub ident: RawAtomIdent,
    pub data: Vec<RawAtomData>,
}

#[derive(Debug, Clone, Copy)]
struct BoxSpan {
    fourcc: [u8; 4],
    offset: usize,
    header_len: usize,
    end: usize,
    explicit_zero_size: bool,
}

impl BoxSpan {
    fn body<'a>(&self, buf: &'a [u8]) -> &'a [u8] {
        &buf[self.offset + self.header_len..self.end]
    }
}

fn parse_spans(buf: &[u8]) -> Vec<BoxSpan> {
    let mut spans = Vec::new();
    let mut pos = 0usize;
    while pos + 8 <= buf.len() {
        let size32 = u32::from_be_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        let fourcc = [buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]];
        let (size, header_len, explicit_zero_size) = match size32 {
            0 => (buf.len() - pos, 8usize, true),
            1 => {
                if pos + 16 > buf.len() {
                    break;
                }
                let size64 =
                    u64::from_be_bytes(buf[pos + 8..pos + 16].try_into().expect("8 bytes"));
                let Ok(size) = usize::try_from(size64) else {
                    break;
                };
                (size, 16usize, false)
            }
            other => (other as usize, 8usize, false),
        };
        if size < header_len || pos + size > buf.len() {
            break;
        }
        spans.push(BoxSpan {
            fourcc,
            offset: pos,
            header_len,
            end: pos + size,
            explicit_zero_size,
        });
        pos += size;
        if explicit_zero_size {
            break;
        }
    }
    spans
}

fn encode_box(fourcc: [u8; 4], content: &[u8]) -> Vec<u8> {
    let total = 8u64 + content.len() as u64;
    if let Ok(size32) = u32::try_from(total) {
        let mut out = Vec::with_capacity(total as usize);
        out.extend_from_slice(&size32.to_be_bytes());
        out.extend_from_slice(&fourcc);
        out.extend_from_slice(content);
        out
    } else {
        let total64 = 16u64 + content.len() as u64;
        let mut out = Vec::with_capacity(total64 as usize);
        out.extend_from_slice(&1u32.to_be_bytes());
        out.extend_from_slice(&fourcc);
        out.extend_from_slice(&total64.to_be_bytes());
        out.extend_from_slice(content);
        out
    }
}

fn encode_full_box(fourcc: [u8; 4], body: &[u8]) -> Vec<u8> {
    let mut content = Vec::with_capacity(4 + body.len());
    content.extend_from_slice(&[0, 0, 0, 0]);
    content.extend_from_slice(body);
    encode_box(fourcc, &content)
}

fn encode_data_atom(type_code: u32, payload: &[u8]) -> Vec<u8> {
    let mut content = Vec::with_capacity(8 + payload.len());
    content.push(0);
    content.push(((type_code >> 16) & 0xFF) as u8);
    content.push(((type_code >> 8) & 0xFF) as u8);
    content.push((type_code & 0xFF) as u8);
    content.extend_from_slice(&[0, 0, 0, 0]);
    content.extend_from_slice(payload);
    encode_box(*b"data", &content)
}

pub fn mp4_fourcc_utf8(fourcc: [u8; 4], value: &str) -> Vec<u8> {
    mp4_fourcc_data(fourcc, 1, value.as_bytes())
}

pub fn mp4_fourcc_data(fourcc: [u8; 4], type_code: u32, payload: &[u8]) -> Vec<u8> {
    encode_box(fourcc, &encode_data_atom(type_code, payload))
}

pub fn mp4_fourcc_multi(fourcc: [u8; 4], entries: &[(u32, &[u8])]) -> Vec<u8> {
    let mut children = Vec::new();
    for &(type_code, payload) in entries {
        children.extend_from_slice(&encode_data_atom(type_code, payload));
    }
    encode_box(fourcc, &children)
}

pub fn mp4_freeform_utf8(mean: &str, name: &str, value: &str) -> Vec<u8> {
    let mut children = Vec::new();
    children.extend_from_slice(&encode_full_box(*b"mean", mean.as_bytes()));
    children.extend_from_slice(&encode_full_box(*b"name", name.as_bytes()));
    children.extend_from_slice(&encode_data_atom(1, value.as_bytes()));
    encode_box(*b"----", &children)
}

fn default_hdlr_box() -> Vec<u8> {
    encode_box(
        *b"hdlr",
        &[
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x6d, 0x64, 0x69, 0x72, 0x61, 0x70,
            0x70, 0x6c, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ],
    )
}

fn looks_like_child_at(slice: &[u8]) -> bool {
    if slice.len() < 8 {
        return false;
    }
    let size = u32::from_be_bytes([slice[0], slice[1], slice[2], slice[3]]) as usize;
    let fourcc = &slice[4..8];
    size >= 8
        && size <= slice.len()
        && fourcc
            .iter()
            .all(|byte| byte.is_ascii_graphic() || *byte == b' ')
}

fn meta_header_len(content: &[u8]) -> usize {
    if content.len() >= 4
        && content[0..4] == [0, 0, 0, 0]
        && (content.len() == 4 || looks_like_child_at(&content[4..]))
    {
        return 4;
    }
    if looks_like_child_at(content) {
        return 0;
    }
    usize::from(content.len() >= 4) * 4
}

fn find_ilst_slice(moov_body: &[u8]) -> Option<&[u8]> {
    for udta in parse_spans(moov_body) {
        if &udta.fourcc != b"udta" {
            continue;
        }
        let udta_body = udta.body(moov_body);
        for meta in parse_spans(udta_body) {
            if &meta.fourcc != b"meta" {
                continue;
            }
            let meta_body = meta.body(udta_body);
            let skip = meta_header_len(meta_body);
            for ilst in parse_spans(&meta_body[skip..]) {
                if &ilst.fourcc == b"ilst" {
                    return Some(ilst.body(&meta_body[skip..]));
                }
            }
        }
    }
    None
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| u16::from_be_bytes(pair))
        .collect();
    String::from_utf16_lossy(&units)
}

fn parse_data_atom(content: &[u8]) -> Option<RawAtomData> {
    if content.len() < 8 || content[0] != 0 {
        return None;
    }
    let code = u32::from_be_bytes([0, content[1], content[2], content[3]]);
    let payload = &content[8..];
    Some(match code {
        0 => RawAtomData::Reserved(payload.to_vec()),
        1 => RawAtomData::Utf8(String::from_utf8_lossy(payload).into_owned()),
        2 => RawAtomData::Utf16(decode_utf16_be(payload)),
        13 => RawAtomData::Jpeg(payload.to_vec()),
        14 => RawAtomData::Png(payload.to_vec()),
        21 => RawAtomData::BeSigned(payload.to_vec()),
        27 => RawAtomData::Bmp(payload.to_vec()),
        other => RawAtomData::Unknown {
            code: other,
            data: payload.to_vec(),
        },
    })
}

pub fn read_ilst_atoms(path: &Path) -> Vec<RawIlstEntry> {
    let bytes = fs::read(path).expect("read MP4 file for atom inspection");
    let top = parse_spans(&bytes);
    let moov = top
        .into_iter()
        .find(|span| &span.fourcc == b"moov")
        .expect("moov atom in MP4 file");
    let Some(ilst_body) = find_ilst_slice(moov.body(&bytes)) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for item in parse_spans(ilst_body) {
        if &item.fourcc == b"free" || &item.fourcc == b"skip" {
            continue;
        }
        let body = item.body(ilst_body);
        if &item.fourcc == b"----" {
            let mut mean = String::new();
            let mut name = None;
            let mut data = Vec::new();
            for child in parse_spans(body) {
                let cbody = child.body(body);
                match &child.fourcc {
                    b"mean" if cbody.len() >= 4 => {
                        mean = String::from_utf8_lossy(&cbody[4..]).into_owned();
                    }
                    b"name" if cbody.len() >= 4 => {
                        name = Some(String::from_utf8_lossy(&cbody[4..]).into_owned());
                    }
                    b"data" => {
                        if let Some(datum) = parse_data_atom(cbody) {
                            data.push(datum);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(name) = name {
                entries.push(RawIlstEntry {
                    ident: RawAtomIdent::Freeform { mean, name },
                    data,
                });
            }
        } else {
            let data = parse_spans(body)
                .into_iter()
                .filter(|child| &child.fourcc == b"data")
                .filter_map(|child| parse_data_atom(child.body(body)))
                .collect();
            entries.push(RawIlstEntry {
                ident: RawAtomIdent::Fourcc(item.fourcc),
                data,
            });
        }
    }
    entries
}

pub fn read_freeform_strings(path: &Path, mean: &str, name: &str) -> Vec<String> {
    read_ilst_atoms(path)
        .into_iter()
        .filter(|entry| {
            matches!(
                &entry.ident,
                RawAtomIdent::Freeform { mean: m, name: n } if m == mean && n == name
            )
        })
        .flat_map(|entry| entry.data)
        .filter_map(|datum| match datum {
            RawAtomData::Utf8(s) | RawAtomData::Utf16(s) => Some(s),
            _ => None,
        })
        .collect()
}

fn replace_or_append_child(container: &[u8], target: [u8; 4], replacement: &[u8]) -> Vec<u8> {
    let spans = parse_spans(container);
    let target_idx = spans.iter().position(|span| span.fourcc == target);
    let mut out = Vec::with_capacity(container.len() + replacement.len());
    let mut cursor = 0usize;
    for (idx, span) in spans.iter().enumerate() {
        if span.offset > cursor {
            out.extend_from_slice(&container[cursor..span.offset]);
        }
        if target_idx == Some(idx) {
            out.extend_from_slice(replacement);
        } else {
            out.extend_from_slice(&container[span.offset..span.end]);
        }
        cursor = span.end;
    }
    if target_idx.is_none() {
        out.extend_from_slice(replacement);
    }
    if cursor < container.len() {
        out.extend_from_slice(&container[cursor..]);
    }
    out
}

fn rebuild_meta(meta_body: &[u8], raw_items: &[Vec<u8>]) -> Vec<u8> {
    let skip = meta_header_len(meta_body);
    let children_slice = &meta_body[skip..];
    let existing_ilst = parse_spans(children_slice)
        .into_iter()
        .find(|child| &child.fourcc == b"ilst")
        .map(|ilst| ilst.body(children_slice).to_vec())
        .unwrap_or_default();
    let mut ilst_payload = existing_ilst;
    for item in raw_items {
        ilst_payload.extend_from_slice(item);
    }
    let ilst_box = encode_box(*b"ilst", &ilst_payload);
    let mut rebuilt = meta_body[..skip].to_vec();
    rebuilt.extend_from_slice(&replace_or_append_child(
        children_slice,
        *b"ilst",
        &ilst_box,
    ));
    encode_box(*b"meta", &rebuilt)
}

fn fresh_meta(raw_items: &[Vec<u8>]) -> Vec<u8> {
    let mut ilst_payload = Vec::new();
    for item in raw_items {
        ilst_payload.extend_from_slice(item);
    }
    let mut meta_content = vec![0u8; 4];
    meta_content.extend_from_slice(&default_hdlr_box());
    meta_content.extend_from_slice(&encode_box(*b"ilst", &ilst_payload));
    encode_box(*b"meta", &meta_content)
}

fn rebuild_udta(udta_body: &[u8], raw_items: &[Vec<u8>]) -> Vec<u8> {
    let meta = parse_spans(udta_body)
        .into_iter()
        .find(|child| &child.fourcc == b"meta")
        .map(|span| rebuild_meta(span.body(udta_body), raw_items))
        .unwrap_or_else(|| fresh_meta(raw_items));
    encode_box(
        *b"udta",
        &replace_or_append_child(udta_body, *b"meta", &meta),
    )
}

pub fn append_ilst_items(path: &Path, raw_items: &[Vec<u8>]) {
    let mut bytes = Vec::new();
    let mut file = File::open(path).expect("open MP4 to append ilst items");
    file.read_to_end(&mut bytes).expect("read MP4");
    drop(file);

    let spans = parse_spans(&bytes);
    let moov_idx = spans
        .iter()
        .position(|span| &span.fourcc == b"moov")
        .expect("moov box");
    let moov = spans[moov_idx];
    let moov_body = moov.body(&bytes);
    let udta = parse_spans(moov_body)
        .into_iter()
        .find(|child| &child.fourcc == b"udta")
        .map(|span| rebuild_udta(span.body(moov_body), raw_items))
        .unwrap_or_else(|| encode_box(*b"udta", &fresh_meta(raw_items)));
    let new_moov_body = replace_or_append_child(moov_body, *b"udta", &udta);
    let new_moov = encode_box(*b"moov", &new_moov_body);

    let mut out = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("open MP4 for writing");
    if moov_idx + 1 == spans.len() {
        out.seek(SeekFrom::Start(moov.offset as u64))
            .expect("seek moov");
        out.write_all(&new_moov).expect("write moov");
        out.set_len((moov.offset + new_moov.len()) as u64)
            .expect("set_len");
    } else {
        let moov_size = moov.end - moov.offset;
        assert!(moov_size >= 8);
        let free_hdr: Vec<u8> = if moov.header_len == 16 || moov_size > u32::MAX as usize {
            [
                &1u32.to_be_bytes()[..],
                b"free",
                &(moov_size as u64).to_be_bytes()[..],
            ]
            .concat()
        } else {
            [&(moov_size as u32).to_be_bytes()[..], b"free"].concat()
        };
        out.seek(SeekFrom::Start(moov.offset as u64))
            .expect("seek old moov");
        out.write_all(&free_hdr).expect("write free tombstone");
        if let Some(last) = spans.last().filter(|last| last.explicit_zero_size) {
            let last_size = (last.end - last.offset) as u32;
            out.seek(SeekFrom::Start(last.offset as u64))
                .expect("seek explicit-zero box");
            out.write_all(&last_size.to_be_bytes())
                .expect("patch last size");
        }
        out.seek(SeekFrom::End(0)).expect("seek EOF");
        out.write_all(&new_moov).expect("append moov");
    }
}
