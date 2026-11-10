//! Spike proof for issue #621 item C2: the wrapper decrypt copy, kept here as
//! the reference, against the copy the engine ships.

use std::path::Path;

use abb_audible_core::{demuxer_options, AudibleDecryptionMaterial, DemuxerOption};
use ffmpeg_next as ff;
use secrecy::{ExposeSecret, SecretBox};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::super::decrypt::{stream_copy, CopyControl};
use crate::test_cases::encrypted_audible_books::{
    books, edit_list_books, hex_16, hex_bytes, AAXC_IV_HEX, AAXC_KEY_HEX, AAX_ACTIVATION_HEX,
};

fn aax() -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aax {
        activation_bytes: SecretBox::new(Box::new(
            hex_bytes(AAX_ACTIVATION_HEX).try_into().expect("4 bytes"),
        )),
    }
}

fn aaxc() -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aaxc {
        key: SecretBox::new(Box::new(hex_16(AAXC_KEY_HEX))),
        iv: SecretBox::new(Box::new(hex_16(AAXC_IV_HEX))),
    }
}

fn sha256(path: &Path) -> String {
    let bytes = std::fs::read(path).expect("read output");
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn reference_copy(input: &Path, output: &Path, options: &[DemuxerOption]) {
    ff::init().expect("init");
    let mut dictionary = ff::Dictionary::new();
    for option in options {
        dictionary.set(option.name, option.value.expose_secret());
    }
    let mut ictx = ff::format::input_with_dictionary(input, dictionary).expect("open");
    let mut octx = ff::format::output_as(output, "mp4").expect("output");
    let mut mapping = vec![None; ictx.streams().len()];
    for istream in ictx.streams() {
        let copied = match istream.parameters().medium() {
            ff::media::Type::Audio => true,
            ff::media::Type::Video => istream
                .disposition()
                .contains(ff::format::stream::Disposition::ATTACHED_PIC),
            _ => false,
        };
        if !copied {
            continue;
        }
        let codec =
            ff::codec::context::Context::from_parameters(istream.parameters()).expect("params");
        let mut ostream = octx.add_stream_with(&codec).expect("stream");
        ostream.set_time_base(istream.time_base());
        ostream.set_metadata(istream.metadata().to_owned());
        crate::metadata::set_stream_disposition_and_clear_codec_tag(
            &mut ostream,
            istream.disposition(),
        );
        mapping[istream.index()] = Some(ostream.index());
    }
    for chapter in ictx.chapters() {
        let title = chapter.metadata().get("title").unwrap_or("").to_string();
        octx.add_chapter(
            chapter.id(),
            chapter.time_base(),
            chapter.start(),
            chapter.end(),
            title,
        )
        .expect("chapter");
    }
    octx.set_metadata(ictx.metadata().to_owned());
    crate::audio::write_output_header(&mut octx, ff::Dictionary::new()).expect("header");
    loop {
        let mut packet = ff::Packet::empty();
        match packet.read(&mut ictx) {
            Ok(()) => {}
            Err(ff::Error::Eof) => break,
            Err(error) => panic!("read: {error}"),
        }
        let Some(out_index) = mapping.get(packet.stream()).copied().flatten() else {
            continue;
        };
        let in_base = ictx.stream(packet.stream()).expect("in").time_base();
        let out_base = octx.stream(out_index).expect("out").time_base();
        packet.set_stream(out_index);
        packet.rescale_ts(in_base, out_base);
        packet.write_interleaved(&mut octx).expect("write");
    }
    octx.write_trailer().expect("trailer");
}

type Case = (&'static str, &'static Path, AudibleDecryptionMaterial);

fn cases() -> Vec<Case> {
    vec![
        ("aax", books().aax.as_path(), aax()),
        ("aaxc", books().aaxc.as_path(), aaxc()),
        ("aax-edit-list", edit_list_books().aax.as_path(), aax()),
        ("aaxc-edit-list", edit_list_books().aaxc.as_path(), aaxc()),
    ]
}

#[test]
fn the_wrapper_copy_is_deterministic() {
    let dir = TempDir::new().expect("dir");
    for (name, input, material) in cases() {
        let options = demuxer_options(&material);
        let first = dir.path().join(format!("{name}-1.m4b"));
        let second = dir.path().join(format!("{name}-2.m4b"));
        reference_copy(input, &first, &options);
        reference_copy(input, &second, &options);
        let (a, b) = (sha256(&first), sha256(&second));
        eprintln!("identity {name} wrapper run1={a} run2={b}");
        assert_eq!(a, b, "{name}: two wrapper runs differ");
    }
}

#[test]
fn the_shipped_copy_is_byte_identical_to_the_wrapper_copy() {
    let dir = TempDir::new().expect("dir");
    for (name, input, material) in cases() {
        let options = demuxer_options(&material);
        let reference = dir.path().join(format!("{name}-wrapper.m4b"));
        let shipped = dir.path().join(format!("{name}-shipped.m4b"));
        reference_copy(input, &reference, &options);
        stream_copy(input, &shipped, &options, &CopyControl::default()).expect("shipped copy");
        let (a, b) = (sha256(&reference), sha256(&shipped));
        eprintln!("identity {name} wrapper={a} shipped={b}");
        assert_eq!(a, b, "{name}: shipped copy differs from the wrapper copy");
    }
}

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

/// Positive control for the `leaks` run: the wrapper's `output_as` leaks its
/// format context when the file cannot open. Run only by hand.
#[test]
#[ignore = "leak positive control, run under /usr/bin/leaks"]
fn wrapper_output_as_leaks_on_an_unopenable_path() {
    let dir = TempDir::new().expect("dir");
    let file = dir.path().join("file");
    std::fs::write(&file, b"x").expect("write");
    assert!(ff::format::output_as(&file.join("out.m4b"), "mp4").is_err());
}
