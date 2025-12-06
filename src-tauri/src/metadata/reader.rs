//! Metadata reading via ffmpeg-next
use super::AudiobookMetadata;
use crate::errors::{AppError, Result};
use ffmpeg_next as ff;
use std::path::Path;

/// Reads container-level metadata and attached cover art using ffmpeg-next.
pub fn read_metadata<P: AsRef<Path>>(file_path: P) -> Result<AudiobookMetadata> {
    let path = file_path.as_ref();

    if !path.exists() {
        return Err(AppError::FileValidation(format!(
            "File not found: {}",
            path.display()
        )));
    }

    ff::init().map_err(AppError::Ffmpeg)?;

    let ictx = ff::format::input(path).map_err(AppError::Ffmpeg)?;
    let dict = ictx.metadata();

    let mut metadata = AudiobookMetadata::new();

    metadata.title = dict.get("title").map(str::to_string);
    metadata.artist = dict.get("artist").map(str::to_string);
    metadata.album = dict.get("album").map(str::to_string);
    metadata.composer = dict.get("composer").map(str::to_string);
    metadata.genre = dict.get("genre").map(str::to_string);
    metadata.comment = dict.get("comment").map(str::to_string);
    metadata.description = dict.get("description").map(str::to_string);
    metadata.album_sort = dict.get("sort_album").map(str::to_string);

    // Series metadata mapped to ffmpeg's show/episode_sort keys
    metadata.series = dict.get("show").map(str::to_string);
    metadata.series_part = dict.get("episode_sort").map(str::to_string);

    // Year/date can be stored under `date` or `year`
    metadata.date = dict
        .get("date")
        .or_else(|| dict.get("year"))
        .and_then(|v| v.parse::<u32>().ok());

    // Attached picture (cover art)
    metadata.cover_art = extract_attached_pic(&ictx);

    Ok(metadata)
}

/// Extracts the first attached picture (cover art) from the container streams.
fn extract_attached_pic(ictx: &ff::format::context::Input) -> Option<Vec<u8>> {
    use ff::format::stream::Disposition;

    for stream in ictx.streams() {
        if stream.disposition().contains(Disposition::ATTACHED_PIC) {
            unsafe {
                let av_stream = stream.as_ptr();
                let pic = (*av_stream).attached_pic;
                if !pic.data.is_null() && pic.size > 0 {
                    let bytes = std::slice::from_raw_parts(pic.data, pic.size as usize);
                    return Some(bytes.to_vec());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_nonexistent_file_returns_error() {
        let result = read_metadata("does-not-exist.m4b");
        assert!(matches!(result, Err(AppError::FileValidation(_))));
    }

    #[test]
    fn invalid_file_surfaces_ffmpeg_error() {
        let temp = TempDir::new().expect("temp dir");
        let path = temp.path().join("invalid.m4b");
        std::fs::write(&path, b"not audio").expect("write");

        let result = read_metadata(&path);
        assert!(matches!(result, Err(AppError::Ffmpeg(_))));
    }
}
