use abb_engine::work_runtime::{OperationId, OperationSnapshot, WorkOperationsSnapshot};

use crate::commands::{CommandResult, EngineState};

#[tauri::command]
#[specta::specta]
pub fn list_work_operations(engine: EngineState<'_>) -> CommandResult<WorkOperationsSnapshot> {
    Ok(engine.list_work_operations()?)
}

#[tauri::command]
#[specta::specta]
pub fn cancel_work_operation(
    engine: EngineState<'_>,
    operation_id: OperationId,
    child_job_id: Option<String>,
) -> CommandResult<OperationSnapshot> {
    Ok(engine.cancel_work_operation(operation_id, child_job_id)?)
}
