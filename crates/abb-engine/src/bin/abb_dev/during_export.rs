use std::time::{Duration, Instant};

use abb_engine::session::{MetadataField, MetadataStatus, SessionIntent};
use abb_engine::work_runtime::{ChildJobStatus, OperationId, OperationPhase, OperationSnapshot};
use abb_engine::Engine;

use super::{save_status_error, send, AbbDevStatus, OperationFeed};

const POLL: Duration = Duration::from_millis(10);
const START_LIMIT: Duration = Duration::from_secs(60);
const LAND_LIMIT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone)]
pub(super) enum DuringExport {
    Set(MetadataField, String),
    Reimport,
}

impl DuringExport {
    fn describe(&self) -> String {
        match self {
            Self::Set(field, value) => format!("--during-export-set {field:?}={value}"),
            Self::Reimport => "--during-export-reimport".to_string(),
        }
    }
}

fn operation_of(engine: &Engine, operation_id: &OperationId) -> Result<OperationSnapshot, String> {
    engine
        .list_work_operations()
        .map_err(|error| error.to_string())?
        .operations
        .into_iter()
        .find(|operation| &operation.operation_id == operation_id)
        .ok_or_else(|| "the export is no longer listed".to_string())
}

fn is_active(operation: &OperationSnapshot) -> bool {
    operation.phase != OperationPhase::Finished
}

fn is_reading(operation: &OperationSnapshot) -> bool {
    is_active(operation)
        && operation
            .children
            .iter()
            .any(|child| child.status == ChildJobStatus::Running)
}

async fn await_reading(feed: &mut OperationFeed, operation_id: &OperationId) -> Result<(), String> {
    let seen = tokio::time::timeout(
        START_LIMIT,
        feed.wait_for(|operations| {
            operations
                .get(operation_id)
                .is_some_and(|operation| is_reading(operation) || !is_active(operation))
        }),
    )
    .await
    .map_err(|_| "the export never started reading".to_string())?
    .map_err(|_| "the engine stopped reporting the export".to_string())?;
    if seen.get(operation_id).is_some_and(is_reading) {
        Ok(())
    } else {
        Err("the export finished before it was seen reading; use a longer input".to_string())
    }
}

fn require_reading(
    engine: &Engine,
    operation_id: &OperationId,
    step: &DuringExport,
) -> Result<(), String> {
    if is_reading(&operation_of(engine, operation_id)?) {
        Ok(())
    } else {
        Err(format!(
            "the export had finished before {}; use a longer input",
            step.describe()
        ))
    }
}

fn waiting_saves(engine: &Engine) -> Result<usize, String> {
    match engine
        .session_snapshot()
        .metadata
        .and_then(|metadata| metadata.status)
    {
        Some(MetadataStatus::SaveComplete { waiting, .. }) => Ok(waiting),
        other => Err(format!("the Save did not report its result: {other:?}")),
    }
}

async fn set_and_save(
    engine: &Engine,
    step: &DuringExport,
    field: MetadataField,
    value: &str,
) -> Result<(), String> {
    let binding = engine
        .session_snapshot()
        .metadata
        .ok_or("the session shows no metadata form")?
        .binding;
    send(
        engine,
        SessionIntent::SetField {
            binding,
            field,
            value: value.to_string(),
        },
    )
    .await?;
    send(engine, SessionIntent::Save).await?;
    if waiting_saves(engine)? == 0 {
        return Err(format!(
            "the Save at {} did not wait for the export; use a longer input",
            step.describe()
        ));
    }
    Ok(())
}

async fn reimport(engine: &Engine, paths: &[String]) -> Result<(), String> {
    send(engine, SessionIntent::ClearAll).await?;
    send(
        engine,
        SessionIntent::Import {
            paths: paths.to_vec(),
        },
    )
    .await?;
    send(engine, SessionIntent::SelectAll).await
}

pub(super) async fn run_steps(
    engine: &Engine,
    feed: &mut OperationFeed,
    operation_id: &OperationId,
    paths: &[String],
    steps: &[DuringExport],
) -> Result<(), String> {
    if steps.is_empty() {
        return Ok(());
    }
    await_reading(feed, operation_id).await?;
    for step in steps {
        require_reading(engine, operation_id, step)?;
        match step {
            DuringExport::Set(field, value) => set_and_save(engine, step, *field, value).await?,
            DuringExport::Reimport => reimport(engine, paths).await?,
        }
        eprintln!("During export: {}", step.describe());
    }
    Ok(())
}

pub(super) async fn await_landed(
    engine: &Engine,
    steps: &[DuringExport],
) -> Result<(), AbbDevStatus> {
    let started = Instant::now();
    while engine.running_work().waiting_writes > 0 {
        if started.elapsed() > LAND_LIMIT {
            return Err("waiting writes did not land".into());
        }
        tokio::time::sleep(POLL).await;
    }
    let save_expected = steps
        .iter()
        .any(|step| matches!(step, DuringExport::Set(..)));
    let session = engine.session_snapshot();
    landed_verdict(
        session
            .metadata
            .as_ref()
            .and_then(|metadata| metadata.status.as_ref()),
        save_expected,
    )
}

pub(super) fn landed_verdict(
    status: Option<&MetadataStatus>,
    save_expected: bool,
) -> Result<(), AbbDevStatus> {
    save_status_error(status).map_err(AbbDevStatus::SaveFailed)?;
    if save_expected && !matches!(status, Some(MetadataStatus::DeferredWritesFinished { .. })) {
        return Err(AbbDevStatus::SaveFailed(
            "the waiting Save did not report that it finished".into(),
        ));
    }
    Ok(())
}
