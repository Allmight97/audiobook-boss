//! Where exports are written and how they are named, and the path a title
//! would get.

use std::path::Path;

use serde::Serialize;

use crate::app_settings::OutputDefaults;
use crate::audio::AudiobookFormat;
use crate::metadata::{AudiobookMetadata, NamingMetadata};
use crate::output_artifact::{
    build_output_path_preview, derive_output_artifact_path, NamingPreset, OutputKind,
    OutputNamingConfig,
};

/// A custom naming template left empty names files this way.
pub const DEFAULT_CUSTOM_TEMPLATE: &str = "{author}/{title}";

/// The path the selected title would be written to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OutputPreview {
    NoDirectory,
    /// No title to name yet.
    NoTitle,
    Path {
        path: String,
    },
    /// The metadata or template cannot name a file; `message` says why.
    Unavailable {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OutputSnapshot {
    pub revision: u64,
    pub directory: Option<String>,
    pub preset: NamingPreset,
    pub include_year: bool,
    /// The custom template exactly as typed.
    pub template: String,
    /// The naming processing receives: an empty custom template names files
    /// `{author}/{title}`.
    pub naming: OutputNamingConfig,
    pub preview: OutputPreview,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct OutputPlan {
    directory: Option<String>,
    preset: Option<NamingPreset>,
    include_year: bool,
    template: String,
}

impl OutputPlan {
    pub(crate) fn from_defaults(defaults: &OutputDefaults) -> Self {
        Self {
            directory: defaults
                .output_directory
                .clone()
                .filter(|directory| !directory.trim().is_empty()),
            preset: Some(defaults.output_naming.preset),
            include_year: defaults.output_naming.include_year,
            template: defaults
                .output_naming
                .custom_template
                .clone()
                .unwrap_or_default(),
        }
    }

    pub(crate) fn set_directory(&mut self, directory: String) {
        self.directory = Some(directory).filter(|directory| !directory.trim().is_empty());
    }

    pub(crate) fn set_preset(&mut self, preset: NamingPreset) {
        self.preset = Some(preset);
    }

    pub(crate) fn set_include_year(&mut self, include_year: bool) {
        self.include_year = include_year;
    }

    pub(crate) fn set_template(&mut self, template: String) {
        self.template = template;
    }

    fn preset(&self) -> NamingPreset {
        self.preset.unwrap_or(NamingPreset::AbsDefault)
    }

    /// The naming processing receives.
    pub(crate) fn naming(&self) -> OutputNamingConfig {
        let custom = self.preset() == NamingPreset::CustomTemplate;
        OutputNamingConfig {
            preset: self.preset(),
            include_year: self.include_year,
            custom_template: custom.then(|| {
                if self.template.trim().is_empty() {
                    DEFAULT_CUSTOM_TEMPLATE.to_string()
                } else {
                    self.template.clone()
                }
            }),
        }
    }

    /// The choices as durable defaults.
    pub(crate) fn defaults(&self) -> OutputDefaults {
        OutputDefaults {
            output_directory: self.directory.clone(),
            output_naming: OutputNamingConfig {
                custom_template: Some(self.template.clone())
                    .filter(|template| !template.is_empty()),
                ..self.naming()
            },
        }
    }

    /// The path a title with `metadata` would be written to.
    pub(crate) fn preview(
        &self,
        title: Option<(&AudiobookMetadata, &Path, AudiobookFormat)>,
    ) -> OutputPreview {
        let Some(directory) = &self.directory else {
            return OutputPreview::NoDirectory;
        };
        let Some((metadata, source, format)) = title else {
            return OutputPreview::NoTitle;
        };
        let naming = NamingMetadata::from_metadata(metadata);
        let path = build_output_path_preview(
            Path::new(directory),
            Some(&naming),
            self.naming(),
            Some(source),
        )
        .and_then(|requested| derive_output_artifact_path(&requested, OutputKind::Final));
        match path {
            Ok(path) => OutputPreview::Path {
                path: path
                    .with_extension(format.extension())
                    .to_string_lossy()
                    .into_owned(),
            },
            Err(error) => OutputPreview::Unavailable {
                message: error.to_string(),
            },
        }
    }

    pub(crate) fn snapshot(&self, revision: u64, preview: OutputPreview) -> OutputSnapshot {
        OutputSnapshot {
            revision,
            directory: self.directory.clone(),
            preset: self.preset(),
            include_year: self.include_year,
            template: self.template.clone(),
            naming: self.naming(),
            preview,
        }
    }
}
