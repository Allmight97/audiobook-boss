//! Failure paths of the decode, resample, and encode operations, on a WAV
//! written here. Each returns a typed error and frees what it allocated;
//! `leaks --atExit` on this test binary checks the second part.

use std::path::{Path, PathBuf};

use abb_ffmpeg::{
    AudioDecoder, AudioEncoder, AudioFrame, AudioShape, ChannelLayout, EncoderRequest, Error,
    Input, Output, Packet, Progress, Resampler, SampleFormat, SecretOptions, SkipSamples, Step,
    Tags,
};
use tempfile::TempDir;

const RATE: u32 = 44_100;

fn write_wav(dir: &Path) -> PathBuf {
    let samples: Vec<u8> = (0..RATE)
        .flat_map(|n| (((n % 100) as i16 - 50) * 300).to_le_bytes())
        .collect();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&samples);
    let path = dir.join("tone.wav");
    std::fs::write(&path, bytes).expect("write wav");
    path
}

fn open_wav(dir: &Path) -> (Input, usize) {
    let input = Input::open(&write_wav(dir), &SecretOptions::default())
        .expect("open wav")
        .input;
    let index = input.best_audio_stream().expect("audio stream");
    (input, index)
}

fn mono(format: SampleFormat, rate: u32) -> AudioShape {
    AudioShape {
        format,
        layout: ChannelLayout::default_for(1),
        rate,
    }
}

fn options(pairs: &[(&str, &str)]) -> Tags {
    let mut tags = Tags::default();
    for (name, value) in pairs {
        tags.push(name, value, Step::OpenEncoder).expect("option");
    }
    tags
}

fn aac_request<'a>(options: &'a Tags) -> EncoderRequest<'a> {
    EncoderRequest {
        codec_name: "aac",
        rate: RATE,
        layout: ChannelLayout::default_for(1),
        format: SampleFormat::F32_PLANAR,
        bit_rate: 64_000,
        compression_level: None,
        global_header: true,
        options,
    }
}

fn open_error<T>(result: Result<T, Error>) -> Error {
    match result {
        Ok(_) => panic!("the operation succeeded"),
        Err(error) => error,
    }
}

#[test]
fn a_decoder_that_this_build_lacks_is_a_typed_open_error() {
    let dir = TempDir::new().expect("dir");
    let (input, index) = open_wav(dir.path());

    let error = open_error(AudioDecoder::open(&input, index, Some("no_such_decoder")));

    assert_eq!(
        error,
        Error::CodecNotFound {
            step: Step::OpenDecoder
        }
    );
}

#[test]
fn a_decoder_for_another_codec_fails_to_open() {
    let dir = TempDir::new().expect("dir");
    let (input, index) = open_wav(dir.path());

    let error = open_error(AudioDecoder::open(&input, index, Some("aac")));

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::OpenDecoder,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_decoder_for_a_stream_the_input_lacks_is_a_typed_open_error() {
    let dir = TempDir::new().expect("dir");
    let (input, _) = open_wav(dir.path());

    let error = open_error(AudioDecoder::open(&input, 7, None));

    assert_eq!(
        error,
        Error::MissingStream {
            step: Step::OpenDecoder,
            index: 7
        }
    );
}

#[test]
fn an_encoder_reports_the_private_option_it_did_not_take_by_name_only() {
    let options = options(&[("aac_coder", "nmr"), ("no_such_option", "secret-value")]);

    let opened = AudioEncoder::open(&aac_request(&options)).expect("FFmpeg opens the encoder");

    assert_eq!(opened.unused_options, vec!["no_such_option".to_string()]);
    assert_eq!(opened.encoder.option_int("aac_coder").expect("coder"), 2);
}

#[test]
fn an_encoder_refuses_a_value_its_private_option_does_not_have() {
    let options = options(&[("aac_coder", "no-such-coder")]);

    let error = open_error(AudioEncoder::open(&aac_request(&options)));

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::OpenEncoder,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!error.to_string().contains("no-such-coder"), "{error}");
}

#[test]
fn an_encoder_that_this_build_lacks_and_a_format_it_refuses_are_typed_open_errors() {
    let none = Tags::default();
    let missing = EncoderRequest {
        codec_name: "no_such_encoder",
        ..aac_request(&none)
    };
    assert_eq!(
        open_error(AudioEncoder::open(&missing)),
        Error::CodecNotFound {
            step: Step::OpenEncoder
        }
    );

    let packed = EncoderRequest {
        format: SampleFormat::S16,
        ..aac_request(&none)
    };
    let error = open_error(AudioEncoder::open(&packed));
    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::OpenEncoder,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn reading_an_option_the_encoder_lacks_is_a_typed_error() {
    let none = Tags::default();
    let encoder = AudioEncoder::open(&aac_request(&none))
        .expect("open")
        .encoder;

    let error = open_error(encoder.option_int("no_such_option"));

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::ReadEncoderOption,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_resampler_refuses_a_zero_rate_and_an_option_it_does_not_have() {
    let error = open_error(Resampler::new(
        mono(SampleFormat::S16, 0),
        mono(SampleFormat::F32_PLANAR, RATE),
        &Tags::default(),
    ));
    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::CreateResampler,
                ..
            }
        ),
        "{error:?}"
    );

    let mut unknown = Tags::default();
    unknown
        .push("no_such_option", "secret-value", Step::CreateResampler)
        .expect("option");
    let error = open_error(Resampler::new(
        mono(SampleFormat::S16, RATE),
        mono(SampleFormat::F32_PLANAR, RATE),
        &unknown,
    ));
    assert_eq!(
        error,
        Error::OptionsNotTaken {
            step: Step::CreateResampler,
            names: vec!["no_such_option".to_string()]
        }
    );
    assert!(!error.to_string().contains("secret-value"));
}

#[test]
fn a_resampler_refuses_a_frame_of_another_shape() {
    let mut resampler = Resampler::new(
        mono(SampleFormat::S16, RATE),
        mono(SampleFormat::F32_PLANAR, RATE),
        &Tags::default(),
    )
    .expect("resampler");
    let other = AudioFrame::allocate(SampleFormat::S16, ChannelLayout::default_for(2), 22_050, 64)
        .expect("frame");
    let mut output = AudioFrame::allocate(
        SampleFormat::F32_PLANAR,
        ChannelLayout::default_for(1),
        RATE,
        64,
    )
    .expect("frame");

    let error = open_error(resampler.convert(&other, &mut output));

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::Resample,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn a_frame_gives_only_the_planes_its_format_has() {
    let mut planar = AudioFrame::allocate(
        SampleFormat::F32_PLANAR,
        ChannelLayout::default_for(2),
        RATE,
        32,
    )
    .expect("frame");
    assert_eq!(planar.planes(), 2);
    assert_eq!(planar.plane_f32_mut(1).expect("plane").len(), 32);
    assert_eq!(open_error(planar.plane_f32(2)), Error::WrongFrameShape);
    assert_eq!(open_error(planar.packed_bytes()), Error::WrongFrameShape);

    let packed = AudioFrame::allocate(SampleFormat::S16, ChannelLayout::default_for(2), RATE, 32)
        .expect("frame");
    assert_eq!(packed.packed_bytes().expect("bytes").len(), 32 * 2 * 2);
    assert_eq!(open_error(packed.plane_f32(0)), Error::WrongFrameShape);

    let empty = AudioFrame::empty().expect("frame");
    assert_eq!(empty.planes(), 0);
    assert_eq!(open_error(empty.plane_f32(0)), Error::WrongFrameShape);
}

#[test]
fn skip_samples_side_data_round_trips_and_replaces_itself() {
    let mut packet = Packet::key_frame_of(&[1, 2, 3]).expect("packet");
    assert_eq!(packet.skip_samples().expect("read"), None);
    let first = SkipSamples {
        start: 2080,
        end: 0,
        start_reason: 1,
        end_reason: 2,
    };
    packet.set_skip_samples(first).expect("set");
    assert_eq!(packet.skip_samples().expect("read"), Some(first));

    let second = SkipSamples {
        end: 311,
        ..SkipSamples::default()
    };
    packet.set_skip_samples(second).expect("replace");
    assert_eq!(packet.skip_samples().expect("read"), Some(second));
}

/// Decode, convert, encode, and mux the WAV, then stop: every value is
/// dropped mid-stream, and the muxer removes its file.
#[test]
fn an_encode_stopped_part_way_removes_its_file() {
    let dir = TempDir::new().expect("dir");
    let (mut input, index) = open_wav(dir.path());
    let mut decoder = AudioDecoder::open(&input, index, None).expect("decoder");
    let source = AudioShape {
        format: decoder.format(),
        layout: ChannelLayout::default_for(decoder.channels()),
        rate: decoder.rate(),
    };
    decoder.set_layout(source.layout);
    let none = Tags::default();
    let mut encoder = AudioEncoder::open(&aac_request(&none))
        .expect("encoder")
        .encoder;
    let target = mono(encoder.format(), encoder.rate());
    let mut resampler = Resampler::new(source, target, &none).expect("resampler");

    let path = dir.path().join("stopped.m4b");
    let mut output = Output::create(&path, "ipod").expect("create");
    let stream = output.add_stream_for_encoder(&encoder).expect("stream");
    output
        .set_stream_time_base(stream, encoder.time_base())
        .expect("time base");
    let (mut muxer, unused) = output.write_header(&none).expect("header");
    assert!(unused.is_empty());
    let out_base = muxer.stream_time_base(stream).expect("out base");

    let mut packet = Packet::new().expect("packet");
    let mut encoded = Packet::new().expect("packet");
    let mut frame = AudioFrame::empty().expect("frame");
    let (mut submitted, mut written) = (0_i64, 0);
    'read: while input.read_packet(&mut packet).expect("read") {
        decoder.send_packet(&packet).expect("send");
        while decoder.receive_frame(&mut frame).expect("receive") == Progress::Ready {
            let mut converted =
                AudioFrame::allocate(target.format, target.layout, target.rate, 1024)
                    .expect("frame");
            let mut pending = Some(&frame);
            loop {
                match pending.take() {
                    Some(frame) => resampler.convert(frame, &mut converted).expect("convert"),
                    None => resampler.flush(&mut converted).expect("drain"),
                }
                if converted.samples() < 1024 {
                    break;
                }
                converted.set_pts(Some(submitted));
                submitted += 1024;
                encoder.send_frame(&converted).expect("send frame");
                while encoder
                    .receive_packet(&mut encoded)
                    .expect("receive packet")
                    == Progress::Ready
                {
                    encoded.set_stream_index(stream);
                    encoded.rescale_ts(encoder.time_base(), out_base);
                    muxer.write_interleaved(&mut encoded).expect("write");
                    written += 1;
                    if written == 3 {
                        break 'read;
                    }
                }
                converted = AudioFrame::allocate(target.format, target.layout, target.rate, 1024)
                    .expect("frame");
            }
        }
    }
    assert_eq!(written, 3, "the encode reached the muxer before it stopped");
    assert!(path.exists(), "the muxer wrote to its file");

    drop(muxer);

    assert!(!path.exists(), "a stopped muxer removes its file");
}
