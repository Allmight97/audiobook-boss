//! Tauri command ingress. Each command forwards to the engine and returns its
//! result in the wire error shape; no product rule lives here.

pub mod audio;
pub mod frontend_log;
pub mod session;
pub mod work_runtime;

pub type CommandResult<T> = std::result::Result<T, abb_engine::AppErrorEnvelope>;
pub(crate) type EngineState<'a> = tauri::State<'a, abb_engine::Engine>;

pub use audio::*;
pub use frontend_log::*;
pub use session::*;
pub use work_runtime::*;
