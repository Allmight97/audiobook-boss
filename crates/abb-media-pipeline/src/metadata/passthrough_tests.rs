use super::{
    extract_passthrough_metadata, merge_passthrough_cover_art, prepare_output_cover_art,
    ChapterSpec, PassthroughMetadata, PassthroughSource,
};
use crate::metadata::{AudiobookMetadata, CoverArtPassthroughPolicy};

#[test]
fn chapterless_sources_synthesize_one_chapter_per_file() {
    let source = |path: &str, duration: f64| PassthroughSource {
        chapters: None,
        path: std::path::PathBuf::from(path),
        duration: Some(duration),
        is_valid: true,
    };

    let passthrough = extract_passthrough_metadata(&[
        source("chapter-one.m4b", 60.0),
        source("chapter-two.m4b", 90.0),
    ]);

    assert_eq!(passthrough.chapters.len(), 2);
    assert_eq!(passthrough.chapters[0].start_ms, 0);
    assert_eq!(passthrough.chapters[1].start_ms, 60_000);
}

#[test]
fn a_source_with_no_length_is_named_because_it_shifts_later_chapters() {
    let source = |path: &str, duration: Option<f64>| PassthroughSource {
        chapters: None,
        path: std::path::PathBuf::from(path),
        duration,
        is_valid: true,
    };

    let passthrough = extract_passthrough_metadata(&[
        source("/books/one.m4b", None),
        source("/books/two.m4b", Some(90.0)),
    ]);

    assert!(passthrough.warnings.contains(
        &"one.m4b has no known length, so the chapters after it may start too early.".to_string()
    ));
    assert_eq!(passthrough.chapters.len(), 1);
}

#[test]
fn a_source_with_no_length_is_named_only_when_a_later_source_has_chapters() {
    let chapter = || ChapterSpec {
        title: Some("Chapter".to_string()),
        start_ms: 0,
        end_ms: 1_000,
    };
    let source =
        |path: &str, duration: Option<f64>, chapters: Vec<ChapterSpec>| PassthroughSource {
            chapters: Some(chapters),
            path: std::path::PathBuf::from(path),
            duration,
            is_valid: true,
        };
    let no_length_warnings = |sources: &[PassthroughSource]| -> Vec<String> {
        extract_passthrough_metadata(sources)
            .warnings
            .into_iter()
            .filter(|warning| warning.contains("no known length"))
            .collect()
    };
    let one = "one.m4b has no known length, so the chapters after it may start too early.";

    assert_eq!(
        no_length_warnings(&[
            source("/books/one.m4b", None, vec![chapter()]),
            source("/books/two.m4b", Some(90.0), vec![chapter()]),
        ]),
        [one]
    );
    assert!(no_length_warnings(&[
        source("/books/one.m4b", None, vec![chapter()]),
        source("/books/two.m4b", Some(90.0), Vec::new()),
    ])
    .is_empty());
    assert!(no_length_warnings(&[
        source("/books/two.m4b", Some(90.0), vec![chapter()]),
        source("/books/one.m4b", None, vec![chapter()]),
    ])
    .is_empty());
}

#[test]
fn dropping_the_cover_leaves_nothing_when_only_the_cover_was_passed_through() {
    let passthrough = PassthroughMetadata {
        chapters: Vec::new(),
        cover_art: Some(vec![1, 2, 3]),
        ..Default::default()
    };

    assert!(passthrough.without_cover_art().is_none());
    assert!(PassthroughMetadata::default().into_option().is_none());
}

#[test]
fn cover_art_passthrough_policy_drops_only_cover_art_after_explicit_clear() {
    let passthrough = PassthroughMetadata {
        chapters: vec![ChapterSpec {
            title: Some("Chapter 1".to_string()),
            start_ms: 0,
            end_ms: 1_000,
        }],
        cover_art: Some(vec![1, 2, 3]),
        ..Default::default()
    };

    let effective = CoverArtPassthroughPolicy::SuppressAfterExplicitClear
        .apply_to_passthrough(Some(passthrough))
        .expect("chapter passthrough remains");

    assert_eq!(effective.chapters.len(), 1);
    assert_eq!(effective.cover_art, None);
}

#[test]
fn a_source_cover_fills_only_a_missing_cover() {
    let passthrough = PassthroughMetadata {
        chapters: Vec::new(),
        cover_art: Some(vec![1, 2, 3]),
        ..Default::default()
    };

    let missing = merge_passthrough_cover_art(
        Some(AudiobookMetadata {
            title: Some("Known title".to_string()),
            cover_art: None,
            ..Default::default()
        }),
        Some(&passthrough),
    )
    .expect("metadata remains");
    assert_eq!(missing.title.as_deref(), Some("Known title"));
    assert_eq!(missing.cover_art, Some(vec![1, 2, 3]));

    let chosen = merge_passthrough_cover_art(
        Some(AudiobookMetadata {
            cover_art: Some(vec![9]),
            ..Default::default()
        }),
        Some(&passthrough),
    )
    .expect("metadata remains");
    assert_eq!(chosen.cover_art, Some(vec![9]));

    let no_metadata =
        merge_passthrough_cover_art(None, Some(&passthrough)).expect("cover becomes metadata");
    assert_eq!(no_metadata.cover_art, Some(vec![1, 2, 3]));
}

#[test]
fn prepare_output_cover_art_drops_passthrough_cover_after_adopting_jpeg() {
    let jpeg = {
        use image::{DynamicImage, ImageBuffer, ImageFormat};
        use std::io::Cursor;
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(64, 64, image::Rgb([1, 2, 3])));
        let mut source = Cursor::new(Vec::new());
        image
            .write_to(&mut source, ImageFormat::Jpeg)
            .expect("fixture jpeg");
        source.into_inner()
    };
    let passthrough = PassthroughMetadata {
        chapters: vec![ChapterSpec {
            title: Some("Chapter 1".to_string()),
            start_ms: 0,
            end_ms: 1_000,
        }],
        cover_art: Some(jpeg.clone()),
        ..Default::default()
    };

    let (metadata, passthrough, warning) =
        prepare_output_cover_art(None, Some(passthrough), CoverArtPassthroughPolicy::Preserve)
            .expect("write-ready cover should prepare");
    assert_eq!(warning, None);

    assert_eq!(
        metadata.expect("cover adopted onto metadata").cover_art,
        Some(jpeg)
    );
    let passthrough = passthrough.expect("chapters remain");
    assert_eq!(passthrough.chapters.len(), 1);
    assert_eq!(passthrough.cover_art, None);
}
