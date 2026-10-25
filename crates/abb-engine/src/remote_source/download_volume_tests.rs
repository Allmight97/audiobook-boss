//! TEMPORARY measurement for the idle-redraw / remote-view PR; removed before merge.
use super::super::types::{RemoteTitleAvailability, RemoteTitleAvailabilityStatus};
use super::super::{ProviderId, RemoteTitle, RemoteUiIntent};

fn title(id: &str, acquirable: bool, pdf: bool) -> RemoteTitle {
    RemoteTitle {
        provider_id: ProviderId::Audible,
        title_id: id.into(),
        title: id.into(),
        authors: vec![],
        narrators: vec![],
        duration_seconds: None,
        cover_url: None,
        supplemental_pdf_available: pdf,
        acquired: false,
        availability: RemoteTitleAvailability {
            status: RemoteTitleAvailabilityStatus::Available,
            acquirable,
            label: "Available".into(),
            detail: None,
        },
        unsupported_reasons: vec![],
    }
}

// Run: ABB_PERF_OUT=<file> cargo test -p abb-engine --lib download_event_volume -- --ignored
#[test]
#[ignore = "measurement"]
fn download_event_volume() {
    use abb_remote_source_core::{acquisition_progress, AcquisitionStage};
    #[derive(Default)]
    struct Sizes(std::sync::Mutex<Vec<usize>>);
    impl crate::EventSink for Sizes {
        fn emit(&self, event: crate::EngineEvent) {
            if let crate::EngineEvent::Session(update) = event {
                let bytes = serde_json::to_vec(&update).expect("serialize").len();
                self.0.lock().expect("sizes").push(bytes);
            }
        }
    }
    let label = std::env::var("ABB_PERF_LABEL").unwrap_or_default();
    for count in [1_000usize, 5_000] {
        let root = tempfile::TempDir::new().expect("temp dir");
        let sizes = std::sync::Arc::new(Sizes::default());
        let runtime = super::super::tests::test_runtime_with(&root, None, Some(sizes.clone()));
        let titles = (0..count)
            .map(|index| title(&format!("B{index:09}"), true, index % 5 == 0))
            .collect();
        runtime.ui().library_loaded(titles);
        for index in 0..50 {
            let _ = runtime.ui_begin(RemoteUiIntent::ToggleTitle {
                title_id: format!("B{index:09}"),
            });
        }
        assert_eq!(runtime.ui_snapshot().selected_title_ids.len(), 50);
        let job_id = "perf-job";
        runtime.inner.lifecycle.jobs.lock().expect("jobs").insert(
            job_id.into(),
            super::super::RemoteAcquisitionJob {
                job_id: job_id.into(),
                provider_id: ProviderId::Audible,
                status: super::super::types::RemoteAcquisitionStatus::Acquiring,
                progress: acquisition_progress(AcquisitionStage::License, Some(0.0), None, None),
                materialized_files: Vec::new(),
                supplemental_assets: Vec::new(),
                diagnostics: Vec::new(),
                handoff: None,
            },
        );
        sizes.0.lock().expect("sizes").clear();
        let started = std::time::Instant::now();
        let mut calls = 0u32;
        while started.elapsed() < std::time::Duration::from_secs(3) {
            calls += 1;
            runtime.inner.lifecycle.update_job_progress(
                job_id,
                acquisition_progress(
                    AcquisitionStage::Download,
                    Some((calls % 1000) as f32 / 1000.0),
                    Some(u64::from(calls)),
                    Some(1_000_000),
                ),
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let seconds = started.elapsed().as_secs_f64();
        let sizes = sizes.0.lock().expect("sizes").clone();
        let total: usize = sizes.iter().sum();
        let mut sorted = sizes.clone();
        sorted.sort_unstable();
        let median = sorted.get(sorted.len() / 2).copied().unwrap_or(0);
        let line = serde_json::json!({
            "label": label,
            "metric": "engine-download-events",
            "titles": count,
            "selected": 50,
            "progressCalls": calls,
            "seconds": (seconds * 10.0).round() / 10.0,
            "events": sizes.len(),
            "eventsPerSecond": ((sizes.len() as f64 / seconds) * 10.0).round() / 10.0,
            "medianEventBytes": median,
            "bytesPerSecond": (total as f64 / seconds).round(),
        });
        match std::env::var("ABB_PERF_OUT") {
            Ok(path) => {
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .expect("perf out");
                writeln!(file, "{line}").expect("write perf");
            }
            Err(_) => println!("{line}"),
        }
    }
}
