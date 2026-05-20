//! Canonical supported audio extension handling.

use std::path::Path;

use crate::errors::{AppError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportedAudioFormat {
    pub extension: &'static str,
    pub label: &'static str,
}

pub const SUPPORTED_AUDIO_FORMATS: &[SupportedAudioFormat] = &[
    SupportedAudioFormat {
        extension: "mp3",
        label: "MP3",
    },
    SupportedAudioFormat {
        extension: "m4a",
        label: "M4A/M4B",
    },
    SupportedAudioFormat {
        extension: "m4b",
        label: "M4A/M4B",
    },
    SupportedAudioFormat {
        extension: "aac",
        label: "AAC",
    },
    SupportedAudioFormat {
        extension: "wav",
        label: "WAV",
    },
    SupportedAudioFormat {
        extension: "flac",
        label: "FLAC",
    },
];

pub fn audio_format_for_path(path: &Path) -> Result<&'static SupportedAudioFormat> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .ok_or_else(|| AppError::InvalidInput("File has no extension".to_string()))?;

    SUPPORTED_AUDIO_FORMATS
        .iter()
        .find(|format| format.extension == ext)
        .ok_or_else(|| AppError::InvalidInput(format!("Unsupported audio format: {ext}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn mixed_case(extension: &str) -> String {
        extension
            .chars()
            .enumerate()
            .map(|(index, character)| {
                if index % 2 == 0 {
                    character.to_ascii_uppercase()
                } else {
                    character
                }
            })
            .collect()
    }

    #[test]
    fn supported_audio_formats_accept_extension_case_variants() {
        for expected in SUPPORTED_AUDIO_FORMATS {
            for ext in [
                expected.extension.to_string(),
                expected.extension.to_ascii_uppercase(),
                mixed_case(expected.extension),
            ] {
                let path = format!("book.{ext}");
                let actual = audio_format_for_path(Path::new(&path))
                    .expect("supported extension case variant should be accepted");
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn supported_audio_extensions_come_from_canonical_format_table() {
        let extensions = SUPPORTED_AUDIO_FORMATS
            .iter()
            .map(|format| format.extension)
            .collect::<Vec<_>>();
        assert_eq!(extensions, ["mp3", "m4a", "m4b", "aac", "wav", "flac"]);
    }
}
