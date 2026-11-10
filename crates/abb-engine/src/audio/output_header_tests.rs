use std::path::{Path, PathBuf};

use ffmpeg_next as ff;
use tempfile::TempDir;

use super::*;
use crate::audio::Muxer;
use crate::metadata::{add_chapters, add_cover_stream, write_cover_packet, ChapterSpec};
use crate::test_cases::media_fixtures::ffmpeg;

type Chapters<'a> = &'a [(i64, i64, &'a str)];

const LONG_CHAPTERS_MS: Chapters = &[
    (0, 600_000, "One"),
    (600_000, 1_200_000, "Two"),
    (1_200_000, 1_201_000, "Three"),
];

const FOURTEEN_HOURS_MS: i64 = 14 * 3_600_000;

const BEYOND_THE_SAMPLE_RATE_MS: Chapters = &[
    (0, FOURTEEN_HOURS_MS, "Whole day"),
    (FOURTEEN_HOURS_MS, FOURTEEN_HOURS_MS + 1_000, "Epilogue"),
];

const BEYOND_THE_SAMPLE_RATE_GAP_MS: Chapters = &[
    (0, 1_000, "Prologue"),
    (FOURTEEN_HOURS_MS, FOURTEEN_HOURS_MS + 1_000, "Epilogue"),
];

struct Inputs {
    audio: PathBuf,
    cover: Vec<u8>,
    _dir: TempDir,
}

fn inputs() -> Inputs {
    let dir = TempDir::new().expect("temp dir");
    let audio = dir.path().join("audio.m4a");
    let cover = dir.path().join("cover.jpg");
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=44100:cl=stereo",
        "-t",
        "1",
        "-c:a",
        "aac",
        audio.to_str().expect("audio path"),
    ]);
    ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=64x64:d=1",
        "-frames:v",
        "1",
        cover.to_str().expect("cover path"),
    ]);
    Inputs {
        audio,
        cover: std::fs::read(&cover).expect("read cover"),
        _dir: dir,
    }
}

/// Writes an MP4 the way the decrypt copy does: the audio stream copied, a
/// cover whose time base is the 1/90000 an Audible download carries, and the
/// given chapters.
fn write_book_with_cover(inputs: &Inputs, output_path: &Path, chapters: Chapters) {
    let mut input = abb_ffmpeg::Input::open(&inputs.audio, &abb_ffmpeg::SecretOptions::default())
        .expect("open audio")
        .input;
    let mut output =
        abb_ffmpeg::Output::create(output_path, Muxer::Mp4.ffmpeg_name()).expect("output");
    let in_audio = input.streams().remove(0);
    let out_audio = output
        .add_stream_copied_from(&input, in_audio.index)
        .expect("audio out");
    output
        .set_stream_time_base(out_audio, in_audio.time_base)
        .expect("audio time base");
    output
        .set_stream_disposition_and_clear_codec_tag(out_audio, in_audio.disposition)
        .expect("clear codec tag");
    let cover = add_cover_stream(&mut output, &inputs.cover)
        .expect("cover stream")
        .expect("cover added");
    output
        .set_stream_time_base(cover.index, abb_ffmpeg::Rational::new(1, 90_000))
        .expect("cover time base");
    let specs: Vec<ChapterSpec> = chapters
        .iter()
        .map(|(start, end, title)| ChapterSpec {
            title: Some((*title).to_string()),
            start_ms: *start,
            end_ms: *end,
        })
        .collect();
    add_chapters(&mut output, &specs).expect("chapters");

    let mut muxer = write_layer_output_header(output).expect("header");

    write_cover_packet(&mut muxer, cover, &inputs.cover).expect("cover packet");
    let out_time_base = muxer.stream_time_base(out_audio).expect("audio out");
    let mut packet = abb_ffmpeg::Packet::new().expect("packet");
    while input.read_packet(&mut packet).expect("read packet") {
        if packet.stream_index() != in_audio.index {
            continue;
        }
        packet.rescale_ts(in_audio.time_base, out_time_base);
        packet.set_stream_index(out_audio);
        muxer.write_interleaved(&mut packet).expect("audio packet");
    }
    muxer.finish().expect("trailer");
}

fn chapter_starts_and_titles(path: &Path) -> Vec<(i64, String)> {
    let input = abb_ffmpeg::Input::open(path, &abb_ffmpeg::SecretOptions::default())
        .expect("read back")
        .input;
    input
        .chapters()
        .iter()
        .map(|chapter| {
            let start_ms = chapter.start * 1000 * i64::from(chapter.time_base.numerator)
                / i64::from(chapter.time_base.denominator);
            let title = chapter
                .title()
                .map(abb_ffmpeg::Text::display)
                .unwrap_or_default();
            (start_ms, title)
        })
        .collect()
}

fn expected_starts_and_titles(chapters: Chapters) -> Vec<(i64, String)> {
    chapters
        .iter()
        .map(|(start, _, title)| (*start, (*title).to_string()))
        .collect()
}

#[test]
fn an_mp4_with_a_cover_keeps_chapters_longer_than_eight_minutes() {
    let inputs = inputs();
    let output = inputs._dir.path().join("book.m4b");

    write_book_with_cover(&inputs, &output, LONG_CHAPTERS_MS);

    assert_eq!(
        chapter_starts_and_titles(&output),
        expected_starts_and_titles(LONG_CHAPTERS_MS)
    );
}

#[test]
fn a_chapter_longer_than_the_sample_rate_allows_keeps_its_start_and_title() {
    let inputs = inputs();
    let output = inputs._dir.path().join("book.m4b");

    write_book_with_cover(&inputs, &output, BEYOND_THE_SAMPLE_RATE_MS);

    assert_eq!(
        chapter_starts_and_titles(&output),
        expected_starts_and_titles(BEYOND_THE_SAMPLE_RATE_MS)
    );
}

#[test]
fn a_gap_between_chapter_starts_longer_than_the_sample_rate_allows_keeps_every_start() {
    let inputs = inputs();
    let output = inputs._dir.path().join("book.m4b");

    write_book_with_cover(&inputs, &output, BEYOND_THE_SAMPLE_RATE_GAP_MS);

    assert_eq!(
        chapter_starts_and_titles(&output),
        expected_starts_and_titles(BEYOND_THE_SAMPLE_RATE_GAP_MS)
    );
}

#[test]
fn an_option_the_muxer_does_not_take_is_an_error() {
    let dir = TempDir::new().expect("temp dir");
    ff::init().expect("ffmpeg");
    let output = dir.path().join("book.m4b");
    let mut octx = ff::format::output_as(&output, Muxer::Mp4.ffmpeg_name()).expect("output");
    let mut stream = octx
        .add_stream(ff::encoder::find(ff::codec::Id::AAC))
        .expect("stream");
    stream.set_time_base((1, 44_100));
    // SAFETY: the stream belongs to `octx` and the header is not written yet.
    unsafe {
        let parameters = stream.parameters().as_mut_ptr();
        (*parameters).codec_type = ff::ffi::AVMediaType::AVMEDIA_TYPE_AUDIO;
        (*parameters).codec_id = ff::ffi::AVCodecID::AV_CODEC_ID_AAC;
        (*parameters).sample_rate = 44_100;
        ff::ffi::av_channel_layout_default(&mut (*parameters).ch_layout, 2);
    }
    let mut options = ff::Dictionary::new();
    options.set("no_such_option", "1");

    let error = write_output_header(&mut octx, options).expect_err("unknown option");

    assert!(error.to_string().contains("no_such_option"), "{error}");
}
