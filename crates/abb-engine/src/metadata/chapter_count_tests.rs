use super::{join_chapters, JoinedSource};
use crate::metadata::ChapterSpec;
use std::path::Path;

fn source(name: &str, duration: Option<f64>, chapters: usize) -> (&Path, Option<f64>, usize) {
    (Path::new(name), duration, chapters)
}

fn join(sources: &[(&Path, Option<f64>, usize)]) -> super::JoinedChapters {
    let sources: Vec<_> = sources
        .iter()
        .map(|&(path, duration, chapters)| JoinedSource {
            path,
            duration,
            chapters,
        })
        .collect();
    join_chapters(&sources)
}

fn chapter(title: &str, start_ms: i64, end_ms: i64) -> ChapterSpec {
    ChapterSpec {
        title: Some(title.to_string()),
        start_ms,
        end_ms,
    }
}

#[test]
fn several_sources_without_chapters_get_one_chapter_each() {
    let joined = join(&[
        source("/books/one.m4b", Some(1.5), 0),
        source("/books/two.m4b", Some(2.0004), 0),
        source("/books/three.m4b", Some(0.25), 0),
    ]);
    assert_eq!(
        joined.synthetic,
        [
            chapter("one", 0, 1500),
            chapter("two", 1500, 3500),
            chapter("three", 3500, 3750),
        ]
    );
    assert_eq!(joined.count, 3);
    assert!(joined.warnings.is_empty());
}

#[test]
fn one_chaptered_source_keeps_the_real_chapters_only() {
    let joined = join(&[
        source("a.m4b", Some(1.0), 0),
        source("b.m4b", Some(1.0), 4),
        source("c.m4b", Some(1.0), 0),
    ]);
    assert!(joined.synthetic.is_empty());
    assert_eq!(joined.count, 4);
}

#[test]
fn a_single_chapterless_source_gets_no_chapter() {
    let joined = join(&[source("a.m4b", Some(1.0), 0)]);
    assert!(joined.synthetic.is_empty());
    assert_eq!(joined.count, 0);
}

#[test]
fn one_real_chapter_among_chapterless_sources_is_the_only_chapter() {
    let joined = join(&[source("a.m4b", Some(1.0), 1), source("b.m4b", Some(1.0), 0)]);
    assert!(joined.synthetic.is_empty());
    assert_eq!(joined.count, 1);
}

#[test]
fn a_source_without_a_length_gets_no_chapter_and_a_warning() {
    let joined = join(&[
        source("a.m4b", Some(1.0), 0),
        source("/books/b.m4b", None, 0),
        source("c.m4b", Some(2.0), 0),
    ]);
    assert_eq!(
        joined.synthetic,
        [chapter("a", 0, 1000), chapter("c", 1000, 3000)]
    );
    assert_eq!(joined.count, 2, "the count is what the output carries");
    assert_eq!(
        joined.warnings,
        ["b.m4b has no known length, so the chapters after it may start too early."]
    );
}
