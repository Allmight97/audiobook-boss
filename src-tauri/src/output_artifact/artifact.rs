use super::types::OutputKind;
use crate::errors::Result;
use std::path::{Path, PathBuf};

pub(crate) fn derive_output_artifact_path(
    requested_final_path: &Path,
    kind: OutputKind,
) -> Result<PathBuf> {
    match kind {
        OutputKind::Final => Ok(requested_final_path.to_path_buf()),
        OutputKind::Preview => {
            let parent = requested_final_path
                .parent()
                .unwrap_or_else(|| Path::new("."));
            let stem = requested_final_path
                .file_stem()
                .map(|value| value.to_string_lossy())
                .unwrap_or_else(|| "output".into());
            Ok(parent.join(format!("{stem}.preview.m4b")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::derive_output_artifact_path;
    use crate::output_artifact::OutputKind;
    use std::path::Path;

    #[test]
    fn derive_output_artifact_path_appends_preview_suffix() {
        let path = Path::new("/tmp/Book 1.m4b");
        let preview = derive_output_artifact_path(path, OutputKind::Preview).expect("preview");
        assert_eq!(preview, Path::new("/tmp/Book 1.preview.m4b"));
    }

    #[test]
    fn derive_output_artifact_path_keeps_final_path_unchanged() {
        let path = Path::new("/tmp/Book 1.m4b");
        let final_path = derive_output_artifact_path(path, OutputKind::Final).expect("final");
        assert_eq!(final_path, path);
    }
}
