#[path = "id3_frames.rs"]
pub(crate) mod id3_frames;
#[path = "integration_audible_handoff_tests.rs"]
mod integration_audible_handoff_tests;
#[path = "integration_cancel_events_tests.rs"]
mod integration_cancel_events_tests;
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
#[path = "integration_job_log_tests.rs"]
mod integration_job_log_tests;
#[path = "integration_library_readback_tests.rs"]
mod integration_library_readback_tests;
#[path = "integration_media_album_sort_tests.rs"]
mod integration_media_album_sort_tests;
#[path = "integration_media_cue_scale_tests.rs"]
mod integration_media_cue_scale_tests;
#[path = "integration_media_encoded_run_tests.rs"]
mod integration_media_encoded_run_tests;
#[path = "integration_media_encoding_choice_tests.rs"]
mod integration_media_encoding_choice_tests;
#[path = "integration_media_merge_chapters_tests.rs"]
mod integration_media_merge_chapters_tests;
#[path = "integration_media_opus_tests.rs"]
mod integration_media_opus_tests;
#[path = "integration_media_preserve_bytes_tests.rs"]
mod integration_media_preserve_bytes_tests;
#[path = "integration_media_preserve_decision_tests.rs"]
mod integration_media_preserve_decision_tests;
#[path = "integration_media_preserve_tests.rs"]
mod integration_media_preserve_tests;
#[path = "integration_media_residue_tests.rs"]
mod integration_media_residue_tests;
#[path = "integration_media_route_parity_tests.rs"]
mod integration_media_route_parity_tests;
#[path = "integration_media_tag_save_tests.rs"]
mod integration_media_tag_save_tests;
#[path = "integration_media_tag_snapshot_tests.rs"]
mod integration_media_tag_snapshot_tests;
#[path = "integration_media_vorbis_fields_tests.rs"]
mod integration_media_vorbis_fields_tests;
#[path = "integration_output_safety_tests.rs"]
mod integration_output_safety_tests;
#[path = "integration_preview_plan_tests.rs"]
mod integration_preview_plan_tests;
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
#[path = "log_capture.rs"]
pub(crate) mod log_capture;
#[path = "media_fixtures.rs"]
pub(crate) mod media_fixtures;
#[path = "support/mod.rs"]
pub(crate) mod support;
#[path = "tag_interop_dump.rs"]
pub(crate) mod tag_interop_dump;
#[path = "tag_interop_seed.rs"]
pub(crate) mod tag_interop_seed;

#[expect(
    clippy::disallowed_methods,
    reason = "tests move their own temp files to stage a scenario"
)]
pub(crate) fn move_test_file(from: &std::path::Path, to: &std::path::Path) {
    std::fs::rename(from, to).expect("move a test file");
}
