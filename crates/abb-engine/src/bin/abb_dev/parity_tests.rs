use serde_json::json;

use super::*;

fn arguments(line: &str) -> impl Iterator<Item = String> + '_ {
    line.split_whitespace().map(str::to_string)
}

fn recorded() -> FileRecord {
    FileRecord {
        bytes: "b1".into(),
        audio: "a1".into(),
        covers: vec!["c1".into()],
        container: json!({ "format_name": "mov,mp4", "tags": { "title": "Book" } }),
        streams: json!([{ "index": 0, "codec_name": "aac" }]),
        chapters: json!([{ "start_time": "0.000000", "tags": { "title": "One" } }]),
    }
}

#[test]
fn a_check_takes_files_a_reference_and_the_bytes_flag() {
    assert_eq!(
        parse(arguments("check a.m4b b.m4b --reference r.json --bytes")),
        Ok(Options {
            mode: Mode::Check { bytes: true },
            files: vec!["a.m4b".into(), "b.m4b".into()],
            reference: "r.json".into(),
        })
    );
}

#[test]
fn a_command_line_that_cannot_run_is_refused() {
    for line in [
        "",
        "compare a.m4b --reference r.json",
        "record --reference r.json",
        "record a.m4b",
        "record a.m4b --reference r.json --bytes",
        "check a.m4b --reference r.json --fast",
    ] {
        assert!(parse(arguments(line)).is_err(), "{line}");
    }
}

#[test]
fn equal_content_with_other_bytes_differs_only_when_bytes_are_compared() {
    let rewritten = FileRecord {
        bytes: "b2".into(),
        ..recorded()
    };

    assert!(differences(&recorded(), &rewritten, false).is_empty());
    assert_eq!(differences(&recorded(), &rewritten, true), ["bytes"]);
}

#[test]
fn each_changed_part_is_named() {
    let changed = FileRecord {
        bytes: "b1".into(),
        audio: "a2".into(),
        covers: Vec::new(),
        container: json!({ "format_name": "mov,mp4", "tags": { "title": "Other" } }),
        streams: json!([{ "index": 0, "codec_name": "opus" }]),
        chapters: json!([]),
    };

    assert_eq!(
        differences(&recorded(), &changed, true),
        ["audio", "covers", "container", "streams", "chapters"]
    );
}

#[test]
fn the_muxer_version_stamp_is_not_content() {
    let probed = json!({
        "format": { "tags": { "title": "Book", "encoder": "Lavf62.1.100" } },
        "streams": [{ "tags": { "ENCODER": "Lavc62.3.100 aac", "language": "eng" } }],
    });

    assert_eq!(
        without_muxer_stamp(probed),
        json!({
            "format": { "tags": { "title": "Book" } },
            "streams": [{ "tags": { "language": "eng" } }],
        })
    );
}

#[test]
fn covers_are_the_attached_picture_streams() {
    let streams = json!([
        { "index": 0, "disposition": { "attached_pic": 0 } },
        { "index": 1, "disposition": { "attached_pic": 1 } },
    ]);

    assert_eq!(cover_stream_indexes(&streams), [1]);
}
