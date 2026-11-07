use super::{AudioFile, EncoderSettings, FileListInfo, SampleRateConfig};
use crate::{
    errors::{AppError, Result},
    processing::AudioHandling,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AudiobookFormat {
    #[default]
    M4b,
    M4aOpus,
    MkaOpus,
}
impl AudiobookFormat {
    pub fn extension(self) -> &'static str {
        self.spec().extension
    }
    pub fn is_opus(self) -> bool {
        self.spec().codec_family == super::CodecFamily::Opus
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AudioIntent {
    #[default]
    Auto,
    Preserve,
    Encode,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "format", rename_all = "camelCase")]
pub enum OutputAudio {
    M4b { settings: EncoderSettings },
    M4aOpus { settings: EncoderSettings },
    MkaOpus { settings: EncoderSettings },
}
impl OutputAudio {
    pub fn new(format: AudiobookFormat, settings: EncoderSettings) -> Self {
        match format {
            AudiobookFormat::M4b => Self::M4b { settings },
            AudiobookFormat::M4aOpus => Self::M4aOpus { settings },
            AudiobookFormat::MkaOpus => Self::MkaOpus { settings },
        }
    }
    pub fn format(&self) -> AudiobookFormat {
        match self {
            Self::M4b { .. } => AudiobookFormat::M4b,
            Self::M4aOpus { .. } => AudiobookFormat::M4aOpus,
            Self::MkaOpus { .. } => AudiobookFormat::MkaOpus,
        }
    }
    pub fn encoder_settings(&self) -> &EncoderSettings {
        match self {
            Self::M4b { settings } | Self::M4aOpus { settings } | Self::MkaOpus { settings } => {
                settings
            }
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleAudioRequest {
    #[serde(flatten)]
    pub output: OutputAudio,
    pub intent: AudioIntent,
    pub sample_rate: SampleRateConfig,
}
impl TitleAudioRequest {
    pub fn format(&self) -> AudiobookFormat {
        self.output.format()
    }
}
#[cfg(test)]
impl TitleAudioRequest {
    pub(crate) fn default_m4b(intent: AudioIntent) -> Self {
        Self {
            output: OutputAudio::M4b {
                settings: EncoderSettings::default(),
            },
            intent,
            sample_rate: SampleRateConfig::Auto,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleAudioPlan {
    pub format: AudiobookFormat,
    #[serde(flatten)]
    pub handling: AudioHandling,
    pub sample_rate: u32,
    pub channels: u8,
    pub source_codec: String,
    /// Why Auto needs encoding even though the output codec matches.
    pub reason: Option<String>,
}

/// The audio setting a title's plan failed on, when one setting is the cause.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AudioPlanField {
    SampleRate,
    Channels,
}

/// Why a title's audio cannot be planned, and the setting to change when one
/// setting is the cause.
#[derive(Debug)]
pub struct TitleAudioError {
    pub error: AppError,
    pub field: Option<AudioPlanField>,
}

impl TitleAudioError {
    fn on(field: AudioPlanField) -> impl FnOnce(AppError) -> Self {
        move |error| Self {
            error,
            field: Some(field),
        }
    }

    /// The reason in the user's terms.
    pub fn message(&self) -> String {
        match &self.error {
            AppError::InvalidInput(message) => message.clone(),
            error => error.to_string(),
        }
    }
}

impl std::fmt::Display for TitleAudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}

impl From<AppError> for TitleAudioError {
    fn from(error: AppError) -> Self {
        Self { error, field: None }
    }
}

impl From<TitleAudioError> for AppError {
    fn from(error: TitleAudioError) -> Self {
        error.error
    }
}

pub fn resolve_title_audio(
    request: &TitleAudioRequest,
    info: &FileListInfo,
    preview: bool,
) -> std::result::Result<TitleAudioPlan, TitleAudioError> {
    let first = info
        .files
        .first()
        .ok_or_else(|| AppError::InvalidInput("This title has no sources.".into()))?;
    if info.files.iter().any(|file| !file.is_valid) {
        return Err(AppError::InvalidInput("Every title source must be valid.".into()).into());
    }
    let source_rate = first
        .sample_rate
        .ok_or_else(|| AppError::InvalidInput("Source sample rate is unknown.".into()))
        .map_err(TitleAudioError::on(AudioPlanField::SampleRate))?;
    let source_codec = if info
        .files
        .iter()
        .all(|file| file.codec_label == first.codec_label)
    {
        first
            .codec_label
            .clone()
            .unwrap_or_else(|| "source audio".into())
    } else {
        "mixed audio".into()
    };
    let format = request.format();
    if preview && request.intent == AudioIntent::Preserve {
        return Err(AppError::InvalidInput(
            "Previews need encoded audio. Choose encoding settings to preview this title.".into(),
        )
        .into());
    }
    let cue = info
        .files
        .iter()
        .any(|file| file.chapter_plan.as_ref().is_some_and(|plan| plan.from_cue));
    if cue && info.files.len() > 1 {
        return Err(AppError::InvalidInput("Merging CUE-bearing inputs is not supported. Separate the titles or ignore CUE chapters.".into()).into());
    }
    let mut reason = cue.then(|| "Encoding applies the accepted CUE chapters.".into());
    if !preview && request.intent != AudioIntent::Encode && !cue {
        reason = copy_rejection(request, info)?;
        if reason.is_none() {
            return Ok(TitleAudioPlan {
                format,
                handling: AudioHandling::Preserve,
                sample_rate: source_rate,
                channels: first.channels.unwrap_or(0) as u8,
                source_codec,
                reason: None,
            });
        }
    }
    if request.intent == AudioIntent::Preserve {
        return Err(
            AppError::InvalidInput(reason.expect("a rejected copy always has a reason")).into(),
        );
    }
    encoding_plan(
        request,
        request.output.encoder_settings(),
        info,
        source_rate,
        source_codec,
        reason,
    )
}

fn copy_rejection(request: &TitleAudioRequest, info: &FileListInfo) -> Result<Option<String>> {
    let format = request.format();
    let keeping_formats = formats_that_keep(&info.files);
    if !keeping_formats.contains(&format) {
        let extension = format.extension().to_uppercase();
        return Ok(Some(if request.intent != AudioIntent::Preserve {
            format!("Encoding is required for the selected {extension} output.")
        } else if keeping_formats.is_empty() {
            "This audio cannot be kept unchanged in any output format. Choose encoding settings."
                .to_string()
        } else {
            format!("This audio cannot be kept unchanged in {extension}. Change the output format or choose encoding settings.")
        }));
    }
    if request.intent == AudioIntent::Auto
        && !info.files.iter().all(|file| {
            file.bitrate.is_some_and(|rate| {
                (1..=super::constants::COMPACT_AUDIO_MAX_BITRATE).contains(&rate)
            })
        })
    {
        return Ok(Some(
            "Default will encode to reduce size. Choose Keep original audio to leave it unchanged."
                .into(),
        ));
    }
    match super::validate_preserved_title(&info.files) {
        Ok(()) => Ok(None),
        Err(AppError::InvalidInput(detail)) => Ok(Some(format!(
            "{detail} Choose encoding settings or keep the sources as separate titles."
        ))),
        Err(error) => Err(error),
    }
}

fn encoding_plan(
    request: &TitleAudioRequest,
    requested: &EncoderSettings,
    info: &FileListInfo,
    source_rate: u32,
    source_codec: String,
    reason: Option<String>,
) -> std::result::Result<TitleAudioPlan, TitleAudioError> {
    use super::processor::resolution::{resolve_linked_encoder, resolve_output_channels};
    let format = request.format();
    let default_m4b = request.intent == AudioIntent::Auto && format == AudiobookFormat::M4b;
    let mut settings = if default_m4b {
        EncoderSettings::default()
    } else {
        requested.clone()
    };
    if !format.spec().encoders.contains(&settings.encoder_type) {
        return Err(AppError::InvalidInput(
            "The encoder must match the selected output format.".into(),
        )
        .into());
    }
    let encoder_type = resolve_linked_encoder(&settings)?;
    settings.resolve_encoder(encoder_type);
    settings.channels = resolve_output_channels(settings.channels, &info.files)
        .map_err(TitleAudioError::on(AudioPlanField::Channels))?;
    let requested_rate = if default_m4b {
        &SampleRateConfig::Auto
    } else {
        &request.sample_rate
    };
    let sample_rate = match requested_rate {
        SampleRateConfig::Explicit(rate) => *rate,
        SampleRateConfig::Auto => super::settings::automatic_sample_rate(
            settings.encoder_type,
            settings.faac_profile,
            source_rate,
        ),
    };
    super::processor::validate_resolved_audio_inputs(
        encoder_type,
        &settings,
        std::slice::from_ref(info),
        &SampleRateConfig::Explicit(sample_rate),
    )?;
    Ok(TitleAudioPlan {
        format,
        channels: settings
            .channels
            .forced_channels()
            .expect("resolved channels"),
        handling: AudioHandling::Encode { settings },
        sample_rate,
        source_codec,
        reason,
    })
}

fn formats_that_keep(files: &[AudioFile]) -> Vec<AudiobookFormat> {
    super::FORMATS
        .into_iter()
        .filter(|spec| {
            files.iter().all(|file| {
                file.preservation
                    .is_some_and(|source| source.codec_family == Some(spec.codec_family))
            })
        })
        .map(|spec| spec.format)
        .collect()
}
