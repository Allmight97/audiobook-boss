//! Output streams built from facts instead of an input or an encoder, the
//! lookups that answer what this FFmpeg build has, and the text rules.

use std::path::Path;

use abb_ffmpeg::{
    encoder_takes_options, has_decoder, has_decoder_named, has_encoder, has_encoder_for, AacStream,
    ChannelLayout, CodecId, Disposition, Error, Input, Medium, Output, Packet, SecretOptions, Step,
    Tags, Text,
};
use tempfile::TempDir;

/// AudioSpecificConfig: AAC-LC, 44.1 kHz, stereo.
const AAC_LC_CONFIG: [u8; 2] = [0x12, 0x10];

fn aac(config: &[u8]) -> AacStream<'_> {
    AacStream {
        high_efficiency: false,
        rate: 44_100,
        channels: 2,
        bit_rate: 64_000,
        frame_samples: 1_024,
        priming_samples: 1_024,
        config,
    }
}

fn tags(pairs: &[(&str, &str)]) -> Tags {
    let mut tags = Tags::default();
    for (key, value) in pairs {
        tags.push(key, value, Step::SetTags).expect("tag");
    }
    tags
}

fn reopen(path: &Path) -> Input {
    Input::open(path, &SecretOptions::default())
        .expect("reopen")
        .input
}

#[test]
fn an_aac_stream_built_from_facts_reads_back_with_those_facts() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("facts.m4a");
    let mut output = Output::create(&path, "mp4").expect("create");
    let index = output
        .add_aac_stream(&aac(&AAC_LC_CONFIG))
        .expect("aac stream");
    let (mut muxer, unused) = output.write_header(&Tags::default()).expect("header");
    assert!(unused.is_empty());
    muxer.set_tag("encoding_tool", "A Tool").expect("tag");
    let mut packet = Packet::copy_of(&[0x21, 0x10, 0x04, 0x60, 0x8c, 0x1c]).expect("packet");
    packet.set_stream_index(index);
    packet.set_pts(Some(0));
    packet.set_dts(Some(0));
    packet.set_duration(1_024);
    muxer.write_interleaved(&mut packet).expect("write");
    muxer.finish().expect("finish");

    let input = reopen(&path);
    let stream = input.streams().remove(0);

    assert_eq!(stream.codec, CodecId::AAC);
    assert_eq!(stream.sample_rate, 44_100);
    assert_eq!(stream.channels, 2);
    assert_eq!(
        input.stream_extradata(0).as_deref(),
        Some(&AAC_LC_CONFIG[..])
    );
    assert_eq!(
        input.tags().get("encoder").map(Text::display).as_deref(),
        Some("A Tool")
    );
}

#[test]
fn an_attachment_is_written_with_the_header_and_reads_back_whole() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("attached.mka");
    let picture = [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46];
    let mut output = Output::create(&path, "matroska").expect("create");
    let audio = output
        .add_aac_stream(&aac(&AAC_LC_CONFIG))
        .expect("aac stream");
    let attachment = output
        .add_attachment_stream(
            CodecId::MJPEG,
            &picture,
            &tags(&[("filename", "cover.jpg"), ("mimetype", "image/jpeg")]),
        )
        .expect("attachment");
    let (mut muxer, _) = output.write_header(&Tags::default()).expect("header");
    let mut packet = Packet::copy_of(&[0x21, 0x10, 0x04, 0x60, 0x8c, 0x1c]).expect("packet");
    packet.set_stream_index(audio);
    packet.set_pts(Some(0));
    packet.set_dts(Some(0));
    packet.set_duration(23);
    muxer.write_interleaved(&mut packet).expect("write");
    muxer.finish().expect("finish");

    let mut input = reopen(&path);
    let stream = input.streams().remove(attachment);
    let mut read = Packet::new().expect("packet");
    let mut pictures = Vec::new();
    while input.read_packet(&mut read).expect("read") {
        if read.stream_index() == attachment {
            pictures.push(read.data().to_vec());
        }
    }

    assert_eq!(stream.medium, Medium::Video);
    assert!(stream.disposition.contains(Disposition::ATTACHED_PIC));
    assert_eq!(pictures, [picture.to_vec()]);
    assert_eq!(
        stream.tags.get("FILENAME").map(Text::display).as_deref(),
        Some("cover.jpg")
    );
}

#[test]
fn a_stream_that_does_not_exist_is_a_typed_error_and_the_dropped_output_leaves_no_file() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("dropped.m4a");
    let mut output = Output::create(&path, "mp4").expect("create");

    let error = output
        .restate_stream_sample_clock(3, 48_000)
        .expect_err("no stream 3");
    drop(output);

    assert!(
        matches!(error, Error::MissingStream { index: 3, .. }),
        "{error:?}"
    );
    assert!(!path.exists());
}

#[test]
fn an_audio_stream_restated_on_another_clock_reports_the_new_rate() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("clock.m4a");
    let mut output = Output::create(&path, "mp4").expect("create");
    let half_rate = AacStream {
        rate: 24_000,
        frame_samples: 480,
        priming_samples: 156,
        ..aac(&AAC_LC_CONFIG)
    };
    let index = output.add_aac_stream(&half_rate).expect("stream");

    output
        .restate_stream_sample_clock(index, 48_000)
        .expect("restate");

    assert_eq!(output.streams()[index].sample_rate, 48_000);
}

#[test]
fn a_tag_with_a_nul_byte_is_refused_after_the_header() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("nul.m4a");
    let mut output = Output::create(&path, "mp4").expect("create");
    output
        .add_aac_stream(&aac(&AAC_LC_CONFIG))
        .expect("aac stream");
    let (mut muxer, _) = output.write_header(&Tags::default()).expect("header");

    let error = muxer
        .set_tag("encoding_tool", "a\0b")
        .expect_err("NUL in a tag");

    assert_eq!(
        error,
        Error::NulInText {
            step: Step::SetTags
        }
    );
}

#[test]
fn a_build_reports_the_codecs_it_has_and_the_options_they_take() {
    let options = |pairs: &[(&str, &str)]| {
        let mut options = Tags::default();
        for (key, value) in pairs {
            options.push(key, value, Step::OpenEncoder).expect("option");
        }
        options
    };

    assert!(has_decoder(CodecId::AAC));
    assert!(!has_decoder(CodecId::NONE));
    assert!(has_decoder_named("aac"));
    assert!(!has_decoder_named("no_such_decoder"));
    assert!(has_encoder("aac"));
    assert!(!has_encoder("no_such_encoder"));
    assert!(has_encoder_for(CodecId::AAC));
    assert!(!has_encoder_for(CodecId::NONE));
    assert!(encoder_takes_options(
        "aac",
        &options(&[("aac_coder", "nmr")])
    ));
    assert!(!encoder_takes_options(
        "aac",
        &options(&[("aac_coder", "no_such_coder")])
    ));
    assert!(!encoder_takes_options(
        "aac",
        &options(&[("no_such_option", "1")])
    ));
    assert!(!encoder_takes_options("no_such_encoder", &Tags::default()));
}

#[test]
fn a_speaker_mask_names_a_layout_and_an_empty_mask_names_none() {
    assert_eq!(
        ChannelLayout::from_mask(0x3),
        Some(ChannelLayout::default_for(2))
    );
    assert_eq!(
        ChannelLayout::from_mask(0x60f).map(ChannelLayout::channels),
        Some(6)
    );
    assert_eq!(ChannelLayout::from_mask(0), None);
}

#[test]
fn a_tag_is_found_whatever_the_case_of_its_key() {
    let tags = tags(&[("iTunSMPB", " 00000000 00000840"), ("title", "Book")]);

    assert_eq!(
        tags.get("ITUNSMPB").map(Text::display).as_deref(),
        Some(" 00000000 00000840")
    );
    assert_eq!(
        tags.get("Title").map(Text::display).as_deref(),
        Some("Book")
    );
    assert!(tags.get("album").is_none());
}

#[test]
fn text_reads_as_utf8_when_it_is_and_as_latin1_when_it_is_not() {
    let utf8 = Text::new("Æsop — Fables", Step::SetTags).expect("text");

    assert_eq!(utf8.display(), "Æsop — Fables");
    assert_eq!(
        Text::from_bytes(b"Caf\xe9 M\xfcller", Step::SetTags)
            .expect("latin-1 bytes")
            .display(),
        "Café Müller"
    );
}

#[test]
fn an_error_names_an_operating_system_failure_apart_from_ffmpegs_own() {
    let dir = TempDir::new().expect("dir");
    let missing = match Input::open(&dir.path().join("missing.m4b"), &SecretOptions::default()) {
        Ok(_) => panic!("the input opened"),
        Err(error) => error,
    };
    let junk = dir.path().join("junk.m4b");
    std::fs::write(&junk, vec![0x5a_u8; 4096]).expect("write junk");
    let invalid = match Input::open(&junk, &SecretOptions::default()) {
        Ok(_) => panic!("the input opened"),
        Err(error) => error,
    };

    assert!(missing.os_error().is_some(), "{missing:?}");
    assert_eq!(invalid.os_error(), None, "{invalid:?}");
    assert!(invalid.is_invalid_data(), "{invalid:?}");
}
