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
    assert_eq!(
        extensions,
        ["mka", "mp3", "m4a", "m4b", "aac", "wav", "flac"]
    );
}
