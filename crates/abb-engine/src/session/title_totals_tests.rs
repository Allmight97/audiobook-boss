use super::*;
use crate::audio::AudioChapter;
use crate::metadata::ChapterPlan;
use std::path::PathBuf;

fn source(name: &str, duration: f64, size: f64, chapters: usize) -> AudioFile {
    let mut file = AudioFile::new(PathBuf::from(format!("/books/{name}")));
    file.input_id = name.to_string();
    file.is_valid = true;
    file.duration = Some(duration);
    file.size = Some(size);
    file.chapters = (0..chapters)
        .map(|index| AudioChapter {
            title: Some(format!("Chapter {index}")),
            start_ms: index as i64,
            end_ms: index as i64 + 1,
        })
        .collect();
    file
}

fn grouped(sources: Vec<AudioFile>) -> (Vec<AudioFile>, BTreeMap<String, Vec<AudioFile>>) {
    let anchor = sources[0].clone();
    let sources_by_id = BTreeMap::from([(anchor.input_id.clone(), sources)]);
    (vec![anchor], sources_by_id)
}

#[test]
fn a_group_of_chapterless_files_counts_one_chapter_per_file() {
    let (files, sources) = grouped(vec![
        source("a.mp3", 60.0, 10.0, 0),
        source("b.mp3", 90.0, 20.0, 0),
        source("c.mp3", 30.0, 5.0, 0),
    ]);

    let totals = title_totals(&files, &sources);

    let title = &totals.by_title["a.mp3"];
    assert_eq!(title.chapter_count, 3);
    assert_eq!(title.duration_seconds, 180.0);
    assert_eq!(title.size_bytes, 35.0);
}

#[test]
fn a_group_with_real_chapters_counts_only_those() {
    let (files, sources) = grouped(vec![
        source("a.m4b", 60.0, 10.0, 2),
        source("b.m4b", 90.0, 20.0, 0),
    ]);

    assert_eq!(
        title_totals(&files, &sources).by_title["a.m4b"].chapter_count,
        2
    );
}

#[test]
fn a_chapter_plan_replaces_the_embedded_chapters() {
    let mut file = source("a.m4b", 60.0, 10.0, 5);
    file.chapter_plan = Some(ChapterPlan {
        chapters: vec![crate::metadata::ChapterSpec {
            title: None,
            start_ms: 0,
            end_ms: 1,
        }],
        from_cue: true,
        source_fingerprint: String::new(),
    });

    let totals = title_totals(&[file], &BTreeMap::new());

    assert_eq!(totals.by_title["a.m4b"].chapter_count, 1);
}

#[test]
fn combined_size_counts_valid_sources_of_every_title() {
    let mut broken = source("broken.mp3", 10.0, 99.0, 0);
    broken.is_valid = false;
    let (mut files, sources) = grouped(vec![
        source("a.mp3", 1.0, 10.0, 0),
        source("b.mp3", 1.0, 20.0, 0),
    ]);
    files.push(source("solo.mp3", 1.0, 5.0, 0));
    files.push(broken);

    assert_eq!(title_totals(&files, &sources).total_size_bytes, 35.0);
}

#[test]
fn the_preview_length_spans_the_titles_in_the_list() {
    let many = (0..30)
        .map(|index| source(&format!("{index}.mp3"), 60.0, 1.0, 0))
        .collect::<Vec<_>>();
    let (mut files, sources) = grouped(many);
    files.push(source("single.m4b", 600.0, 1.0, 0));

    assert_eq!(
        title_totals(&files, &sources).preview_length,
        PreviewLength {
            shortest_seconds: 90,
            longest_seconds: 150,
        }
    );
}

#[test]
fn a_source_shorter_than_its_share_shortens_the_preview_length() {
    let (mut files, sources) = grouped(vec![
        source("a.mp3", 10.0, 1.0, 0),
        source("b.mp3", 600.0, 1.0, 0),
    ]);
    files.push(source("short.m4b", 12.0, 1.0, 0));

    assert_eq!(
        title_totals(&files, &sources).preview_length,
        PreviewLength {
            shortest_seconds: 12,
            longest_seconds: 55,
        }
    );
}

#[test]
fn an_empty_list_reports_the_standard_preview_length() {
    assert_eq!(
        title_totals(&[], &BTreeMap::new()).preview_length,
        PreviewLength {
            shortest_seconds: 90,
            longest_seconds: 90,
        }
    );
}

#[test]
fn a_title_with_no_valid_source_adds_no_preview_length() {
    let mut broken = source("broken.mp3", 10.0, 1.0, 0);
    broken.is_valid = false;
    let files = vec![source("good.m4b", 600.0, 1.0, 0), broken];

    assert_eq!(
        title_totals(&files, &BTreeMap::new()).preview_length,
        PreviewLength {
            shortest_seconds: 90,
            longest_seconds: 90,
        }
    );
}
