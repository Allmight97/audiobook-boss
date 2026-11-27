use super::mp4::writer::SurgicalPatch;
use super::validate_source_fingerprint;
use super::verify::verify_surgical_mp4;
use crate::diagnostics::artifact_id;
use crate::errors::{AppError, Result};
use crate::file_replace::write_atomically;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

const JOURNAL_MAGIC: &[u8; 8] = b"ABBJRN01";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryOutcome {
    None,
    DiscardedCorrupt,
    RolledBack,
}

#[derive(Debug)]
struct JournalExtent {
    offset: u64,
    pre_bytes: Vec<u8>,
    post_len: u32,
    post_sha256: [u8; 32],
}

#[derive(Debug)]
struct ParsedJournal {
    original_file_len: u64,
    extents: Vec<JournalExtent>,
}

pub(crate) fn journal_path_for(target: &Path) -> Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Target path has no parent folder".to_string()))?;
    let name = target
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("Target path has no file name".to_string()))?
        .to_string_lossy();
    Ok(parent.join(format!(".abb_journal_{name}.jrn")))
}

pub(crate) fn recover_if_present(target: &Path) -> Result<RecoveryOutcome> {
    let journal_path = journal_path_for(target)?;
    let bytes = match std::fs::read(&journal_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(RecoveryOutcome::None),
        Err(error) => return Err(AppError::Io(error)),
    };

    let Some(parsed) = parse_and_verify_journal(&bytes) else {
        log::warn!(
            "Discarding corrupt or truncated metadata journal for {}",
            artifact_id(target)
        );
        remove_journal_best_effort(&journal_path);
        return Ok(RecoveryOutcome::DiscardedCorrupt);
    };

    restore_from_parsed(target, &parsed)?;
    std::fs::remove_file(&journal_path).map_err(AppError::Io)?;
    log::warn!(
        "Rolled back interrupted metadata write for {} from journal",
        artifact_id(target)
    );
    Ok(RecoveryOutcome::RolledBack)
}

pub(crate) fn is_single_link(file: &File) -> io::Result<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(file.metadata()?.nlink() == 1)
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Ok(true)
    }
}

pub(crate) fn apply_journaled_patch(
    target: &Path,
    patch: &SurgicalPatch,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    apply_journaled_patch_with_hooks(
        target,
        patch,
        expected_fingerprint,
        before_commit,
        || Ok(()),
    )
}

pub(crate) fn apply_journaled_patch_with_hooks(
    target: &Path,
    patch: &SurgicalPatch,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
    after_writes_before_verify: impl FnOnce() -> Result<()>,
) -> Result<()> {
    ensure_edits_disjoint_from_mdat(patch)?;
    let journal_path = journal_path_for(target)?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .map_err(AppError::Io)?;
    if !is_single_link(&file).map_err(AppError::Io)? {
        return Err(AppError::FileValidation(
            "Refusing in-place journaled patch on multi-link file".to_string(),
        ));
    }
    #[cfg(unix)]
    rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
        .map_err(|errno| AppError::Io(io::Error::from(errno)))?;

    let initial_stat = file.metadata().map_err(AppError::Io)?;
    validate_source_fingerprint(&initial_stat, expected_fingerprint)?;
    let original_file_len = initial_stat.len();
    let (extents, journal_bytes) = capture_journal(&mut file, original_file_len, patch)?;
    let parsed = ParsedJournal {
        original_file_len,
        extents,
    };

    write_atomically(&journal_path, &journal_bytes).map_err(AppError::Io)?;

    before_commit();
    let rechecked_stat = std::fs::metadata(target).map_err(AppError::Io)?;
    if let Err(error) = validate_source_fingerprint(&rechecked_stat, expected_fingerprint) {
        remove_journal_best_effort(&journal_path);
        return Err(error);
    }

    let apply_result = crate::diagnostics::stage_with("metadata_mp4_tags", target, || {
        let write_result = apply_edits_and_sync(
            &mut file,
            original_file_len,
            patch,
            after_writes_before_verify,
        );
        let fields = format!(
            "layout_before={} layout_after={} mdat_shifted=false close_ms={} bytes_written={}",
            patch.layout_before,
            patch.layout_after,
            write_result.as_ref().copied().unwrap_or(0),
            patch.bytes_written
        );
        write_result.map(|_| ((), fields))
    });

    if let Err(error) = apply_result {
        return rollback_and_fail(file, &journal_path, target, &parsed, error);
    }

    let verify_result = crate::diagnostics::stage_with("metadata_verify_audio", target, || {
        let bytes_read = verify_surgical_mp4(target, &patch.mdat_ranges)?;
        Ok(((), format!("mode=structural bytes_read={bytes_read}")))
    });

    if let Err(error) = verify_result {
        return rollback_and_fail(file, &journal_path, target, &parsed, error);
    }

    crate::diagnostics::stage_with("metadata_replace", target, || {
        drop(file);
        std::fs::remove_file(&journal_path).map_err(AppError::Io)?;
        Ok(((), "sync_ms=0 chmod_ms=0 rename_ms=0".to_string()))
    })
}

fn ensure_edits_disjoint_from_mdat(patch: &SurgicalPatch) -> Result<()> {
    for edit in &patch.edits {
        let edit_start = edit.offset;
        let edit_end = edit
            .offset
            .checked_add(edit.data.len() as u64)
            .ok_or_else(|| AppError::General("Surgical patch edit offset overflow".to_string()))?;
        for &(mdat_start, mdat_end) in &patch.mdat_ranges {
            if edit_start < mdat_end && edit_end > mdat_start {
                return Err(AppError::FileValidation(format!(
                    "Refusing surgical MP4 write overlapping mdat [{mdat_start}..{mdat_end})"
                )));
            }
        }
    }
    Ok(())
}

fn capture_journal(
    file: &mut File,
    original_file_len: u64,
    patch: &SurgicalPatch,
) -> Result<(Vec<JournalExtent>, Vec<u8>)> {
    let mut extents = Vec::with_capacity(patch.edits.len() + 1);
    for edit in &patch.edits {
        let pre_len = if edit.offset < original_file_len {
            usize::try_from((original_file_len - edit.offset).min(edit.data.len() as u64))
                .unwrap_or(0)
        } else {
            0
        };
        let pre_bytes = read_exact_or_eof(file, edit.offset, pre_len)?;
        let post_sha256: [u8; 32] = Sha256::digest(&edit.data).into();
        let post_len = u32::try_from(edit.data.len())
            .map_err(|_| AppError::General("Patch extent exceeds u32 length".to_string()))?;
        extents.push(JournalExtent {
            offset: edit.offset,
            pre_bytes,
            post_len,
            post_sha256,
        });
    }
    if patch.final_len < original_file_len {
        let tail_len = usize::try_from(original_file_len - patch.final_len).map_err(|_| {
            AppError::General("Shrinking truncation tail exceeds memory".to_string())
        })?;
        let pre_bytes = read_exact_or_eof(file, patch.final_len, tail_len)?;
        extents.push(JournalExtent {
            offset: patch.final_len,
            pre_bytes,
            post_len: 0,
            post_sha256: Sha256::digest([]).into(),
        });
    }
    let bytes = encode_journal(original_file_len, patch.final_len, &extents)?;
    Ok((extents, bytes))
}

fn read_exact_or_eof(file: &mut File, offset: u64, len: usize) -> Result<Vec<u8>> {
    if len == 0 {
        return Ok(Vec::new());
    }
    file.seek(SeekFrom::Start(offset)).map_err(AppError::Io)?;
    let mut buf = vec![0u8; len];
    file.read_exact(&mut buf).map_err(AppError::Io)?;
    Ok(buf)
}

fn apply_edits_and_sync(
    file: &mut File,
    original_file_len: u64,
    patch: &SurgicalPatch,
    after_writes_before_verify: impl FnOnce() -> Result<()>,
) -> Result<u128> {
    for edit in &patch.edits {
        file.seek(SeekFrom::Start(edit.offset))
            .map_err(AppError::Io)?;
        file.write_all(&edit.data).map_err(AppError::Io)?;
    }
    if patch.final_len != original_file_len {
        file.set_len(patch.final_len).map_err(AppError::Io)?;
    }
    let sync_started = Instant::now();
    file.sync_all().map_err(AppError::Io)?;
    after_writes_before_verify()?;
    Ok(sync_started.elapsed().as_millis())
}

fn rollback_and_fail(
    mut file: File,
    journal_path: &Path,
    target: &Path,
    parsed: &ParsedJournal,
    original_error: AppError,
) -> Result<()> {
    match restore_with_open_file(&mut file, parsed) {
        Ok(()) => {
            drop(file);
            remove_journal_best_effort(journal_path);
            Err(original_error)
        }
        Err(rollback_error) => {
            drop(file);
            Err(AppError::General(format!(
                "{original_error}; rollback on '{}' also failed (journal kept at {}): {rollback_error}",
                crate::errors::sanitize_path_for_display(target),
                journal_path.display()
            )))
        }
    }
}

fn restore_from_parsed(target: &Path, parsed: &ParsedJournal) -> Result<()> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .map_err(AppError::Io)?;
    #[cfg(unix)]
    rustix::fs::flock(&file, rustix::fs::FlockOperation::LockExclusive)
        .map_err(|errno| AppError::Io(io::Error::from(errno)))?;
    restore_with_open_file(&mut file, parsed)
}

fn restore_with_open_file(file: &mut File, parsed: &ParsedJournal) -> Result<()> {
    let current_len = file.metadata().map_err(AppError::Io)?.len();
    if parsed.original_file_len > current_len {
        file.set_len(parsed.original_file_len)
            .map_err(AppError::Io)?;
    }
    for extent in parsed.extents.iter().rev() {
        if !extent.pre_bytes.is_empty() {
            file.seek(SeekFrom::Start(extent.offset))
                .map_err(AppError::Io)?;
            file.write_all(&extent.pre_bytes).map_err(AppError::Io)?;
        }
    }
    if parsed.original_file_len != current_len {
        file.set_len(parsed.original_file_len)
            .map_err(AppError::Io)?;
    }
    file.sync_all().map_err(AppError::Io)?;
    Ok(())
}

fn remove_journal_best_effort(journal_path: &Path) {
    if let Err(error) = std::fs::remove_file(journal_path) {
        if error.kind() != io::ErrorKind::NotFound {
            log::warn!(
                "Failed to remove journal {}: {error}",
                journal_path.display()
            );
        }
    }
}

fn encode_journal(
    original_file_len: u64,
    target_file_len: u64,
    extents: &[JournalExtent],
) -> Result<Vec<u8>> {
    let count = u32::try_from(extents.len())
        .map_err(|_| AppError::General("Too many journal extents".to_string()))?;
    let mut out = Vec::new();
    out.extend_from_slice(JOURNAL_MAGIC);
    out.extend_from_slice(&original_file_len.to_be_bytes());
    out.extend_from_slice(&target_file_len.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    for extent in extents {
        let pre_len = u32::try_from(extent.pre_bytes.len())
            .map_err(|_| AppError::General("Pre-image extent exceeds u32".to_string()))?;
        out.extend_from_slice(&extent.offset.to_be_bytes());
        out.extend_from_slice(&pre_len.to_be_bytes());
        out.extend_from_slice(&extent.pre_bytes);
        out.extend_from_slice(&extent.post_len.to_be_bytes());
        out.extend_from_slice(&extent.post_sha256);
    }
    let checksum: [u8; 32] = Sha256::digest(&out).into();
    out.extend_from_slice(&checksum);
    Ok(out)
}

fn parse_and_verify_journal(bytes: &[u8]) -> Option<ParsedJournal> {
    if bytes.len() < 8 + 8 + 8 + 4 + 32 || &bytes[0..8] != JOURNAL_MAGIC {
        return None;
    }
    let payload_len = bytes.len() - 32;
    let actual_digest: [u8; 32] = Sha256::digest(&bytes[..payload_len]).into();
    if actual_digest.as_slice() != &bytes[payload_len..] {
        return None;
    }
    let original_file_len = read_u64_be(bytes, 8)?;
    let _target_file_len = read_u64_be(bytes, 16)?;
    let count = read_u32_be(bytes, 24)? as usize;
    let mut pos = 28usize;
    let mut extents = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let offset = read_u64_be(&bytes[..payload_len], pos)?;
        let pre_len = read_u32_be(&bytes[..payload_len], pos + 8)? as usize;
        pos += 12;
        if pos.checked_add(pre_len + 36)? > payload_len {
            return None;
        }
        let pre_bytes = bytes[pos..pos + pre_len].to_vec();
        pos += pre_len;
        let post_len = read_u32_be(&bytes[..payload_len], pos)?;
        pos += 4;
        let mut post_sha256 = [0u8; 32];
        post_sha256.copy_from_slice(&bytes[pos..pos + 32]);
        pos += 32;
        extents.push(JournalExtent {
            offset,
            pre_bytes,
            post_len,
            post_sha256,
        });
    }
    if pos != payload_len {
        return None;
    }
    Some(ParsedJournal {
        original_file_len,
        extents,
    })
}

fn read_u64_be(bytes: &[u8], at: usize) -> Option<u64> {
    let slice: [u8; 8] = bytes.get(at..at + 8)?.try_into().ok()?;
    Some(u64::from_be_bytes(slice))
}

fn read_u32_be(bytes: &[u8], at: usize) -> Option<u32> {
    let slice: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
    Some(u32::from_be_bytes(slice))
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
