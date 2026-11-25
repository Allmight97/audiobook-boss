use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::output_artifact::lock_output_file;
use crate::work_runtime::{
    ChildJobStatus, OperationId, SubmitProcessingOperationRequest, WorkRuntime,
};

pub(crate) struct BlockedExport {
    _root: tempfile::TempDir,
    locks: Vec<Box<dyn Send>>,
    settled: Arc<AtomicBool>,
}

impl BlockedExport {
    pub(crate) async fn start(registry: &crate::ManagedJobRegistry, titles: usize) -> Self {
        let root = tempfile::TempDir::new().expect("export folder");
        let output_dir = root.path().join("out");
        std::fs::create_dir_all(&output_dir).expect("output folder");
        let sources: Vec<_> = (0..titles)
            .map(|index| {
                let source = root.path().join(format!("title-{index}.wav"));
                abb_test_fixtures::wav::write_sine_wav(&source, 0.2, 440.0);
                source
            })
            .collect();
        let request = crate::audio::TitleAudioRequest {
            output: crate::audio::OutputAudio::M4b {
                settings: super::encoders::native_encoder_settings(),
            },
            intent: crate::audio::AudioIntent::Encode,
            sample_rate: crate::audio::SampleRateConfig::Auto,
        };
        let payload: crate::processing::ProcessPayload =
            serde_json::from_value(serde_json::json!({
                "inputFiles": sources,
                "audioRequests": vec![request; titles],
                "outputDir": output_dir,
                "aacDecoder": "auto",
            }))
            .expect("export request");
        let plan = crate::processing::run::inspect_processing_plan(&payload, None, false)
            .expect("inspect the export")
            .expect_export();
        let locks = plan
            .inspected()
            .plan
            .jobs
            .iter()
            .map(|job| {
                let output = &job.output.resolved_path;
                std::fs::create_dir_all(output.parent().expect("output folder"))
                    .expect("create the output folder");
                Box::new(lock_output_file(output)) as Box<dyn Send>
            })
            .collect();
        let runtime = WorkRuntime::new(crate::engine_tasks::EngineTasks::default());
        let host = crate::host::Host::new(
            Arc::new(crate::DiscardEvents),
            crate::power::PowerManager::default(),
        );
        let settled = Arc::new(AtomicBool::new(false));
        runtime
            .submit_processing_operation(
                host,
                Arc::clone(registry),
                crate::processing::run::ProcessingRoots::for_cache(&root.path().join("cache")),
                SubmitProcessingOperationRequest {
                    operation_id: OperationId::new(),
                    payload,
                    metadata: None,
                    title: "Blocked export".to_string(),
                },
                Some(Box::new({
                    let settled = Arc::clone(&settled);
                    move |_| settled.store(true, Ordering::SeqCst)
                })),
                plan,
            )
            .await
            .expect("accept the export");
        abb_test_fixtures::poll::until_true("every export title holds a scheduler slot", || {
            runtime.list_operations().operations[0]
                .children
                .iter()
                .all(|child| child.status == ChildJobStatus::Running)
        })
        .await;
        Self {
            _root: root,
            locks,
            settled,
        }
    }

    pub(crate) async fn finish(mut self) {
        self.locks.clear();
        let settled = Arc::clone(&self.settled);
        abb_test_fixtures::poll::until_true("the blocked export settles", || {
            settled.load(Ordering::SeqCst)
        })
        .await;
    }
}
