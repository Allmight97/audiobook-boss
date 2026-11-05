#[path = "integration_collision_review_tests.rs"]
mod integration_collision_review_tests;
#[path = "integration_estimated_length_tests.rs"]
mod integration_estimated_length_tests;
#[path = "integration_faac_execution_tests.rs"]
mod integration_faac_execution_tests;
#[path = "integration_library_readback_tests.rs"]
mod integration_library_readback_tests;
#[path = "integration_media_execution_tests.rs"]
pub(crate) mod integration_media_execution_tests;
#[path = "integration_media_route_parity_tests.rs"]
mod integration_media_route_parity_tests;
#[path = "integration_preview_tests.rs"]
mod integration_preview_tests;
#[path = "integration_reqwest_resolver_tests.rs"]
mod integration_reqwest_resolver_tests;
#[path = "integration_session_desk.rs"]
mod integration_session_desk;
#[path = "integration_session_tests.rs"]
mod integration_session_tests;
#[path = "media_fixtures.rs"]
pub(crate) mod media_fixtures;
#[path = "unit_audio_command_result_schema_tests.rs"]
mod unit_audio_command_result_schema_tests;

#[path = "unit_error_envelope_tests.rs"]
mod unit_error_envelope_tests;
#[path = "unit_error_path_sanitization_tests.rs"]
mod unit_error_path_sanitization_tests;
#[path = "unit_runtime_settings_capabilities_tests.rs"]
mod unit_runtime_settings_capabilities_tests;
#[path = "unit_settings_encoder_tests.rs"]
mod unit_settings_encoder_tests;
#[path = "unit_settings_validation_tests.rs"]
mod unit_settings_validation_tests;

#[expect(
    clippy::disallowed_methods,
    reason = "tests move their own temp files to stage a scenario"
)]
pub(crate) fn move_test_file(from: &std::path::Path, to: &std::path::Path) {
    std::fs::rename(from, to).expect("move a test file");
}
