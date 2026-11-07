use super::*;
use crate::app_settings::{EditStamp, OutputDefaults, SettingsIntent};

#[tokio::test]
async fn the_output_preview_follows_the_form_the_directory_and_the_format() {
    let rig = rig();
    rig.load(&["alpha"]);
    rig.select(&[0]).await;
    assert_eq!(
        output(&rig).preview,
        crate::session::OutputPreview::NoDirectory
    );

    rig.send(SessionIntent::SetOutputDirectory {
        directory: "/out".to_string(),
    })
    .await;
    let first = preview_path(&rig);
    assert!(first.starts_with("/out/Author"), "{first}");
    assert!(first.ends_with(".m4b"), "{first}");

    rig.type_into(MetadataField::Title, "Dune").await;
    rig.send(SessionIntent::SetTitleAudio {
        title_ids: vec!["alpha".to_string()],
        edit: AudioEdit::Format(AudiobookFormat::M4aOpus),
    })
    .await;
    let renamed = preview_path(&rig);
    assert!(
        renamed.contains("Dune") && renamed.ends_with(".m4a"),
        "{renamed}"
    );
}

#[tokio::test(start_paused = true)]
async fn output_choices_are_recorded_and_template_typing_once_it_pauses() {
    let rig = rig();
    let saved = || async {
        rig.session
            .inner
            .deps
            .settings
            .snapshot()
            .await
            .settings
            .output_defaults
    };

    rig.send(SessionIntent::SetNamingPreset {
        preset: NamingPreset::CustomTemplate,
    })
    .await;
    assert_eq!(
        saved().await.output_naming.preset,
        NamingPreset::CustomTemplate
    );

    for template in ["{a", "{author}", "{author}/{title}x"] {
        rig.send(SessionIntent::SetNamingTemplate {
            template: template.to_string(),
        })
        .await;
    }
    assert_eq!(saved().await.output_naming.custom_template, None);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while saved().await.output_naming.custom_template.is_none() {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the typed template is saved once its pause ends");
    assert_eq!(
        saved().await.output_naming.custom_template.as_deref(),
        Some("{author}/{title}x")
    );
}

#[tokio::test]
async fn a_default_audio_edit_is_recorded_in_the_settings_and_announced() {
    let rig = rig();
    rig.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Bitrate(96),
    })
    .await;

    let settings = rig.session.inner.deps.settings.snapshot().await;
    let saved = settings.settings.encoder_defaults;
    assert_eq!(saved.settings.bitrate_kbps, 96);
    assert_eq!(saved.intent, AudioIntent::Encode);
    let announced = rig.events.1.lock().expect("events").last().cloned();
    assert_eq!(
        announced.map(|snapshot| snapshot.revision),
        Some(settings.revision)
    );

    let before = rig.events.1.lock().expect("events").len();
    rig.send(SessionIntent::SetDefaultAudio {
        edit: AudioEdit::Bitrate(96),
    })
    .await;
    assert_eq!(rig.events.1.lock().expect("events").len(), before);
}

fn typed_edit(rig: &Rig, directory: &str) -> (OutputDefaults, EditStamp) {
    rig.session.transition(|state| {
        state.output.set_directory(directory.to_string());
        let (stamp, _) = state.edits.note_output_edit();
        (state.output.defaults(), stamp)
    })
}

async fn reset_after(rig: &Rig, edit_turn_first: bool, directory: &str) -> Option<String> {
    let session = &rig.session;
    let settings = session.inner.deps.settings.clone();
    let tasks = session.inner.deps.tasks.clone();
    let (typed, stamp) = typed_edit(rig, directory);
    let (reset, edit) = if edit_turn_first {
        let edit = settings.remember_defaults(None, Some(typed), stamp, &tasks);
        (settings.begin(SettingsIntent::Reset, &tasks), edit)
    } else {
        let reset = settings.begin(SettingsIntent::Reset, &tasks);
        (
            reset,
            settings.remember_defaults(None, Some(typed), stamp, &tasks),
        )
    };
    let reply = reset.finish().await;
    session.adopt_reset(&reply.snapshot);
    edit.finish().await;

    let in_file = settings.snapshot().await.settings.output_defaults;
    assert_eq!(output(rig).directory, in_file.output_directory);
    in_file.output_directory
}

#[tokio::test]
async fn a_default_edit_whose_settings_turn_follows_a_reset_stays_on_screen_and_in_the_file() {
    assert_eq!(
        reset_after(&rig(), false, "/typed").await.as_deref(),
        Some("/typed")
    );
}

#[tokio::test]
async fn a_default_edit_whose_settings_turn_precedes_a_reset_is_reset_on_screen_and_in_the_file() {
    assert_eq!(reset_after(&rig(), true, "/typed").await, None);
}
