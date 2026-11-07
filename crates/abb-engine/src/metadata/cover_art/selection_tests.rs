use super::*;

fn set_cover(bytes: Vec<u8>) -> PatchOp<Vec<u8>> {
    PatchOp::Set(bytes)
}

fn passthrough_with_cover(bytes: Vec<u8>) -> PassthroughMetadata {
    PassthroughMetadata {
        cover_art: Some(bytes),
        ..Default::default()
    }
}

#[test]
fn explicit_cover_wins_over_passthrough() {
    let explicit = set_cover(vec![1, 2, 3]);
    let passthrough = passthrough_with_cover(vec![9, 9, 9]);
    let selection =
        ResolvedCover::select(Some(&explicit), Some(&passthrough)).expect("a cover is selected");
    assert!(matches!(selection, ResolvedCover::Explicit(_)));
    assert_eq!(selection.bytes(), &vec![1, 2, 3]);
}

#[test]
fn passthrough_used_when_no_explicit_cover() {
    let passthrough = passthrough_with_cover(vec![9, 9, 9]);
    let selection = ResolvedCover::select(None, Some(&passthrough)).expect("a cover is selected");
    assert!(matches!(selection, ResolvedCover::Passthrough(_)));
    assert_eq!(selection.bytes(), &vec![9, 9, 9]);
}

#[test]
fn empty_cover_is_dropped() {
    let passthrough = passthrough_with_cover(vec![9, 9, 9]);
    assert!(ResolvedCover::select(Some(&PatchOp::Clear), Some(&passthrough)).is_none());
    assert!(ResolvedCover::select(None, None).is_none());
    let empty_passthrough = passthrough_with_cover(Vec::new());
    assert!(ResolvedCover::select(None, Some(&empty_passthrough)).is_none());
}

#[test]
fn passthrough_embed_failure_warns_and_continues() {
    let passthrough = passthrough_with_cover(vec![9, 9, 9]);
    let selection = ResolvedCover::select(None, Some(&passthrough)).expect("selected");
    let result = selection.handle_embed_failure(AppError::General("boom".to_string()), "cover art");
    assert!(result.is_ok());
}

#[test]
fn explicit_embed_failure_propagates() {
    let explicit = set_cover(vec![1, 2, 3]);
    let selection = ResolvedCover::select(Some(&explicit), None).expect("selected");
    let result = selection.handle_embed_failure(AppError::General("boom".to_string()), "cover art");
    assert!(result.is_err());
}
