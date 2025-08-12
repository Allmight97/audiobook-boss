pub mod guard;
pub mod ops;

pub use guard::CleanupGuard;

#[cfg(any(test, feature = "safe-ffmpeg"))]
pub use guard::ProcessGuard;


