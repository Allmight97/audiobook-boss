use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::extensions::{audio_format_for_path, SUPPORTED_AUDIO_FORMATS};
use super::path_validation::{
    validate_input_audio_path, validate_input_directory_path, validate_input_pdf_path,
};
use crate::errors::{sanitize_path_for_display, AppError};
use crate::natural_order::natural_cmp;
use crate::output_artifact::SupplementalProcessingAsset;

pub fn supported_audio_import_extensions() -> Vec<&'static str> {
    SUPPORTED_AUDIO_FORMATS
        .iter()
        .map(|format| format.extension)
        .collect()
}

pub fn supported_audio_import_formats_text() -> String {
    join_display_labels(&unique_supported_labels())
}

pub fn supported_audio_import_support_text() -> String {
    format!(
        "Supports {} audio files",
        supported_audio_import_formats_text()
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedImportPath {
    pub path: String,
    pub reason: String,
}

impl SkippedImportPath {
    pub fn new(path: &Path, error: &AppError) -> Self {
        Self {
            path: path.display().to_string(),
            reason: error.to_string(),
        }
    }
}

#[derive(Debug, Default)]
pub struct ImportDiscovery {
    pub paths: Vec<PathBuf>,
    pub companions: HashMap<PathBuf, Vec<SupplementalProcessingAsset>>,
    pub skipped: Vec<(PathBuf, AppError)>,
}

impl ImportDiscovery {
    fn skip(&mut self, path: &Path, error: AppError) {
        log::warn!(
            "Skipping local audio import path '{}': {}",
            sanitize_path_for_display(path),
            error
        );
        self.skipped.push((path.to_path_buf(), error));
    }
}

struct ScopedDirectory {
    root: PathBuf,
    audio: Vec<PathBuf>,
    pdfs: Vec<PathBuf>,
}

struct DiscoveryWalk {
    discovery: ImportDiscovery,
    seen_audio: HashSet<PathBuf>,
    scoped_dirs: Vec<ScopedDirectory>,
    direct_pdfs: Vec<PathBuf>,
}

pub fn discover_audio_import_paths(input_paths: &[PathBuf]) -> ImportDiscovery {
    let mut walk = DiscoveryWalk {
        discovery: ImportDiscovery::default(),
        seen_audio: HashSet::new(),
        scoped_dirs: Vec::new(),
        direct_pdfs: Vec::new(),
    };

    for input_path in input_paths {
        discover_input_path(input_path, &mut walk);
    }

    sort_paths_naturally(&mut walk.discovery.paths);
    attach_discovered_companions(&mut walk.discovery, walk.scoped_dirs, walk.direct_pdfs);
    walk.discovery
}

fn sort_paths_naturally(paths: &mut [PathBuf]) {
    paths.sort_by(|left, right| natural_cmp(&left.to_string_lossy(), &right.to_string_lossy()));
}

fn is_pdf_path(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
}

fn unique_supported_labels() -> Vec<String> {
    let mut labels = Vec::new();
    let mut seen = HashSet::new();

    for format in SUPPORTED_AUDIO_FORMATS {
        if seen.insert(format.label) {
            labels.push(format.label.to_string());
        }
    }

    labels
}

fn join_display_labels(labels: &[String]) -> String {
    match labels {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => {
            if let Some((last, leading)) = labels.split_last() {
                format!("{}, and {last}", leading.join(", "))
            } else {
                String::new()
            }
        }
    }
}

fn discover_input_path(path: &Path, walk: &mut DiscoveryWalk) {
    let file_type = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata.file_type(),
        Err(error) => {
            return walk.discovery.skip(
                path,
                AppError::FileValidation(format!("Cannot read it: {error}")),
            )
        }
    };

    if file_type.is_symlink() {
        return walk.discovery.skip(
            path,
            AppError::InvalidInput(
                "Symlinks are not supported for audio import. Please use the original path."
                    .to_string(),
            ),
        );
    }

    if file_type.is_dir() {
        if let Some(scoped) = discover_directory(path, walk) {
            walk.scoped_dirs.push(scoped);
        }
        return;
    }

    if file_type.is_file() {
        if is_pdf_path(path) {
            collect_pdf_file(path, &mut walk.discovery, &mut walk.direct_pdfs);
        } else {
            collect_supported_file(path, &mut walk.discovery, &mut walk.seen_audio, None);
        }
        return;
    }

    walk.discovery.skip(
        path,
        AppError::FileValidation("It is not a file or folder.".to_string()),
    );
}

fn discover_directory(path: &Path, walk: &mut DiscoveryWalk) -> Option<ScopedDirectory> {
    let root = match validate_input_directory_path(path) {
        Ok(root) => root,
        Err(error) => {
            walk.discovery.skip(path, error);
            return None;
        }
    };
    let mut scoped = ScopedDirectory {
        root: root.clone(),
        audio: Vec::new(),
        pdfs: Vec::new(),
    };
    let mut pending_dirs = vec![root];

    while let Some(directory) = pending_dirs.pop() {
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                walk.discovery.skip(
                    &directory,
                    AppError::FileValidation(format!("Cannot read the folder: {error}")),
                );
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    walk.discovery.skip(&directory, AppError::Io(error));
                    continue;
                }
            };
            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(error) => {
                    walk.discovery.skip(&path, AppError::Io(error));
                    continue;
                }
            };

            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                pending_dirs.push(path);
                continue;
            }

            if file_type.is_file() {
                if is_pdf_path(&path) {
                    collect_pdf_file(&path, &mut walk.discovery, &mut scoped.pdfs);
                } else {
                    collect_supported_file(
                        &path,
                        &mut walk.discovery,
                        &mut walk.seen_audio,
                        Some(&mut scoped.audio),
                    );
                }
            }
        }
    }

    Some(scoped)
}

fn collect_supported_file(
    path: &Path,
    discovery: &mut ImportDiscovery,
    seen: &mut HashSet<PathBuf>,
    scoped_audio: Option<&mut Vec<PathBuf>>,
) {
    if audio_format_for_path(path).is_err() {
        return;
    }

    match validate_input_audio_path(path) {
        Ok(canonical_path) => {
            if let Some(audio) = scoped_audio {
                audio.push(canonical_path.clone());
            }
            if seen.insert(canonical_path.clone()) {
                discovery.paths.push(canonical_path);
            }
        }
        Err(error) => discovery.skip(path, error),
    }
}

fn collect_pdf_file(path: &Path, discovery: &mut ImportDiscovery, pdfs: &mut Vec<PathBuf>) {
    match validate_input_pdf_path(path) {
        Ok(canonical_path) => pdfs.push(canonical_path),
        Err(error) => discovery.skip(path, error),
    }
}

fn attach_discovered_companions(
    discovery: &mut ImportDiscovery,
    scoped_dirs: Vec<ScopedDirectory>,
    mut direct_pdfs: Vec<PathBuf>,
) {
    let mut inspected = HashMap::new();
    for mut scoped in scoped_dirs {
        if scoped.audio.is_empty() || scoped.pdfs.is_empty() {
            continue;
        }
        sort_paths_naturally(&mut scoped.pdfs);
        for pdf in &scoped.pdfs {
            let targets = scoped_pdf_targets(pdf, &scoped.root, &scoped.audio);
            attach_pdf(discovery, &mut inspected, pdf, &targets);
        }
    }
    if !direct_pdfs.is_empty() && !discovery.paths.is_empty() {
        sort_paths_naturally(&mut direct_pdfs);
        let all_audio = discovery.paths.clone();
        for pdf in &direct_pdfs {
            let targets = sibling_audio_targets(pdf, &all_audio);
            attach_pdf(discovery, &mut inspected, pdf, &targets);
        }
    }
}

fn scoped_pdf_targets(pdf: &Path, root: &Path, audio: &[PathBuf]) -> Vec<PathBuf> {
    let Some(pdf_dir) = pdf.parent() else {
        return Vec::new();
    };
    let direct = sibling_audio_targets(pdf, audio);
    if !direct.is_empty() {
        return direct;
    }
    let descendants: Vec<PathBuf> = audio
        .iter()
        .filter(|candidate| candidate.starts_with(pdf_dir))
        .cloned()
        .collect();
    if !descendants.is_empty() {
        return descendants;
    }
    let mut cursor = pdf_dir.parent();
    while let Some(ancestor) = cursor {
        if !ancestor.starts_with(root) {
            break;
        }
        let under_ancestor: Vec<PathBuf> = audio
            .iter()
            .filter(|candidate| candidate.starts_with(ancestor))
            .cloned()
            .collect();
        if !under_ancestor.is_empty() {
            if under_ancestor
                .iter()
                .all(|candidate| candidate.parent() == Some(ancestor))
            {
                return narrow_by_stem(pdf, under_ancestor);
            }
            return Vec::new();
        }
        cursor = ancestor.parent();
    }
    Vec::new()
}

fn sibling_audio_targets(pdf: &Path, audio: &[PathBuf]) -> Vec<PathBuf> {
    let Some(pdf_dir) = pdf.parent() else {
        return Vec::new();
    };
    narrow_by_stem(pdf, audio_in_dir(audio, pdf_dir))
}

fn audio_in_dir(audio: &[PathBuf], dir: &Path) -> Vec<PathBuf> {
    audio
        .iter()
        .filter(|candidate| candidate.parent() == Some(dir))
        .cloned()
        .collect()
}

fn stem_folded(path: &Path) -> Option<String> {
    path.file_stem()?
        .to_str()
        .map(str::to_lowercase)
        .filter(|stem| !stem.is_empty())
}

fn narrow_by_stem(pdf: &Path, candidates: Vec<PathBuf>) -> Vec<PathBuf> {
    let Some(pdf_stem) = stem_folded(pdf) else {
        return candidates;
    };
    let exact: Vec<PathBuf> = candidates
        .iter()
        .filter(|audio| stem_folded(audio).as_deref() == Some(pdf_stem.as_str()))
        .cloned()
        .collect();
    if exact.is_empty() {
        candidates
    } else {
        exact
    }
}

fn attach_pdf(
    discovery: &mut ImportDiscovery,
    inspected: &mut HashMap<PathBuf, Option<SupplementalProcessingAsset>>,
    pdf_path: &Path,
    targets: &[PathBuf],
) {
    if targets.is_empty() {
        return;
    }
    let asset = inspected
        .entry(pdf_path.to_path_buf())
        .or_insert_with(|| match inspect_local_pdf(pdf_path) {
            Ok(asset) => Some(asset),
            Err(error) => {
                discovery.skip(pdf_path, error);
                None
            }
        })
        .clone();
    let Some(asset) = asset else {
        return;
    };
    for audio_path in targets {
        let entry = discovery.companions.entry(audio_path.clone()).or_default();
        if !entry.iter().any(|existing| existing.path == asset.path) {
            entry.push(asset.clone());
        }
    }
}

fn inspect_local_pdf(pdf_path: &Path) -> crate::errors::Result<SupplementalProcessingAsset> {
    let identity = crate::output_artifact::inspect_supplemental_pdf(pdf_path)?;
    let file_name = pdf_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Supplemental PDF.pdf".to_string());
    Ok(SupplementalProcessingAsset {
        path: pdf_path.to_path_buf(),
        file_name,
        size_bytes: identity.size_bytes,
        sha256: identity.sha256,
    })
}

#[cfg(test)]
#[path = "imports_tests.rs"]
mod imports_tests;
