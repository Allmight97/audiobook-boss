use super::*;
use crate::metadata::{ChapterPlan, CueSource, CueStatus};
use std::path::PathBuf;

fn source(name: &str) -> AudioFile {
    let mut file = AudioFile::new(PathBuf::from(format!("/books/{name}.mp3")));
    file.is_valid = true;
    file
}

fn with_cue_chapters(mut file: AudioFile) -> AudioFile {
    file.chapter_plan = Some(ChapterPlan {
        chapters: Vec::new(),
        from_cue: true,
        source_fingerprint: "fingerprint".to_string(),
    });
    file
}

fn with_cue_status(mut file: AudioFile, status: CueStatus) -> AudioFile {
    file.cue_source = Some(CueSource::new(
        "alpha.cue".to_string(),
        status,
        String::new(),
    ));
    file
}

fn refusal(files: &[AudioFile]) -> String {
    match FileListInfo::check_cue_chapters(files).expect_err("refused") {
        AppError::InvalidInput(message) => message,
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_merged_title_cannot_carry_cue_chapters() {
    let files = [with_cue_chapters(source("one")), source("two")];
    assert_eq!(
        refusal(&files),
        "Merging CUE-bearing inputs is not supported. Separate the titles or ignore CUE chapters."
    );
}

#[test]
fn a_cue_waiting_for_review_names_its_file_in_every_unreviewed_state() {
    for status in [CueStatus::NeedsConfirmation, CueStatus::Invalid] {
        let files = [with_cue_status(source("one"), status)];
        assert_eq!(
            refusal(&files),
            "Review alpha.cue: confirm its timestamp interpretation or ignore the CUE before converting."
        );
    }
}

#[test]
fn a_single_source_title_may_carry_cue_chapters_and_a_merge_without_cue_passes() {
    FileListInfo::check_cue_chapters(&[with_cue_chapters(source("one"))])
        .expect("one source keeps its CUE chapters");
    FileListInfo::check_cue_chapters(&[source("one"), source("two")])
        .expect("sources without a CUE merge");
}

#[test]
fn the_payload_check_for_a_cue_without_an_accepted_plan_uses_the_same_wording() {
    let mut info = FileListInfo::from_files(vec![with_cue_status(source("one"), CueStatus::Ready)]);
    let error = apply_chapter_plans(&mut info, None).expect_err("an unreviewed sidecar");
    assert_eq!(
        error.to_string(),
        "Invalid input: Review alpha.cue: confirm its timestamp interpretation or ignore the CUE before converting."
    );
}
