use tokio::io::AsyncWriteExt;

use crate::remote_source::cancellation::ensure_not_cancelled;
use crate::errors::Result;

/// Stream one HTTP response body into append-mode `file`, invoking `on_chunk` for each
/// non-empty chunk after a cancellation check. Returns `true` if the body read failed
/// midway (a resumable condition for ranged audio downloads).
pub(in crate::remote_source::providers::audible) async fn stream_response_chunks(
    response: &mut reqwest::Response,
    file: &mut tokio::fs::File,
    is_cancelled: &impl Fn() -> bool,
    mut on_chunk: impl FnMut(&[u8]) -> Result<()>,
) -> Result<bool> {
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) => return Ok(true),
        };
        ensure_not_cancelled(is_cancelled)?;
        if chunk.is_empty() {
            continue;
        }
        on_chunk(&chunk)?;
        file.write_all(&chunk).await?;
    }
    Ok(false)
}
