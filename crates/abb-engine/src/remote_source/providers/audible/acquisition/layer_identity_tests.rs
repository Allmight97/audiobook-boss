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
