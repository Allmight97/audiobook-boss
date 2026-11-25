use std::path::{Path, PathBuf};

use abb_test_fixtures::fs_listing::files_under;

use super::{abb_dev, entries_under, format_tag, refused, succeeded, tone};

struct Layout {
    root: tempfile::TempDir,
}

impl Layout {
    fn new() -> Self {
        let root = tempfile::TempDir::new().expect("temp root");
        std::fs::create_dir(root.path().join("inputs")).expect("inputs folder");
        Self { root }
    }

    fn input(&self, name: &str) -> PathBuf {
        self.root.path().join("inputs").join(name)
    }

    fn dune(&self) -> PathBuf {
        let path = self.input("dune.m4b");
        tone(&path, "Dune", "Frank Herbert");
        path
    }

    fn out(&self) -> PathBuf {
        self.root.path().join("out")
    }

    fn state(&self) -> PathBuf {
        self.root.path().join("state")
    }

    fn assert_nothing_escaped(&self) {
        let mut top_level: Vec<_> = std::fs::read_dir(self.root.path())
            .expect("read temp root")
            .map(|entry| entry.expect("temp root entry").file_name())
            .collect();
        top_level.sort();
        assert_eq!(top_level, ["inputs", "out", "state"], "an export escaped");
    }
}

fn export(layout: &Layout, source: &Path, args: &[&str]) {
    let mut command = abb_dev(&layout.state());
    command
        .args(args)
        .arg("--out")
        .arg(layout.out())
        .arg("--export")
        .arg(source);
    succeeded(command);
    layout.assert_nothing_escaped();
}

#[test]
fn a_series_book_exports_to_author_series_book_number_and_title() {
    let layout = Layout::new();
    let source = layout.dune();
    export(
        &layout,
        &source,
        &["--set", "series=Dune", "--set", "series-part=1"],
    );
    assert_eq!(
        files_under(&layout.out()),
        [layout
            .out()
            .join("Frank Herbert/Dune/Book 1 - Dune/Book 1 - Dune.m4b")]
    );
}

#[test]
fn a_book_without_a_series_exports_to_author_and_title() {
    let layout = Layout::new();
    let source = layout.dune();
    export(&layout, &source, &[]);
    assert_eq!(
        files_under(&layout.out()),
        [layout.out().join("Frank Herbert/Dune/Dune.m4b")]
    );
}

#[test]
fn hostile_tags_sanitize_the_path_but_keep_the_title_tag() {
    let layout = Layout::new();
    let source = layout.input("escape.m4b");
    tone(&source, "../../Escape: Part/One", "..");
    export(
        &layout,
        &source,
        &[
            "--set",
            "series=..",
            "--template",
            "{author}/{series}/{title}",
        ],
    );
    let exported = layout.out().join("_/_/.. .. Escape - Part One.m4b");
    assert_eq!(files_under(&layout.out()), std::slice::from_ref(&exported));
    assert_eq!(format_tag(&exported, "title"), "../../Escape: Part/One");
}

#[test]
fn a_template_that_climbs_out_of_the_output_folder_is_refused_and_writes_nothing() {
    let layout = Layout::new();
    let source = layout.dune();
    let mut command = abb_dev(&layout.state());
    command
        .args(["--template", "../{title}", "--out"])
        .arg(layout.out())
        .arg("--export")
        .arg(&source);
    refused(command, "Custom template path traversal is not allowed");
    layout.assert_nothing_escaped();
    assert!(
        layout.out().is_dir(),
        "the export folder abb-dev made is missing"
    );
    assert_eq!(entries_under(&layout.out()), Vec::<PathBuf>::new());
}

#[cfg(unix)]
#[test]
fn a_symlinked_book_is_refused() {
    let layout = Layout::new();
    let source = layout.dune();
    let link = layout.input("link.m4b");
    std::os::unix::fs::symlink(&source, &link).expect("symlink the book");
    let mut command = abb_dev(&layout.state());
    command.arg(&link);
    refused(command, "Symlinks are not supported");
}
