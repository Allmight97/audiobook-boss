use super::PreviewState;

#[test]
fn an_excerpt_takes_exactly_its_budget_across_frames() {
    let mut state = PreviewState::new(2500);
    assert_eq!(state.take(1024), 1024);
    assert_eq!(state.take(1024), 1024);
    assert!(!state.excerpt_complete());
    assert_eq!(state.take(1024), 452);
    assert!(state.excerpt_complete());
    assert_eq!(state.take(1024), 0);
}

#[test]
fn a_new_file_starts_a_fresh_excerpt() {
    let mut state = PreviewState::new(100);
    state.take(100);
    state.start_new_file();
    assert!(!state.excerpt_complete());
    assert_eq!(state.take(150), 100);
}
