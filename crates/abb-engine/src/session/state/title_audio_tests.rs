use super::*;
use crate::session::audio_choice::AudioEdit;

#[test]
fn grouped_surround_sources_publish_downmix_warning_only_for_explicit_channels() {
    let mut state = SessionState {
        audio: AudioDefaults::new(None, Some(crate::audio::encoder_settings_capabilities())),
        ..SessionState::default()
    };
    let mut first = audio_file("first", true);
    first.channels = Some(2);
    let mut second = audio_file("second", true);
    second.channels = Some(6);
    let request = state.audio.request();
    state
        .working_set
        .append_analyzed(vec![first, second], &request);
    state.working_set.select_file(0, ONE);
    state.working_set.select_file(1, ADD);
    state.working_set.group_selected();
    assert!(
        !state.audio_snapshot(0).titles["first"]
            .facts
            .downmix_warning
    );
    let request = state
        .audio
        .edit_title(
            state
                .working_set
                .audio_request("first")
                .expect("group audio"),
            AudioEdit::Channels(crate::audio::ChannelConfig::Stereo),
        )
        .expect("accepted channel choice");
    state.working_set.set_audio_request("first", request);
    assert!(
        state.audio_snapshot(0).titles["first"]
            .facts
            .downmix_warning
    );
}
