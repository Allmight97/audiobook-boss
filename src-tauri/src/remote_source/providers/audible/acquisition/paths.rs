use std::fs;
use std::io::Read;
use std::path::Path;

use abb_audible_core::{download_extension_for_strategy, remote_materialized_filename_stem};
use abb_remote_source_core::AcquisitionStrategy;
use sha2::{Digest, Sha256};

use crate::errors::Result;

pub(in crate::remote_source::providers::audible) fn staged_protected_source_path(
    job_dir: &Path,
    strategy: AcquisitionStrategy,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "source.{}",
        download_extension_for_strategy(strategy)
    ))
}

pub(in crate::remote_source::providers::audible) fn staged_materialized_path(
    job_dir: &Path,
    title_name: Option<&str>,
    title_id: &str,
) -> std::path::PathBuf {
    job_dir.join(format!(
        "{}.m4b",
        remote_materialized_filename_stem(title_name, title_id)
    ))
}

pub(in crate::remote_source::providers::audible) fn generated_staging_path(
    job_dir: &Path,
    extension: &str,
) -> std::path::PathBuf {
    job_dir.join(format!("{}.{}", uuid::Uuid::new_v4(), extension))
}

pub(in crate::remote_source::providers::audible) fn sha256_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
