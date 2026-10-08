use super::{PacketTrim, Trimmer};

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

#[test]
fn trailing_padding_applies_only_when_it_fits_the_kept_frame() {
    let mut trimmer = Trimmer::default();
    assert_eq!(trimmer.range(&trim(1024, 0, 300, false)), Some((0, 724)));
    assert_eq!(trimmer.range(&trim(1024, 0, 1024, false)), None);
    // libavcodec ignores padding longer than the frame.
    assert_eq!(trimmer.range(&trim(1024, 0, 2000, false)), Some((0, 1024)));
    // Padding compares against the samples left after the leading skip.
    assert_eq!(
        trimmer.range(&trim(1024, 600, 500, false)),
        Some((600, 1024))
    );
    assert_eq!(trimmer.counts.padding_samples, 300 + 1024);
}
