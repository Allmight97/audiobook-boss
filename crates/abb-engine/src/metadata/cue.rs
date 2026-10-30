use super::{parse_cue, validate_chapters, ChapterSpec, CueInterpretation};
use crate::errors::{AppError, Result};
use abb_metadata_core::normalize_embedded_chapters;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterPlan {
    pub chapters: Vec<ChapterSpec>,
    pub from_cue: bool,
    pub source_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum CueStatus {
    Ready,
    NeedsConfirmation,
    Invalid,
    Ignored,
    EmbeddedPreferred,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CueSource {
    pub file_name: String,
    pub status: CueStatus,
    pub message: String,
}

fn source_fingerprint_from_metadata(stat: &std::fs::Metadata) -> Result<String> {
    let modified = stat
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AppError::InvalidInput("Audio modification time is invalid".into()))?;
    Ok(format!("{}:{}", stat.len(), modified.as_nanos()))
}

pub(crate) fn source_fingerprint(path: &Path) -> Result<String> {
    let stat = std::fs::metadata(path)?;
    source_fingerprint_from_metadata(&stat)
}

pub(crate) fn validate_source_fingerprint(stat: &std::fs::Metadata, expected: &str) -> Result<()> {
    if source_fingerprint_from_metadata(stat)? != expected {
        return Err(AppError::InvalidInput(
            "Audio changed since inspection. Remove and import it again.".into(),
        ));
    }
    Ok(())
}

/// Repairs `embedded` in place, once, so every route (an ignored CUE
/// included) writes and verifies the same accepted chapters. A
/// `length_estimated` duration is no audio end for that repair.
pub(crate) fn inspect_chapter_source(
    path: &Path,
    duration_ms: i64,
    length_estimated: bool,
    embedded: &mut Vec<ChapterSpec>,
) -> Result<(ChapterPlan, Option<CueSource>)> {
    *embedded = normalize_embedded_chapters(embedded, (!length_estimated).then_some(duration_ms))?;
    let mut plan = ChapterPlan {
        chapters: embedded.clone(),
        from_cue: false,
        source_fingerprint: source_fingerprint(path)?,
    };
    if !path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mp3"))
    {
        return Ok((plan, None));
    }
    let parent = path
        .parent()
        .ok_or_else(|| AppError::InvalidInput("Audio has no parent directory".into()))?;
    let mut matches = Vec::new();
    for entry in std::fs::read_dir(parent)? {
        let candidate = entry?.path();
        if candidate
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("cue"))
            && candidate.file_stem() == path.file_stem()
        {
            matches.push(candidate);
        }
    }
    if matches.is_empty() {
        return Ok((plan, None));
    }
    let mut cue = CueSource {
        file_name: matches[0]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        status: CueStatus::Invalid,
        message: String::new(),
    };
    if !embedded.is_empty() {
        cue.status = CueStatus::EmbeddedPreferred;
        cue.message = "Using embedded chapters; sibling CUE was not applied.".into();
    } else if matches.len() != 1 {
        cue.message =
            "Multiple same-stem CUE files found. Keep one sidecar or explicitly ignore CUE.".into();
    } else {
        match read_cue(&matches[0], parent, duration_ms) {
            Ok(sheet) => {
                cue.status = match sheet.interpretation {
                    CueInterpretation::Frames75 => CueStatus::Ready,
                    CueInterpretation::Hundredths => CueStatus::NeedsConfirmation,
                };
                if path.file_name() != Some(std::ffi::OsStr::new(&sheet.file_name)) {
                    cue.message = format!(
                        "CUE FILE names ‘{}’; using the selected same-stem MP3.",
                        sheet.file_name
                    );
                }
                plan.chapters = sheet.chapters;
                plan.from_cue = true;
            }
            Err(error) => cue.message = error.to_string(),
        }
    }
    Ok((plan, Some(cue)))
}

fn read_cue(path: &Path, parent: &Path, duration_ms: i64) -> Result<super::CueSheet> {
    if path.canonicalize()?.parent() != Some(parent) {
        return Err(AppError::InvalidInput(
            "CUE sidecar must remain in the audio directory".into(),
        ));
    }
    if !std::fs::metadata(path)?.is_file() {
        return Err(AppError::InvalidInput(
            "CUE sidecar is not a regular file".into(),
        ));
    }
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err(AppError::InvalidInput(
            "CUE sidecar is not a regular file".into(),
        ));
    }
    let mut text = String::new();
    file.take(1_048_577).read_to_string(&mut text).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            AppError::InvalidInput("CUE text must be UTF-8. Save the sidecar as UTF-8 and import the MP3 again, or ignore CUE.".into())
        } else {
            error.into()
        }
    })?;
    if text.len() > 1_048_576 {
        return Err(AppError::InvalidInput(
            "CUE exceeds the 1 MiB text limit".into(),
        ));
    }
    Ok(parse_cue(&text, duration_ms)?)
}

pub(crate) fn validate_chapter_plan(
    path: &Path,
    duration_ms: i64,
    plan: &ChapterPlan,
) -> Result<()> {
    if source_fingerprint(path)? != plan.source_fingerprint {
        return Err(AppError::InvalidInput(
            "Audio changed since chapter inspection. Remove and import it again.".into(),
        ));
    }
    validate_chapters(&plan.chapters, duration_ms)?;
    Ok(())
}

#[cfg(test)]
#[path = "cue_tests.rs"]
mod tests;
