use super::*;
#[test]
fn cue_timestamp_interpretations_and_short_final_entry() {
    let standard = parse_cue("FILE \"book.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Opening\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nINDEX 01 00:13:74", 20_000).expect("standard sheet");
    assert_eq!(standard.interpretation, CueInterpretation::Frames75);
    assert_eq!(standard.chapters[1].start_ms, 13_987);
    assert_eq!(standard.chapters[0].end_ms, 13_987);
    let pregap = parse_cue(
        "FILE \"book.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 00 00:00:94\nINDEX 01 00:01:50",
        20_000,
    )
    .expect("whole-sheet interpretation includes pregap timestamps");
    assert_eq!(pregap.interpretation, CueInterpretation::Hundredths);
    assert_eq!(pregap.chapters[0].start_ms, 1_500);
    let nonstandard = parse_cue("FILE \"Lost Time.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Chapter 01\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nTITLE \"Chapter 02\"\nINDEX 01 00:13:94\nTRACK 42 AUDIO\nTITLE \"Chapter 42\"\nINDEX 01 580:52:68", 34_852_687).expect("hundredths proposal");
    assert_eq!(nonstandard.interpretation, CueInterpretation::Hundredths);
    assert_eq!(nonstandard.chapters[1].start_ms, 13_940);
    assert_eq!(
        nonstandard.chapters[2],
        ChapterSpec {
            title: Some("Chapter 42".into()),
            start_ms: 34_852_680,
            end_ms: 34_852_687
        }
    );
}
fn chapter(title: &str, start_ms: i64, end_ms: i64) -> ChapterSpec {
    ChapterSpec {
        title: Some(title.into()),
        start_ms,
        end_ms,
    }
}
#[test]
fn embedded_chapters_are_repaired_within_one_second() {
    let repaired = normalize_embedded_chapters(
        &[
            chapter("Two", 5_000, 10_030),
            chapter("Marker", 4_200, 4_200),
            chapter("One", 0, 4_990),
        ],
        Some(10_000),
    )
    .expect("small drift is repaired");
    assert_eq!(
        repaired,
        vec![chapter("One", 0, 5_000), chapter("Two", 5_000, 10_000)]
    );
}
#[test]
fn an_estimated_audio_length_neither_moves_nor_refuses_the_last_chapter() {
    let kept = normalize_embedded_chapters(
        &[chapter("One", 0, 4_990), chapter("Two", 5_000, 90_000)],
        None,
    )
    .expect("no audio end to measure against");
    assert_eq!(
        kept,
        vec![chapter("One", 0, 5_000), chapter("Two", 5_000, 90_000)]
    );
}
#[test]
fn embedded_chapters_beyond_repair_are_refused_by_chapter() {
    let past_end = normalize_embedded_chapters(
        &[
            chapter("One", 0, 5_000),
            chapter("Epilogue", 10_000, 12_000),
        ],
        Some(10_000),
    )
    .expect_err("a chapter cannot start at the audio end")
    .to_string();
    assert!(
        past_end.contains("Chapter 2") && past_end.contains("Epilogue"),
        "{past_end}"
    );
    assert!(!past_end.contains("CUE"), "{past_end}");
    let gap = normalize_embedded_chapters(
        &[chapter("One", 0, 3_000), chapter("Two", 5_000, 10_000)],
        Some(10_000),
    )
    .expect_err("a two-second gap is not drift")
    .to_string();
    assert!(gap.contains("Chapter 1") && gap.contains("One"), "{gap}");
}
#[test]
fn chapter_validation_names_the_chapter() {
    let error = validate_chapters(&[chapter("Prologue", 0, 2_000)], 1_000)
        .expect_err("ends after the audio")
        .to_string();
    assert!(
        error.contains("Chapter 1") && error.contains("Prologue") && !error.contains("CUE"),
        "{error}"
    );
}
#[test]
fn cue_file_text_is_a_label_and_never_refuses_a_sheet() {
    for file in ["Dune: Book One.mp3", "C:\\Rips\\Dune.mp3", "../book.mp3"] {
        let sheet = parse_cue(
            &format!("FILE \"{file}\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:00:00"),
            20_000,
        )
        .expect(file);
        assert_eq!(sheet.file_name, file);
    }
    parse_cue(
        "FILE \"C:\\Rips\\Dune.mp3\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00",
        20_000,
    )
    .expect("WAVE sheet");
}
#[test]
fn cue_rejects_unsafe_ambiguous_and_invalid_sheets() {
    for text in [
        "FILE \"a.mp3\" MP3\nFILE \"b.mp3\" MP3",
        "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO",
        "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:60:00",
        "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:00:100",
        "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:00:00\nINDEX 01 00:01:00",
        "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:02:00\nTRACK 02 AUDIO\nINDEX 01 00:01:00",
        "FILE \"a.mp3\" MP3\nTRACK 01 AUDIO\nINDEX 01 00:20:00",
    ] {
        assert!(parse_cue(text, 20_000)
            .expect_err(text)
            .to_string()
            .contains("CUE line"));
    }
}
