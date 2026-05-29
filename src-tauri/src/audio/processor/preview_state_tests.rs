use super::PreviewState;
use crate::audio::processor::frame_pipeline::PreviewAction;

#[test]
fn preview_state_new_initializes_correctly() {
    let state = PreviewState::new(5, 10.0);
    assert_eq!(state.file_count, 5);
    assert!((state.per_file_seconds - 10.0).abs() < 0.001);
    assert_eq!(state.current_file_index, 0);
    assert_eq!(state.current_file_elapsed_samples, 0);
}

#[test]
fn preview_state_start_new_file_resets_counters() {
    let mut state = PreviewState::new(5, 10.0);
    state.current_file_elapsed_samples = 48_000;
    state.start_new_file(2);

    assert_eq!(state.current_file_index, 2);
    assert_eq!(state.current_file_elapsed_samples, 0);
}

#[test]
fn preview_state_all_files_complete() {
    let mut state = PreviewState::new(3, 10.0);
    assert!(!state.all_files_complete());

    state.current_file_index = 1;
    assert!(!state.all_files_complete());

    state.current_file_index = 2;
    assert!(state.all_files_complete());
}

#[test]
fn preview_action_enum_values() {
    assert_eq!(PreviewAction::Continue, PreviewAction::Continue);
    assert_ne!(PreviewAction::Continue, PreviewAction::NextFile);
    assert_ne!(PreviewAction::NextFile, PreviewAction::StopAll);
}
