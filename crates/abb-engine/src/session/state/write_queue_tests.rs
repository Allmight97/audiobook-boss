use std::collections::HashSet;
use std::path::PathBuf;

use super::{SaveItem, WriteQueue};
use crate::metadata::MetadataIntentPatch;

fn item(name: &str) -> SaveItem {
    SaveItem {
        path: PathBuf::from(name),
        patch: MetadataIntentPatch::default(),
        revision: 1,
    }
}

#[test]
fn a_save_is_running_exactly_while_it_holds_a_file() {
    let mut queue = WriteQueue::default();
    let routing = queue.route(vec![item("/a.m4b")], &HashSet::new(), |_| false);
    assert_eq!(routing.immediate.len(), 1);
    assert!(queue.save_running());
    assert!(queue.is_writing());

    queue.release(&[PathBuf::from("/a.m4b")]);

    assert!(!queue.save_running());
    assert!(!queue.is_writing());
}

#[test]
fn a_save_with_only_waiting_files_is_not_running() {
    let mut queue = WriteQueue::default();
    let reading = HashSet::from([PathBuf::from("/a.m4b")]);

    let routing = queue.route(vec![item("/a.m4b")], &reading, |_| false);

    assert_eq!(routing.waiting, 1);
    assert!(!queue.save_running());
}
