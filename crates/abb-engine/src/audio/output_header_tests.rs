use std::path::{Path, PathBuf};

use ffmpeg_next as ff;
use tempfile::TempDir;

use super::*;
use crate::audio::Muxer;
use crate::metadata::{add_cover_art_stream_pre_header, write_cover_art_packet_post_header};
use crate::test_cases::media_fixtures::ffmpeg;

const LONG_CHAPTERS_MS: [(i64, i64, &str); 3] = [
    (0, 600_000, "One"),
    (600_000, 1_200_000, "Two"),
    (1_200_000, 1_201_000, "Three"),
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
fn write_book_with_cover(inputs: &Inputs, output: &Path) {
    ff::init().expect("ffmpeg");
    let mut ictx = ff::format::input(&inputs.audio).expect("open audio");
    let mut octx = ff::format::output_as(output, Muxer::Mp4.ffmpeg_name()).expect("output");
    let in_audio = ictx.stream(0).expect("audio stream");
    let in_time_base = in_audio.time_base();
    let mut out_audio = octx
        .add_stream(ff::encoder::find(ff::codec::Id::None))
        .expect("audio out");
    out_audio.set_parameters(in_audio.parameters());
    out_audio.set_time_base(in_time_base);
    // SAFETY: the stream belongs to `octx` and the header is not written yet.
    unsafe { (*out_audio.parameters().as_mut_ptr()).codec_tag = 0 };
    let (cover_index, cover_format) = add_cover_art_stream_pre_header(&mut octx, &inputs.cover)
        .expect("cover stream")
        .expect("cover added");
    octx.stream_mut(cover_index)
        .expect("cover stream")
        .set_time_base((1, 90_000));
    for (id, (start, end, title)) in LONG_CHAPTERS_MS.iter().enumerate() {
        octx.add_chapter(id as i64, (1, 1000), *start, *end, title)
            .expect("chapter");
    }

    write_output_header(&mut octx, ff::Dictionary::new()).expect("header");

    write_cover_art_packet_post_header(&mut octx, cover_index, &inputs.cover, cover_format)
        .expect("cover packet");
    let out_time_base = octx.stream(0).expect("audio out").time_base();
    for (stream, mut packet) in ictx.packets() {
        if stream.index() != 0 {
            continue;
        }
        packet.rescale_ts(in_time_base, out_time_base);
        packet.set_stream(0);
        packet.set_position(-1);
        packet.write_interleaved(&mut octx).expect("audio packet");
    }
    octx.write_trailer().expect("trailer");
}

fn chapter_starts_and_titles(path: &Path) -> Vec<(i64, String)> {
    let input = ff::format::input(path).expect("read back");
    input
        .chapters()
        .map(|chapter| {
            let start_ms = chapter.start() * 1000 * i64::from(chapter.time_base().numerator())
                / i64::from(chapter.time_base().denominator());
            let title = chapter
                .metadata()
                .get("title")
                .unwrap_or_default()
                .to_string();
            (start_ms, title)
        })
        .collect()
}

#[test]
fn an_mp4_with_a_cover_keeps_chapters_longer_than_eight_minutes() {
    let inputs = inputs();
    let output = inputs._dir.path().join("book.m4b");

    write_book_with_cover(&inputs, &output);

    let expected: Vec<(i64, String)> = LONG_CHAPTERS_MS
        .iter()
        .map(|(start, _, title)| (*start, (*title).to_string()))
        .collect();
    assert_eq!(chapter_starts_and_titles(&output), expected);
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
