use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use abb_audible_core::{
    mp4_box_header, protected_file_facts, Mp4BoxHeader, ProtectedFileFacts,
    MP4_BOX_HEADER_PROBE_LEN,
};

const SMALL_BOX_READ_LIMIT: u64 = 64 * 1024;
const SAMPLE_DESCRIPTION_PATH: [[u8; 4]; 5] = [*b"trak", *b"mdia", *b"minf", *b"stbl", *b"stsd"];

#[derive(Clone, Copy)]
struct BoxSpan {
    kind: [u8; 4],
    payload_start: u64,
    end: u64,
}

/// Reads the brands in the top-level `ftyp` box. Walks the box headers down to
/// each track's sample description, seeking past every other payload, and reads
/// only those small boxes. The walk is best effort: when it fails, the facts
/// say the protection boxes are unknown and the AAXC lane, which needs only the
/// brand, is unaffected.
pub(super) fn read_protected_file_facts(path: &Path) -> std::io::Result<ProtectedFileFacts> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let top_level = child_boxes(&mut file, 0, file_len, &[*b"ftyp", *b"moov"])?;
    let ftyp = match top_level.iter().find(|span| span.kind == *b"ftyp") {
        Some(span) => read_small_box(&mut file, *span)?,
        None => Vec::new(),
    };
    let sample_descriptions = top_level
        .iter()
        .find(|span| span.kind == *b"moov")
        .map(|moov| read_sample_descriptions(&mut file, *moov))
        .transpose()
        .unwrap_or_else(|error| {
            log::warn!("remote_source audible stage=protected_file_sample_descriptions_unreadable: {error}");
            None
        });
    Ok(protected_file_facts(&ftyp, sample_descriptions.as_deref()))
}

fn read_sample_descriptions(file: &mut File, moov: BoxSpan) -> std::io::Result<Vec<Vec<u8>>> {
    let mut level = vec![moov];
    for kind in SAMPLE_DESCRIPTION_PATH {
        let mut next = Vec::new();
        for parent in level {
            next.extend(
                child_boxes(file, parent.payload_start, parent.end, &[])?
                    .into_iter()
                    .filter(|child| child.kind == kind),
            );
        }
        level = next;
    }
    level
        .into_iter()
        .map(|stsd| read_small_box(file, stsd))
        .collect()
}

fn child_boxes(
    file: &mut File,
    start: u64,
    end: u64,
    stop_once_found: &[[u8; 4]],
) -> std::io::Result<Vec<BoxSpan>> {
    let mut children: Vec<BoxSpan> = Vec::new();
    let mut offset = start;
    let found_all = |children: &[BoxSpan]| {
        !stop_once_found.is_empty()
            && stop_once_found
                .iter()
                .all(|kind| children.iter().any(|child| child.kind == *kind))
    };
    while offset < end && !found_all(&children) {
        let header = box_header_at(file, offset)?;
        let size = header.size.unwrap_or(end - offset);
        if size > end - offset {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("MP4 box at byte {offset} runs past the end of its parent"),
            ));
        }
        children.push(BoxSpan {
            kind: header.kind,
            payload_start: offset + header.header_len,
            end: offset + size,
        });
        offset += size;
    }
    Ok(children)
}

fn box_header_at(file: &mut File, offset: u64) -> std::io::Result<Mp4BoxHeader> {
    file.seek(SeekFrom::Start(offset))?;
    let mut probe = [0_u8; MP4_BOX_HEADER_PROBE_LEN];
    let read = file.read(&mut probe)?;
    mp4_box_header(&probe[..read]).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("no MP4 box at byte {offset}"),
        )
    })
}

fn read_small_box(file: &mut File, span: BoxSpan) -> std::io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(span.payload_start))?;
    let mut payload = Vec::new();
    file.take((span.end - span.payload_start).min(SMALL_BOX_READ_LIMIT))
        .read_to_end(&mut payload)?;
    Ok(payload)
}

#[cfg(test)]
#[path = "protected_file_probe_tests.rs"]
mod tests;
