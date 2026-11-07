use serde::{Deserialize, Serialize};

use crate::app_settings::EncoderDefaults;
use crate::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, BitrateModeKind, ChannelConfig, CodecFamily,
    EncoderConfigurationCapability, EncoderSettings, EncoderSettingsCapabilities, EncoderType,
    FaacProfile, OutputAudio, SampleRateConfig, TitleAudioRequest, FORMATS,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum FaacRateControl {
    #[default]
    Abr,
    Vbr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "field", content = "value", rename_all = "camelCase")]
pub enum AudioEdit {
    Format(AudiobookFormat),
    Intent(AudioIntent),
    /// The AAC encoder; Opus formats always use Opus.
    Encoder(EncoderType),
    FaacProfile(FaacProfile),
    RateControl(FaacRateControl),
    Quality(u16),
    NativeSpeed(u8),
    /// Target kbps for the format's encoder, across all channels.
    Bitrate(u16),
    SampleRate(SampleRateConfig),
    Channels(ChannelConfig),
}

impl AudioEdit {
    fn selects_encode(self) -> bool {
        !matches!(self, Self::Format(_) | Self::Intent(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditResult {
    Changed,
    Same,
    Refused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditScope {
    Defaults,
    Title,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RateOffer {
    by_any_encoder: bool,
    by_this_encoder: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioChoice {
    pub format: AudiobookFormat,
    pub intent: AudioIntent,
    /// The AAC encoder chosen; never Opus.
    pub encoder: EncoderType,
    pub aac_bitrate_kbps: u16,
    pub opus_bitrate_kbps: u16,
    /// The bitrate mode the saved settings carried, used until capabilities
    /// name the encoder's default.
    #[serde(skip)]
    pub saved_mode: BitrateMode,
    pub faac_profile: FaacProfile,
    pub faac_rate_control: FaacRateControl,
    pub faac_quality: u16,
    pub native_speed: u8,
    pub channels: ChannelConfig,
    pub sample_rate: SampleRateConfig,
}

impl Default for AudioChoice {
    fn default() -> Self {
        let settings = EncoderSettings::default();
        Self {
            format: AudiobookFormat::M4b,
            intent: AudioIntent::Auto,
            encoder: settings.encoder_type,
            aac_bitrate_kbps: settings.bitrate_kbps,
            opus_bitrate_kbps: 64,
            saved_mode: settings.bitrate_mode,
            faac_profile: FaacProfile::Auto,
            faac_rate_control: FaacRateControl::Abr,
            faac_quality: crate::audio::DEFAULT_FAAC_QUALITY,
            native_speed: 0,
            channels: ChannelConfig::Auto,
            sample_rate: SampleRateConfig::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EncoderOption {
    pub encoder: EncoderType,
    pub available: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatOption {
    pub format: AudiobookFormat,
    pub codec_family: CodecFamily,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioChoiceFacts {
    pub format_options: Vec<FormatOption>,
    /// The codec family of the chosen format.
    pub codec_family: CodecFamily,
    pub intent_options: Vec<AudioIntent>,
    pub channel_options: Vec<ChannelConfig>,
    /// The encoder that would run: Opus for Opus formats, and what Auto
    /// resolves to.
    pub effective_encoder: EncoderType,
    pub encoder_options: Vec<EncoderOption>,
    pub encoder_locked: bool,
    pub downmix_warning: bool,
    pub bitrate_mode: BitrateMode,
    /// The target kbps the request carries: the Opus bitrate for an Opus
    /// format, else the AAC one.
    pub bitrate_kbps: u16,
    pub bitrate_kbps_min: u16,
    pub bitrate_kbps_max: u16,
    pub allowed_modes: Vec<BitrateModeKind>,
    pub faac_profiles: Vec<FaacProfile>,
    /// Every speed the native AAC encoder accepts, slowest search first.
    pub native_speed_options: Vec<u8>,
    pub allowed_sample_rates: Vec<u32>,
    /// False when an explicit sample rate is kept that this encoder does not
    /// accept; the user must choose another before export.
    pub sample_rate_supported: bool,
    /// Total target kbps, or `None` when a quality setting owns the bitrate.
    #[serde(skip)]
    pub estimate_kbps: Option<u16>,
}

impl AudioChoice {
    pub(crate) fn from_defaults(
        defaults: &EncoderDefaults,
        caps: Option<&EncoderSettingsCapabilities>,
    ) -> Self {
        let mut choice = Self::default();
        choice.take_settings(&defaults.settings);
        choice.format = defaults.format;
        choice.intent = defaults.intent;
        choice.sample_rate = defaults.sample_rate;
        choice.fit(caps);
        choice
    }

    pub(crate) fn for_title(&self, request: &TitleAudioRequest) -> Self {
        let mut choice = self.clone();
        choice.take_settings(request.output.encoder_settings());
        choice.format = request.format();
        choice.intent = request.intent;
        choice.sample_rate = request.sample_rate;
        choice
    }

    fn take_settings(&mut self, settings: &EncoderSettings) {
        if settings.encoder_type == EncoderType::Opus {
            self.opus_bitrate_kbps = settings.bitrate_kbps;
        } else {
            self.encoder = settings.encoder_type;
        }
        self.saved_mode = settings.bitrate_mode;
        self.faac_profile = settings.faac_profile;
        if settings.encoder_type == EncoderType::Faac {
            match settings.bitrate_mode {
                BitrateMode::Vbr(quality) => {
                    self.faac_rate_control = FaacRateControl::Vbr;
                    self.faac_quality = quality;
                }
                _ => self.faac_rate_control = FaacRateControl::Abr,
            }
        }
        self.native_speed = settings.native_aac_speed;
        self.aac_bitrate_kbps = settings.bitrate_kbps;
        self.channels = settings.channels;
    }

    pub(crate) fn fit(&mut self, caps: Option<&EncoderSettingsCapabilities>) {
        let Some(caps) = caps else {
            return;
        };
        let aac = Self {
            format: AudiobookFormat::M4b,
            ..self.clone()
        };
        if let Some(config) = aac.configuration(Some(caps)) {
            self.aac_bitrate_kbps = self
                .aac_bitrate_kbps
                .clamp(config.bitrate_kbps_min, config.bitrate_kbps_max);
        }
        let opus = Self {
            format: AudiobookFormat::M4aOpus,
            ..self.clone()
        };
        if let Some(config) = opus.configuration(Some(caps)) {
            self.opus_bitrate_kbps = self
                .opus_bitrate_kbps
                .clamp(config.bitrate_kbps_min, config.bitrate_kbps_max);
        }
        if !caps.faac_quality_presets.contains(&self.faac_quality) {
            self.faac_quality = caps.faac_quality_default;
        }
        self.native_speed = self.native_speed.min(caps.native_speed_max);
        if let SampleRateConfig::Explicit(rate) = self.sample_rate {
            if !self.rate_offer(rate, caps).by_any_encoder {
                self.sample_rate = if caps.sample_rate_auto {
                    SampleRateConfig::Auto
                } else {
                    caps.explicit_sample_rates
                        .first()
                        .map_or(SampleRateConfig::Auto, |rate| {
                            SampleRateConfig::Explicit(*rate)
                        })
                };
            }
        } else if !caps.sample_rate_auto {
            if let Some(rate) = caps.explicit_sample_rates.first() {
                self.sample_rate = SampleRateConfig::Explicit(*rate);
            }
        }
        if !caps.channel_options.contains(&self.channels) {
            if let Some(first) = caps.channel_options.first() {
                self.channels = *first;
            }
        }
    }

    pub(crate) fn edit(
        &mut self,
        edit: AudioEdit,
        caps: Option<&EncoderSettingsCapabilities>,
        scope: EditScope,
    ) -> EditResult {
        let result = self.apply(edit, caps);
        let selects = match result {
            EditResult::Changed => true,
            EditResult::Same => scope == EditScope::Title,
            EditResult::Refused => false,
        };
        if selects && edit.selects_encode() {
            self.intent = AudioIntent::Encode;
        }
        result
    }

    fn apply(&mut self, edit: AudioEdit, caps: Option<&EncoderSettingsCapabilities>) -> EditResult {
        match edit {
            AudioEdit::Format(format) => set(&mut self.format, format),
            AudioEdit::Intent(intent) => set(&mut self.intent, intent),
            AudioEdit::Encoder(encoder) => {
                let available = encoder_available(encoder, caps);
                if encoder == EncoderType::Opus || !available {
                    return EditResult::Refused;
                }
                set(&mut self.encoder, encoder)
            }
            AudioEdit::FaacProfile(profile) => set(&mut self.faac_profile, profile),
            AudioEdit::RateControl(mode) => {
                if self.encoder != EncoderType::Faac {
                    return EditResult::Refused;
                }
                set(&mut self.faac_rate_control, mode)
            }
            AudioEdit::Quality(quality) => {
                let preset = caps.is_some_and(|caps| caps.faac_quality_presets.contains(&quality));
                if self.encoder != EncoderType::Faac || !preset {
                    return EditResult::Refused;
                }
                set(&mut self.faac_quality, quality)
            }
            AudioEdit::NativeSpeed(speed) => match caps {
                Some(caps) if speed <= caps.native_speed_max => set(&mut self.native_speed, speed),
                _ => EditResult::Refused,
            },
            AudioEdit::Bitrate(kbps) => {
                let Some(config) = self.configuration(caps) else {
                    return EditResult::Refused;
                };
                if !(config.bitrate_kbps_min..=config.bitrate_kbps_max).contains(&kbps) {
                    return EditResult::Refused;
                }
                if self.format.is_opus() {
                    set(&mut self.opus_bitrate_kbps, kbps)
                } else {
                    set(&mut self.aac_bitrate_kbps, kbps)
                }
            }
            AudioEdit::SampleRate(SampleRateConfig::Auto) => {
                set(&mut self.sample_rate, SampleRateConfig::Auto)
            }
            AudioEdit::SampleRate(SampleRateConfig::Explicit(hz)) => {
                let Some(caps) = caps else {
                    return EditResult::Refused;
                };
                let offer = self.rate_offer(hz, caps);
                if !offer.by_this_encoder {
                    return EditResult::Refused;
                }
                set(
                    &mut self.sample_rate,
                    if offer.by_any_encoder {
                        SampleRateConfig::Explicit(hz)
                    } else {
                        SampleRateConfig::Auto
                    },
                )
            }
            AudioEdit::Channels(channels) => {
                if caps.is_some_and(|caps| !caps.channel_options.contains(&channels)) {
                    return EditResult::Refused;
                }
                set(&mut self.channels, channels)
            }
        }
    }

    pub(crate) fn effective_encoder(
        &self,
        caps: Option<&EncoderSettingsCapabilities>,
    ) -> EncoderType {
        if let Some(fixed) = self.format.spec().fixed_encoder() {
            return fixed;
        }
        match (self.encoder, caps) {
            (EncoderType::Auto, Some(caps)) => caps.availability.auto_encoder,
            (encoder, _) => encoder,
        }
    }

    fn configuration<'a>(
        &self,
        caps: Option<&'a EncoderSettingsCapabilities>,
    ) -> Option<&'a EncoderConfigurationCapability> {
        let encoder = self.effective_encoder(caps);
        caps?
            .encoder_configurations
            .iter()
            .find(|config| config.encoder_type == encoder)
    }

    fn uses_faac(&self) -> bool {
        !self.format.is_opus() && self.encoder == EncoderType::Faac
    }

    fn allowed_sample_rates(&self, caps: Option<&EncoderSettingsCapabilities>) -> Vec<u32> {
        let Some(config) = self.configuration(caps) else {
            return Vec::new();
        };
        if self.uses_faac() {
            return config
                .faac_profiles
                .iter()
                .find(|entry| entry.profile == self.faac_profile)
                .map(|entry| entry.explicit_sample_rates.clone())
                .unwrap_or_default();
        }
        config.explicit_sample_rates.clone()
    }

    fn rate_offer(&self, hz: u32, caps: &EncoderSettingsCapabilities) -> RateOffer {
        RateOffer {
            by_any_encoder: caps.explicit_sample_rates.contains(&hz),
            by_this_encoder: self.allowed_sample_rates(Some(caps)).contains(&hz),
        }
    }

    fn bitrate_mode(&self, caps: Option<&EncoderSettingsCapabilities>) -> BitrateMode {
        if self.format.is_opus() {
            return BitrateMode::VbrTarget;
        }
        if self.encoder == EncoderType::Faac {
            return match self.faac_rate_control {
                FaacRateControl::Vbr => BitrateMode::Vbr(self.faac_quality),
                FaacRateControl::Abr => BitrateMode::Abr,
            };
        }
        self.configuration(caps)
            .map_or(self.saved_mode, |config| config.default_mode)
    }

    fn settings(&self, caps: Option<&EncoderSettingsCapabilities>) -> EncoderSettings {
        let opus = self.format.is_opus();
        EncoderSettings {
            encoder_type: self.format.spec().fixed_encoder().unwrap_or(self.encoder),
            bitrate_kbps: if opus {
                self.opus_bitrate_kbps
            } else {
                self.aac_bitrate_kbps
            },
            bitrate_mode: self.bitrate_mode(caps),
            channels: self.channels,
            native_aac_speed: self.native_speed,
            faac_profile: self.faac_profile,
        }
    }

    /// The sample rate a request carries: an explicit rate no encoder offers
    /// becomes Auto.
    fn request_sample_rate(&self, caps: Option<&EncoderSettingsCapabilities>) -> SampleRateConfig {
        match (self.sample_rate, caps) {
            (SampleRateConfig::Explicit(hz), Some(caps))
                if !self.rate_offer(hz, caps).by_any_encoder =>
            {
                SampleRateConfig::Auto
            }
            (rate, _) => rate,
        }
    }

    pub(crate) fn request(&self, caps: Option<&EncoderSettingsCapabilities>) -> TitleAudioRequest {
        TitleAudioRequest {
            output: OutputAudio::new(self.format, self.settings(caps)),
            intent: self.intent,
            sample_rate: self.request_sample_rate(caps),
        }
    }

    pub(crate) fn defaults(&self, caps: Option<&EncoderSettingsCapabilities>) -> EncoderDefaults {
        EncoderDefaults {
            format: self.format,
            intent: self.intent,
            settings: self.settings(caps),
            sample_rate: self.request_sample_rate(caps),
        }
    }

    pub(crate) fn facts(&self, caps: Option<&EncoderSettingsCapabilities>) -> AudioChoiceFacts {
        let config = self.configuration(caps);
        let allowed_sample_rates = self.allowed_sample_rates(caps);
        let bitrate_mode = self.bitrate_mode(caps);
        let settings = self.settings(caps);
        AudioChoiceFacts {
            format_options: FORMATS
                .into_iter()
                .map(|spec| FormatOption {
                    format: spec.format,
                    codec_family: spec.codec_family,
                })
                .collect(),
            codec_family: self.format.spec().codec_family,
            intent_options: AudioIntent::ALL.to_vec(),
            channel_options: caps.map_or_else(Vec::new, |caps| caps.channel_options.clone()),
            effective_encoder: self.effective_encoder(caps),
            encoder_options: caps.map_or_else(Vec::new, |caps| {
                caps.encoder_types
                    .iter()
                    .copied()
                    .filter(|encoder| {
                        *encoder != EncoderType::Auto
                            && self.format.spec().encoders.contains(encoder)
                    })
                    .map(|encoder| EncoderOption {
                        encoder,
                        available: encoder_available(encoder, Some(caps)),
                    })
                    .collect()
            }),
            encoder_locked: self.format.spec().fixed_encoder().is_some() || caps.is_none(),
            downmix_warning: false,
            bitrate_mode,
            bitrate_kbps: settings.bitrate_kbps,
            bitrate_kbps_min: config.map_or(1, |config| config.bitrate_kbps_min),
            bitrate_kbps_max: config.map_or(0, |config| config.bitrate_kbps_max),
            allowed_modes: config.map_or_else(Vec::new, |config| config.allowed_modes.clone()),
            faac_profiles: config.map_or_else(Vec::new, |config| {
                config
                    .faac_profiles
                    .iter()
                    .map(|entry| entry.profile)
                    .collect()
            }),
            sample_rate_supported: match (self.sample_rate, caps) {
                (SampleRateConfig::Explicit(hz), Some(caps)) => {
                    self.rate_offer(hz, caps).by_this_encoder
                }
                _ => true,
            },
            native_speed_options: caps
                .map_or_else(Vec::new, |caps| (0..=caps.native_speed_max).collect()),
            allowed_sample_rates,
            estimate_kbps: bitrate_mode.fixed_kbps(settings.bitrate_kbps),
        }
    }
}

fn encoder_available(encoder: EncoderType, caps: Option<&EncoderSettingsCapabilities>) -> bool {
    caps.is_none_or(|caps| match encoder {
        EncoderType::AacAt => caps.availability.aac_at_available,
        EncoderType::NativeAac => caps.availability.native_aac_available,
        _ => true,
    })
}

fn set<T: PartialEq>(slot: &mut T, value: T) -> EditResult {
    if *slot == value {
        return EditResult::Same;
    }
    *slot = value;
    EditResult::Changed
}

#[cfg(test)]
#[path = "audio_choice_tests.rs"]
mod tests;
