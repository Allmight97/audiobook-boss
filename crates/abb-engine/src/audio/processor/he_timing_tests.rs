use super::*;

#[test]
fn tool_provenance_preserves_old_he_timing_and_excludes_other_encoders() {
    for (tool, expected) in [
        (Some("AudioBook Boss FAAC HE-AAC"), Some(2079)),
        (Some("AudioBook Boss FAAC HE-AAC timing-2"), Some(2080)),
        (Some("AudioBook Boss FAAC AAC-LC"), None),
        (Some("FAAC"), None),
        (None, None),
    ] {
        assert_eq!(core_priming(tool), expected);
    }
}

#[test]
fn itunsmpb_reads_priming_and_length_like_ffmpeg() {
    let apple = " 00000000 00000840 00000518 000000000006BAA8 00000000 00000000";
    assert_eq!(
        itunes_gapless(apple),
        Some(Gapless {
            priming: 0x840,
            samples: 0x6BAA8,
            remainder: 0x518
        })
    );
    for unusable in [
        "",
        "garbage",
        " 00000000 00000000 00000518 000000000006BAA8",
        " 00000000 00004000 0 1",
        " 00000000 00000840 not-hex 000000000006BAA8",
        " 00000000 00000840 00000518",
    ] {
        assert_eq!(itunes_gapless(unusable), None, "{unusable:?}");
    }
}

#[test]
fn stale_itunsmpb_length_keeps_ffmpeg_trimming() {
    let (priming, samples, remainder) = (0x840, 0x6BAA8, 0x518);
    let coded = priming + samples + remainder;
    assert!(agrees_with_duration(samples, coded, samples));
    assert!(agrees_with_duration(samples, coded, coded));
    assert!(!agrees_with_duration(samples, coded, coded + 44_100 * 60));
    assert!(!agrees_with_duration(samples, coded, samples - 4096));
}
