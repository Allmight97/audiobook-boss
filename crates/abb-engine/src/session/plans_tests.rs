//! Title plan, chapter plan, and size estimate rules.

use std::path::PathBuf;

use super::*;
use crate::audio::{AudiobookFormat, EncoderSettings, SampleRateConfig};
use crate::metadata::CueSource;

fn source(name: &str, seconds: f64, bytes: f64) -> AudioFile {
    let mut file = AudioFile::new(PathBuf::from(format!("/books/{name}.m4b")));
    file.input_id = name.to_string();
    file.is_valid = true;
    file.duration = Some(seconds);
    file.size = Some(bytes);
    file
}

fn request(intent: AudioIntent, settings: Option<EncoderSettings>) -> TitleAudioRequest {
    TitleAudioRequest {
        format: AudiobookFormat::M4b,
        intent,
        settings,
        sample_rate: SampleRateConfig::Auto,
    }
}

fn cbr(kbps: u16) -> EncoderSettings {
    EncoderSettings {
        bitrate_kbps: kbps,
        ..EncoderSettings::default()
    }
}

fn chapter_plan(from_cue: bool) -> ChapterPlan {
    ChapterPlan {
        chapters: Vec::new(),
        from_cue,
        source_fingerprint: "fingerprint".to_string(),
    }
}

#[test]
fn a_cue_sheet_waiting_for_confirmation_refuses_the_title() {
    let mut file = source("alpha", 60.0, 1.0);
    file.cue_source = Some(CueSource {
        file_name: "alpha.cue".to_string(),
        status: CueStatus::NeedsConfirmation,
        message: String::new(),
    });

    let error = chapter_plans_for(&[file]).expect_err("refused");
    assert!(error.contains("alpha.cue"), "{error}");
}

#[test]
fn a_merged_title_cannot_carry_cue_chapters() {
    let mut first = source("one", 60.0, 1.0);
    first.chapter_plan = Some(chapter_plan(true));
    let second = source("two", 60.0, 1.0);

    assert!(chapter_plans_for(&[first.clone(), second]).is_err());
    // Alone, the same source carries its plan.
    assert_eq!(chapter_plans_for(&[first]).expect("plans").len(), 1);
}

fn input<'a>(request: &'a TitleAudioRequest, file: &AudioFile) -> PlanInput<'a> {
    PlanInput {
        title_id: "alpha",
        request,
        sources: vec![file.clone()],
        choice_required: false,
    }
}

#[test]
fn a_plan_is_resolved_again_only_when_its_request_or_sources_change() {
    let mut plans = Plans::default();
    let alpha = source("alpha", 60.0, 1.0);
    let encode = &request(AudioIntent::Encode, Some(cbr(64)));

    let tickets = plans.refresh([input(encode, &alpha)]);
    assert_eq!(tickets.len(), 1);
    assert_eq!(plans.plan("alpha"), TitlePlan::Pending);
    plans.finish(&tickets[0], Err("no".to_string().into()));
    assert!(plans.refresh([input(encode, &alpha)]).is_empty());

    // A changed request makes the earlier ticket stale.
    let preserve = &request(AudioIntent::Preserve, None);
    let stale = tickets[0].clone();
    let fresh = plans.refresh([input(preserve, &alpha)]);
    plans.finish(&stale, Err("late".to_string().into()));
    assert_eq!(plans.plan("alpha"), TitlePlan::Pending);
    plans.finish(&fresh[0], Err("current".to_string().into()));
    assert_eq!(
        plans.plan("alpha"),
        TitlePlan::Failed {
            message: "current".to_string(),
            field: None,
        }
    );
    // A removed title has no plan.
    assert!(plans.refresh([]).is_empty());
    assert!(plans.all().is_empty());
}

#[test]
fn kept_audio_is_estimated_from_source_sizes_and_encoded_audio_from_bitrate() {
    let sources = [source("one", 100.0, 1_000.0), source("two", 50.0, 2_000.0)];
    let preserve = request(AudioIntent::Preserve, None);
    let encode = request(AudioIntent::Encode, Some(cbr(64)));

    assert_eq!(
        estimate_size(&preserve, &TitlePlan::Pending, &sources, None),
        Some(SizeEstimate::Bytes { bytes: 3_000 })
    );
    // 150 s at 64 kbps plus 3 percent.
    assert_eq!(
        estimate_size(&encode, &TitlePlan::Pending, &sources, Some(64)),
        Some(SizeEstimate::Bytes { bytes: 1_236_000 })
    );
    assert_eq!(
        estimate_size(&encode, &TitlePlan::Pending, &sources, None),
        Some(SizeEstimate::VariesWithAudio)
    );
}

#[test]
fn auto_waits_for_its_plan_and_a_missing_fact_means_no_estimate() {
    let sources = [source("one", 100.0, 1_000.0)];
    let auto = request(AudioIntent::Auto, Some(cbr(64)));
    assert_eq!(
        estimate_size(&auto, &TitlePlan::Pending, &sources, Some(64)),
        None
    );

    let kept = TitlePlan::Resolved {
        plan: TitleAudioPlan {
            format: AudiobookFormat::M4b,
            handling: AudioHandling::Preserve,
            settings: None,
            sample_rate: 44_100,
            channels: 2,
            source_codec: "aac".to_string(),
            reason: None,
        },
    };
    assert_eq!(
        estimate_size(&auto, &kept, &sources, Some(64)),
        Some(SizeEstimate::Bytes { bytes: 1_000 })
    );

    let mut unknown = source("one", 100.0, 1_000.0);
    unknown.size = None;
    assert_eq!(estimate_size(&auto, &kept, &[unknown], Some(64)), None);
}

#[test]
fn a_failure_one_audio_setting_causes_names_that_setting() {
    let resolve = |file: &AudioFile| {
        let encode = request(AudioIntent::Encode, Some(cbr(64)));
        let mut plans = Plans::default();
        let ticket = plans.refresh([input(&encode, file)]).remove(0);
        ticket.resolve().expect_err("refused")
    };
    let mut surround = source("alpha", 60.0, 1.0);
    surround.sample_rate = Some(44_100);
    surround.channels = Some(6);
    let failure = resolve(&surround);
    assert_eq!(failure.field, Some(AudioPlanField::Channels));
    assert!(failure.message.starts_with("'"), "{}", failure.message);

    let mut unknown_rate = source("alpha", 60.0, 1.0);
    unknown_rate.channels = Some(2);
    assert_eq!(
        resolve(&unknown_rate).field,
        Some(AudioPlanField::SampleRate)
    );

    // A failure no single setting causes names none.
    let mut cue = unknown_rate.clone();
    cue.cue_source = Some(CueSource {
        file_name: "alpha.cue".to_string(),
        status: CueStatus::NeedsConfirmation,
        message: String::new(),
    });
    assert_eq!(resolve(&cue).field, None);
}
