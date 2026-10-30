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
fn into_option_returns_none_for_empty_passthrough() {
    assert!(PassthroughMetadata::default().into_option().is_none());
}

#[test]
fn cover_art_only_drops_chapters_and_keeps_cover_art() {
    let passthrough = PassthroughMetadata {
        chapters: vec![ChapterSpec {
            title: Some("Chapter 1".to_string()),
            start_ms: 0,
            end_ms: 1_000,
        }],
        cover_art: Some(vec![1, 2, 3]),
    };

    let cover_only = passthrough.cover_art_only().expect("cover art passthrough");
    assert!(
        cover_only.chapters.is_empty(),
        "preview should not preserve chapters"
    );
    assert_eq!(cover_only.cover_art, Some(vec![1, 2, 3]));
}

#[test]
fn cover_art_only_returns_none_when_passthrough_only_has_chapters() {
    let passthrough = PassthroughMetadata {
        chapters: vec![ChapterSpec {
            title: Some("Chapter 1".to_string()),
            start_ms: 0,
            end_ms: 1_000,
        }],
        cover_art: None,
    };

    assert!(passthrough.cover_art_only().is_none());
}

#[test]
fn without_cover_art_preserves_chapters_and_drops_cover_art() {
    let passthrough = PassthroughMetadata {
        chapters: vec![ChapterSpec {
            title: Some("Chapter 1".to_string()),
            start_ms: 0,
            end_ms: 1_000,
        }],
        cover_art: Some(vec![1, 2, 3]),
    };

    let without_cover = passthrough
        .without_cover_art()
        .expect("chapter passthrough should remain");

    assert_eq!(without_cover.chapters.len(), 1);
    assert_eq!(without_cover.cover_art, None);
}

#[test]
fn without_cover_art_returns_none_when_passthrough_only_has_cover_art() {
    let passthrough = PassthroughMetadata {
        chapters: Vec::new(),
        cover_art: Some(vec![1, 2, 3]),
    };

    assert!(passthrough.without_cover_art().is_none());
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
    };

    let effective = CoverArtPassthroughPolicy::SuppressAfterExplicitClear
        .apply_to_passthrough(Some(passthrough))
        .expect("chapter passthrough remains");

    assert_eq!(effective.chapters.len(), 1);
    assert_eq!(effective.cover_art, None);
}

#[test]
fn merge_passthrough_cover_art_fills_only_missing_cover_art() {
    let passthrough = PassthroughMetadata {
        chapters: Vec::new(),
        cover_art: Some(vec![1, 2, 3]),
    };
    let metadata = AudiobookMetadata {
        title: Some("Known title".to_string()),
        cover_art: None,
        ..Default::default()
    };

    let merged =
        merge_passthrough_cover_art(Some(metadata), Some(&passthrough)).expect("metadata remains");

    assert_eq!(merged.title.as_deref(), Some("Known title"));
    assert_eq!(merged.cover_art, Some(vec![1, 2, 3]));
}

#[test]
fn merge_passthrough_cover_art_keeps_explicit_cover_art() {
    let passthrough = PassthroughMetadata {
        chapters: Vec::new(),
        cover_art: Some(vec![1, 2, 3]),
    };
    let metadata = AudiobookMetadata {
        cover_art: Some(vec![9]),
        ..Default::default()
    };

    let merged =
        merge_passthrough_cover_art(Some(metadata), Some(&passthrough)).expect("metadata remains");

    assert_eq!(merged.cover_art, Some(vec![9]));
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
