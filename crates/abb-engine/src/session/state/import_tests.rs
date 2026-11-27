use super::*;
use abb_media_pipeline::output_artifact::SupplementalProcessingAsset;

fn companion(name: &str) -> SupplementalProcessingAsset {
    SupplementalProcessingAsset {
        path: PathBuf::from(format!("/books/{name}")),
        file_name: name.to_string(),
        size_bytes: 16,
        sha256: "abc".to_string(),
    }
}

#[test]
fn local_import_attaches_companions_only_to_valid_files_and_backfills_when_folder_reimported() {
    let mut desk = Desk::open(&[], &[]);
    let valid = audio_file("alpha", true);
    let invalid = audio_file("broken", false);
    let initial = HashMap::from([(path("broken"), vec![companion("ignored.pdf")])]);

    desk.state.import_local(vec![valid, invalid], &initial, &[]);
    desk.state.settle();

    let titles = desk.state.update_since(None).titles.expect("titles");
    assert!(titles.companions.is_empty());
    assert!(desk.state.staged.is_empty());

    let reimport = audio_file("alpha", true);
    let with_pdf = HashMap::from([(path("alpha"), vec![companion("guide.pdf")])]);
    desk.state.import_local(vec![reimport], &with_pdf, &[]);
    desk.state.settle();

    let titles = desk.state.update_since(None).titles.expect("titles");
    assert_eq!(titles.companions["alpha"], ["guide.pdf"]);
    assert!(desk.state.staged.is_empty());

    let replacement = audio_file("alpha", true);
    let other_pdf = HashMap::from([(path("alpha"), vec![companion("other.pdf")])]);
    desk.state.import_local(vec![replacement], &other_pdf, &[]);
    desk.state.settle();

    let titles = desk.state.update_since(None).titles.expect("titles");
    assert_eq!(
        titles.companions["alpha"],
        ["guide.pdf"],
        "existing companions stay untouched on duplicate reimport"
    );

    desk.state.working_set.clear_all();
    desk.state.settle();
    assert!(desk
        .state
        .update_since(None)
        .titles
        .expect("titles")
        .companions
        .is_empty());
}
