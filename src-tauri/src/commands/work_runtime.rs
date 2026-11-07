use abb_engine::work_runtime::OperationId;

use crate::commands::{CommandResult, EngineState};

#[tauri::command]
#[specta::specta]
pub fn cancel_work_operation(
    engine: EngineState<'_>,
    operation_id: OperationId,
    child_job_id: Option<String>,
) -> CommandResult<()> {
    engine.cancel_work_operation(operation_id, child_job_id)?;
    Ok(())
}
