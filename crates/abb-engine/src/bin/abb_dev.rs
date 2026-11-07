//! Developer tool: drives the engine's working session from a terminal, the
//! way a UI host would, with no window.
//!
//! It imports audio, prints the session, and can edit metadata on every
//! imported title, save, choose audio and output, export or preview, show
//! progress, cancel one title, edit and re-import while an export reads its
//! files, and read back the exported tags. It runs under
//! its own identity and state folder, so it never reads or changes the app's
//! settings or stored credentials.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

use abb_engine::app_settings::{SettingsIntent, SettingsOutcome};
use abb_engine::audio::{AacDecoder, AudioIntent, AudiobookFormat};
use abb_engine::output_artifact::{CollisionPolicy, NamingPreset};
use abb_engine::session::{
    AudioEdit, CollisionReview, MetadataField, MetadataStatus, SessionIntent, SessionOutcome,
    SessionUpdate, SubmissionStatus,
};
use abb_engine::work_runtime::{ChildJobStatus, OperationId, OperationPhase, OperationSnapshot};
use abb_engine::{Engine, EngineConfig, EngineEvent, EventSink};

#[path = "abb_dev/during_export.rs"]
mod during_export;
use during_export::DuringExport;

const USAGE: &str = "\
Usage: abb-dev <file-or-folder>... [options]

Imports audio into an engine session and prints the session.

Options:
  --set <field>=<value>   Edit a field on every imported title. Repeatable.
                          Fields: title, date, author, narrator, series,
                          series-part, subseries, subseries-part, genre,
                          description. An empty value clears the field.
  --save                  Write pending edits to the files.
  --format <format>       Output format for every title: m4b, m4aOpus,
                          or mkaOpus.
  --intent <intent>       Audio handling for every title: auto, preserve, or
                          encode.
  --bitrate <kbps>        Target bitrate for encoded titles.
  --decoder <decoder>     AAC decoder for import and export: auto (FFmpeg)
                          or faad (bundled FAAD3).
  --out <folder>          Export folder; created if missing.
  --template <template>   Name exports with a custom template, such as
                          '{author}/{title}'.
  --export                Export every valid title and wait for it to finish.
  --preview               Render each title's preview instead.
  --on-collision <policy> What to do when an export already exists: rename,
                          replace, or skip. Without it, a collision stops.
  --cancel-title <n>      Cancel the nth title (from 1) while queued or running.
  --during-export-set <field>=<value>
                          Once the export is running and reading the file,
                          edit that field and Save. Repeatable.
  --during-export-reimport
                          At that point in the order, remove every title and
                          import the same paths again.
                          These steps run in the order given, before the tool
                          waits for the export and for waiting writes to land.
                          The tool fails if the export had finished before a
                          step, or if a Save did not wait for the export.
  --json                  Print the session as JSON.
  --state-dir <dir>       Keep engine state here instead of a folder that is
                          removed when the tool exits.
";

const APP_IDENTIFIER: &str = "com.audiobook-boss.devtool";

#[derive(Default)]
struct Options {
    paths: Vec<String>,
    edits: Vec<(MetadataField, String)>,
    save: bool,
    audio: Vec<AudioEdit>,
    decoder: Option<AacDecoder>,
    out: Option<String>,
    template: Option<String>,
    export: bool,
    preview: bool,
    on_collision: Option<CollisionPolicy>,
    cancel_title: Option<usize>,
    during_export: Vec<DuringExport>,
    json: bool,
    state_dir: Option<PathBuf>,
}

fn named<T: serde::de::DeserializeOwned>(what: &str, name: &str) -> Result<T, String> {
    serde_json::from_value(serde_json::Value::String(name.to_string()))
        .map_err(|_| format!("unknown {what} '{name}'"))
}

fn collision_policy(name: &str) -> Result<CollisionPolicy, String> {
    match name {
        "rename" => Ok(CollisionPolicy::RenameNew),
        "replace" => Ok(CollisionPolicy::ReplaceExisting),
        "skip" => Ok(CollisionPolicy::SkipExisting),
        _ => Err(format!("unknown collision policy '{name}'")),
    }
}

fn field_named(name: &str) -> Option<MetadataField> {
    Some(match name {
        "title" => MetadataField::Title,
        "date" => MetadataField::Date,
        "author" => MetadataField::Author,
        "narrator" => MetadataField::Narrator,
        "series" => MetadataField::Series,
        "series-part" => MetadataField::SeriesPart,
        "subseries" => MetadataField::Subseries,
        "subseries-part" => MetadataField::SubseriesPart,
        "genre" => MetadataField::Genre,
        "description" => MetadataField::Description,
        _ => return None,
    })
}

fn parse_edit(edit: &str, flag: &str) -> Result<(MetadataField, String), String> {
    let (name, value) = edit
        .split_once('=')
        .ok_or_else(|| format!("{flag} needs <field>=<value>, got '{edit}'"))?;
    let field = field_named(name).ok_or_else(|| format!("unknown field '{name}'"))?;
    Ok((field, value.to_string()))
}

fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options::default();
    let mut args = args;
    let value = |args: &mut dyn Iterator<Item = String>, flag: &str| {
        args.next().ok_or_else(|| format!("{flag} needs a value"))
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--save" => options.save = true,
            "--export" => options.export = true,
            "--format" => {
                let format: AudiobookFormat = named("format", &value(&mut args, "--format")?)?;
                options.audio.push(AudioEdit::Format(format));
            }
            "--intent" => {
                let intent: AudioIntent = named("intent", &value(&mut args, "--intent")?)?;
                options.audio.push(AudioEdit::Intent(intent));
            }
            "--bitrate" => {
                let kbps = value(&mut args, "--bitrate")?;
                let kbps = kbps.parse().map_err(|_| format!("bad bitrate '{kbps}'"))?;
                options.audio.push(AudioEdit::Bitrate(kbps));
            }
            "--decoder" => {
                options.decoder = Some(named("decoder", &value(&mut args, "--decoder")?)?);
            }
            "--out" => options.out = Some(value(&mut args, "--out")?),
            "--template" => options.template = Some(value(&mut args, "--template")?),
            "--preview" => options.preview = true,
            "--on-collision" => {
                options.on_collision =
                    Some(collision_policy(&value(&mut args, "--on-collision")?)?);
            }
            "--cancel-title" => {
                let title = value(&mut args, "--cancel-title")?;
                let title = title
                    .parse()
                    .ok()
                    .filter(|title| *title > 0)
                    .ok_or_else(|| format!("bad title number '{title}'"))?;
                options.cancel_title = Some(title);
            }
            "--json" => options.json = true,
            "--set" => {
                let edit = args.next().ok_or("--set needs <field>=<value>")?;
                options.edits.push(parse_edit(&edit, "--set")?);
            }
            "--during-export-set" => {
                let edit = value(&mut args, "--during-export-set")?;
                let (field, value) = parse_edit(&edit, "--during-export-set")?;
                options.during_export.push(DuringExport::Set(field, value));
            }
            "--during-export-reimport" => options.during_export.push(DuringExport::Reimport),
            "--state-dir" => {
                options.state_dir = Some(args.next().ok_or("--state-dir needs a folder")?.into());
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option '{flag}'")),
            _ => options.paths.push(arg),
        }
    }
    if options.paths.is_empty() {
        return Err("give at least one file or folder to import".to_string());
    }
    Ok(options)
}

/// The newest snapshot of every operation the engine has reported, by the
/// snapshot's own revision.
type OperationFeed = watch::Receiver<HashMap<OperationId, OperationSnapshot>>;

struct PrintProgress {
    seen: Mutex<HashMap<String, String>>,
    operations: watch::Sender<HashMap<OperationId, OperationSnapshot>>,
}

impl Default for PrintProgress {
    fn default() -> Self {
        Self {
            seen: Mutex::default(),
            operations: watch::channel(HashMap::new()).0,
        }
    }
}

impl PrintProgress {
    fn feed(&self) -> OperationFeed {
        self.operations.subscribe()
    }

    fn keep_newest(&self, operation: &OperationSnapshot) {
        self.operations.send_if_modified(|operations| {
            let newer = operations
                .get(&operation.operation_id)
                .is_none_or(|known| known.revision < operation.revision);
            if newer {
                operations.insert(operation.operation_id.clone(), operation.clone());
            }
            newer
        });
    }
}

impl EventSink for PrintProgress {
    fn emit(&self, event: EngineEvent) {
        let EngineEvent::WorkOperations(update) = event else {
            return;
        };
        let operation = update.changed;
        self.keep_newest(&operation);
        let Ok(mut seen) = self.seen.lock() else {
            return;
        };
        for child in &operation.children {
            let line = format!("{:?} {:?}", child.status, child.progress.stage);
            if seen.get(&child.child_job_id) != Some(&line) {
                eprintln!("  {}: {line}", child.label);
                seen.insert(child.child_job_id.clone(), line);
            }
        }
    }
}

async fn shut_down(engine: &Engine) {
    if let abb_engine::ShutdownOutcome::SettingsUnsaved { error } = engine.shutdown().await {
        eprintln!("settings were not saved: {}", error.message);
    }
}

async fn send(engine: &Engine, intent: SessionIntent) -> Result<(), String> {
    let description = format!("{intent:?}");
    match engine.session_dispatch(intent).await.outcome {
        SessionOutcome::Applied => Ok(()),
        outcome => Err(format!("{description} was not applied: {outcome:?}")),
    }
}

fn print_session(session: &SessionUpdate) {
    let Some(titles) = &session.titles else {
        return;
    };
    println!("Titles: {}", titles.files.len());
    for file in &titles.files {
        let state = if file.is_valid { "" } else { "  [invalid]" };
        println!("  {}{state}", file.path.display());
    }
    if let Some(notice) = &titles.notice {
        println!("Import notice: {notice:?}");
    }
    let Some(metadata) = &session.metadata else {
        return;
    };
    println!("Form ({:?}):", metadata.form.mode);
    for field in &metadata.form.fields {
        let shown = if field.mixed {
            "(mixed)"
        } else {
            field.value.as_str()
        };
        let edited = if field.dirty { "  *" } else { "" };
        println!("  {:?}: {shown}{edited}", field.field);
    }
    if let Some(message) = &metadata.form.validation_message {
        println!("Problem: {message}");
    }
    if let Some(status) = &metadata.status {
        println!("Status: {status:?}");
    }
    println!("Pending edits: {}", metadata.has_pending_edits);
}

fn submission(engine: &Engine) -> Option<SubmissionStatus> {
    engine
        .session_snapshot()
        .output
        .and_then(|output| output.submission)
}

fn collision_review(engine: &Engine) -> Option<CollisionReview> {
    engine
        .session_snapshot()
        .output
        .and_then(|output| output.collision_review)
}

async fn plan(engine: &Engine, options: &Options) -> Result<(), String> {
    let title_ids: Vec<String> = engine
        .session_snapshot()
        .titles
        .map(|titles| titles.files.into_iter().map(|file| file.input_id).collect())
        .unwrap_or_default();
    for edit in &options.audio {
        send(
            engine,
            SessionIntent::SetTitleAudio {
                title_ids: title_ids.clone(),
                edit: *edit,
            },
        )
        .await?;
    }
    if let Some(directory) = &options.out {
        let directory = std::path::absolute(directory)
            .map_err(|error| format!("bad export folder: {error}"))?;
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("could not create the export folder: {error}"))?;
        send(
            engine,
            SessionIntent::SetOutputDirectory {
                directory: directory.to_string_lossy().into_owned(),
            },
        )
        .await?;
    }
    if let Some(template) = &options.template {
        send(
            engine,
            SessionIntent::SetNamingPreset {
                preset: NamingPreset::CustomTemplate,
            },
        )
        .await?;
        send(
            engine,
            SessionIntent::SetNamingTemplate {
                template: template.clone(),
            },
        )
        .await?;
    }
    Ok(())
}

async fn submit(engine: &Engine, options: &Options) -> Result<SubmissionStatus, AbbDevStatus> {
    let intent = if options.preview {
        SessionIntent::Preview
    } else {
        SessionIntent::Submit
    };
    send(engine, intent).await?;
    let status = submission(engine).ok_or("the engine did not answer the submission")?;
    if status != SubmissionStatus::ReviewRequired {
        return Ok(status);
    }
    let CollisionReview {
        review_id, outputs, ..
    } = collision_review(engine).ok_or("the engine did not hold the review")?;
    let Some(policy) = options.on_collision else {
        send(engine, SessionIntent::CancelCollisionReview { review_id }).await?;
        let paths: Vec<String> = outputs
            .iter()
            .map(|output| output.resolved_path.clone())
            .collect();
        return Err(AbbDevStatus::PendingReview(format!(
            "these exports already exist; choose --on-collision: {}",
            paths.join(", ")
        )));
    };
    send(
        engine,
        SessionIntent::ChooseCollisionPolicy { review_id, policy },
    )
    .await?;
    Ok(submission(engine).ok_or("the engine did not answer the review")?)
}

/// Waits for an export to finish, calling `each` with every snapshot the
/// engine reports for it. A requested title is cancelled from the first
/// snapshot that shows it queued or running, so a fast passthrough cannot
/// finish it unseen.
async fn await_finished(
    feed: &mut OperationFeed,
    operation_id: &OperationId,
    mut each: impl FnMut(&OperationSnapshot) -> Result<(), String>,
) -> Result<OperationSnapshot, String> {
    loop {
        let known = feed.borrow_and_update().get(operation_id).cloned();
        if let Some(operation) = known {
            each(&operation)?;
            if operation.phase == OperationPhase::Finished {
                return Ok(operation);
            }
        }
        feed.changed()
            .await
            .map_err(|_| "the engine stopped reporting the export".to_string())?;
    }
}

async fn follow(
    engine: &Engine,
    feed: &mut OperationFeed,
    operation_id: &OperationId,
    cancel_title: Option<usize>,
) -> Result<OperationSnapshot, String> {
    let mut cancelled = cancel_title.is_none();
    await_finished(feed, operation_id, |operation| {
        if cancelled {
            return Ok(());
        }
        let index = cancel_title.unwrap_or(1) - 1;
        let child = operation
            .children
            .get(index)
            .ok_or_else(|| format!("there is no title {}", index + 1))?;
        if matches!(
            child.status,
            ChildJobStatus::Queued | ChildJobStatus::Running
        ) {
            engine
                .cancel_work_operation(operation_id.clone(), Some(child.child_job_id.clone()))
                .map_err(|error| error.to_string())?;
            eprintln!("Cancelled title {}: {}", index + 1, child.label);
            cancelled = true;
        }
        Ok(())
    })
    .await
}

async fn print_export(engine: &Engine, operation: &OperationSnapshot) {
    println!("Export: {:?}", operation.status);
    for child in &operation.children {
        println!("  {}: {:?}", child.label, child.status);
        for warning in child.supplemental_warning.iter().chain(&child.warnings) {
            println!("    warning: {warning}");
        }
        let Some(path) = &child.output_path else {
            continue;
        };
        println!("    {path}");
        match engine.read_audio_metadata(path.clone()).await {
            Ok(tags) => {
                for (name, value) in [
                    ("title", tags.title),
                    ("author", tags.artist),
                    ("album", tags.album),
                    ("genre", tags.genre),
                    ("series", tags.series),
                ] {
                    if let Some(value) = value {
                        println!("    {name}: {value}");
                    }
                }
            }
            Err(error) => println!("    could not read tags: {error}"),
        }
    }
}

fn export_verdict(
    operation: &OperationSnapshot,
    cancel_title: Option<usize>,
) -> Result<(), String> {
    if operation.children.iter().enumerate().all(|(index, child)| {
        let expected = if cancel_title == Some(index + 1) {
            ChildJobStatus::Cancelled
        } else {
            ChildJobStatus::Completed
        };
        child.status == expected
    }) {
        Ok(())
    } else {
        Err(format!("the export ended {:?}", operation.status))
    }
}

async fn produce(
    engine: &Engine,
    feed: &mut OperationFeed,
    options: &Options,
) -> Result<(), AbbDevStatus> {
    match submit(engine, options).await? {
        SubmissionStatus::Submitted {
            operation_id,
            title,
            ..
        } => {
            eprintln!("Exporting {title}");
            during_export::run_steps(
                engine,
                feed,
                &operation_id,
                &options.paths,
                &options.during_export,
            )
            .await?;
            let operation = follow(engine, feed, &operation_id, options.cancel_title).await?;
            during_export::await_landed(engine).await?;
            print_export(engine, &operation).await;
            Ok(export_verdict(&operation, options.cancel_title)?)
        }
        SubmissionStatus::PreviewFinished { result } => {
            for entry in &result.results {
                let output = entry.output_path.clone().unwrap_or_default();
                println!("Preview {}: {:?} {output}", entry.input_index, entry.status);
            }
            match result.terminal_class {
                abb_engine::processing::RunTerminalClass::Success => Ok(()),
                other => Err(format!("the preview ended {other:?}").into()),
            }
        }
        other => Err(format!("not exported: {other:?}").into()),
    }
}

async fn apply_edits(engine: &Engine, options: &Options) -> Result<(), String> {
    for (field, value) in options.edits.clone() {
        let binding = engine
            .session_snapshot()
            .metadata
            .ok_or("the session shows no metadata form")?
            .binding;
        let edit = SessionIntent::SetField {
            binding,
            field,
            value,
        };
        send(engine, edit).await?;
    }
    Ok(())
}

async fn run(options: Options, state_dir: PathBuf) -> AbbDevStatus {
    match drive(&options, state_dir).await {
        Ok(()) => AbbDevStatus::Success,
        Err(status) => status,
    }
}

async fn drive(options: &Options, state_dir: PathBuf) -> Result<(), AbbDevStatus> {
    let progress = Arc::new(PrintProgress::default());
    let mut feed = progress.feed();
    let engine = Engine::start(EngineConfig {
        cache_dir: state_dir.join("cache"),
        config_dir: state_dir.join("config"),
        app_identifier: APP_IDENTIFIER.to_string(),
        events: progress,
    })
    .map_err(|error| format!("engine failed to start: {error}"))?;

    if let Some(decoder) = options.decoder {
        let outcome = engine
            .settings_dispatch(SettingsIntent::SetAacDecoder { decoder })
            .await
            .outcome;
        if outcome != SettingsOutcome::Applied {
            return Err(format!("decoder choice was not applied: {outcome:?}").into());
        }
    }

    send(
        &engine,
        SessionIntent::Import {
            paths: options.paths.clone(),
        },
    )
    .await?;
    send(&engine, SessionIntent::SelectAll).await?;
    apply_edits(&engine, options).await?;
    if options.save {
        send(&engine, SessionIntent::Save).await?;
    }
    plan(&engine, options).await?;
    if options.export || options.preview {
        if let Err(status) = produce(&engine, &mut feed, options).await {
            shut_down(&engine).await;
            return Err(status);
        }
    }

    shut_down(&engine).await;
    let session = engine.session_snapshot();
    if options.json {
        let json = serde_json::to_string_pretty(&session)
            .map_err(|error| format!("could not print the session: {error}"))?;
        println!("{json}");
    } else {
        print_session(&session);
    }
    if nothing_imported(&session) {
        return Err(AbbDevStatus::NothingImported);
    }
    if options.save {
        save_written(&session).map_err(AbbDevStatus::SaveFailed)?;
    }
    if session
        .output
        .as_ref()
        .and_then(|output| output.collision_review.as_ref())
        .is_some()
    {
        return Err(AbbDevStatus::PendingReview(
            "a collision review is still pending".into(),
        ));
    }
    Ok(())
}

fn save_written(session: &SessionUpdate) -> Result<(), String> {
    let status = session
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.status.as_ref());
    match status {
        Some(MetadataStatus::SaveFailed { error }) => {
            Err(format!("Save failed: {}", error.message))
        }
        Some(MetadataStatus::SaveInvalid) => Err("Save was refused: the edits are invalid".into()),
        Some(
            MetadataStatus::SaveComplete { failed, .. }
            | MetadataStatus::DeferredWritesFinished { failed, .. },
        ) if *failed > 0 => Err(format!("Save could not write {failed} file(s)")),
        _ => Ok(()),
    }
}

fn nothing_imported(session: &SessionUpdate) -> bool {
    session
        .titles
        .as_ref()
        .is_none_or(|titles| titles.files.is_empty())
}

enum AbbDevStatus {
    Success,
    Help,
    Usage(String),
    NothingImported,
    SaveFailed(String),
    PendingReview(String),
    Failed(String),
}

impl From<String> for AbbDevStatus {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

impl From<&str> for AbbDevStatus {
    fn from(message: &str) -> Self {
        Self::Failed(message.to_string())
    }
}

fn exit_code(status: AbbDevStatus) -> ExitCode {
    match status {
        AbbDevStatus::Help => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        AbbDevStatus::Success => ExitCode::SUCCESS,
        AbbDevStatus::Usage(message) => {
            eprintln!("abb-dev: {message}\n\n{USAGE}");
            ExitCode::from(2)
        }
        AbbDevStatus::NothingImported => {
            eprintln!("abb-dev: nothing imported");
            ExitCode::FAILURE
        }
        AbbDevStatus::SaveFailed(message) => {
            eprintln!("abb-dev: {message}");
            ExitCode::FAILURE
        }
        AbbDevStatus::PendingReview(message) => {
            eprintln!("abb-dev: {message}");
            ExitCode::FAILURE
        }
        AbbDevStatus::Failed(message) => {
            eprintln!("abb-dev: {message}");
            ExitCode::FAILURE
        }
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let status = if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        AbbDevStatus::Help
    } else {
        match parse(std::env::args().skip(1)) {
            Err(message) => AbbDevStatus::Usage(message),
            Ok(options) => {
                let temporary = options.state_dir.is_none();
                let state_dir = options.state_dir.clone().unwrap_or_else(|| {
                    std::env::temp_dir().join(format!("abb-dev-{}", std::process::id()))
                });
                let status = run(options, state_dir.clone()).await;
                if temporary {
                    #[expect(
                        clippy::let_underscore_must_use,
                        reason = "the throwaway state folder of a finished dev run"
                    )]
                    let _ = std::fs::remove_dir_all(&state_dir);
                }
                status
            }
        }
    };
    exit_code(status)
}

#[cfg(test)]
#[path = "abb_dev/tests.rs"]
mod tests;
