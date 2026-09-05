//! Sibling CUE discovery and immutable, source-bound chapter intake facts.
use super::{parse_cue, validate_chapters, ChapterSpec, CueInterpretation};
use crate::errors::{AppError, Result};
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

pub(crate) fn source_fingerprint(path: &Path) -> Result<String> {
    let stat = std::fs::metadata(path)?;
    let modified = stat
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AppError::InvalidInput("Audio modification time is invalid".into()))?;
    Ok(format!("{}:{}", stat.len(), modified.as_nanos()))
}

pub(crate) fn inspect_chapter_source(
    path: &Path,
    duration_ms: i64,
    embedded: &[ChapterSpec],
) -> Result<(ChapterPlan, Option<CueSource>)> {
    let mut plan = ChapterPlan {
        chapters: embedded.to_vec(),
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
mod tests {
    use super::*;
    #[test]
    fn sibling_cue_association_diagnostics_and_accepted_plan_isolation() {
        let tmp = tempfile::TempDir::new().expect("intake tempdir");
        let dir = tmp.path().canonicalize().expect("canonical fixture parent");
        let one = dir.join("one.mp3");
        let two = dir.join("two.mp3");
        std::fs::write(&one, b"one").expect("first audio identity");
        std::fs::write(&two, b"two").expect("second audio identity");
        std::fs::write(
            dir.join("one.cue"),
            "FILE \"stale.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"First\"\nINDEX 01 00:00:94",
        )
        .expect("nonstandard sidecar");
        std::fs::write(
            dir.join("two.cue"),
            "FILE \"two.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Second\"\nINDEX 01 00:01:00",
        )
        .expect("standard sidecar");
        let (first, cue) = inspect_chapter_source(&one, 2000, &[]).expect("first intake");
        let cue = cue.expect("first sidecar");
        assert_eq!(cue.status, CueStatus::NeedsConfirmation);
        assert!(cue.message.contains("stale.mp3"));
        let (second, cue) = inspect_chapter_source(&two, 2000, &[]).expect("second intake");
        assert_eq!(cue.expect("standard sidecar").status, CueStatus::Ready);
        std::fs::write(dir.join("one.cue"), "FILE \"../unsafe.mp3\" MP3")
            .expect("change sidecar after intake");
        let files = [&one, &two].map(|path| {
            let mut file = crate::audio::AudioFile::new(path.clone());
            file.is_valid = true;
            file.duration = Some(2.0);
            file
        });
        let mut inputs = crate::audio::FileListInfo {
            files: files.to_vec(),
            selected_decoders: vec![None, None],
            total_duration: 4.0,
            total_size: 6.0,
            valid_count: 2,
            invalid_count: 0,
        };
        let accepted = std::collections::HashMap::from([
            (one.to_string_lossy().into_owned(), first.clone()),
            (two.to_string_lossy().into_owned(), second.clone()),
        ]);
        crate::audio::apply_chapter_plans(&mut inputs, Some(&accepted), false)
            .expect("job handoff retains accepted data despite changed CUE");
        assert_eq!(inputs.files[0].chapter_plan.as_ref(), Some(&first));
        assert_eq!(inputs.files[1].chapter_plan.as_ref(), Some(&second));
        assert_eq!(first.chapters[0].start_ms, 940);
        assert_eq!(second.chapters[0].start_ms, 1000);
        assert!(crate::audio::apply_chapter_plans(&mut inputs, Some(&accepted), true).is_err());
        assert_eq!(
            inspect_chapter_source(&one, 2000, &[])
                .expect("invalid diagnostic")
                .1
                .expect("sidecar")
                .status,
            CueStatus::Invalid
        );
        std::fs::write(&one, b"changed audio").expect("mutate audio");
        assert!(validate_chapter_plan(&one, 2000, &first).is_err());
        std::fs::write(dir.join("two.CUE"), "duplicate").expect("ambiguous sidecar");
        // On case-sensitive volumes these are two candidates; on insensitive
        // volumes the second write changes the single candidate to invalid text.
        assert_eq!(
            inspect_chapter_source(&two, 2000, &[])
                .expect("ambiguous diagnostic")
                .1
                .expect("sidecar")
                .status,
            CueStatus::Invalid
        );
        assert_eq!(
            inspect_chapter_source(&two, 2000, &second.chapters)
                .expect("embedded precedence")
                .1
                .expect("sidecar")
                .status,
            CueStatus::EmbeddedPreferred
        );
    }
}
