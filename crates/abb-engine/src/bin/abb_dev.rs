//! Developer tool: drives the engine's working session from a terminal, the
//! way a UI host would, with no window.
//!
//! It imports audio, prints the session, and can edit metadata on every
//! imported title, save, choose audio and output, export or preview, show
//! progress, cancel one title, and read back the exported tags. It runs under
//! its own identity and state folder, so it never reads or changes the app's
//! settings or stored credentials.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use abb_engine::audio::{AudioIntent, AudiobookFormat};
use abb_engine::output_artifact::{CollisionPolicy, NamingPreset};
use abb_engine::session::{
    AudioEdit, MetadataField, MetadataStatus, SessionIntent, SessionOutcome, SessionUpdate,
    SubmissionStatus,
};
use abb_engine::work_runtime::{
    ChildJobStatus, OperationId, OperationSnapshot, WorkOperationStatus,
};
use abb_engine::{Engine, EngineConfig, EngineEvent, EventSink};

const USAGE: &str = "\
Usage: abb-dev <file-or-folder>... [options]

Imports audio into an engine session and prints the session.

Options:
  --set <field>=<value>   Edit a field on every imported title. Repeatable.
                          Fields: title, date, author, narrator, series,
                          series-part, subseries, subseries-part, genre,
                          description. An empty value clears the field.
  --save                  Write pending edits to the files.
  --format <format>       Output format for every title: m4b, mp3, m4aOpus,
                          or mkaOpus.
  --intent <intent>       Audio handling for every title: auto, preserve, or
                          encode.
  --bitrate <kbps>        Target bitrate for encoded titles.
  --out <folder>          Export folder; created if missing.
  --template <template>   Name exports with a custom template, such as
                          '{author}/{title}'.
  --export                Export every valid title and wait for it to finish.
  --preview <seconds>     Render the first seconds of each title instead.
  --on-collision <policy> What to do when an export already exists: rename,
                          replace, or skip. Without it, a collision stops.
  --cancel-title <n>      Cancel the nth title (from 1) once the export runs.
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
    out: Option<String>,
    template: Option<String>,
    export: bool,
    preview: Option<f64>,
    on_collision: Option<CollisionPolicy>,
    cancel_title: Option<usize>,
    json: bool,
    state_dir: Option<PathBuf>,
}

/// Reads an engine enum from its wire name.
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
            "--out" => options.out = Some(value(&mut args, "--out")?),
            "--template" => options.template = Some(value(&mut args, "--template")?),
            "--preview" => {
                let seconds = value(&mut args, "--preview")?;
                let seconds = seconds
                    .parse()
                    .map_err(|_| format!("bad preview length '{seconds}'"))?;
                options.preview = Some(seconds);
            }
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
                let (name, value) = edit
                    .split_once('=')
                    .ok_or_else(|| format!("--set needs <field>=<value>, got '{edit}'"))?;
                let field = field_named(name).ok_or_else(|| format!("unknown field '{name}'"))?;
                options.edits.push((field, value.to_string()));
            }
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

/// Prints each title's progress as an export runs, once per change of stage.
#[derive(Default)]
struct PrintProgress {
    seen: Mutex<HashMap<String, String>>,
}

impl EventSink for PrintProgress {
    fn emit(&self, event: EngineEvent) {
        let EngineEvent::WorkOperations(update) = event else {
            return;
        };
        let operation = update.changed;
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

/// Sends an intent and reports one that the session did not apply.
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

/// Chooses audio and output for every title.
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

/// Submits the session, settling a collision review with `--on-collision`.
async fn submit(engine: &Engine, options: &Options) -> Result<SubmissionStatus, String> {
    let intent = match options.preview {
        Some(seconds) => SessionIntent::Preview { seconds },
        None => SessionIntent::Submit,
    };
    send(engine, intent).await?;
    let status = submission(engine).ok_or("the engine did not answer the submission")?;
    let SubmissionStatus::ReviewRequired {
        review_id, outputs, ..
    } = &status
    else {
        return Ok(status);
    };
    let Some(policy) = options.on_collision else {
        send(
            engine,
            SessionIntent::CancelCollisionReview {
                review_id: *review_id,
            },
        )
        .await?;
        let paths: Vec<String> = outputs
            .iter()
            .map(|output| output.resolved_path.clone())
            .collect();
        return Err(format!(
            "these exports already exist; choose --on-collision: {}",
            paths.join(", ")
        ));
    };
    send(
        engine,
        SessionIntent::ChooseCollisionPolicy {
            review_id: *review_id,
            policy,
        },
    )
    .await?;
    submission(engine).ok_or_else(|| "the engine did not answer the review".to_string())
}

/// Waits for an export, cancelling one title once it runs if asked.
async fn follow(
    engine: &Engine,
    operation_id: &OperationId,
    cancel_title: Option<usize>,
) -> Result<OperationSnapshot, String> {
    let mut cancelled = cancel_title.is_none();
    loop {
        let operation = engine
            .list_work_operations()
            .map_err(|error| error.to_string())?
            .operations
            .into_iter()
            .find(|operation| &operation.operation_id == operation_id)
            .ok_or("the export is no longer listed")?;
        if !matches!(
            operation.status,
            WorkOperationStatus::Accepted
                | WorkOperationStatus::Running
                | WorkOperationStatus::Cancelling
        ) {
            return Ok(operation);
        }
        if !cancelled && operation.status == WorkOperationStatus::Running {
            let index = cancel_title.unwrap_or(1) - 1;
            let child = operation
                .children
                .get(index)
                .ok_or_else(|| format!("there is no title {}", index + 1))?;
            engine
                .cancel_work_operation(operation_id.clone(), Some(child.child_job_id.clone()))
                .map_err(|error| error.to_string())?;
            eprintln!("Cancelled title {}: {}", index + 1, child.label);
            cancelled = true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Prints each title's outcome and the tags read back from its export.
async fn print_export(engine: &Engine, operation: &OperationSnapshot) {
    println!("Export: {:?}", operation.status);
    for child in &operation.children {
        println!("  {}: {:?}", child.label, child.status);
        if let Some(warning) = &child.supplemental_warning {
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

/// Succeeds only when every title was published, or when the only titles
/// not published are the one `--cancel-title` cancelled.
fn export_verdict(operation: &OperationSnapshot, cancelled_one: bool) -> Result<(), String> {
    let published = |child: &abb_engine::work_runtime::ChildJobSnapshot| {
        child.status == ChildJobStatus::Completed
            || (cancelled_one && child.status == ChildJobStatus::Cancelled)
    };
    if operation.children.iter().all(published) {
        Ok(())
    } else {
        Err(format!("the export ended {:?}", operation.status))
    }
}

/// Exports or previews, and prints how it went.
async fn produce(engine: &Engine, options: &Options) -> Result<(), String> {
    match submit(engine, options).await? {
        SubmissionStatus::Submitted {
            operation_id,
            title,
        } => {
            eprintln!("Exporting {title}");
            let operation = follow(engine, &operation_id, options.cancel_title).await?;
            print_export(engine, &operation).await;
            export_verdict(&operation, options.cancel_title.is_some())
        }
        SubmissionStatus::PreviewFinished { result } => {
            for entry in &result.results {
                let output = entry.output_path.clone().unwrap_or_default();
                println!("Preview {}: {:?} {output}", entry.input_index, entry.status);
            }
            match result.terminal_class {
                abb_engine::processing::RunTerminalClass::Success => Ok(()),
                other => Err(format!("the preview ended {other:?}")),
            }
        }
        other => Err(format!("not exported: {other:?}")),
    }
}

async fn run(options: Options, state_dir: PathBuf) -> Result<(), String> {
    let engine = Engine::start(EngineConfig {
        cache_dir: state_dir.join("cache"),
        config_dir: state_dir.join("config"),
        app_identifier: APP_IDENTIFIER.to_string(),
        events: Arc::new(PrintProgress::default()),
        aaxclean_helper: None,
    })
    .map_err(|error| format!("engine failed to start: {error}"))?;

    send(
        &engine,
        SessionIntent::Import {
            paths: options.paths.clone(),
        },
    )
    .await?;
    send(&engine, SessionIntent::SelectAll).await?;
    for (field, value) in options.edits.clone() {
        send(&engine, SessionIntent::SetField { field, value }).await?;
    }
    if options.save {
        send(&engine, SessionIntent::Save).await?;
    }
    plan(&engine, &options).await?;
    if options.export || options.preview.is_some() {
        if let Err(message) = produce(&engine, &options).await {
            engine.shutdown().await;
            return Err(message);
        }
    }

    // Waiting saves are written and background work settles before the
    // state folder can be removed.
    engine.shutdown().await;
    let session = engine.session_snapshot();
    if options.json {
        let json = serde_json::to_string_pretty(&session)
            .map_err(|error| format!("could not print the session: {error}"))?;
        println!("{json}");
    } else {
        print_session(&session);
    }
    if options.save {
        save_written(&session)?;
    }
    Ok(())
}

/// A Save the engine accepted can still fail to write; that fails the run.
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

#[tokio::main]
async fn main() -> ExitCode {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let options = match parse(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("abb-dev: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let temporary = options.state_dir.is_none();
    let state_dir = options
        .state_dir
        .clone()
        .unwrap_or_else(|| std::env::temp_dir().join(format!("abb-dev-{}", std::process::id())));

    let result = run(options, state_dir.clone()).await;
    if temporary {
        let _ = std::fs::remove_dir_all(&state_dir);
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("abb-dev: {message}");
            ExitCode::FAILURE
        }
    }
}
