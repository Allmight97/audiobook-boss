use abb_audible_core::DemuxerOption;
use secrecy::ExposeSecret;
use tempfile::TempDir;

use super::super::decrypt::{stream_copy, CopyControl};
use crate::test_cases::encrypted_audible_books::{books, AAXC_IV_HEX, AAX_ACTIVATION_HEX};

fn option(name: &'static str, value: &str) -> DemuxerOption {
    DemuxerOption {
        name,
        value: secrecy::SecretString::from(value.to_string()),
    }
}

#[test]
fn a_key_of_the_wrong_shape_fails_at_open_and_leaves_no_partial() {
    let dir = TempDir::new().expect("dir");
    let cases = [
        (
            "aaxc odd hex",
            books().aaxc.as_path(),
            vec![
                option("audible_key", "000"),
                option("audible_iv", AAXC_IV_HEX),
            ],
        ),
        (
            "aax short activation",
            books().aax.as_path(),
            vec![option("activation_bytes", "1ceb00")],
        ),
        (
            "unknown option",
            books().aax.as_path(),
            vec![
                option("activation_bytes", AAX_ACTIVATION_HEX),
                option("no_such_option", "1"),
            ],
        ),
    ];
    for (name, input, options) in cases {
        let partial = dir.path().join(format!("{name}.m4b"));

        let result = stream_copy(input, &partial, &options, &CopyControl::default());

        let message = result.expect_err(name).to_string();
        assert!(
            message.contains("opening the download"),
            "{name}: {message}"
        );
        assert!(!partial.exists(), "{name}: no partial");
        assert!(!message.contains('/'), "{name}: no path");
        for value in options.iter().map(|o| o.value.expose_secret().to_string()) {
            assert!(
                value.len() < 2 || !message.contains(&value),
                "{name}: no option value"
            );
        }
    }
}

/// FFmpeg's mov demuxer logs a short AAXC key as fatal but ignores the error
/// (`mov_aaxc_crypto`'s result is dropped), so the file opens undecrypted.
/// Only the engine's decodes-as-audio gate rejects it; `AudibleDecryptionMaterial`
/// holds a 16-byte key, so production cannot pass one.
#[test]
fn a_short_aaxc_key_opens_undecrypted_because_ffmpeg_ignores_its_check() {
    let dir = TempDir::new().expect("dir");
    let partial = dir.path().join("short-key.m4b");
    let options = vec![
        option("audible_key", "0001020304050607"),
        option("audible_iv", AAXC_IV_HEX),
    ];

    stream_copy(&books().aaxc, &partial, &options, &CopyControl::default())
        .expect("FFmpeg copies the file without decrypting it");

    assert!(partial.exists());
}

#[test]
fn a_muxer_stopped_after_some_packets_removes_its_file() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("stopped.m4b");
    let opened = abb_ffmpeg::Input::open(&books().source, &abb_ffmpeg::SecretOptions::default())
        .expect("open");
    let mut input = opened.input;
    let mut output = abb_ffmpeg::Output::create(&path, "mp4").expect("create");
    let audio = input
        .streams()
        .into_iter()
        .find(|stream| stream.medium == abb_ffmpeg::Medium::Audio)
        .expect("audio stream");
    let index = output
        .add_stream_copied_from(&input, audio.index)
        .expect("stream");
    output
        .set_stream_time_base(index, audio.time_base)
        .expect("time base");
    let (mut muxer, unused) = output
        .write_header(&abb_ffmpeg::Tags::default())
        .expect("header");
    assert!(unused.is_empty());
    let out_base = muxer.stream_time_base(index).expect("out base");
    let mut packet = abb_ffmpeg::Packet::new().expect("packet");
    let mut written = 0;
    while written < 5 && input.read_packet(&mut packet).expect("read") {
        if packet.stream_index() != audio.index {
            continue;
        }
        packet.set_stream_index(index);
        packet.rescale_ts(audio.time_base, out_base);
        muxer.write_interleaved(&mut packet).expect("write");
        written += 1;
    }
    assert_eq!(written, 5);
    assert!(path.exists(), "the muxer wrote to its file");

    drop(muxer);

    assert!(!path.exists(), "a stopped muxer removes its file");
}

#[test]
fn a_short_activation_is_a_typed_open_error() {
    let mut options = abb_ffmpeg::SecretOptions::default();
    options.push("activation_bytes", "1ceb00").expect("option");

    let error = match abb_ffmpeg::Input::open(&books().aax, &options) {
        Ok(_) => panic!("opened"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            abb_ffmpeg::Error::Ffmpeg {
                step: abb_ffmpeg::Step::OpenInput,
                ..
            }
        ),
        "{error:?}"
    );
}
