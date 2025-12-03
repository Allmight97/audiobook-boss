use std::path::{Path, PathBuf};

/// Returns a path to a small audio fixture in the repo.
pub fn sample_mp3() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("manifest dir parent")
        .join("media")
        .join("media_20sec.mp3")
}
