use tokio_util::sync::CancellationToken;

pub(crate) fn cancelled_token() -> CancellationToken {
    let token = CancellationToken::new();
    token.cancel();
    token
}
