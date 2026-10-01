//! Developer tool: drives the engine's working session from a terminal, the
//! way a UI host would, with no window.
//!
//! It imports audio, prints the session, optionally edits metadata on every
//! imported title, and optionally saves. It runs under its own identity and
//! state folder, so it never reads or changes the app's settings or stored
//! credentials.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use abb_engine::app_settings::EncoderDefaults;
use abb_engine::audio::TitleAudioRequest;
use abb_engine::session::{MetadataField, SessionIntent, SessionOutcome, SessionUpdate};
use abb_engine::{DiscardEvents, Engine, EngineConfig};

const USAGE: &str = "\
Usage: abb-dev <file-or-folder>... [options]

Imports audio into an engine session and prints the session.

Options:
  --set <field>=<value>   Edit a field on every imported title. Repeatable.
                          Fields: title, date, author, narrator, series,
                          series-part, subseries, subseries-part, genre,
                          description. An empty value clears the field.
  --save                  Write pending edits to the files.
  --json                  Print the session as JSON.
  --state-dir <dir>       Keep engine state here instead of a folder that is
                          removed when the tool exits.
";

const APP_IDENTIFIER: &str = "com.audiobook-boss.devtool";

struct Options {
    paths: Vec<String>,
    edits: Vec<(MetadataField, String)>,
    save: bool,
    json: bool,
    state_dir: Option<PathBuf>,
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
    let mut options = Options {
        paths: Vec::new(),
        edits: Vec::new(),
        save: false,
        json: false,
        state_dir: None,
    };
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--save" => options.save = true,
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

async fn run(options: Options, state_dir: PathBuf) -> Result<(), String> {
    let engine = Engine::start(EngineConfig {
        cache_dir: state_dir.join("cache"),
        config_dir: state_dir.join("config"),
        app_identifier: APP_IDENTIFIER.to_string(),
        events: Arc::new(DiscardEvents),
        aaxclean_helper: None,
    })
    .map_err(|error| format!("engine failed to start: {error}"))?;

    let defaults = EncoderDefaults::default();
    send(
        &engine,
        SessionIntent::Import {
            paths: options.paths,
            default_audio: TitleAudioRequest {
                format: defaults.format,
                intent: defaults.intent,
                settings: Some(defaults.settings),
                sample_rate: defaults.sample_rate,
            },
        },
    )
    .await?;
    send(&engine, SessionIntent::SelectAll).await?;
    for (field, value) in options.edits {
        send(&engine, SessionIntent::SetField { field, value }).await?;
    }
    if options.save {
        send(&engine, SessionIntent::Save).await?;
    }

    let session = engine.session_snapshot();
    if options.json {
        let json = serde_json::to_string_pretty(&session)
            .map_err(|error| format!("could not print the session: {error}"))?;
        println!("{json}");
    } else {
        print_session(&session);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
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
