use std::path::PathBuf;

use super::*;
use crate::audio::{AudioIntent, TitleAudioRequest};
use crate::metadata::{ChapterPlan, CueSource, CueStatus, PatchOp};
use crate::output_artifact::{NamingPreset, OutputNamingConfig};

fn file(name: &str, valid: bool) -> AudioFile {
    let mut file = AudioFile::new(PathBuf::from(format!("/books/{name}.m4b")));
    file.input_id = name.to_string();
    file.is_valid = valid;
    file
}

fn request() -> TitleAudioRequest {
    TitleAudioRequest::default_m4b(AudioIntent::Auto)
}

fn title<'a>(anchor: &'a AudioFile, sources: &'a [AudioFile]) -> SubmittedTitle<'a> {
    SubmittedTitle {
        anchor,
        sources,
        request: request(),
        choice_required: false,
        has_title: true,
    }
}

fn inputs() -> DraftInputs {
    DraftInputs {
        output_directory: Some("/library".to_string()),
        naming: OutputNamingConfig {
            preset: NamingPreset::AbsDefault,
            include_year: false,
            custom_template: None,
        },
        supplemental_assets: None,
        preview: false,
    }
}

fn no_edits(_: &[String]) -> HashMap<String, MetadataIntentPatch> {
    HashMap::new()
}

#[test]
fn invalid_standalone_titles_are_left_out_and_the_rest_are_sent_in_order() {
    let alpha = file("alpha", true);
    let broken = file("broken", false);
    let beta = file("beta", true);
    let titles = [
        title(&alpha, std::slice::from_ref(&alpha)),
        title(&broken, std::slice::from_ref(&broken)),
        title(&beta, std::slice::from_ref(&beta)),
    ];

    let draft = build_draft(&titles, inputs(), no_edits, title_label).expect("draft");

    assert_eq!(
        draft.payload.input_files,
        ["/books/alpha.m4b", "/books/beta.m4b"]
    );
    assert_eq!(draft.payload.audio_requests.len(), 2);
    assert_eq!(draft.title, "alpha.m4b + 1 more");
    assert_eq!(draft.sources.len(), 2);
    assert_eq!(draft.left_out, 1);
}

#[test]
fn a_grouped_title_sends_its_ordered_sources_and_refuses_an_invalid_one() {
    let first = file("part1", true);
    let second = file("part2", true);
    let sources = [second.clone(), first.clone()];
    let titles = [title(&first, &sources)];

    let draft = build_draft(&titles, inputs(), no_edits, title_label).expect("draft");
    let grouped = &draft.payload.title_sources.as_ref().expect("sources")["/books/part1.m4b"];
    assert_eq!(
        grouped
            .iter()
            .map(|source| source.path.as_str())
            .collect::<Vec<_>>(),
        ["/books/part2.m4b", "/books/part1.m4b"]
    );

    let broken = [first.clone(), file("broken", false)];
    let refused = build_draft(&[title(&first, &broken)], inputs(), no_edits, title_label);
    assert_eq!(refused.err(), Some(SubmitRefusal::InvalidSource));

    let anchor = file("broken", false);
    let led_by_broken = [anchor.clone(), first.clone()];
    let other = file("other", true);
    let refused = build_draft(
        &[
            title(&anchor, &led_by_broken),
            title(&other, std::slice::from_ref(&other)),
        ],
        inputs(),
        no_edits,
        title_label,
    );
    assert_eq!(refused.err(), Some(SubmitRefusal::InvalidSource));
}

#[test]
fn a_submission_refuses_what_it_cannot_send() {
    let alpha = file("alpha", true);
    let one = [alpha.clone()];
    let mut choosing = title(&alpha, &one);
    choosing.choice_required = true;
    assert_eq!(
        build_draft(&[choosing], inputs(), no_edits, title_label).err(),
        Some(SubmitRefusal::AudioChoiceRequired)
    );
    assert_eq!(
        build_draft(
            &[title(&alpha, &one)],
            DraftInputs {
                output_directory: None,
                ..inputs()
            },
            no_edits,
            title_label
        )
        .err(),
        Some(SubmitRefusal::NoOutputDirectory)
    );
    assert_eq!(
        build_draft(&[], inputs(), no_edits, title_label).err(),
        Some(SubmitRefusal::NoTitles)
    );
    let broken = file("broken", false);
    assert_eq!(
        build_draft(
            &[title(&broken, std::slice::from_ref(&broken))],
            inputs(),
            no_edits,
            title_label
        )
        .err(),
        Some(SubmitRefusal::NoValidTitles)
    );

    let mut cue = alpha.clone();
    cue.cue_source = Some(CueSource {
        file_name: "alpha.cue".to_string(),
        status: CueStatus::NeedsConfirmation,
        message: String::new(),
    });
    let refused = build_draft(
        &[title(&cue, std::slice::from_ref(&cue))],
        inputs(),
        no_edits,
        title_label,
    );
    assert!(matches!(refused, Err(SubmitRefusal::ChapterReview { .. })));
}

#[test]
fn the_operation_is_named_after_the_edited_title_and_carries_pending_edits() {
    let mut alpha = file("alpha", true);
    alpha.set_tag_title(Some("Tagged".to_string()));
    alpha.chapter_plan = Some(ChapterPlan {
        chapters: Vec::new(),
        from_cue: false,
        source_fingerprint: "fingerprint".to_string(),
    });
    let one = [alpha.clone()];
    let edits = |paths: &[String]| {
        paths
            .iter()
            .map(|path| {
                (
                    path.clone(),
                    MetadataIntentPatch {
                        title: Some(PatchOp::Set("Dune".to_string())),
                        ..Default::default()
                    },
                )
            })
            .collect()
    };

    let draft = build_draft(&[title(&alpha, &one)], inputs(), edits, title_label).expect("draft");

    assert_eq!(draft.title, "Dune");
    assert!(draft
        .metadata
        .expect("edits")
        .contains_key("/books/alpha.m4b"));
    assert_eq!(draft.payload.chapter_plans.expect("plans").len(), 1);
    assert_eq!(title_label(&alpha), "Tagged");
}

fn removed(name: &str) -> AudioFile {
    let mut file = file(name, false);
    file.source_removed = true;
    file
}

#[test]
fn a_finished_grouped_title_is_left_out_of_both_start_and_preview() {
    let part1 = removed("part1");
    let part2 = removed("part2");
    let finished = [part1.clone(), part2];
    let next = file("next", true);

    for preview in [false, true] {
        let draft = build_draft(
            &[
                title(&part1, &finished),
                title(&next, std::slice::from_ref(&next)),
            ],
            DraftInputs {
                preview,
                ..inputs()
            },
            no_edits,
            title_label,
        )
        .expect("draft");

        assert_eq!(draft.payload.input_files, ["/books/next.m4b"]);
        assert_eq!(draft.sources, std::slice::from_ref(&next.path));
        assert_eq!(draft.left_out, 1);
    }
}

#[test]
fn a_group_with_one_removed_and_one_valid_source_is_history() {
    let part1 = removed("part1");
    let part2 = file("part2", true);
    let mixed = [part1.clone(), part2];
    let next = file("next", true);

    let draft = build_draft(
        &[
            title(&part1, &mixed),
            title(&next, std::slice::from_ref(&next)),
        ],
        inputs(),
        no_edits,
        title_label,
    )
    .expect("draft");

    assert_eq!(draft.payload.input_files, ["/books/next.m4b"]);
    assert_eq!(draft.left_out, 1);
}

#[test]
fn only_finished_titles_leave_nothing_to_submit() {
    let part1 = removed("part1");
    let finished = [part1.clone(), removed("part2")];

    assert_eq!(
        build_draft(&[title(&part1, &finished)], inputs(), no_edits, title_label).err(),
        Some(SubmitRefusal::NoValidTitles)
    );
}

#[test]
fn a_group_with_a_failed_source_that_was_not_removed_still_refuses() {
    let part1 = file("part1", true);
    let failed = file("part2", false);
    let broken = [part1.clone(), failed];
    let next = file("next", true);

    let refused = build_draft(
        &[
            title(&part1, &broken),
            title(&next, std::slice::from_ref(&next)),
        ],
        inputs(),
        no_edits,
        title_label,
    );

    assert_eq!(refused.err(), Some(SubmitRefusal::InvalidSource));
}
