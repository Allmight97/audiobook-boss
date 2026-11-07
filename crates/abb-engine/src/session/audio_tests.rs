use super::*;
use crate::audio::{AudioPlanField, ChannelConfig, SampleRateConfig};
use crate::processing::AudioHandling;
use crate::session::audio_choice::AudioChoice;

fn title(choice: AudioChoice, plan: TitlePlan) -> TitleAudio {
    let view = AudioChoiceView {
        facts: choice.facts(None),
        request: choice.request(None),
        choice,
    };
    TitleAudio::new(view, plan, None)
}

fn resolved(sample_rate: u32, channels: u8) -> TitlePlan {
    TitlePlan::Resolved {
        plan: crate::audio::TitleAudioPlan {
            format: AudiobookFormat::M4b,
            handling: AudioHandling::Preserve,
            sample_rate,
            channels,
            source_codec: "aac".to_string(),
            reason: None,
        },
    }
}

fn on_auto(plan: TitlePlan) -> TitleAudio {
    title(AudioChoice::default(), plan)
}

fn at_rate(hz: u32, plan: TitlePlan) -> TitleAudio {
    let choice = AudioChoice {
        sample_rate: SampleRateConfig::Explicit(hz),
        ..AudioChoice::default()
    };
    title(choice, plan)
}

fn select(titles: &[&TitleAudio]) -> SelectionAudio {
    let ids: Vec<String> = (0..titles.len()).map(|index| index.to_string()).collect();
    let pairs: Vec<_> = ids.iter().zip(titles.iter().copied()).collect();
    selection_audio(&pairs).expect("a selection")
}

#[test]
fn auto_reads_one_value_when_every_title_resolves_the_same() {
    let a = on_auto(resolved(44_100, 2));
    let b = on_auto(resolved(44_100, 1));

    let selection = select(&[&a, &b]);
    assert_eq!(
        selection.auto_sample_rate.outcome,
        AutoOutcome::Value { value: 44_100 }
    );
    assert_eq!(selection.auto_channels.outcome, AutoOutcome::Differs);
}

#[test]
fn auto_differs_when_titles_resolve_apart_and_is_unresolved_while_any_title_waits() {
    let a = on_auto(resolved(44_100, 2));
    let b = on_auto(resolved(48_000, 2));
    let waiting = on_auto(TitlePlan::Pending);

    assert_eq!(
        select(&[&a, &b]).auto_sample_rate.outcome,
        AutoOutcome::Differs
    );
    assert_eq!(
        select(&[&a, &waiting]).auto_sample_rate.outcome,
        AutoOutcome::Unresolved
    );
}

#[test]
fn a_mixed_field_reads_plain_auto_in_either_selection_order() {
    let auto = on_auto(resolved(44_100, 2));
    let explicit = at_rate(48_000, resolved(48_000, 2));

    for selection in [select(&[&auto, &explicit]), select(&[&explicit, &auto])] {
        assert!(selection.mixed.contains(&AudioField::SampleRate));
        assert_eq!(selection.auto_sample_rate, AutoReading::default());
    }
}

#[test]
fn a_field_that_differs_only_in_a_later_title_is_mixed() {
    let auto = on_auto(resolved(44_100, 2));
    let also_auto = on_auto(resolved(44_100, 2));
    let explicit = at_rate(48_000, resolved(48_000, 2));

    let selection = select(&[&auto, &also_auto, &explicit]);

    assert_eq!(selection.mixed, [AudioField::SampleRate]);
    assert_eq!(selection.auto_sample_rate, AutoReading::default());
}

#[test]
fn an_explicit_field_has_no_auto_value() {
    let a = at_rate(48_000, resolved(48_000, 2));
    let b = at_rate(48_000, resolved(48_000, 2));

    assert_eq!(
        select(&[&a, &b]).auto_sample_rate.outcome,
        AutoOutcome::Unresolved
    );
}

#[test]
fn a_plan_problem_is_reported_whether_or_not_the_field_is_mixed() {
    let failed = |field| TitlePlan::Failed {
        message: "48000 Hz is not available".to_string(),
        field: Some(field),
    };
    let broken = at_rate(48_000, failed(AudioPlanField::SampleRate));
    let fine = at_rate(48_000, resolved(48_000, 2));

    let selection = select(&[&fine, &broken]);
    assert_eq!(
        selection.auto_sample_rate.problem.as_deref(),
        Some("48000 Hz is not available")
    );
    assert_eq!(selection.auto_channels.problem, None);

    let mixed = at_rate(44_100, failed(AudioPlanField::SampleRate));
    let reading = select(&[&broken, &mixed]).auto_sample_rate;
    assert_eq!(reading.outcome, AutoOutcome::Unresolved);
    assert_eq!(
        reading.problem.as_deref(),
        Some("48000 Hz is not available")
    );
    let stereo = AudioChoice {
        channels: ChannelConfig::Stereo,
        ..AudioChoice::default()
    };
    let channel_problem = title(stereo, failed(AudioPlanField::Channels));
    assert!(select(&[&channel_problem]).auto_channels.problem.is_some());
}

#[test]
fn a_title_reads_its_own_auto_values_and_problem() {
    let reads = on_auto(resolved(44_100, 1));
    assert_eq!(
        reads.auto_sample_rate.outcome,
        AutoOutcome::Value { value: 44_100 }
    );
    assert_eq!(reads.auto_channels.outcome, AutoOutcome::Value { value: 1 });

    let explicit = at_rate(48_000, resolved(48_000, 2));
    assert_eq!(explicit.auto_sample_rate, AutoReading::default());
    assert_eq!(
        explicit.auto_channels.outcome,
        AutoOutcome::Value { value: 2 }
    );

    let failed = on_auto(TitlePlan::Failed {
        message: "48000 Hz is not available".to_string(),
        field: Some(AudioPlanField::SampleRate),
    });
    assert_eq!(
        failed.auto_sample_rate.problem.as_deref(),
        Some("48000 Hz is not available")
    );
    assert_eq!(failed.auto_channels, AutoReading::default());
}
