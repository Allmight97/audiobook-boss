use super::{output_chapter_count, synthesizes_chapter_per_source};

#[test]
fn several_sources_without_chapters_get_one_chapter_each() {
    assert!(synthesizes_chapter_per_source(3, 0));
    assert_eq!(output_chapter_count(&[0, 0, 0]), 3);
}

#[test]
fn one_chaptered_source_keeps_the_real_chapters_only() {
    assert!(!synthesizes_chapter_per_source(3, 4));
    assert_eq!(output_chapter_count(&[0, 4, 0]), 4);
}

#[test]
fn a_single_chapterless_source_gets_no_chapter() {
    assert!(!synthesizes_chapter_per_source(1, 0));
    assert_eq!(output_chapter_count(&[0]), 0);
}

#[test]
fn one_real_chapter_among_chapterless_sources_is_the_only_chapter() {
    assert!(!synthesizes_chapter_per_source(2, 1));
    assert_eq!(output_chapter_count(&[1, 0]), 1);
}
