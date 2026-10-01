//! Audio owns the decision shared by title previews and processing preflight.
use super::{EncoderSettings, EncoderType, FileListInfo, SampleRateConfig};
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
    Mp3,
    M4aOpus,
    MkaOpus,
}
impl AudiobookFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::M4b => "m4b",
            Self::Mp3 => "mp3",
            Self::M4aOpus => "m4a",
            Self::MkaOpus => "mka",
        }
    }
    pub fn is_opus(self) -> bool {
        matches!(self, Self::M4aOpus | Self::MkaOpus)
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleAudioRequest {
    pub format: AudiobookFormat,
    pub intent: AudioIntent,
    pub settings: Option<EncoderSettings>,
    pub sample_rate: SampleRateConfig,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleAudioPlan {
    pub format: AudiobookFormat,
    pub handling: AudioHandling,
    pub settings: Option<EncoderSettings>,
    pub sample_rate: u32,
    pub channels: u8,
    pub source_codec: String,
    /// Why Auto needs encoding even though the output codec matches.
    pub reason: Option<String>,
}

pub fn resolve_title_audio(
    request: &TitleAudioRequest,
    info: &FileListInfo,
    preview: bool,
) -> Result<TitleAudioPlan> {
    let first = info
        .files
        .first()
        .ok_or_else(|| AppError::InvalidInput("This title has no sources.".into()))?;
    if info.files.iter().any(|file| !file.is_valid) {
        return Err(AppError::InvalidInput(
            "Every title source must be valid.".into(),
        ));
    }
    let source_rate = first
        .sample_rate
        .ok_or_else(|| AppError::InvalidInput("Source sample rate is unknown.".into()))?;
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
    if preview
        && (request.format == AudiobookFormat::Mp3 || request.intent == AudioIntent::Preserve)
    {
        return Err(AppError::InvalidInput(
            "Encoding previews are available with M4B or Opus output.".into(),
        ));
    }
    let cue = info
        .files
        .iter()
        .any(|file| file.chapter_plan.as_ref().is_some_and(|plan| plan.from_cue));
    if cue && info.files.len() > 1 {
        return Err(AppError::InvalidInput("Merging CUE-bearing inputs is not supported. Separate the titles or ignore CUE chapters.".into()));
    }
    let mut reason = cue.then(|| "Encoding applies the accepted CUE chapters.".into());
    if !preview && request.intent != AudioIntent::Encode && !cue {
        reason = copy_rejection(request, info)?;
        if reason.is_none() {
            return Ok(TitleAudioPlan {
                format: request.format,
                handling: AudioHandling::Preserve,
                settings: None,
                sample_rate: source_rate,
                channels: first.channels.unwrap_or(0) as u8,
                source_codec,
                reason: None,
            });
        }
    }
    if request.format == AudiobookFormat::Mp3 || request.intent == AudioIntent::Preserve {
        return Err(AppError::InvalidInput(reason.unwrap_or_else(|| {
            "Choose M4B or Opus to encode this title.".into()
        })));
    }
    encoding_plan(request, info, source_rate, source_codec, reason)
}

fn copy_rejection(request: &TitleAudioRequest, info: &FileListInfo) -> Result<Option<String>> {
    if !sources_match_codec(request.format, info)? {
        return Ok(Some(match request.format {
                AudiobookFormat::Mp3 => "MP3 output needs MP3 source audio. Choose M4B or Opus to encode this title.".into(),
                _ if request.intent == AudioIntent::Preserve => format!("This audio cannot be kept unchanged in {}. Change the output format or choose encoding settings.", request.format.extension().to_uppercase()),
                _ => format!("Encoding is required for the selected {} output.", request.format.extension().to_uppercase()),
            }));
    }
    if request.intent == AudioIntent::Auto
        && request.format != AudiobookFormat::Mp3
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
    info: &FileListInfo,
    source_rate: u32,
    source_codec: String,
    reason: Option<String>,
) -> Result<TitleAudioPlan> {
    use super::processor::resolution::{resolve_linked_encoder, resolve_output_channels};
    let mut settings = request
        .settings
        .clone()
        .ok_or_else(|| AppError::InvalidInput("Choose encoding settings for this title.".into()))?;
    if request.intent == AudioIntent::Auto && request.format == AudiobookFormat::M4b {
        settings = EncoderSettings::default();
    }
    if request.format.is_opus() != (settings.encoder_type == EncoderType::Opus) {
        return Err(AppError::InvalidInput(
            "The encoder must match the selected output format.".into(),
        ));
    }
    let encoder_type = resolve_linked_encoder(&settings)?;
    settings.resolve_encoder(encoder_type);
    settings.channels = resolve_output_channels(settings.channels, &info.files)?;
    let requested_rate =
        if request.intent == AudioIntent::Auto && request.format == AudiobookFormat::M4b {
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
        format: request.format,
        handling: AudioHandling::Encode,
        channels: settings
            .channels
            .forced_channels()
            .expect("resolved channels"),
        settings: Some(settings),
        sample_rate,
        source_codec,
        reason,
    })
}

fn sources_match_codec(format: AudiobookFormat, info: &FileListInfo) -> Result<bool> {
    let expected = match format {
        AudiobookFormat::M4b => ffmpeg_next::codec::Id::AAC,
        AudiobookFormat::Mp3 => ffmpeg_next::codec::Id::MP3,
        _ => ffmpeg_next::codec::Id::OPUS,
    };
    for file in &info.files {
        let path = super::validate_input_audio_path(&file.path)?;
        let input = ffmpeg_next::format::input(&path)?;
        if !input
            .streams()
            .best(ffmpeg_next::media::Type::Audio)
            .is_some_and(|stream| stream.parameters().id() == expected)
        {
            return Ok(false);
        }
    }
    Ok(true)
}
