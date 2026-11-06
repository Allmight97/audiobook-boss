#[path = "id3_frames.rs"]
pub(crate) mod id3_frames;
#[path = "integration_audible_handoff_tests.rs"]
mod integration_audible_handoff_tests;
#[path = "integration_collision_review_tests.rs"]
mod integration_collision_review_tests;
#[path = "integration_estimated_length_tests.rs"]
mod integration_estimated_length_tests;
#[path = "integration_faac_execution_tests.rs"]
mod integration_faac_execution_tests;
#[path = "integration_import_variants_tests.rs"]
mod integration_import_variants_tests;
#[path = "integration_indexer_connection_tests.rs"]
mod integration_indexer_connection_tests;
#[path = "integration_library_readback_tests.rs"]
mod integration_library_readback_tests;
#[path = "integration_media_album_sort_tests.rs"]
mod integration_media_album_sort_tests;
#[path = "integration_media_cue_scale_tests.rs"]
mod integration_media_cue_scale_tests;
#[path = "integration_media_encoding_choice_tests.rs"]
mod integration_media_encoding_choice_tests;
#[path = "integration_media_execution_tests.rs"]
mod integration_media_execution_tests;
#[path = "integration_media_route_parity_tests.rs"]
mod integration_media_route_parity_tests;
#[path = "integration_media_tag_clear_tests.rs"]
mod integration_media_tag_clear_tests;
#[path = "integration_output_safety_tests.rs"]
mod integration_output_safety_tests;
#[path = "integration_preview_tests.rs"]
mod integration_preview_tests;
#[path = "integration_session_desk.rs"]
mod integration_session_desk;
#[path = "integration_session_save_tests.rs"]
mod integration_session_save_tests;
#[path = "integration_session_tests.rs"]
mod integration_session_tests;
#[path = "integration_settings_relaunch_tests.rs"]
mod integration_settings_relaunch_tests;
#[path = "media_fixtures.rs"]
pub(crate) mod media_fixtures;
#[path = "unit_error_envelope_tests.rs"]
mod unit_error_envelope_tests;
#[path = "unit_runtime_settings_capabilities_tests.rs"]
mod unit_runtime_settings_capabilities_tests;
#[path = "unit_settings_encoder_tests.rs"]
mod unit_settings_encoder_tests;
#[path = "unit_settings_validation_tests.rs"]
mod unit_settings_validation_tests;
#[path = "wav_fixture.rs"]
pub(crate) mod wav_fixture;

#[expect(
    clippy::disallowed_methods,
    reason = "tests move their own temp files to stage a scenario"
)]
pub(crate) fn move_test_file(from: &std::path::Path, to: &std::path::Path) {
    std::fs::rename(from, to).expect("move a test file");
}
