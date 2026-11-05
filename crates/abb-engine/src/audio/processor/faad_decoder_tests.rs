use super::{speaker_layout, FaadFormat, PacketTrim, Trimmer};
use ffmpeg_next as ff;

fn trim(frames: usize, skip: u32, padding: u32, discard: bool) -> PacketTrim {
    PacketTrim {
        frames,
        skip,
        padding,
        discard,
    }
}

#[test]
fn leading_skip_spans_frames_like_libavcodec() {
    let mut trimmer = Trimmer::default();
    assert_eq!(trimmer.range(&trim(2048, 3000, 0, false)), None);
    // A zero skip field keeps the running count instead of resetting it.
    assert_eq!(trimmer.range(&trim(2048, 0, 0, false)), Some((952, 2048)));
    assert_eq!(trimmer.range(&trim(2048, 0, 0, false)), Some((0, 2048)));
    assert_eq!(trimmer.counts.skipped_samples, 3000);
}

#[test]
fn discard_flag_drops_the_frame_and_consumes_leading_skip() {
    let mut trimmer = Trimmer::default();
    assert_eq!(trimmer.range(&trim(1024, 1500, 0, true)), None);
    assert_eq!(trimmer.range(&trim(1024, 0, 0, false)), Some((476, 1024)));
    assert_eq!(trimmer.counts.discarded_packets, 1);
}

/// FFmpeg's `tests/ref/fate/api-skip-samples` at the pinned source:
/// (skip, padding, samples kept) for one 1000-sample frame.
const UPSTREAM_SKIP_SAMPLES: [(u32, u32, usize); 7] = [
    (0, 0, 1000),
    (300, 0, 700),
    (0, 400, 600),
    (300, 400, 300),
    (300, 700, 0),
    (300, 800, 0),
    (300, 1100, 700),
];

#[test]
fn skip_and_padding_keep_what_libavcodec_keeps() {
    for (skip, padding, kept) in UPSTREAM_SKIP_SAMPLES {
        let range = Trimmer::default().range(&trim(1000, skip, padding, false));
        let samples = range.map_or(0, |(start, end)| end - start);
        assert_eq!(samples, kept, "skip {skip} padding {padding}");
    }
}

#[test]
fn padding_counts_only_the_samples_it_removes() {
    let mut trimmer = Trimmer::default();
    trimmer.range(&trim(1000, 0, 400, false));
    trimmer.range(&trim(1000, 300, 800, false));
    trimmer.range(&trim(1000, 0, 1100, false));
    assert_eq!(trimmer.counts.padding_samples, 400 + 700);
}

fn format(channels: u32, channel_mask: u32) -> FaadFormat {
    FaadFormat {
        rate: 44_100,
        channels,
        channel_mask,
        object_type: 2,
        decoder_delay: 0,
        frame_samples: 1024,
    }
}

#[test]
fn speaker_mask_names_the_center_channel_not_lfe() {
    let layout = speaker_layout(&format(3, 0x7)).expect("FL FR FC");
    assert_eq!(layout.channels(), 3);
    assert_eq!(layout.bits(), ff::sys::AV_CH_LAYOUT_SURROUND);
    assert_ne!(
        layout,
        ff::ChannelLayout::default(3),
        "count-based 3 channels is 2.1"
    );
}

#[test]
fn a_stream_without_a_speaker_mask_is_refused() {
    assert!(speaker_layout(&format(4, 0)).is_err());
    assert!(
        speaker_layout(&format(2, 0x7)).is_err(),
        "mask and count disagree"
    );
}

#[test]
fn packets_of_other_streams_do_not_use_up_format_discovery() {
    let dir = tempfile::tempdir().expect("fixture directory");
    let subtitles = dir.path().join("cues.srt");
    let cues: String = (0..150)
        .map(|cue| {
            let ms = cue * 6;
            format!(
                "{}\n00:00:00,{ms:03} --> 00:00:00,{:03}\ncue\n\n",
                cue + 1,
                ms + 5
            )
        })
        .collect();
    std::fs::write(&subtitles, cues).expect("write cues");
    let fixture = dir.path().join("late-audio.mkv");
    crate::test_cases::media_fixtures::ffmpeg([
        "-hide_banner",
        "-loglevel",
        "error",
        "-itsoffset",
        "1",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=44100:duration=1",
        "-i",
        subtitles.to_str().expect("utf8 path"),
        "-map",
        "0:a",
        "-map",
        "1:s",
        "-c:a",
        "aac",
        "-c:s",
        "srt",
        "-y",
        fixture.to_str().expect("utf8 path"),
    ]);

    let input = ff::format::input(&fixture).expect("open fixture");
    let audio = input
        .streams()
        .best(ff::media::Type::Audio)
        .expect("audio stream");
    // SAFETY: the parameters belong to `input`, which outlives this copy of the extradata.
    let asc = unsafe {
        let raw = &*audio.parameters().as_ptr();
        std::slice::from_raw_parts(raw.extradata, raw.extradata_size as usize).to_vec()
    };
    let format = super::discover_format(&fixture, audio.index(), Some(&asc)).expect("discovers");
    assert_eq!((format.rate, format.channels), (44_100, 1));
}
