//! Every extension ABB imports, and every codec its containers are known to
//! carry, probes as valid audio, decodes, and reaches an encoded book. The
//! bundled FFmpeg is built from an explicit component list
//! (`vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md`); a format added to the
//! import list needs its recipe here and its components there. Fixtures are
//! synthesized with the FFmpeg CLI at test time.

use super::media_fixtures::{decode_pcm_f32, ffmpeg, MediaLane};
use crate::audio::{get_file_list_info, supported_audio_import_extensions, AacDecoder};
use std::path::{Path, PathBuf};

struct Recipe {
    extension: &'static str,
    codec: &'static str,
    ffmpeg_args: &'static [&'static str],
}

const RECIPES: &[Recipe] = &[
    Recipe {
        extension: "m4b",
        codec: "aac",
        ffmpeg_args: &["-c:a", "aac"],
    },
    Recipe {
        extension: "m4b",
        codec: "eac3",
        ffmpeg_args: &["-c:a", "eac3", "-f", "mp4"],
    },
    Recipe {
        extension: "m4a",
        codec: "aac",
        ffmpeg_args: &["-c:a", "aac"],
    },
    Recipe {
        extension: "m4a",
        codec: "ac3",
        ffmpeg_args: &["-c:a", "ac3"],
    },
    Recipe {
        extension: "m4a",
        codec: "alac",
        ffmpeg_args: &["-c:a", "alac"],
    },
    Recipe {
        extension: "m4a",
        codec: "flac",
        ffmpeg_args: &["-c:a", "flac", "-f", "mp4"],
    },
    Recipe {
        extension: "m4a",
        codec: "mp3",
        ffmpeg_args: &["-c:a", "libmp3lame", "-f", "mp4"],
    },
    Recipe {
        extension: "m4a",
        codec: "opus",
        ffmpeg_args: &["-c:a", "libopus", "-ar", "48000", "-f", "mp4"],
    },
    Recipe {
        extension: "aac",
        codec: "adts",
        ffmpeg_args: &["-c:a", "aac"],
    },
    Recipe {
        extension: "mp3",
        codec: "mp3",
        ffmpeg_args: &["-c:a", "libmp3lame"],
    },
    Recipe {
        extension: "mp3",
        codec: "mp2",
        ffmpeg_args: &["-c:a", "mp2", "-f", "mp2"],
    },
    Recipe {
        extension: "flac",
        codec: "flac",
        ffmpeg_args: &["-c:a", "flac"],
    },
    Recipe {
        extension: "flac",
        codec: "flac24",
        ffmpeg_args: &["-c:a", "flac", "-sample_fmt", "s32"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_u8",
        ffmpeg_args: &["-c:a", "pcm_u8"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_s16le",
        ffmpeg_args: &["-c:a", "pcm_s16le"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_s24le",
        ffmpeg_args: &["-c:a", "pcm_s24le"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_s32le",
        ffmpeg_args: &["-c:a", "pcm_s32le"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_f32le",
        ffmpeg_args: &["-c:a", "pcm_f32le"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_f64le",
        ffmpeg_args: &["-c:a", "pcm_f64le"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_alaw",
        ffmpeg_args: &["-c:a", "pcm_alaw"],
    },
    Recipe {
        extension: "wav",
        codec: "pcm_mulaw",
        ffmpeg_args: &["-c:a", "pcm_mulaw"],
    },
    Recipe {
        extension: "wav",
        codec: "adpcm_ima_wav",
        ffmpeg_args: &["-c:a", "adpcm_ima_wav"],
    },
    Recipe {
        extension: "wav",
        codec: "adpcm_ms",
        ffmpeg_args: &["-c:a", "adpcm_ms"],
    },
    Recipe {
        extension: "mka",
        codec: "opus",
        ffmpeg_args: &["-c:a", "libopus", "-ar", "48000"],
    },
    Recipe {
        extension: "mka",
        codec: "aac",
        ffmpeg_args: &["-c:a", "aac"],
    },
    Recipe {
        extension: "mka",
        codec: "flac",
        ffmpeg_args: &["-c:a", "flac"],
    },
    Recipe {
        extension: "mka",
        codec: "mp3",
        ffmpeg_args: &["-c:a", "libmp3lame"],
    },
    Recipe {
        extension: "mka",
        codec: "vorbis",
        ffmpeg_args: &["-c:a", "vorbis", "-strict", "-2", "-ac", "2"],
    },
    Recipe {
        extension: "mka",
        codec: "pcm_s16le",
        ffmpeg_args: &["-c:a", "pcm_s16le"],
    },
    Recipe {
        extension: "mka",
        codec: "pcm_s16be",
        ffmpeg_args: &["-c:a", "pcm_s16be"],
    },
];

const FIXTURE_SECONDS: f64 = 1.0;
const SINE_RMS: f64 = 0.088;

fn write_fixture(dir: &Path, recipe: &Recipe) -> PathBuf {
    let path = dir.join(format!(
        "{}-{}.{}",
        recipe.extension, recipe.codec, recipe.extension
    ));
    ffmpeg(
        [
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=1",
        ]
        .into_iter()
        .chain(recipe.ffmpeg_args.iter().copied())
        .chain([path.to_str().expect("utf8 path")]),
    );
    path
}

fn rms(samples: &[f32]) -> f64 {
    (samples
        .iter()
        .map(|value| f64::from(*value).powi(2))
        .sum::<f64>()
        / samples.len() as f64)
        .sqrt()
}

#[test]
fn every_supported_extension_has_a_fixture_recipe_and_no_recipe_is_unsupported() {
    let supported = supported_audio_import_extensions();
    for extension in &supported {
        assert!(
            RECIPES.iter().any(|recipe| recipe.extension == *extension),
            "no fixture recipe for the supported extension {extension}"
        );
    }
    for recipe in RECIPES {
        assert!(
            supported.contains(&recipe.extension),
            "recipe {}-{} names an extension ABB does not import",
            recipe.extension,
            recipe.codec
        );
    }
}

#[tokio::test]
async fn every_supported_format_and_codec_imports_decodes_and_encodes() {
    let dir = tempfile::tempdir().expect("fixture folder");
    let fixtures: Vec<_> = RECIPES
        .iter()
        .map(|recipe| {
            (
                format!("{}-{}", recipe.extension, recipe.codec),
                write_fixture(dir.path(), recipe),
            )
        })
        .collect();

    let unreadable: Vec<_> = fixtures
        .iter()
        .filter(|(_, source)| {
            get_file_list_info(&[source], AacDecoder::Auto)
                .map_or(true, |info| info.invalid_count != 0)
        })
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(unreadable.is_empty(), "do not import: {unreadable:?}");

    for (name, source) in fixtures {
        let info = get_file_list_info(&[&source], AacDecoder::Auto).expect("probe the fixture");
        let duration = info.files[0].duration.expect("a duration");
        assert!(
            (duration - FIXTURE_SECONDS).abs() < 0.1,
            "{name} duration {duration}"
        );

        let lane = MediaLane::for_inputs(vec![source]);
        let output = lane.process(None).await;
        let book = get_file_list_info(&[&output], AacDecoder::Auto).expect("probe the book");
        let book = &book.files[0];
        let values_per_second = f64::from(
            book.sample_rate.expect("the book's rate") * book.channels.expect("channels"),
        );
        let samples = decode_pcm_f32(&output);
        let seconds = samples.len() as f64 / values_per_second;
        assert!(
            (seconds - FIXTURE_SECONDS).abs() < 0.1,
            "{name} decodes to {seconds} s"
        );
        let level = rms(&samples);
        assert!(
            (SINE_RMS * 0.5..SINE_RMS * 2.0).contains(&level),
            "{name} decoded level {level}"
        );
    }
}
