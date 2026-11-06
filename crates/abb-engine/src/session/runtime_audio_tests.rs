use super::*;
use crate::session::audio_choice::AudioEdit;

#[tokio::test]
async fn a_new_title_starts_from_the_default_audio_choice() {
    let rig = rig();
    rig.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Format(AudiobookFormat::MkaOpus),
    })
    .await;
    rig.load(&["alpha"]);

    let title = &audio(&rig).titles["alpha"];
    assert_eq!(title.choice.format, AudiobookFormat::MkaOpus);
}

#[tokio::test]
async fn title_audio_edits_change_only_the_named_titles_and_not_while_locked() {
    let rig = rig();
    rig.load(&["alpha", "beta"]);

    rig.send(SessionIntent::SetTitleAudio {
        title_ids: vec!["alpha".to_string()],
        edit: AudioEdit::Format(AudiobookFormat::M4aOpus),
    })
    .await;
    let titles = audio(&rig).titles;
    assert_eq!(titles["alpha"].choice.format, AudiobookFormat::M4aOpus);
    assert_eq!(titles["beta"].choice.format, AudiobookFormat::M4b);

    rig.lock_order(true);
    rig.send(SessionIntent::ApplyDefaultAudio {
        title_ids: vec!["alpha".to_string()],
    })
    .await;
    assert_eq!(
        audio(&rig).titles["alpha"].choice.format,
        AudiobookFormat::M4aOpus
    );

    rig.lock_order(false);
    rig.send(SessionIntent::ApplyDefaultAudio {
        title_ids: vec!["alpha".to_string()],
    })
    .await;
    assert_eq!(
        audio(&rig).titles["alpha"].choice.format,
        AudiobookFormat::M4b
    );
    assert_eq!(audio(&rig).defaults.choice.format, AudiobookFormat::M4b);
}

#[tokio::test]
async fn a_batch_audio_edit_applies_to_every_title_or_none() {
    use crate::audio::{EncoderType, FaacProfile, SampleRateConfig};
    use crate::session::{AudioField, AudioRefusal};

    let rig = rig();
    rig.load(&["alpha", "beta"]);
    let both = vec!["alpha".to_string(), "beta".to_string()];
    let set = |title_ids: Vec<String>, edit| SessionIntent::SetTitleAudio { title_ids, edit };
    rig.send(set(both.clone(), AudioEdit::Encoder(EncoderType::Faac)))
        .await;
    rig.send(set(
        vec!["alpha".to_string()],
        AudioEdit::FaacProfile(FaacProfile::HeAacV1),
    ))
    .await;
    rig.send(set(
        vec!["beta".to_string()],
        AudioEdit::FaacProfile(FaacProfile::AacLc),
    ))
    .await;
    let titles = audio(&rig).titles;
    let only_lc = titles["beta"]
        .facts
        .allowed_sample_rates
        .iter()
        .copied()
        .find(|rate| !titles["alpha"].facts.allowed_sample_rates.contains(rate))
        .expect("a rate only AAC-LC accepts");
    let beta_rate = titles["beta"].choice.sample_rate;

    rig.send(set(
        both.clone(),
        AudioEdit::SampleRate(SampleRateConfig::Explicit(only_lc)),
    ))
    .await;
    let after = audio(&rig);
    assert_eq!(
        after.titles["beta"].choice.sample_rate, beta_rate,
        "nothing changed"
    );
    assert_eq!(
        after.refusal,
        Some(AudioRefusal::NotAccepted {
            title_ids: vec!["alpha".to_string()],
            labels: vec!["alpha.m4b".to_string()],
        })
    );

    rig.select(&[0, 1]).await;
    let selection = audio(&rig).selection.expect("selection audio");
    assert!(!selection.facts.allowed_sample_rates.contains(&only_lc));
    assert!(selection.facts.allowed_sample_rates.contains(&44_100));
    assert!(selection.mixed.contains(&AudioField::FaacProfile));

    rig.send(set(both.clone(), AudioEdit::Encoder(EncoderType::Faac)))
        .await;
    assert_eq!(
        audio(&rig).refusal,
        None,
        "a value already set is not a refusal"
    );

    rig.lock_order(true);
    rig.send(SessionIntent::ApplyDefaultAudio { title_ids: both })
        .await;
    assert_eq!(audio(&rig).refusal, Some(AudioRefusal::Locked));
    rig.lock_order(false);
    assert_eq!(audio(&rig).refusal, None, "unlocking answers the refusal");
}
