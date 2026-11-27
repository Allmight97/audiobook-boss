use super::boxes::encode_full_box;
use crate::errors::{AppError, Result};

pub(crate) fn encode_chpl_box(chapters: &[(u64, &str)]) -> Result<Vec<u8>> {
    let count = u8::try_from(chapters.len()).map_err(|_| {
        AppError::InvalidInput("Nero chpl test fixture supports at most 255 chapters".to_string())
    })?;
    let mut body = Vec::new();
    body.push(count);
    for &(start_ms, title) in chapters {
        let start_ticks = start_ms.saturating_mul(10_000);
        body.extend_from_slice(&start_ticks.to_be_bytes());
        let title_bytes = title.as_bytes();
        let truncated_len = truncate_utf8_len(title, title_bytes.len().min(u8::MAX as usize));
        body.push(truncated_len as u8);
        body.extend_from_slice(&title_bytes[..truncated_len]);
    }
    Ok(encode_full_box(*b"chpl", 0, [0; 3], &body))
}

fn truncate_utf8_len(text: &str, max_bytes: usize) -> usize {
    let mut len = max_bytes.min(text.len());
    while len > 0 && !text.is_char_boundary(len) {
        len -= 1;
    }
    len
}
