use crate::audio::AudioFile;
use crate::metadata::passthrough::{extract_passthrough_metadata, PassthroughMetadata};

pub(super) fn collect_passthrough_metadata(
    valid_files: &[AudioFile],
    preview: bool,
) -> Option<PassthroughMetadata> {
    let passthrough = extract_passthrough_metadata(valid_files);
    if preview {
        passthrough.cover_art_only()
    } else {
        passthrough.into_option()
    }
}
