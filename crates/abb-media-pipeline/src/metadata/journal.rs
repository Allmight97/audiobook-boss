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
use std::time::{Duration, Instant};

const JOURNAL_MAGIC: &[u8; 8] = b"ABBJRN01";
const ANCHOR_WINDOW_BYTES: u64 = 4096;
const FLOCK_WAIT_BUDGET: Duration = Duration::from_secs(2);
const FLOCK_POLL_STEP: Duration = Duration::from_millis(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecoveryOutcome {
    None,
    DiscardedCorrupt,
    RolledBack,
}

#[derive(Debug)]
struct JournalAnchor {
    offset: u64,
    len: u32,
    sha256: [u8; 32],
}

#[derive(Debug)]
struct JournalExtent {
    offset: u64,
    pre_bytes: Vec<u8>,
    post_bytes: Vec<u8>,
}

#[derive(Debug)]
struct ParsedJournal {
    original_file_len: u64,
    target_file_len: u64,
    anchors: Vec<JournalAnchor>,
    extents: Vec<JournalExtent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetJournalMatch {
    AlreadyOriginal,
    MatchesInterruptedWrite,
    Mismatch,
}

pub(crate) fn journal_path_for(target: &Path) -> Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Target path has no parent folder".to_string()))?;
    let name = target
        .file_name()
        .ok_or_else(|| AppError::InvalidInput("Target path has no file name".to_string()))?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest: [u8; 32] = Sha256::digest(name.as_encoded_bytes()).into();
    let mut hex = String::with_capacity(32);
    for &byte in &digest[..16] {
        hex.push(HEX[(byte >> 4) as usize] as char);
        hex.push(HEX[(byte & 0x0f) as usize] as char);
    }
    Ok(parent.join(format!(".abb_journal_{hex}.jrn")))
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

    let restored = restore_from_parsed(target, &journal_path, &parsed)?;
    if !restored {
        return Ok(RecoveryOutcome::None);
    }
    log::warn!(
        "Rolled back interrupted metadata write for {} from journal",
        artifact_id(target)
    );
    Ok(RecoveryOutcome::RolledBack)
}

#[cfg(unix)]
pub(crate) fn is_flock_unsupported(errno: rustix::io::Errno) -> bool {
    errno == rustix::io::Errno::OPNOTSUPP
        || errno == rustix::io::Errno::NOTSUP
        || errno == rustix::io::Errno::NOLCK
        || errno == rustix::io::Errno::NOSYS
}

#[cfg(unix)]
fn lock_exclusive_with_budget(
    file: &File,
    target: &Path,
    allow_unsupported_fallback: bool,
    max_wait: Duration,
    step: Duration,
) -> Result<bool> {
    let deadline = Instant::now() + max_wait;
    loop {
        match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(true),
            Err(errno) if allow_unsupported_fallback && is_flock_unsupported(errno) => {
                log::info!(
                    "journaled_patch flock unsupported errno={} artifact={}; falling back to data_copy",
                    errno.raw_os_error(),
                    artifact_id(target)
                );
                return Ok(false);
            }
            Err(errno)
                if errno == rustix::io::Errno::WOULDBLOCK || errno == rustix::io::Errno::AGAIN =>
            {
                if Instant::now() >= deadline {
                    return Err(AppError::FileValidation(format!(
                        "Timed out waiting for exclusive file lock on '{}'",
                        crate::errors::sanitize_path_for_display(target)
                    )));
                }
                std::thread::sleep(step);
            }
            Err(errno) => return Err(AppError::Io(io::Error::from(errno))),
        }
    }
}

pub(crate) fn try_lock_for_patch(target: &Path) -> Result<Option<File>> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .map_err(AppError::Io)?;
    if !is_single_link(&file).map_err(AppError::Io)? {
        return Ok(None);
    }
    #[cfg(unix)]
    if !lock_exclusive_with_budget(&file, target, true, FLOCK_WAIT_BUDGET, FLOCK_POLL_STEP)? {
        return Ok(None);
    }
    Ok(Some(file))
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

#[cfg(test)]
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

pub(crate) fn apply_journaled_patch_locked(
    file: File,
    target: &Path,
    patch: &SurgicalPatch,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    apply_journaled_patch_locked_with_hooks(
        file,
        target,
        patch,
        expected_fingerprint,
        before_commit,
        || Ok(()),
    )
}

#[cfg(test)]
pub(crate) fn apply_journaled_patch_with_hooks(
    target: &Path,
    patch: &SurgicalPatch,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
    after_writes_before_verify: impl FnOnce() -> Result<()>,
) -> Result<()> {
    ensure_edits_disjoint_from_mdat(patch)?;
    let file = OpenOptions::new()
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
    lock_exclusive_with_budget(&file, target, false, FLOCK_WAIT_BUDGET, FLOCK_POLL_STEP)?;

    apply_journaled_patch_locked_with_hooks(
        file,
        target,
        patch,
        expected_fingerprint,
        before_commit,
        after_writes_before_verify,
    )
}

fn apply_journaled_patch_locked_with_hooks(
    mut file: File,
    target: &Path,
    patch: &SurgicalPatch,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
    after_writes_before_verify: impl FnOnce() -> Result<()>,
) -> Result<()> {
    ensure_edits_disjoint_from_mdat(patch)?;
    let journal_path = journal_path_for(target)?;

    let initial_stat = file.metadata().map_err(AppError::Io)?;
    validate_source_fingerprint(&initial_stat, expected_fingerprint)?;
    let original_file_len = initial_stat.len();
    let (anchors, extents, journal_bytes) = capture_journal(&mut file, original_file_len, patch)?;
    let parsed = ParsedJournal {
        original_file_len,
        target_file_len: patch.final_len,
        anchors,
        extents,
    };

    write_atomically(&journal_path, &journal_bytes).map_err(AppError::Io)?;
    sync_parent_dir_best_effort(&journal_path);

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
        std::fs::remove_file(&journal_path).map_err(AppError::Io)?;
        sync_parent_dir_best_effort(&journal_path);
        drop(file);
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
) -> Result<(Vec<JournalAnchor>, Vec<JournalExtent>, Vec<u8>)> {
    let anchors = capture_anchors(file, original_file_len, patch)?;
    let mut extents = Vec::with_capacity(patch.edits.len() + 1);
    for edit in &patch.edits {
        let pre_len = if edit.offset < original_file_len {
            usize::try_from((original_file_len - edit.offset).min(edit.data.len() as u64))
                .unwrap_or(0)
        } else {
            0
        };
        let pre_bytes = read_exact_or_eof(file, edit.offset, pre_len)?;
        extents.push(JournalExtent {
            offset: edit.offset,
            pre_bytes,
            post_bytes: edit.data.clone(),
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
            post_bytes: Vec::new(),
        });
    }
    let bytes = encode_journal(original_file_len, patch.final_len, &anchors, &extents)?;
    Ok((anchors, extents, bytes))
}

fn untouched_intervals(original_file_len: u64, patch: &SurgicalPatch) -> Vec<(u64, u64)> {
    let mut covered = Vec::with_capacity(patch.edits.len() + 1);
    for edit in &patch.edits {
        let start = edit.offset.min(original_file_len);
        let end = edit
            .offset
            .saturating_add(edit.data.len() as u64)
            .min(original_file_len);
        if start < end {
            covered.push((start, end));
        }
    }
    if patch.final_len < original_file_len {
        covered.push((patch.final_len, original_file_len));
    }
    covered.sort_unstable();
    let mut untouched = Vec::new();
    let mut cursor = 0u64;
    for (start, end) in covered {
        if cursor < start {
            untouched.push((cursor, start));
        }
        cursor = cursor.max(end);
    }
    if cursor < original_file_len {
        untouched.push((cursor, original_file_len));
    }
    untouched
}

fn capture_anchors(
    file: &mut File,
    original_file_len: u64,
    patch: &SurgicalPatch,
) -> Result<Vec<JournalAnchor>> {
    let intervals = untouched_intervals(original_file_len, patch);
    if intervals.is_empty() {
        return Ok(Vec::new());
    }
    let first = intervals[0];
    let largest = intervals
        .iter()
        .copied()
        .max_by_key(|(start, end)| end - start)
        .unwrap_or(first);
    let last = intervals[intervals.len() - 1];

    let mut windows: Vec<(u64, u32)> = Vec::with_capacity(3);
    push_anchor_window(
        &mut windows,
        first.0,
        (first.1 - first.0).min(ANCHOR_WINDOW_BYTES),
    );
    let largest_len = (largest.1 - largest.0).min(ANCHOR_WINDOW_BYTES);
    let largest_off = largest.0 + ((largest.1 - largest.0) - largest_len) / 2;
    push_anchor_window(&mut windows, largest_off, largest_len);
    let last_len = (last.1 - last.0).min(ANCHOR_WINDOW_BYTES);
    push_anchor_window(&mut windows, last.1 - last_len, last_len);

    let mut anchors = Vec::with_capacity(windows.len());
    for (offset, len) in windows {
        let bytes = read_exact_or_eof(file, offset, len as usize)?;
        let sha256: [u8; 32] = Sha256::digest(&bytes).into();
        anchors.push(JournalAnchor {
            offset,
            len,
            sha256,
        });
    }
    Ok(anchors)
}

fn push_anchor_window(windows: &mut Vec<(u64, u32)>, offset: u64, len: u64) {
    let Ok(len) = u32::try_from(len) else {
        return;
    };
    if len == 0 || windows.contains(&(offset, len)) {
        return;
    }
    windows.push((offset, len));
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
                crate::errors::sanitize_path_for_display(journal_path)
            )))
        }
    }
}

fn restore_from_parsed(target: &Path, journal_path: &Path, parsed: &ParsedJournal) -> Result<bool> {
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .map_err(AppError::Io)?;
    #[cfg(unix)]
    let _ = lock_exclusive_with_budget(&file, target, true, FLOCK_WAIT_BUDGET, FLOCK_POLL_STEP)?;
    if !journal_path.exists() {
        return Ok(false);
    }
    match classify_target_against_journal(&mut file, parsed)? {
        TargetJournalMatch::Mismatch => {
            drop(file);
            return Err(AppError::FileValidation(format!(
                "Refusing to roll back stale metadata journal for '{}': file no longer matches the interrupted write (journal kept at {})",
                crate::errors::sanitize_path_for_display(target),
                crate::errors::sanitize_path_for_display(journal_path)
            )));
        }
        TargetJournalMatch::AlreadyOriginal => {}
        TargetJournalMatch::MatchesInterruptedWrite => {
            restore_with_open_file(&mut file, parsed)?;
        }
    }
    match std::fs::remove_file(journal_path) {
        Ok(()) => sync_parent_dir_best_effort(journal_path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(AppError::Io(error)),
    }
    drop(file);
    Ok(true)
}

fn classify_target_against_journal(
    file: &mut File,
    parsed: &ParsedJournal,
) -> Result<TargetJournalMatch> {
    let current_len = file.metadata().map_err(AppError::Io)?.len();
    let min_len = parsed.original_file_len.min(parsed.target_file_len);
    let max_len = parsed.original_file_len.max(parsed.target_file_len);
    if current_len < min_len || current_len > max_len {
        return Ok(TargetJournalMatch::Mismatch);
    }
    if !anchors_match(file, current_len, &parsed.anchors)? {
        return Ok(TargetJournalMatch::Mismatch);
    }
    let mut all_pre = current_len == parsed.original_file_len;
    for extent in &parsed.extents {
        match classify_extent(file, current_len, extent)? {
            Some(matches_all_pre) => all_pre &= matches_all_pre,
            None => return Ok(TargetJournalMatch::Mismatch),
        }
    }
    if all_pre {
        Ok(TargetJournalMatch::AlreadyOriginal)
    } else {
        Ok(TargetJournalMatch::MatchesInterruptedWrite)
    }
}

fn anchors_match(file: &mut File, current_len: u64, anchors: &[JournalAnchor]) -> Result<bool> {
    for anchor in anchors {
        let Some(end) = anchor.offset.checked_add(u64::from(anchor.len)) else {
            return Ok(false);
        };
        if end > current_len {
            return Ok(false);
        }
        let actual = read_exact_or_eof(file, anchor.offset, anchor.len as usize)?;
        let digest: [u8; 32] = Sha256::digest(&actual).into();
        if digest != anchor.sha256 {
            return Ok(false);
        }
    }
    Ok(true)
}

fn classify_extent(
    file: &mut File,
    current_len: u64,
    extent: &JournalExtent,
) -> Result<Option<bool>> {
    let max_span = extent.pre_bytes.len().max(extent.post_bytes.len()) as u64;
    let present_len =
        usize::try_from(current_len.saturating_sub(extent.offset).min(max_span)).unwrap_or(0);
    let actual = read_exact_or_eof(file, extent.offset, present_len)?;
    for (idx, &byte) in actual.iter().enumerate() {
        let matches = match (extent.pre_bytes.get(idx), extent.post_bytes.get(idx)) {
            (Some(&pre), Some(&post)) => byte == pre || byte == post,
            (Some(&pre), None) => byte == pre,
            (None, Some(&post)) => byte == post || byte == 0,
            (None, None) => false,
        };
        if !matches {
            return Ok(None);
        }
    }
    Ok(Some(
        present_len == extent.pre_bytes.len() && actual == extent.pre_bytes,
    ))
}

fn sync_parent_dir_best_effort(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        let dir_path = if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        };
        if let Ok(dir) = File::open(dir_path) {
            if let Err(error) = dir.sync_all() {
                let ignorable = matches!(
                    error.kind(),
                    io::ErrorKind::InvalidInput
                        | io::ErrorKind::Unsupported
                        | io::ErrorKind::PermissionDenied
                ) || matches!(
                    error.raw_os_error(),
                    Some(errno)
                        if errno == rustix::io::Errno::INVAL.raw_os_error()
                            || errno == rustix::io::Errno::BADF.raw_os_error()
                            || errno == rustix::io::Errno::NOTSUP.raw_os_error()
                            || errno == rustix::io::Errno::OPNOTSUPP.raw_os_error()
                            || errno == rustix::io::Errno::ROFS.raw_os_error()
                );
                if !ignorable {
                    log::warn!(
                        "Parent folder fsync failed near {}: {error}",
                        artifact_id(path)
                    );
                }
            }
        }
    }
    #[cfg(not(unix))]
    let _ = path;
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
                artifact_id(journal_path)
            );
        }
    }
}

fn encode_journal(
    original_file_len: u64,
    target_file_len: u64,
    anchors: &[JournalAnchor],
    extents: &[JournalExtent],
) -> Result<Vec<u8>> {
    let anchor_count = u32::try_from(anchors.len())
        .map_err(|_| AppError::General("Too many journal anchors".to_string()))?;
    let count = u32::try_from(extents.len())
        .map_err(|_| AppError::General("Too many journal extents".to_string()))?;
    let mut out = Vec::new();
    out.extend_from_slice(JOURNAL_MAGIC);
    out.extend_from_slice(&original_file_len.to_be_bytes());
    out.extend_from_slice(&target_file_len.to_be_bytes());
    out.extend_from_slice(&anchor_count.to_be_bytes());
    for anchor in anchors {
        out.extend_from_slice(&anchor.offset.to_be_bytes());
        out.extend_from_slice(&anchor.len.to_be_bytes());
        out.extend_from_slice(&anchor.sha256);
    }
    out.extend_from_slice(&count.to_be_bytes());
    for extent in extents {
        let pre_len = u32::try_from(extent.pre_bytes.len())
            .map_err(|_| AppError::General("Pre-image extent exceeds u32".to_string()))?;
        let post_len = u32::try_from(extent.post_bytes.len())
            .map_err(|_| AppError::General("Post-image extent exceeds u32".to_string()))?;
        out.extend_from_slice(&extent.offset.to_be_bytes());
        out.extend_from_slice(&pre_len.to_be_bytes());
        out.extend_from_slice(&extent.pre_bytes);
        out.extend_from_slice(&post_len.to_be_bytes());
        out.extend_from_slice(&extent.post_bytes);
    }
    let checksum: [u8; 32] = Sha256::digest(&out).into();
    out.extend_from_slice(&checksum);
    Ok(out)
}

fn parse_and_verify_journal(bytes: &[u8]) -> Option<ParsedJournal> {
    if bytes.len() < 8 + 8 + 8 + 4 + 4 + 32 || &bytes[0..8] != JOURNAL_MAGIC {
        return None;
    }
    let payload_len = bytes.len() - 32;
    let payload = &bytes[..payload_len];
    let actual_digest: [u8; 32] = Sha256::digest(payload).into();
    if actual_digest.as_slice() != &bytes[payload_len..] {
        return None;
    }
    let original_file_len = read_u64_be(payload, 8)?;
    let target_file_len = read_u64_be(payload, 16)?;
    let mut pos = 24usize;
    let anchors = parse_anchors(payload, &mut pos)?;
    let extents = parse_extents(payload, &mut pos)?;
    if pos != payload.len() {
        return None;
    }
    Some(ParsedJournal {
        original_file_len,
        target_file_len,
        anchors,
        extents,
    })
}

fn parse_anchors(payload: &[u8], pos: &mut usize) -> Option<Vec<JournalAnchor>> {
    let count = read_u32_be(payload, *pos)? as usize;
    *pos += 4;
    let mut anchors = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let end = pos.checked_add(44)?;
        if end > payload.len() {
            return None;
        }
        let offset = read_u64_be(payload, *pos)?;
        let len = read_u32_be(payload, *pos + 8)?;
        let mut sha256 = [0u8; 32];
        sha256.copy_from_slice(&payload[*pos + 12..end]);
        *pos = end;
        anchors.push(JournalAnchor {
            offset,
            len,
            sha256,
        });
    }
    Some(anchors)
}

fn read_len_prefixed_bytes(payload: &[u8], pos: &mut usize) -> Option<Vec<u8>> {
    let len = read_u32_be(payload, *pos)? as usize;
    let start = pos.checked_add(4)?;
    let end = start.checked_add(len)?;
    if end > payload.len() {
        return None;
    }
    *pos = end;
    Some(payload[start..end].to_vec())
}

fn parse_extents(payload: &[u8], pos: &mut usize) -> Option<Vec<JournalExtent>> {
    let count = read_u32_be(payload, *pos)? as usize;
    *pos += 4;
    let mut extents = Vec::with_capacity(count.min(1024));
    for _ in 0..count {
        let offset = read_u64_be(payload, *pos)?;
        *pos += 8;
        let pre_bytes = read_len_prefixed_bytes(payload, pos)?;
        let post_bytes = read_len_prefixed_bytes(payload, pos)?;
        extents.push(JournalExtent {
            offset,
            pre_bytes,
            post_bytes,
        });
    }
    Some(extents)
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
pub(crate) fn plant_interrupted_patch_for_test(target: &Path, patch: &SurgicalPatch) {
    let original_len = std::fs::metadata(target).expect("stat target").len();
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(target)
        .expect("open target");
    let (_, _, journal_bytes) = capture_journal(&mut file, original_len, patch).expect("capture");
    for edit in &patch.edits {
        file.seek(SeekFrom::Start(edit.offset)).expect("seek");
        file.write_all(&edit.data).expect("write");
    }
    file.set_len(patch.final_len).expect("set_len");
    drop(file);
    let journal_path = journal_path_for(target).expect("journal path");
    std::fs::write(&journal_path, &journal_bytes).expect("plant journal");
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
