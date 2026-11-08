use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut folders = vec![root.to_path_buf()];
    while let Some(folder) = folders.pop() {
        for entry in fs::read_dir(&folder).expect("read folder") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                folders.push(path);
            } else {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

pub(crate) fn names_in(folder: &Path) -> Vec<String> {
    let mut names: Vec<_> = fs::read_dir(folder)
        .expect("read folder")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}
