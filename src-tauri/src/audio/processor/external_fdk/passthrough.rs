use crate::audio::processor::passthrough_sources_from_audio_files;
use crate::audio::AudioFile;
use crate::metadata::{extract_passthrough_metadata, PassthroughMetadata};

pub(super) fn collect_passthrough_metadata(
    valid_files: &[AudioFile],
    preview: bool,
) -> Option<PassthroughMetadata> {
    if valid_files.is_empty() {
        return None;
    }

    let sources = passthrough_sources_from_audio_files(valid_files);
    let passthrough = extract_passthrough_metadata(&sources);
    if preview {
        passthrough.cover_art_only()
    } else {
        passthrough.into_option()
    }
}
