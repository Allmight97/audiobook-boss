//! What an opened file reports, and the facts FFmpeg does not know.

use std::path::{Path, PathBuf};

use abb_ffmpeg::{
    library_versions, log_level, set_log_level, AacStream, CodecId, Disposition, Error, Input,
    LogLevel, Medium, Output, Packet, PictureKind, SecretOptions, Step, Tags,
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

fn open(path: &Path) -> Input {
    Input::open(path, &SecretOptions::default())
        .expect("open")
        .input
}

#[test]
fn a_wav_reports_its_container_stream_and_measured_length() {
    let dir = TempDir::new().expect("dir");
    let input = open(&write_wav(dir.path()));
    let index = input.best_audio_stream().expect("audio stream");
    let stream = input.streams().remove(index);

    assert_eq!(input.format_name(), "wav");
    assert_eq!(input.duration_micros(), Some(1_000_000));
    assert!(!input.duration_is_estimated());
    assert!(input.chapters().is_empty());
    assert_eq!(stream.medium, Medium::Audio);
    assert_eq!(stream.codec, CodecId::PCM_S16LE);
    assert_eq!((stream.sample_rate, stream.channels), (44_100, 1));
    assert_eq!(stream.duration, Some(i64::from(RATE)));
    assert_eq!(input.attached_picture(index), None);
    assert_eq!(input.stream_extradata(index), None);
    assert_eq!(input.attached_picture(index + 1), None);
}

#[test]
fn a_packet_knows_where_it_came_from_until_that_is_cleared() {
    let dir = TempDir::new().expect("dir");
    let mut input = open(&write_wav(dir.path()));
    let mut packet = Packet::new().expect("packet");

    assert!(input.read_packet(&mut packet).expect("read"));

    assert_eq!(packet.position(), Some(44));
    assert_eq!(packet.dts(), Some(0));
    assert!(!packet.is_discard());
    assert!(!packet.data().is_empty());
    packet.clear_position();
    assert_eq!(packet.position(), None);
}

#[test]
fn an_output_named_only_by_its_file_takes_the_muxer_of_the_extension() {
    let dir = TempDir::new().expect("dir");

    let output = Output::create_for_file_name(&dir.path().join("book.mka")).expect("create");

    assert_eq!(output.muxer_name(), "matroska");
}

#[test]
fn a_file_name_no_muxer_claims_is_a_create_error_and_leaves_no_file() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("book.no-such-extension");

    let error = match Output::create_for_file_name(&path) {
        Ok(_) => panic!("the output was created"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::CreateOutput,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!path.exists());
}

#[test]
fn a_cover_written_as_an_attached_picture_reads_back_with_its_bytes_and_size() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("cover.m4b");
    let picture = [0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10, 0x4a, 0x46, 0x49, 0x46];
    let mut output = Output::create(&path, "mp4").expect("create");
    let audio = output
        .add_aac_stream(&AacStream {
            high_efficiency: false,
            rate: RATE,
            channels: 2,
            bit_rate: 64_000,
            frame_samples: 1_024,
            priming_samples: 1_024,
            config: &[0x12, 0x10],
        })
        .expect("audio");
    let cover = output
        .add_attached_picture_stream(PictureKind::Jpeg, 320, 240)
        .expect("cover stream");
    let (mut muxer, _) = output.write_header(&Tags::default()).expect("header");
    let mut packet = Packet::key_frame_of(&picture).expect("cover packet");
    packet.set_stream_index(cover);
    packet.set_pts(Some(0));
    packet.set_dts(Some(0));
    muxer.write_interleaved(&mut packet).expect("write cover");
    let mut packet = Packet::copy_of(&[0x21, 0x10, 0x04, 0x60, 0x8c, 0x1c]).expect("packet");
    packet.set_stream_index(audio);
    packet.set_pts(Some(0));
    packet.set_dts(Some(0));
    packet.set_duration(1_024);
    muxer.write_interleaved(&mut packet).expect("write audio");
    muxer.finish().expect("finish");

    let input = open(&path);
    let pictures: Vec<_> = input
        .streams()
        .into_iter()
        .filter(|stream| stream.disposition.contains(Disposition::ATTACHED_PIC))
        .collect();

    assert_eq!(pictures.len(), 1);
    assert_eq!(pictures[0].codec, CodecId::MJPEG);
    assert_eq!(
        input.attached_picture(pictures[0].index).as_deref(),
        Some(&picture[..])
    );
    assert_eq!(
        input.indexed_byte_ranges(audio).map(|ranges| ranges.len()),
        Some(1)
    );
}

#[test]
fn a_setting_for_a_stream_the_output_lacks_is_a_typed_error() {
    let dir = TempDir::new().expect("dir");
    let mut output = Output::create(&dir.path().join("none.m4b"), "mp4").expect("create");

    for error in [
        output.set_stream_frame_size(0, 120).expect_err("no stream"),
        output
            .set_stream_picture_size(0, 10, 10)
            .expect_err("no stream"),
    ] {
        assert!(
            matches!(error, Error::MissingStream { index: 0, .. }),
            "{error:?}"
        );
    }
}

#[test]
fn the_log_level_reads_back_as_it_was_set() {
    set_log_level(LogLevel::Info);
    assert_eq!(log_level(), Some(LogLevel::Info));

    set_log_level(LogLevel::Warning);
    assert_eq!(log_level(), Some(LogLevel::Warning));
}

#[test]
fn a_disposition_prints_its_flags_by_name() {
    assert_eq!(
        format!("{:?}", Disposition::ATTACHED_PIC),
        "Disposition(ATTACHED_PIC)"
    );
    assert_eq!(format!("{:?}", Disposition::default()), "Disposition(0x0)");
}

#[test]
fn the_linked_libraries_report_a_version() {
    let (codec, format) = library_versions();

    assert!(codec >> 16 > 0);
    assert!(format >> 16 > 0);
}
