pub mod app_settings;
pub mod audio;
pub mod metadata;
pub mod metadata_lookup;
pub mod remote_source;
pub mod system;
pub mod work_runtime;

pub type CommandResult<T> = std::result::Result<T, crate::errors::AppErrorEnvelope>;

pub use app_settings::*;
pub use audio::*;
pub use metadata::*;
pub use metadata_lookup::*;
pub use remote_source::*;
pub use system::*;
pub use work_runtime::*;
