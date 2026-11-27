use super::container::{self, Container};
use super::journal::{apply_journaled_patch, is_single_link, recover_if_present};
use super::mp4::metadata::apply_write_plan;
use super::mp4::read_tag;
use super::mp4::writer::{execute_patch_plan, plan_mp4_patch_for_tag, Mp4PatchPlan, WriteTagStats};
use super::plan_log::log_write_plan;
use super::remux;
use super::verify::{commit_checked_save, verify_streaming_mp4, verify_surgical_mp4};
use super::{source_fingerprint, tag_support, validate_source_fingerprint, MetadataWritePlan};
use crate::diagnostics::{artifact_id, stage, stage_with};
use crate::errors::{AppError, Result};
use crate::file_copy::{copy_data_only, filesystem_label_for_file, try_cow_clone, CopyStats};
use crate::file_replace::SiblingTemp;
use std::fs::File;
use std::path::Path;

pub(super) fn save_metadata_with_strategy(
    path: &Path,
    plan: &MetadataWritePlan,
    before_commit: impl FnOnce(),
) -> Result<()> {
    let route = stage("metadata_classify", path, || container::classify(path))?;
    tag_support::refuse_unholdable_edits(&route, plan)?;
    let expected_fingerprint = source_fingerprint(path)?;
    match route {
        Container::Mp4 => save_mp4_with_strategy(path, plan, &expected_fingerprint, before_commit),
        non_mp4 => save_non_mp4(path, plan, &non_mp4, &expected_fingerprint, before_commit),
    }
}

fn save_non_mp4(
    path: &Path,
    plan: &MetadataWritePlan,
    route: &Container,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    let saved = remux::meta_temp(path)?;
    remux::remux_into(path, saved.path(), Some(plan), None, route)?;
    before_commit();
    commit_checked_save(path, saved, expected_fingerprint)
}

fn save_mp4_with_strategy(
    path: &Path,
    plan: &MetadataWritePlan,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    recover_if_present(path)?;
    let source_file = File::open(path).map_err(AppError::Io)?;
    let fs = filesystem_label_for_file(&source_file);
    let single_link = is_single_link(&source_file).map_err(AppError::Io)?;
    drop(source_file);

    let saved = remux::meta_temp(path)?;
    if try_cow_clone(path, saved.path()).map_err(AppError::Io)? {
        let cow_stats = CopyStats {
            strategy: "cow_clone",
            fs,
            bytes_written: 0,
        };
        stage_with("metadata_copy_source", path, || {
            Ok(((), cow_stats.log_fields(0)))
        })?;
        return commit_temp_mp4(path, saved, plan, expected_fingerprint, before_commit);
    }

    let mut tag = read_tag(path)?;
    apply_write_plan(&mut tag, plan)?;
    let mp4_plan = plan_mp4_patch_for_tag(path, &tag)?;
    if single_link {
        if let Mp4PatchPlan::Surgical(patch) = mp4_plan {
            drop(saved);
            let journal_stats = CopyStats {
                strategy: "journaled_patch",
                fs,
                bytes_written: 0,
            };
            stage_with("metadata_copy_source", path, || {
                Ok(((), journal_stats.log_fields(0)))
            })?;
            log_write_plan(plan, "mp4_native", &artifact_id(path));
            return apply_journaled_patch(path, &patch, expected_fingerprint, before_commit);
        }
    }

    stage_with("metadata_copy_source", path, || {
        let started = std::time::Instant::now();
        let bytes_written = copy_data_only(path, saved.path()).map_err(AppError::Io)?;
        let copy_stats = CopyStats {
            strategy: "data_copy",
            fs,
            bytes_written,
        };
        Ok(((), copy_stats.log_fields(started.elapsed().as_millis())))
    })?;
    commit_temp_mp4_with_plan(
        saved,
        path,
        plan,
        &mp4_plan,
        expected_fingerprint,
        before_commit,
    )
}

fn commit_temp_mp4(
    original: &Path,
    saved: SiblingTemp,
    plan: &MetadataWritePlan,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    let write_stats = super::mp4::metadata::write_metadata_with_plan_stats(saved.path(), plan)?;
    verify_and_replace_temp(
        original,
        saved,
        &write_stats,
        expected_fingerprint,
        before_commit,
    )
}

fn commit_temp_mp4_with_plan(
    saved: SiblingTemp,
    original: &Path,
    plan: &MetadataWritePlan,
    mp4_plan: &Mp4PatchPlan,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    log_write_plan(plan, "mp4_native", &artifact_id(saved.path()));
    let write_stats = stage_with("metadata_mp4_tags", saved.path(), || {
        let stats = execute_patch_plan(saved.path(), mp4_plan)?;
        let fields = stats.log_fields();
        Ok((stats, fields))
    })?;
    verify_and_replace_temp(
        original,
        saved,
        &write_stats,
        expected_fingerprint,
        before_commit,
    )
}

fn verify_and_replace_temp(
    original: &Path,
    saved: SiblingTemp,
    write_stats: &WriteTagStats,
    expected_fingerprint: &str,
    before_commit: impl FnOnce(),
) -> Result<()> {
    before_commit();
    stage_with("metadata_verify_audio", original, || {
        let (mode, bytes_read) = if let Some((src_sha, out_sha)) = write_stats.streamed_mdat_hashes
        {
            (
                "mdat_sha256",
                verify_streaming_mp4(saved.path(), src_sha, out_sha)?,
            )
        } else {
            (
                "structural",
                verify_surgical_mp4(saved.path(), &write_stats.mdat_ranges)?,
            )
        };
        Ok(((), format!("mode={mode} bytes_read={bytes_read}")))
    })?;
    let stat = std::fs::metadata(original).map_err(AppError::Io)?;
    validate_source_fingerprint(&stat, expected_fingerprint)?;
    stage_with("metadata_replace", original, || {
        let timings = saved.commit_timed(original).map_err(AppError::Io)?;
        Ok(((), timings.log_fields()))
    })
}

#[cfg(test)]
#[path = "write_strategy_tests.rs"]
mod tests;
