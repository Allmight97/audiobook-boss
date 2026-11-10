use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;

use abb_engine::remote_source::{
    AcquisitionSnapshot, ProviderId, RemoteAccountStatus, RemoteTitle, RemoteUiIntent,
};
use abb_engine::session::{SessionIntent, SessionOutcome, SessionReply};
use abb_engine::{Engine, EngineConfig, EngineEvent, EventSink};

use super::audible_outcome::{
    print_library, print_outcomes, title_outcome, LibraryRow, OutcomeStatus, TitleOutcome,
};
use super::{shut_down, AbbDevStatus, APP_IDENTIFIER};

pub(super) const USAGE: &str = "\
Usage: abb-dev audible <command> --state-dir <dir> [options]

Drives Audible through the engine, with no window. Sign in once per
identity; the sign-in stays in the keychain entry of abb-dev's own identity.

Commands:
  sign-in                 Print the address to open in a browser, then read
                          the address the browser lands on from standard
                          input. Nothing else needs a person.
  library                 List the library: title id, title, authors,
                          narrators, whether a PDF is offered, whether the
                          title can be acquired. The engine reports no
                          runtime or purchase date.
  acquire <title-id>...   Download and decrypt each title, one job per title
                          in the order given, and print where each landed or
                          why it failed. Exits nonzero if any title failed.

Options:
  --state-dir <dir>       Engine state folder (required). Acquired files sit in
                          its cache after this run ends; the next abb-dev run
                          with the same folder removes them when it starts.
  --json                  library and acquire: print JSON.
  --pdf                   acquire: also download each title's PDF. Without it
                          no PDF is downloaded.

Only sign-in needs a person. Progress lines (phase: ...) go to standard error.
";

const HEARTBEAT: Duration = Duration::from_secs(600);
const PROGRESS_STEP: u32 = 10;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Command {
    SignIn,
    Library,
    Acquire { title_ids: Vec<String>, pdf: bool },
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Options {
    pub(super) command: Command,
    pub(super) json: bool,
    pub(super) state_dir: PathBuf,
}

#[derive(Default)]
struct Flags {
    json: bool,
    pdf: bool,
    state_dir: Option<PathBuf>,
    positional: Vec<String>,
}

fn collect_flags(args: impl Iterator<Item = String>) -> Result<Flags, String> {
    let mut flags = Flags::default();
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => flags.json = true,
            "--pdf" => flags.pdf = true,
            "--state-dir" => {
                let dir = args.next().ok_or("--state-dir needs a folder")?;
                flags.state_dir = Some(dir.into());
            }
            flag if flag.starts_with("--") => return Err(format!("unknown option '{flag}'")),
            _ => flags.positional.push(arg),
        }
    }
    Ok(flags)
}

fn refuse_unused(name: &str, flags: &Flags, json: bool, pdf: bool) -> Result<(), String> {
    for (given, allowed, flag) in [(flags.json, json, "--json"), (flags.pdf, pdf, "--pdf")] {
        if given && !allowed {
            return Err(format!("{flag} does not apply to audible {name}"));
        }
    }
    Ok(())
}

fn command_from(name: &str, flags: &mut Flags) -> Result<Command, String> {
    match name {
        "sign-in" | "library" => {
            refuse_unused(name, flags, name == "library", false)?;
            if !flags.positional.is_empty() {
                return Err(format!("audible {name} takes no title ids"));
            }
            Ok(if name == "library" {
                Command::Library
            } else {
                Command::SignIn
            })
        }
        "acquire" => {
            let title_ids = std::mem::take(&mut flags.positional);
            if title_ids.is_empty() {
                return Err("audible acquire needs at least one title id".to_string());
            }
            let mut seen = HashSet::new();
            if let Some(twice) = title_ids.iter().find(|id| !seen.insert(*id)) {
                return Err(format!("title id '{twice}' is listed twice"));
            }
            Ok(Command::Acquire {
                title_ids,
                pdf: flags.pdf,
            })
        }
        other => Err(format!(
            "unknown audible command '{other}'; use sign-in, library, or acquire"
        )),
    }
}

pub(super) fn parse(args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut args = args;
    let name = args
        .next()
        .filter(|name| !name.starts_with("--"))
        .ok_or("audible needs a command: sign-in, library, or acquire")?;
    let mut flags = collect_flags(args)?;
    let state_dir = flags
        .state_dir
        .take()
        .ok_or("audible commands need --state-dir <dir>")?;
    let command = command_from(&name, &mut flags)?;
    Ok(Options {
        command,
        json: flags.json,
        state_dir,
    })
}

struct SessionTicks(watch::Sender<u64>);

impl EventSink for SessionTicks {
    fn emit(&self, event: EngineEvent) {
        if matches!(event, EngineEvent::Session(_)) {
            self.0.send_modify(|ticks| *ticks += 1);
        }
    }
}

pub(super) async fn run(options: Options) -> Result<(), AbbDevStatus> {
    let ticks = Arc::new(SessionTicks(watch::channel(0).0));
    let engine = Engine::start(EngineConfig {
        cache_dir: options.state_dir.join("cache"),
        config_dir: options.state_dir.join("config"),
        app_identifier: APP_IDENTIFIER.to_string(),
        events: ticks.clone(),
    })
    .map_err(|error| format!("engine failed to start: {error}"))?;
    let result = match &options.command {
        Command::SignIn => sign_in(&engine).await,
        Command::Library => library(&engine, options.json).await,
        Command::Acquire { title_ids, pdf } => {
            acquire(&engine, &ticks.0, title_ids, *pdf, options.json).await
        }
    };
    shut_down(&engine).await;
    result
}

async fn remote(engine: &Engine, intent: RemoteUiIntent) -> SessionReply {
    engine
        .session_dispatch(SessionIntent::Remote { intent })
        .await
}

fn applied(reply: SessionReply) -> Result<(), String> {
    match reply.outcome {
        SessionOutcome::Applied => Ok(()),
        SessionOutcome::Rejected { error } => Err(error.message),
        other => Err(format!("the engine answered {other:?}")),
    }
}

async fn sign_in(engine: &Engine) -> Result<(), AbbDevStatus> {
    let started = remote(engine, RemoteUiIntent::StartAuth).await;
    let SessionOutcome::RemoteAuthStarted { authorization } = started.outcome else {
        return Err(applied(started)
            .err()
            .unwrap_or_else(|| "the engine did not start the sign-in".to_string())
            .into());
    };
    eprintln!("Open this address in a browser and sign in to Audible.");
    eprintln!("Then paste the address the browser lands on, and press Return.");
    eprintln!("Press Ctrl-C to stop.");
    println!("{}", authorization.authorization_url);
    let long_lines = LongTerminalLines::allow();
    let mut interrupt = signal(SignalKind::interrupt())
        .map_err(|error| format!("could not listen for Ctrl-C: {error}"))?;
    loop {
        let line = tokio::select! {
            read = read_pasted_line_in_background() => {
                read.map_err(|error| error.to_string())?
                    .map_err(|error| error.to_string())?
            }
            _ = interrupt.recv() => stop_after_interrupt(engine, long_lines).await,
        };
        let Some(address) = line.as_deref().and_then(pasted_address) else {
            break;
        };
        let intent = RemoteUiIntent::CompleteAuth {
            response_url_handoff_path: Some(address.to_string()),
        };
        let reply = applied(remote(engine, intent).await);
        match (connected_account(engine), reply) {
            (Ok(()), reply) => {
                if let Err(warning) = reply {
                    eprintln!("Signed in, but: {warning}");
                }
                println!("Signed in to Audible.");
                return Ok(());
            }
            (Err(reason), reply) => {
                let message = reply.err().unwrap_or(reason);
                eprintln!("Sign-in was not completed: {message}\nPaste the address again, or press Ctrl-C to stop.");
            }
        }
    }
    Err("no address was given, so the sign-in did not finish".into())
}

fn pasted_address(line: &str) -> Option<&str> {
    let address = line.trim_matches(|c: char| c.is_whitespace() || c == '\u{4}');
    (!address.is_empty()).then_some(address)
}

#[expect(
    clippy::disallowed_methods,
    reason = "joined by sign_in, which ends the process when interrupted"
)]
fn read_pasted_line_in_background() -> tokio::task::JoinHandle<std::io::Result<Option<String>>> {
    tokio::task::spawn_blocking(read_pasted_line)
}

fn read_pasted_line() -> std::io::Result<Option<String>> {
    let mut line = String::new();
    let read = std::io::stdin().read_line(&mut line)?;
    Ok((read > 0).then_some(line))
}

/// The blocked read of standard input would keep the runtime from shutting
/// down, so the process leaves here after restoring the terminal.
async fn stop_after_interrupt(engine: &Engine, long_lines: Option<LongTerminalLines>) -> ! {
    drop(long_lines);
    shut_down(engine).await;
    eprintln!("abb-dev: sign-in stopped");
    std::process::exit(130)
}

/// The landing address runs past the 1024-byte line limit of a terminal in
/// canonical mode, which drops the rest of the line and the Return with it.
/// Turning canonical mode off for the read lifts that limit. The earlier
/// settings come back exactly when the guard drops.
struct LongTerminalLines {
    saved: String,
}

impl LongTerminalLines {
    fn allow() -> Option<Self> {
        if !std::io::stdin().is_terminal() {
            return None;
        }
        let saved = stty_output("-g")?;
        stty(&["-icanon"]).then_some(Self { saved })
    }
}

impl Drop for LongTerminalLines {
    fn drop(&mut self) {
        if !stty(&[self.saved.as_str()]) {
            eprintln!("Could not restore the terminal; run `stty sane`.");
        }
    }
}

fn stty_output(setting: &str) -> Option<String> {
    let output = std::process::Command::new("stty")
        .arg(setting)
        .stdin(std::process::Stdio::inherit())
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    (output.status.success() && !text.is_empty()).then(|| text.to_string())
}

fn stty(settings: &[&str]) -> bool {
    std::process::Command::new("stty")
        .args(settings)
        .stdin(std::process::Stdio::inherit())
        .status()
        .is_ok_and(|status| status.success())
}

/// Whether the engine now holds a connected Audible account. A sign-in whose
/// credentials were stored can still be answered with a rejection, when the
/// library read after it fails.
fn connected_account(engine: &Engine) -> Result<(), String> {
    let account = engine
        .session_snapshot()
        .remote
        .and_then(|remote| remote.account);
    match account {
        Some(account) if account.status == RemoteAccountStatus::Connected => Ok(()),
        Some(account) => Err(account
            .message
            .unwrap_or_else(|| "Audible did not report a connected account".to_string())),
        None => Err("the engine reported no Audible account".to_string()),
    }
}

/// Selects the Audible lane, which reads the account and loads the library,
/// and returns the library rows.
async fn load_library(engine: &Engine) -> Result<Vec<RemoteTitle>, String> {
    applied(
        remote(
            engine,
            RemoteUiIntent::SelectLane {
                lane: ProviderId::Audible,
            },
        )
        .await,
    )?;
    let snapshot = engine.session_snapshot();
    connected_account(engine).map_err(|reason| {
        format!("Audible is not signed in ({reason}); run abb-dev audible sign-in")
    })?;
    snapshot
        .remote_library
        .map(|library| library.titles)
        .ok_or_else(|| "the engine reported no library".to_string())
}

async fn library(engine: &Engine, json: bool) -> Result<(), AbbDevStatus> {
    let titles = load_library(engine).await?;
    let rows: Vec<LibraryRow> = titles.iter().map(LibraryRow::from).collect();
    Ok(print_library(&rows, json)?)
}

async fn acquire(
    engine: &Engine,
    ticks: &watch::Sender<u64>,
    title_ids: &[String],
    pdf: bool,
    json: bool,
) -> Result<(), AbbDevStatus> {
    let library = load_library(engine).await?;
    let mut outcomes = Vec::new();
    for title_id in title_ids {
        let outcome = match library.iter().find(|title| &title.title_id == title_id) {
            Some(title) => acquire_one(engine, ticks, title, pdf).await,
            None => TitleOutcome::failed(title_id, "the title is not in the library"),
        };
        outcomes.push(outcome);
    }
    print_outcomes(&outcomes, json)?;
    let failed = outcomes
        .iter()
        .filter(|outcome| outcome.status == OutcomeStatus::Failed)
        .count();
    if failed > 0 {
        return Err(format!("{failed} of {} titles failed", outcomes.len()).into());
    }
    Ok(())
}

async fn choose_title(engine: &Engine, title: &RemoteTitle, pdf: bool) -> Result<(), String> {
    applied(remote(engine, RemoteUiIntent::ClearTitles).await)?;
    let title_id = title.title_id.clone();
    applied(
        remote(
            engine,
            RemoteUiIntent::ToggleTitle {
                title_id: title_id.clone(),
            },
        )
        .await,
    )?;
    let included = engine
        .session_snapshot()
        .remote
        .and_then(|remote| remote.include_pdf_by_title_id.get(&title_id).copied())
        .unwrap_or(false);
    if title.supplemental_pdf_available && included != pdf {
        applied(remote(engine, RemoteUiIntent::TogglePdf { title_id }).await)?;
    }
    Ok(())
}

fn current_job(engine: &Engine) -> Option<AcquisitionSnapshot> {
    engine
        .session_snapshot()
        .remote_acquisition
        .and_then(|acquisition| acquisition.job)
        .map(|job| *job)
}

fn session_input_ids(engine: &Engine) -> HashSet<String> {
    engine
        .session_snapshot()
        .titles
        .map(|titles| titles.files.into_iter().map(|file| file.input_id).collect())
        .unwrap_or_default()
}

fn files_added_since(engine: &Engine, known: &HashSet<String>) -> Vec<PathBuf> {
    engine
        .session_snapshot()
        .titles
        .map(|titles| {
            titles
                .files
                .into_iter()
                .filter(|file| !known.contains(&file.input_id))
                .map(|file| file.path)
                .collect()
        })
        .unwrap_or_default()
}

async fn acquire_one(
    engine: &Engine,
    ticks: &watch::Sender<u64>,
    title: &RemoteTitle,
    pdf: bool,
) -> TitleOutcome {
    let title_id = &title.title_id;
    if let Err(message) = choose_title(engine, title, pdf).await {
        return TitleOutcome::failed(title_id, message);
    }
    let previous_job = current_job(engine).map(|job| job.job_id.clone());
    let known = session_input_ids(engine);
    if let Err(message) = applied(remote(engine, RemoteUiIntent::AcquireSelected).await) {
        return TitleOutcome::failed(title_id, message);
    }
    match await_settled(engine, ticks, previous_job, title_id).await {
        Ok(job) => title_outcome(title_id, &job, files_added_since(engine, &known)),
        Err(message) => TitleOutcome::failed(title_id, message),
    }
}

/// Waits on the engine's session events until the job that replaced
/// `previous_job` has settled, which is after its files reached the session.
async fn await_settled(
    engine: &Engine,
    ticks: &watch::Sender<u64>,
    previous_job: Option<String>,
    title_id: &str,
) -> Result<AcquisitionSnapshot, String> {
    let mut changes = ticks.subscribe();
    let started = Instant::now();
    let mut reporter = PhaseReporter::default();
    loop {
        changes.borrow_and_update();
        if let Some(job) =
            current_job(engine).filter(|job| Some(&job.job_id) != previous_job.as_ref())
        {
            reporter.report(title_id, &job);
            if job.settled {
                return Ok(job);
            }
        }
        if tokio::time::timeout(HEARTBEAT, changes.changed())
            .await
            .is_err()
        {
            eprintln!(
                "phase: still working on {title_id}, {} s elapsed",
                started.elapsed().as_secs()
            );
        }
    }
}

#[derive(Default)]
struct PhaseReporter {
    last: Option<(String, u32)>,
}

impl PhaseReporter {
    fn report(&mut self, title_id: &str, job: &AcquisitionSnapshot) {
        let stage = format!("{:?}", job.progress.stage);
        let step = (job.progress.percentage.max(0.0) as u32) / PROGRESS_STEP;
        let now = (stage.clone(), step);
        if self.last.as_ref() != Some(&now) {
            eprintln!(
                "phase: {title_id} {stage} {}%",
                job.progress.percentage.round()
            );
            self.last = Some(now);
        }
    }
}

#[cfg(test)]
#[path = "audible_tests.rs"]
mod tests;
