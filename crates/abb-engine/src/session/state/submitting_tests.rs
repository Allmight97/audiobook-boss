use super::*;
use crate::output_artifact::OutputNamingConfig;
use crate::processing::title_output::UpdateReply;
use crate::processing::title_output::{TitleOutput, TitleOutputPlan};
use crate::work_runtime::OperationId;
use std::sync::Arc;

#[test]
fn an_export_needs_a_title_from_its_tag_or_the_form() {
    let missing = |desk: &mut Desk| {
        assert!(desk.state.begin_submission(false).is_none());
        assert_eq!(
            desk.state.output_snapshot(0).submission,
            Some(SubmissionStatus::Refused {
                reason: SubmitRefusal::MissingTitle {
                    title_id: "alpha".to_string(),
                    label: "alpha.m4b".to_string(),
                }
            })
        );
    };
    let mut untagged = Desk::open(&[("alpha", None)], &[0]);
    untagged.state.output.set_directory("/library".to_string());
    missing(&mut untagged);
    untagged.type_into(MetadataField::Title, "Typed");
    assert!(untagged.state.begin_submission(false).is_some());

    let mut blanked = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    blanked.state.output.set_directory("/library".to_string());
    blanked.act(MetadataField::Title, FieldAction::Blank);
    assert!(blanked.state.begin_submission(false).is_none());
    assert!(matches!(
        blanked.state.output_snapshot(0).submission,
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::MissingTitle { .. }
        })
    ));
    assert!(
        blanked.state.begin_submission(true).is_some(),
        "a preview needs no title"
    );
}

#[test]
fn a_refused_second_submission_does_not_let_a_third_through() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    let first = desk
        .state
        .begin_submission(false)
        .expect("first submission");
    desk.state.await_review(first, Vec::new());
    let review = desk
        .state
        .output_snapshot(0)
        .collision_review
        .expect("held question");

    for _ in 0..2 {
        assert!(desk.state.begin_submission(false).is_none());
        assert_eq!(
            desk.state.output_snapshot(0).submission,
            Some(SubmissionStatus::Refused {
                reason: SubmitRefusal::Busy
            })
        );
    }

    let snapshot = desk.state.output_snapshot(0);
    assert_eq!(snapshot.collision_review, Some(review.clone()));
    assert!(snapshot.submission_in_progress);
    assert!(desk.state.cancel_review_named(review.review_id));
    assert!(!desk.state.working_set.order_locked());
    assert!(desk.state.begin_submission(false).is_some());
}

#[test]
fn collision_answers_are_scoped_to_the_question_even_after_rereview_cancel_and_clear_all() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".into());
    let draft = desk.state.begin_submission(false).expect("submission");
    desk.state.await_review(draft, Vec::new());
    let old = desk
        .state
        .output_snapshot(0)
        .collision_review
        .expect("review")
        .review_id;
    let draft = desk.state.take_review(old).expect("named approval");
    assert!(desk.state.take_review(old).is_none());
    desk.state.await_review(draft, Vec::new());
    let next = desk
        .state
        .output_snapshot(0)
        .collision_review
        .expect("rereview")
        .review_id;
    assert_ne!(old, next);
    assert!(desk.state.take_review(old).is_none());
    assert!(!desk.state.cancel_review_named(old));
    assert!(desk.state.working_set.order_locked());

    assert!(desk.state.cancel_review_named(next));
    desk.change(|set| {
        set.clear_all();
    })
    .expect("clear all");
    assert!(desk.state.reserved.is_empty());
    assert!(!desk.state.output_snapshot(0).submission_in_progress);
    let request = desk.state.audio.request();
    desk.state.working_set.append_analyzed(
        vec![{
            let mut file = audio_file("beta", true);
            file.set_tag_title(Some("Beta".to_string()));
            file
        }],
        &request,
    );
    let draft = desk
        .state
        .begin_submission(false)
        .expect("new session submission");
    desk.state.await_review(draft, Vec::new());
    let current = desk
        .state
        .output_snapshot(0)
        .collision_review
        .expect("new review")
        .review_id;
    assert_ne!(current, next);
    assert!(!desk.state.cancel_review_named(next));
    assert!(desk.state.take_review(next).is_none());
    assert_eq!(
        desk.state
            .output_snapshot(0)
            .collision_review
            .expect("still held")
            .review_id,
        current
    );
    assert!(desk.state.cancel_review_named(current));
    assert!(!desk.state.cancel_review_named(current));
    assert!(desk.state.reserved.is_empty());
}

fn link_both(desk: &mut Desk) -> Vec<Arc<TitleOutput>> {
    let outputs: Vec<_> = ["alpha", "beta"]
        .iter()
        .map(|name| {
            TitleOutput::with_writer(
                TitleOutputPlan {
                    anchor: path(name),
                    sources: Vec::new(),
                    base: crate::metadata::AudiobookMetadata::default(),
                    accepted: None,
                    output_dir: PathBuf::from("/library"),
                    naming: OutputNamingConfig::default(),
                    extension: "m4b".into(),
                    requested: PathBuf::from(format!("/library/{name}.m4b")),
                },
                Box::new(|_, _| Ok(())),
            )
            .expect("output")
        })
        .collect();
    desk.state.exports.link(
        &OperationId("operation".into()),
        [
            ("alpha".into(), path("alpha")),
            ("beta".into(), path("beta")),
        ],
        &outputs,
    );
    outputs
}

fn prompted_title(desk: &Desk) -> Option<String> {
    desk.state
        .output_snapshot(0)
        .restart_prompt
        .map(|offer| offer.title_id)
}

fn save_moves(desk: &mut Desk, titles: &[&str]) -> u64 {
    let only: Vec<String> = titles.iter().map(|title| title.to_string()).collect();
    let edits = desk.state.output_edits(Some(&only));
    let revision = edits[0].revision;
    desk.state.record_output_edits(
        edits
            .into_iter()
            .map(|edit| {
                let reply = UpdateReply::MovesOutput {
                    from: PathBuf::from("/library/old.m4b"),
                    to: PathBuf::from(format!("/library/{}.m4b", edit.title_id)),
                };
                (edit, Ok(reply))
            })
            .collect(),
    );
    revision
}

#[test]
fn output_edits_join_the_written_and_pending_edits_of_the_asked_titles() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    assert!(desk.state.output_edits(None).is_empty());
    link_both(&mut desk);
    desk.state.tags.stage(&path("alpha"), &genre_patch());

    let all = desk.state.output_edits(None);
    let only_beta = desk.state.output_edits(Some(&["beta".to_string()]));

    let genre_of = |edits: &[crate::session::exports::OutputEdit], title: &str| {
        edits
            .iter()
            .find(|edit| edit.title_id == title)
            .map(|edit| edit.intent.genre.clone())
    };
    assert_eq!(all.len(), 2);
    assert_eq!(genre_of(&all, "alpha"), Some(set("Mystery")));
    assert_eq!(genre_of(&all, "beta"), Some(None));
    assert_eq!(only_beta.len(), 1);
    assert_eq!(only_beta[0].title_id, "beta");
    assert!(only_beta[0].revision > all[0].revision);
}

#[test]
fn restart_questions_wait_for_answer_work_and_answered_offers_stay_retryable() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    desk.state.output.set_directory("/library".into());
    let outputs = link_both(&mut desk);
    let revision = save_moves(&mut desk, &["alpha", "beta"]);
    assert_eq!(prompted_title(&desk).as_deref(), Some("alpha"));

    let link = desk
        .state
        .begin_keep_location("alpha", revision)
        .expect("accepted answer");
    assert!(Arc::ptr_eq(&link.title, &outputs[0]));
    assert_eq!(prompted_title(&desk), None);
    desk.state.finish_keep_location();
    assert_eq!(prompted_title(&desk).as_deref(), Some("beta"));
    assert_eq!(desk.state.exports.offers().len(), 2);

    let (draft, _) = desk.state.begin_restart("beta", revision).expect("restart");
    assert_eq!(prompted_title(&desk), None);
    desk.state.await_review(draft, Vec::new());
    assert_eq!(prompted_title(&desk), None);
    desk.state.cancel_review();
    assert_eq!(prompted_title(&desk), None);
    assert_eq!(desk.state.exports.offers()[0].title_id, "alpha");

    save_moves(&mut desk, &["alpha"]);
    let prompt = desk
        .state
        .output_snapshot(0)
        .restart_prompt
        .expect("later Save asks again");
    assert!(prompt.revision > revision);
    desk.state.output.set_directory("/elsewhere".into());
    assert!(desk.state.output_snapshot(0).restart_prompt.is_none());
    assert!(desk.state.begin_keep_location("alpha", revision).is_none());
    assert_eq!(
        desk.state.output_snapshot(0).submission,
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::RestartStale
        })
    );
}

#[test]
fn a_finished_preview_ends_when_the_user_starts_the_next_submission() {
    let finished = || {
        let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
        desk.state.output.set_directory("/library".to_string());
        let draft = desk.state.begin_submission(true).expect("preview");
        desk.state.finish_submission(
            &draft,
            SubmissionStatus::Blocked {
                message: "No room".to_string(),
            },
        );
        assert!(desk.state.output_snapshot(0).preview_run.is_some());
        desk
    };

    let mut accepted = finished();
    assert!(accepted.state.begin_submission(false).is_some());
    assert_eq!(accepted.state.output_snapshot(0).preview_run, None);

    let mut refused = finished();
    refused.act(MetadataField::Title, FieldAction::Blank);
    assert!(refused.state.begin_submission(false).is_none());
    assert_eq!(refused.state.output_snapshot(0).preview_run, None);

    let mut stale_restart = finished();
    assert!(stale_restart.state.begin_restart("alpha", 0).is_none());
    assert_eq!(stale_restart.state.output_snapshot(0).preview_run, None);
}

#[test]
fn a_running_preview_survives_a_busy_refusal() {
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    let draft = desk.state.begin_submission(true).expect("preview");
    let id = draft.operation_id.clone();
    desk.state.start_preview(&id);

    assert!(desk.state.begin_submission(false).is_none());

    let output = desk.state.output_snapshot(0);
    assert_eq!(
        output.submission,
        Some(SubmissionStatus::Refused {
            reason: SubmitRefusal::Busy
        })
    );
    let preview = output.preview_run.expect("the preview still runs");
    assert_eq!(preview.operation.operation_id, id);
}

#[test]
fn a_swept_grouped_title_does_not_block_the_next_start_or_preview() {
    let mut desk = Desk::open(
        &[
            ("alpha", Some(alpha_tags())),
            ("beta", Some(beta_tags())),
            ("gamma", Some(alpha_tags())),
        ],
        &[0, 1],
    );
    desk.state.output.set_directory("/library".to_string());
    assert!(desk.state.working_set.group_selected());
    desk.state
        .working_set
        .sources_removed(&[path("alpha"), path("beta")]);
    desk.select(&[1]).expect("select gamma");

    for preview in [true, false] {
        let draft = desk
            .state
            .begin_submission(preview)
            .expect("the finished title is history");
        assert_eq!(draft.payload.input_files, ["/books/gamma.m4b"]);
        assert_eq!(draft.left_out, 1);
        desk.state
            .finish_submission(&draft, SubmissionStatus::Cancelled);
    }
}

#[test]
fn a_cancel_after_the_preview_returned_ends_it_cancelled_and_opens_nothing() {
    use crate::processing::{ProcessCommandResult, ProcessResultEntry, ProcessResultStatus};
    let mut desk = Desk::open(&[("alpha", Some(alpha_tags()))], &[0]);
    desk.state.output.set_directory("/library".to_string());
    let draft = desk.state.begin_submission(true).expect("preview");
    let id = draft.operation_id.clone();
    desk.state.start_preview(&id);
    let result = ProcessCommandResult::new(vec![ProcessResultEntry {
        input_index: 0,
        status: ProcessResultStatus::Success,
        message: String::new(),
        error: None,
        output_path: Some("/library/alpha.preview.m4b".to_string()),
        supplemental_warning: None,
        warnings: Vec::new(),
        job_id: None,
    }]);

    desk.state
        .preview
        .cancel(id.as_str(), None)
        .expect("cancel the preview");
    desk.state
        .finish_submission(&draft, SubmissionStatus::PreviewFinished { result });

    let preview = desk.state.output_snapshot(0).preview_run.expect("preview");
    assert!(preview.operation.cancel_requested);
    assert!(preview.operation.is_finished());
    assert!(!preview.open_ready);
    assert_eq!(desk.state.preview.take_output(id.as_str()), None);
}
