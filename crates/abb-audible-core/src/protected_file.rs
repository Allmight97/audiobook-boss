/// One MP4 box: its four-character type, where its payload starts, and its
/// total size (`None` when it runs to the end of its parent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mp4BoxHeader {
    pub kind: [u8; 4],
    pub header_len: u64,
    pub size: Option<u64>,
}

/// Bytes a caller reads at a box's start before calling [`mp4_box_header`].
pub const MP4_BOX_HEADER_PROBE_LEN: usize = 16;

/// Parses a box header from the first bytes at a box's offset. Returns `None`
/// when the bytes cannot start a box.
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

/// The DRM boxes found in a download's sample descriptions. FFmpeg's MP4
/// demuxer needs `adrm` to apply AAX activation bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrmBoxes {
    pub adrm_present: bool,
    pub aavd_entry_present: bool,
}

/// What an Audible download declares about its protection. None of it is
/// secret: it decides whether FFmpeg's MP4 demuxer applies AAXC keys (only for
/// major brand `aaxc`) or AAX activation bytes (when an `adrm` box exists).
/// `drm_boxes` is `None` when the sample descriptions could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectedFileFacts {
    pub major_brand: String,
    pub compatible_brands: Vec<String>,
    pub drm_boxes: Option<DrmBoxes>,
}

/// `ftyp` is the payload of the top-level `ftyp` box. `sample_descriptions`
/// holds the payload of each `moov/trak/mdia/minf/stbl/stsd` box; `adrm` and
/// `aavd` are found by their box type names inside them.
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
