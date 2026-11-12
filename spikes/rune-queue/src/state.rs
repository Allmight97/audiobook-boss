//! The engine mirror for the batch queue. Events and intent replies land in
//! one inbox; the frame poll applies the newest snapshot of each part.

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use abb_engine::audio::{AudioFile, TitleStatus, supported_audio_import_extensions};
use abb_engine::session::{
    InputNotice, OutputSnapshot, SelectionModifiers, SelectionSnapshot, SessionIntent,
    SessionOutcome, SessionUpdate, SubmissionStatus, TitlesSnapshot,
};
use abb_engine::work_runtime::{
    ChildJobStatus, OperationId, OperationPhase, OperationSnapshot, WorkOperationsSnapshot,
    WorkOperationsUpdate,
};
use abb_engine::{Engine, EngineConfig, EngineEvent, EventSink, ShutdownOutcome};
use rune::{Tx, Waker};
use tokio::runtime::{Handle, Runtime};

pub static ENGINE_STARTED: AtomicBool = AtomicBool::new(false);

const APP_IDENTIFIER: &str = "com.audiobook-boss.rune-spike";

pub fn probe_requested() -> bool {
    rune::extra_args().iter().any(|arg| arg == "--launch-probe")
}

pub fn platform_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "other"
    }
}

struct Sink {
    notes: Mutex<Vec<Note>>,
    waker: Mutex<Option<Waker>>,
}

enum Note {
    Event(EngineEvent),
    Banner(String),
}

impl Sink {
    fn new(waker: Waker) -> Arc<Self> {
        Arc::new(Self {
            notes: Mutex::new(Vec::new()),
            waker: Mutex::new(Some(waker)),
        })
    }

    fn push_event(&self, event: EngineEvent) {
        self.notes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(Note::Event(event));
        self.wake();
    }

    fn push_banner(&self, message: String) {
        self.notes
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(Note::Banner(message));
        self.wake();
    }

    fn wake(&self) {
        let waker = self
            .waker
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if let Some(waker) = waker {
            waker();
        }
    }

    fn drain(&self) -> Vec<Note> {
        std::mem::take(&mut *self.notes.lock().unwrap_or_else(|error| error.into_inner()))
    }
}

impl EventSink for Sink {
    fn emit(&self, event: EngineEvent) {
        self.push_event(event);
    }
}

pub struct State {
    pub layout_bug: bool,
    pub launch_probe: bool,
    pub drag_over: bool,
    pub banner: String,
    pub visible: String,
    pub files: Vec<AudioFile>,
    pub selected: HashSet<String>,
    pub output: Option<OutputSnapshot>,
    pub engine_directory: String,
    pub output_dir: rune::TextEdit,
    pub concurrency_locked: bool,
    load_path: Option<String>,
    preset_output: Option<String>,
    output_shown: String,
    files_revision: Option<u64>,
    selection_revision: Option<u64>,
    output_revision: Option<u64>,
    selection: Option<SelectionSnapshot>,
    work_revision: Option<u64>,
    work_order: Vec<OperationId>,
    operations: BTreeMap<String, OperationSnapshot>,
    probe_frames: u8,
    probe_printed: bool,
    engine_ready_ms: u128,
    engine: Option<Engine>,
    runtime: Option<Runtime>,
    handle: Option<Handle>,
    sink: Option<Arc<Sink>>,
}

impl State {
    pub fn from_args(args: &[String]) -> Self {
        let mut load_path = None;
        let mut preset_output = None;
        let mut launch_probe = false;
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--launch-probe" => launch_probe = true,
                "--load" => {
                    index += 1;
                    load_path = args.get(index).cloned();
                }
                "--output" => {
                    index += 1;
                    preset_output = args.get(index).cloned();
                }
                _ => {}
            }
            index += 1;
        }
        Self {
            layout_bug: false,
            launch_probe,
            drag_over: false,
            banner: String::new(),
            visible: String::new(),
            files: Vec::new(),
            selected: HashSet::new(),
            output: None,
            engine_directory: String::new(),
            output_dir: rune::TextEdit::new(""),
            concurrency_locked: false,
            load_path,
            preset_output,
            output_shown: String::new(),
            files_revision: None,
            selection_revision: None,
            output_revision: None,
            selection: None,
            work_revision: None,
            work_order: Vec::new(),
            operations: BTreeMap::new(),
            probe_frames: 0,
            probe_printed: false,
            engine_ready_ms: 0,
            engine: None,
            runtime: None,
            handle: None,
            sink: None,
        }
    }

    pub fn attach(&mut self, waker: Waker, started: Instant) {
        let sink = Sink::new(waker);
        let runtime = match Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                self.banner = format!("The Tokio runtime did not start: {error}");
                return;
            }
        };
        let root = std::env::temp_dir().join(format!("abb-rune-spike-{}", std::process::id()));
        let engine = Engine::start(EngineConfig {
            cache_dir: root.join("cache"),
            config_dir: root.join("config"),
            app_identifier: APP_IDENTIFIER.to_string(),
            events: Arc::clone(&sink) as Arc<dyn EventSink>,
        });
        self.engine_ready_ms = started.elapsed().as_millis();
        let engine = match engine {
            Ok(engine) => engine,
            Err(error) => {
                self.banner = error.to_string();
                self.runtime = Some(runtime);
                return;
            }
        };
        ENGINE_STARTED.store(true, Ordering::Relaxed);
        self.handle = Some(runtime.handle().clone());
        self.runtime = Some(runtime);
        self.engine = Some(engine);
        self.sink = Some(sink);
        self.read_engine();
        if let Some(directory) = self.preset_output.clone() {
            self.block_on_intent(SessionIntent::SetOutputDirectory { directory });
        }
        if let Some(path) = self.load_path.clone() {
            self.block_on_intent(SessionIntent::Import { paths: vec![path] });
        }
    }

    pub fn on_frame(&mut self, started: Instant, tx: &mut Tx) -> bool {
        let changed = self.drain();
        if self.launch_probe && !self.probe_printed {
            self.probe_frames = self.probe_frames.saturating_add(1);
            if self.probe_frames >= 2 {
                self.probe_printed = true;
                println!(
                    "cold_launch_ms={ms} platform={platform} phase=second_frame_after_engine_start engine_ready_ms={engine} engine_ok={ok}",
                    ms = started.elapsed().as_millis(),
                    platform = platform_name(),
                    engine = self.engine_ready_ms,
                    ok = ENGINE_STARTED.load(Ordering::Relaxed),
                );
                let _ = std::io::Write::flush(&mut std::io::stdout());
                tx.close_window();
            } else {
                tx.wake_after(std::time::Duration::ZERO);
            }
        }
        changed
    }

    pub fn dispatch(&self, intent: SessionIntent) {
        let (Some(engine), Some(handle), Some(sink)) =
            (self.engine.clone(), self.handle.clone(), self.sink.clone())
        else {
            return;
        };
        handle.spawn(async move {
            let reply = engine.session_dispatch(intent).await;
            sink.push_event(EngineEvent::Session(reply.update));
            if let Some(message) = outcome_banner(&reply.outcome) {
                sink.push_banner(message);
            }
        });
    }

    pub fn import_paths(&self, paths: Vec<PathBuf>) {
        let paths = paths
            .into_iter()
            .map(|path| path.display().to_string())
            .collect();
        self.dispatch(SessionIntent::Import { paths });
    }

    pub fn select_title(&self, title_id: String) {
        self.dispatch(SessionIntent::SelectFile {
            title_id,
            modifiers: SelectionModifiers::default(),
        });
    }

    pub fn commit_output_directory(&mut self) {
        let directory = self.output_dir.text.clone();
        self.output_shown = directory.clone();
        self.dispatch(SessionIntent::SetOutputDirectory { directory });
    }

    pub fn cancel_current(&self) {
        if let Some(run_id) = self.preview_run_id() {
            self.dispatch(SessionIntent::CancelPreview {
                run_id,
                child_job_id: None,
            });
            return;
        }
        if let Some(operation) = self.active_operation() {
            self.cancel_export(operation.operation_id.clone(), None);
        }
    }

    pub fn cancel_child(&self, child_id: String) {
        if let Some(run_id) = self.preview_run_id() {
            self.dispatch(SessionIntent::CancelPreview {
                run_id,
                child_job_id: Some(child_id),
            });
            return;
        }
        if let Some(operation) = self.active_operation() {
            self.cancel_export(operation.operation_id.clone(), Some(child_id));
        }
    }

    pub fn operations_in_order(&self) -> Vec<&OperationSnapshot> {
        self.work_order
            .iter()
            .filter_map(|id| self.operations.get(&id.0))
            .collect()
    }

    pub fn active_operation(&self) -> Option<&OperationSnapshot> {
        self.operations_in_order().into_iter().find(|operation| {
            matches!(
                operation.phase,
                OperationPhase::Active | OperationPhase::Accepted
            )
        })
    }

    pub fn queue_operation(&self) -> Option<&OperationSnapshot> {
        self.output
            .as_ref()
            .and_then(|output| output.preview_run.as_ref())
            .map(|preview| &preview.operation)
            .or_else(|| self.active_operation())
    }

    pub fn files_revision(&self) -> u64 {
        self.files_revision.unwrap_or(0)
    }

    fn preview_run_id(&self) -> Option<String> {
        self.output.as_ref().and_then(|output| {
            output
                .preview_run
                .as_ref()
                .map(|preview| preview.operation.operation_id.0.clone())
        })
    }

    fn cancel_export(&self, operation_id: OperationId, child_job_id: Option<String>) {
        let Some(engine) = &self.engine else {
            return;
        };
        if let Err(error) = engine.cancel_work_operation(operation_id, child_job_id) {
            if let Some(sink) = &self.sink {
                sink.push_banner(error.to_string());
            }
        }
    }

    fn block_on_intent(&mut self, intent: SessionIntent) {
        let (Some(engine), Some(runtime), Some(sink)) = (
            self.engine.clone(),
            self.runtime.as_ref(),
            self.sink.clone(),
        ) else {
            return;
        };
        let reply = runtime.block_on(engine.session_dispatch(intent));
        sink.push_event(EngineEvent::Session(reply.update.clone()));
        if let Some(message) = outcome_banner(&reply.outcome) {
            sink.push_banner(message);
        }
        self.drain();
    }

    fn read_engine(&mut self) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        self.apply_session(engine.session_snapshot());
        match engine.list_work_operations() {
            Ok(snapshot) => self.apply_work_snapshot(snapshot),
            Err(error) => self.banner = error.to_string(),
        }
    }

    fn drain(&mut self) -> bool {
        let Some(sink) = self.sink.clone() else {
            return false;
        };
        let notes = sink.drain();
        if notes.is_empty() {
            return false;
        }
        for note in notes {
            match note {
                Note::Banner(message) => self.banner = message,
                Note::Event(EngineEvent::Session(update)) => self.apply_session(update),
                Note::Event(EngineEvent::WorkOperations(update)) => self.apply_work_update(update),
                Note::Event(EngineEvent::Settings(_) | EngineEvent::QuitPressAgain { .. }) => {}
            }
        }
        true
    }

    fn apply_session(&mut self, update: SessionUpdate) {
        if let Some(titles) = update.titles {
            self.apply_titles(titles);
        }
        if let Some(selection) = update.selection {
            if take_revision(&mut self.selection_revision, selection.revision) {
                self.selection = Some(selection);
                self.rebuild_selection();
            }
        }
        if let Some(output) = update.output {
            if take_revision(&mut self.output_revision, output.revision) {
                self.note_output(output.directory.clone());
                self.output = Some(output);
            }
        }
    }

    fn apply_titles(&mut self, titles: TitlesSnapshot) {
        if !take_revision(&mut self.files_revision, titles.revision) {
            return;
        }
        self.banner = notice_line(titles.notice.as_ref()).unwrap_or_else(|| {
            if self.banner_is_notice() {
                String::new()
            } else {
                self.banner.clone()
            }
        });
        self.files = titles.files;
        self.rebuild_selection();
    }

    fn banner_is_notice(&self) -> bool {
        self.banner.starts_with("Import:")
    }

    fn rebuild_selection(&mut self) {
        self.selected.clear();
        let Some(selection) = &self.selection else {
            return;
        };
        for index in &selection.selected_indices {
            if let Some(file) = self.files.get(*index) {
                self.selected.insert(file.input_id.clone());
            }
        }
    }

    fn note_output(&mut self, directory: Option<String>) {
        let directory = directory.unwrap_or_default();
        self.engine_directory = directory.clone();
        if self.output_dir.focused || self.output_dir.text != self.output_shown {
            return;
        }
        if self.output_dir.text != directory {
            self.output_dir.replace_text(directory.clone());
        }
        self.output_shown = directory;
    }

    fn apply_work_snapshot(&mut self, snapshot: WorkOperationsSnapshot) {
        self.work_revision = Some(snapshot.revision);
        self.work_order = snapshot.order;
        self.concurrency_locked = snapshot.concurrency_locked;
        self.operations = snapshot
            .operations
            .into_iter()
            .map(|operation| (operation.operation_id.0.clone(), operation))
            .collect();
    }

    fn apply_work_update(&mut self, update: WorkOperationsUpdate) {
        if let Some(changed) = update.changed {
            let id = changed.operation_id.0.clone();
            let stale = self
                .operations
                .get(&id)
                .is_some_and(|current| current.revision > changed.revision);
            if !stale {
                self.operations.insert(id, changed);
            }
        }
        if self
            .work_revision
            .is_none_or(|current| update.revision >= current)
        {
            self.work_revision = Some(update.revision);
            self.work_order = update.order;
            self.concurrency_locked = update.concurrency_locked;
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        let engine = self.engine.take();
        let runtime = self.runtime.take();
        if let (Some(engine), Some(runtime)) = (engine, runtime) {
            let outcome = runtime.block_on(engine.shutdown());
            if matches!(outcome, ShutdownOutcome::SettingsUnsaved { .. }) {
                match runtime.block_on(engine.shutdown()) {
                    ShutdownOutcome::Settled | ShutdownOutcome::SettingsUnsaved { .. } => {}
                }
            }
        }
    }
}

fn take_revision(slot: &mut Option<u64>, revision: u64) -> bool {
    if slot.is_some_and(|current| revision < current) {
        return false;
    }
    *slot = Some(revision);
    true
}

fn outcome_banner(outcome: &SessionOutcome) -> Option<String> {
    match outcome {
        SessionOutcome::Applied
        | SessionOutcome::RemoteSaved
        | SessionOutcome::Superseded
        | SessionOutcome::PreviewOutput { .. } => None,
        SessionOutcome::RemoteAuthStarted { .. } => {
            Some("A remote sign-in started. This screen does not open it.".into())
        }
        SessionOutcome::Rejected { error } => Some(error.message.clone()),
        SessionOutcome::DraftRejected { message } => Some(
            message
                .clone()
                .unwrap_or_else(|| "The change was not accepted.".into()),
        ),
        SessionOutcome::CoverLoadFailed => Some("The cover did not load.".into()),
        SessionOutcome::EditLost => {
            Some("That edit was for a form that is no longer on screen.".into())
        }
    }
}

fn notice_line(notice: Option<&InputNotice>) -> Option<String> {
    let notice = notice?;
    Some(match notice {
        InputNotice::OrderLocked => "Import: the list is locked while work is running.".into(),
        InputNotice::NoSupportedFiles { .. } => "Import: no supported audio files.".into(),
        InputNotice::DuplicatesOnly => "Import: those files are already in the list.".into(),
        InputNotice::SkippedPaths { skipped } => {
            format!("Import: {} path(s) were skipped.", skipped.len())
        }
        InputNotice::AnalysisFailed { error } => format!("Import: {}", error.message),
    })
}

pub fn submission_line(status: &SubmissionStatus) -> String {
    match status {
        SubmissionStatus::Preparing { preview: true } => "Preparing preview".into(),
        SubmissionStatus::Preparing { preview: false } => "Preparing export".into(),
        SubmissionStatus::Refused { reason } => format!("Refused: {reason:?}"),
        SubmissionStatus::ReviewRequired => "Waiting for a collision answer".into(),
        SubmissionStatus::Blocked { message } => message.clone(),
        SubmissionStatus::Failed { error } => format!("Failed: {}", error.message),
        SubmissionStatus::Submitted { title, .. } => format!("Exporting {title}"),
        SubmissionStatus::Previewing => "Preview running".into(),
        SubmissionStatus::PreviewFinished { .. } => "Preview finished".into(),
        SubmissionStatus::Cancelled => "Cancelled".into(),
        SubmissionStatus::FinishedBeforeRestart { .. } => "Finished before restart".into(),
    }
}

pub fn title_status_label(status: TitleStatus) -> &'static str {
    match status {
        TitleStatus::Ready => "Ready",
        TitleStatus::Failed => "Failed",
        TitleStatus::RemovedFinished => "Removed",
    }
}

pub fn child_status_label(status: ChildJobStatus) -> &'static str {
    match status {
        ChildJobStatus::Queued => "Queued",
        ChildJobStatus::Running => "Running",
        ChildJobStatus::Completed => "Complete",
        ChildJobStatus::Skipped => "Skipped",
        ChildJobStatus::Cancelled => "Cancelled",
        ChildJobStatus::Failed => "Failed",
    }
}

pub fn accepts_drop(path: &std::path::Path) -> bool {
    if path.is_dir() {
        return true;
    }
    let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
        return false;
    };
    supported_audio_import_extensions()
        .iter()
        .any(|supported| supported.eq_ignore_ascii_case(extension))
}

pub fn audio_filters() -> Vec<String> {
    supported_audio_import_extensions()
        .into_iter()
        .map(str::to_string)
        .collect()
}
