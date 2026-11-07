use super::*;
use std::path::PathBuf;

fn source(name: &str) -> AudioFile {
    let mut file = AudioFile::new(PathBuf::from(format!("/books/{name}")));
    file.input_id = name.to_string();
    file.is_valid = true;
    file
}

fn failed(name: &str) -> AudioFile {
    let mut file = source(name);
    file.is_valid = false;
    file.error = Some("Cannot read".into());
    file
}

fn removed(name: &str) -> AudioFile {
    let mut file = source(name);
    file.is_valid = false;
    file.source_removed = true;
    file.error = Some("Exported. Its download was removed.".into());
    file
}

fn listed(sources: Vec<AudioFile>) -> AudioFile {
    let mut files = vec![sources[0].clone()];
    let mut by_title = BTreeMap::from([(sources[0].input_id.clone(), sources)]);
    publish(&mut files, &mut by_title);
    files.remove(0)
}

#[test]
fn a_single_title_has_the_status_of_its_own_file() {
    let mut files = vec![source("ok"), failed("bad"), removed("gone")];
    publish(&mut files, &mut BTreeMap::new());

    assert_eq!(
        files.iter().map(|file| file.status).collect::<Vec<_>>(),
        [
            TitleStatus::Ready,
            TitleStatus::Failed,
            TitleStatus::RemovedFinished
        ]
    );
}

#[test]
fn a_group_is_ready_only_when_every_source_is() {
    let group = listed(vec![source("a"), source("b")]);
    assert_eq!(group.status, TitleStatus::Ready);
    assert_eq!(group.error, None);

    let group = listed(vec![source("a"), failed("b")]);
    assert_eq!(group.status, TitleStatus::Failed);
    assert_eq!(group.error.as_deref(), Some("Cannot read"));

    let group = listed(vec![removed("a"), source("b")]);
    assert_eq!(group.status, TitleStatus::RemovedFinished);
    assert_eq!(
        group.error.as_deref(),
        Some("Exported. Its download was removed.")
    );
}

#[test]
fn a_failed_source_outranks_a_removed_one() {
    let group = listed(vec![removed("a"), failed("b")]);
    assert_eq!(group.status, TitleStatus::Failed);
    assert_eq!(group.error.as_deref(), Some("Cannot read"));
}

#[test]
fn every_listed_source_has_its_own_status() {
    let mut files = vec![source("a")];
    let mut sources = BTreeMap::from([("a".to_string(), vec![source("a"), failed("b")])]);
    publish(&mut files, &mut sources);

    assert_eq!(
        sources["a"]
            .iter()
            .map(|source| source.status)
            .collect::<Vec<_>>(),
        [TitleStatus::Ready, TitleStatus::Failed]
    );
}
