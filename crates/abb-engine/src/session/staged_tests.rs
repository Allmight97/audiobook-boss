//! When a staged download may be removed.

use super::*;

fn path(name: &str) -> PathBuf {
    PathBuf::from(format!("/staged/{name}.m4b"))
}

fn pdf(input_id: &str) -> SupplementalProcessingAsset {
    SupplementalProcessingAsset {
        asset_id: format!("{input_id}-pdf"),
        input_id: input_id.to_string(),
        title_id: "B0".to_string(),
        path: PathBuf::from(format!("/staged/{input_id}.pdf")),
        file_name: format!("{input_id}.pdf"),
        size_bytes: 10,
        sha256: "00".to_string(),
    }
}

/// One finished export title per `(input id, status, companion warning)`.
pub(crate) fn exported(titles: &[(&str, ChildJobStatus, bool)]) -> Vec<ChildJobSnapshot> {
    // A grouped title lists its sources joined by "+".
    titles
        .iter()
        .map(|(input_id, status, warning)| {
            serde_json::from_value(serde_json::json!({
                "childJobId": format!("child-{input_id}"),
                "operationId": "operation-1",
                "label": input_id,
                "status": status,
                "lane": "encodeCpu",
                "progress": { "stage": "complete", "percentage": 100.0, "message": "" },
                "sourceInputIds": input_id.split('+').collect::<Vec<_>>(),
                "cancellable": false,
                "cancelRequested": false,
                "supplementalWarning": warning.then_some("PDF failed"),
            }))
            .expect("child snapshot")
        })
        .collect()
}

fn staged() -> StagedSources {
    let mut staged = StagedSources::default();
    staged.register("job-1", "alpha", path("alpha"), vec![pdf("alpha")]);
    staged.register("job-1", "beta", path("beta"), Vec::new());
    staged.register("job-2", "gamma", path("gamma"), Vec::new());
    staged
}

#[test]
fn a_download_goes_only_when_every_title_from_it_is_finished_with() {
    let mut staged = staged();
    let free = HashSet::new();

    assert!(staged.finish(["alpha"]));
    assert!(
        staged.removable(&free, Instant::now()).is_empty(),
        "beta still needs job-1"
    );

    // Beta left the list; gamma is still listed.
    assert!(staged.finish_unlisted(&HashSet::from(["alpha", "gamma"])));
    assert_eq!(staged.removable(&free, Instant::now()), ["job-1"]);

    staged.removed("job-1");
    assert!(!staged.finish(["alpha"]), "a removed job is forgotten");
}

#[test]
fn only_titles_published_with_their_companions_finish_with_their_download() {
    let mut staged = staged();
    staged.register("job-3", "delta", path("delta"), Vec::new());
    staged.register("job-4", "omega", path("omega"), Vec::new());

    staged.finish_export(&exported(&[
        ("alpha+beta", ChildJobStatus::Completed, false),
        ("gamma", ChildJobStatus::Completed, false),
        ("delta", ChildJobStatus::Completed, true),
        ("omega", ChildJobStatus::Skipped, false),
    ]));

    assert_eq!(
        staged.removable(&HashSet::new(), Instant::now()),
        ["job-1", "job-2"]
    );
}

#[test]
fn a_finished_download_waits_while_anything_reads_its_files() {
    let mut staged = staged();
    staged.finish(["alpha", "beta"]);

    let reading_audio = HashSet::from([path("beta")]);
    assert!(staged.removable(&reading_audio, Instant::now()).is_empty());
    let reading_pdf = HashSet::from([PathBuf::from("/staged/alpha.pdf")]);
    assert!(staged.removable(&reading_pdf, Instant::now()).is_empty());
    assert_eq!(staged.removable(&HashSet::new(), Instant::now()), ["job-1"]);
}

#[test]
fn exports_take_each_title_companions_by_input_id() {
    let staged = staged();

    let assets = staged
        .assets_for(["alpha", "beta", "unknown"])
        .expect("assets");
    assert_eq!(assets.keys().collect::<Vec<_>>(), ["alpha"]);
    assert!(staged.assets_for(["beta"]).is_none());
    assert_eq!(
        staged.companions(),
        BTreeMap::from([("alpha".to_string(), vec!["alpha.pdf".to_string()])])
    );
}

#[test]
fn a_failed_removal_is_tried_again_only_after_the_retry_delay() {
    let mut staged = staged();
    staged.finish(["alpha", "beta"]);
    let failed = Instant::now();
    staged.removal_failed("job-1", failed);

    let free = HashSet::new();
    assert!(staged.removable(&free, failed).is_empty());
    assert!(staged.removable(&free, failed + RETRY_DELAY / 2).is_empty());
    assert_eq!(staged.removable(&free, failed + RETRY_DELAY), ["job-1"]);
}
