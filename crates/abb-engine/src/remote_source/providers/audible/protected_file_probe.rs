use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use abb_audible_core::{
    mp4_box_header, protected_file_facts, title_ref, Mp4BoxHeader, ProtectedFileFacts,
    MP4_BOX_HEADER_PROBE_LEN,
};

use crate::remote_source::materializer::AaxcleanLane;

const MAX_METADATA_BOX_BYTES: u64 = 64 * 1024 * 1024;

/// Reads the top-level `ftyp` and `moov` payloads of a downloaded file.
pub(super) fn read_protected_file_facts(path: &Path) -> std::io::Result<ProtectedFileFacts> {
    let mut file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let (mut ftyp, mut moov) = (None, None);
    let mut offset = 0;
    while offset < file_len && (ftyp.is_none() || moov.is_none()) {
        let header = box_header_at(&mut file, offset)?;
        let size = header.size.unwrap_or(file_len - offset);
        if size > file_len - offset {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("MP4 box at byte {offset} runs past the end of the file"),
            ));
        }
        let payload_len = size - header.header_len;
        match &header.kind {
            b"ftyp" => {
                ftyp = Some(read_payload(
                    &mut file,
                    offset + header.header_len,
                    payload_len,
                )?)
            }
            b"moov" => {
                moov = Some(read_payload(
                    &mut file,
                    offset + header.header_len,
                    payload_len,
                )?)
            }
            _ => {}
        }
        offset += size;
    }
    Ok(protected_file_facts(
        ftyp.as_deref().unwrap_or_default(),
        moov.as_deref().unwrap_or_default(),
    ))
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

fn read_payload(file: &mut File, start: u64, len: u64) -> std::io::Result<Vec<u8>> {
    if len > MAX_METADATA_BOX_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("metadata box of {len} bytes is over the probe limit"),
        ));
    }
    file.seek(SeekFrom::Start(start))?;
    let mut payload = Vec::with_capacity(usize::try_from(len).unwrap_or_default());
    file.take(len).read_to_end(&mut payload)?;
    Ok(payload)
}

/// Logs what the download declares about its protection, next to the lane the
/// license chose. Diagnostic only: a file it cannot read is logged and the
/// download continues.
pub(super) async fn log_protected_file_facts(
    path: &Path,
    job_id: &str,
    title_id: &str,
    lane: AaxcleanLane,
) {
    let probed = path.to_path_buf();
    #[expect(clippy::disallowed_methods, reason = "awaited on the next line")]
    let facts = tokio::task::spawn_blocking(move || read_protected_file_facts(&probed)).await;
    match facts {
        Ok(Ok(facts)) => log::info!(
            "remote_source audible stage=protected_file_facts job_id={} title_ref={} lane={:?} major_brand={:?} compatible_brands={:?} adrm_present={} aavd_entry_present={}",
            job_id,
            title_ref(title_id),
            lane,
            facts.major_brand,
            facts.compatible_brands,
            facts.adrm_present,
            facts.aavd_entry_present
        ),
        Ok(Err(error)) => log::warn!(
            "remote_source audible stage=protected_file_facts_unreadable job_id={} title_ref={} lane={:?}: {error}",
            job_id,
            title_ref(title_id),
            lane
        ),
        Err(error) => log::warn!(
            "remote_source audible stage=protected_file_facts_unreadable job_id={} title_ref={} lane={:?}: probe task failed: {error}",
            job_id,
            title_ref(title_id),
            lane
        ),
    }
}

#[cfg(test)]
#[path = "protected_file_probe_tests.rs"]
mod tests;
