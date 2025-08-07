use audiobook_boss_lib::audio::session::ProcessingSession;

#[test]
fn test_new_session_has_unique_id() {
    let session1 = ProcessingSession::new();
    let session2 = ProcessingSession::new();
    assert_ne!(session1.id(), session2.id());
}

#[test]
fn test_new_session_not_processing() {
    let session = ProcessingSession::new();
    assert!(!session.is_processing());
    assert!(!session.is_cancelled());
}

#[test]
fn test_session_id_format() {
    let session = ProcessingSession::new();
    let id = session.id();
    assert_eq!(id.len(), 36);
    assert_eq!(id.chars().filter(|&c| c == '-').count(), 4);
}


