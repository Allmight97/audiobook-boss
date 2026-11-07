use std::time::{Duration, Instant};

use abb_engine::session::{MetadataField, MetadataStatus, SessionIntent};
use abb_engine::work_runtime::{ChildJobStatus, OperationId, OperationPhase, OperationSnapshot};
use abb_engine::Engine;

use super::send;

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

async fn await_reading(engine: &Engine, operation_id: &OperationId) -> Result<(), String> {
    let started = Instant::now();
    loop {
        let operation = operation_of(engine, operation_id)?;
        if is_reading(&operation) {
            return Ok(());
        }
        if !is_active(&operation) {
            return Err(
                "the export finished before it was seen reading; use a longer input".to_string(),
            );
        }
        if started.elapsed() > START_LIMIT {
            return Err("the export never started reading".to_string());
        }
        tokio::time::sleep(POLL).await;
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
    operation_id: &OperationId,
    paths: &[String],
    steps: &[DuringExport],
) -> Result<(), String> {
    if steps.is_empty() {
        return Ok(());
    }
    await_reading(engine, operation_id).await?;
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

pub(super) async fn await_landed(engine: &Engine) -> Result<(), String> {
    let started = Instant::now();
    while engine.running_work().waiting_writes > 0 {
        if started.elapsed() > LAND_LIMIT {
            return Err("waiting writes did not land".to_string());
        }
        tokio::time::sleep(POLL).await;
    }
    match engine
        .session_snapshot()
        .metadata
        .and_then(|metadata| metadata.status)
    {
        Some(MetadataStatus::DeferredWritesFinished { failed, .. }) if failed > 0 => {
            Err(format!("Save could not write {failed} file(s)"))
        }
        _ => Ok(()),
    }
}
