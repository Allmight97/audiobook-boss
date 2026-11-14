use crate::errors::{AppError, Result};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const RECORDS_DIR: &str = "output-folders";

/// The cache folder that holds one record file per output folder ABB created
/// and has not yet handed over or removed, so the next start can remove the
/// empty ones a killed process left.
#[derive(Debug, Clone)]
pub(crate) struct OutputFolderRecords {
    root: PathBuf,
}

#[derive(Debug)]
pub(super) struct CreatedFolder {
    pub(super) dir: PathBuf,
    pub(super) anchor: PathBuf,
    record: PathBuf,
}

impl OutputFolderRecords {
    pub(crate) fn for_cache(cache_dir: &Path) -> Self {
        Self {
            root: cache_dir.join(RECORDS_DIR),
        }
    }

    /// Writes the record before the folder exists, so no kill leaves a
    /// created folder without one.
    pub(super) fn record(&self, dir: &Path, anchor: &Path) -> Result<CreatedFolder> {
        std::fs::create_dir_all(&self.root).map_err(record_error)?;
        let record = self.root.join(uuid::Uuid::new_v4().to_string());
        let mut contents = anchor.as_os_str().as_bytes().to_vec();
        contents.push(0);
        contents.extend_from_slice(dir.as_os_str().as_bytes());
        std::fs::write(&record, contents).map_err(record_error)?;
        Ok(CreatedFolder {
            dir: dir.to_path_buf(),
            anchor: anchor.to_path_buf(),
            record,
        })
    }

    /// Startup sweep: removes each recorded folder that is still empty,
    /// deepest first. A record whose anchor is missing (an unmounted disk) or
    /// whose removal failed stays for a later start.
    pub(crate) fn remove_abandoned(&self) -> Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        let mut folders = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let record = entry?.path();
            match read_record(&record) {
                Some(folder) => folders.push(folder),
                None => {
                    log::warn!("output_folder_record status=unreadable");
                    if let Err(error) = std::fs::remove_file(&record) {
                        log::warn!("output_folder_record status=remove_err err={error}");
                    }
                }
            }
        }
        folders.sort_by_key(|folder| std::cmp::Reverse(folder.dir.components().count()));
        for folder in folders {
            if !folder.anchor.exists() {
                log::info!("output_folder_record status=anchor_missing");
                continue;
            }
            match super::parent_dirs::remove_created_empty_dir(&folder.anchor, &folder.dir) {
                Ok(()) => folder.forget(),
                Err(error) => log::warn!("output_folder_record status=kept err={error}"),
            }
        }
        Ok(())
    }
}

impl CreatedFolder {
    pub(super) fn forget(&self) {
        if let Err(error) = std::fs::remove_file(&self.record) {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!("output_folder_record status=forget_err err={error}");
            }
        }
    }
}

fn read_record(record: &Path) -> Option<CreatedFolder> {
    let contents = std::fs::read(record).ok()?;
    let split = contents.iter().position(|byte| *byte == 0)?;
    Some(CreatedFolder {
        anchor: PathBuf::from(OsStr::from_bytes(&contents[..split])),
        dir: PathBuf::from(OsStr::from_bytes(&contents[split + 1..])),
        record: record.to_path_buf(),
    })
}

fn record_error(error: std::io::Error) -> AppError {
    AppError::FileValidation(format!(
        "Cannot record an output folder ABB creates: {error}"
    ))
}
