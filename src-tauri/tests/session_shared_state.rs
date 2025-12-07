use audiobook_boss_lib::audio::session::ProcessingSession;
use audiobook_boss_lib::ProcessingState;

#[test]
fn processing_session_sees_shared_cancel_flag() {
    let shared_state = ProcessingState::default();
    let session = ProcessingSession::from_shared_state(&shared_state);

    {
        let mut cancelled = shared_state.is_cancelled.lock().expect("lock cancel flag");
        *cancelled = true;
    }

    assert!(
        session.is_cancelled(),
        "session should observe shared cancel flag"
    );
}

#[test]
fn processing_session_sees_shared_processing_flag() {
    let shared_state = ProcessingState::default();
    let session = ProcessingSession::from_shared_state(&shared_state);

    {
        let mut is_processing = shared_state
            .is_processing
            .lock()
            .expect("lock processing flag");
        *is_processing = true;
    }

    assert!(
        session.is_processing(),
        "session should observe shared processing flag"
    );
}
