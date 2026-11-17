#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mp4BoxHeader {
    pub kind: [u8; 4],
    pub header_len: u64,
    pub size: Option<u64>,
}

pub const MP4_BOX_HEADER_PROBE_LEN: usize = 16;

pub fn mp4_box_header(bytes: &[u8]) -> Option<Mp4BoxHeader> {
    let size32 = u32::from_be_bytes(bytes.get(0..4)?.try_into().ok()?);
    let kind: [u8; 4] = bytes.get(4..8)?.try_into().ok()?;
    let (header_len, size) = match size32 {
        0 => (8, None),
        1 => (
            16,
            Some(u64::from_be_bytes(bytes.get(8..16)?.try_into().ok()?)),
        ),
        size => (8, Some(u64::from(size))),
    };
    if size.is_some_and(|size| size < header_len) {
        return None;
    }
    Some(Mp4BoxHeader {
        kind,
        header_len,
        size,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmBoxes {
    pub adrm_present: bool,
    pub aavd_entry_present: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedFileFacts {
    pub major_brand: String,
    pub compatible_brands: Vec<String>,
    pub drm_boxes: Option<DrmBoxes>,
}

pub fn protected_file_facts(
    ftyp: &[u8],
    sample_descriptions: Option<&[Vec<u8>]>,
) -> ProtectedFileFacts {
    ProtectedFileFacts {
        major_brand: ftyp.get(0..4).map(brand_text).unwrap_or_default(),
        compatible_brands: ftyp
            .get(8..)
            .unwrap_or_default()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|brand| brand_text(brand))
            .collect(),
        drm_boxes: sample_descriptions.map(|payloads| DrmBoxes {
            adrm_present: payloads
                .iter()
                .any(|payload| contains_box_kind(payload, b"adrm")),
            aavd_entry_present: payloads
                .iter()
                .any(|payload| contains_box_kind(payload, b"aavd")),
        }),
    }
}

fn brand_text(brand: &[u8]) -> String {
    brand
        .iter()
        .map(|&byte| {
            if byte.is_ascii_graphic() || byte == b' ' {
                char::from(byte)
            } else {
                '?'
            }
        })
        .collect()
}

fn contains_box_kind(payload: &[u8], kind: &[u8; 4]) -> bool {
    payload.windows(4).skip(4).any(|window| window == kind)
}

#[cfg(test)]
#[path = "protected_file_tests.rs"]
mod tests;
