use serde::{Deserialize, Serialize};

use crate::app_settings::EncoderDefaults;
use crate::audio::{
    AudioIntent, AudiobookFormat, BitrateMode, BitrateModeKind, ChannelConfig,
    EncoderConfigurationCapability, EncoderSettings, EncoderSettingsCapabilities, EncoderType,
    FaacProfile, OutputAudio, SampleRateConfig, TitleAudioRequest,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioChoiceFacts {
    /// The encoder that would run: Opus for Opus formats, and what Auto
    /// resolves to.
    pub effective_encoder: EncoderType,
    pub encoder_options: Vec<EncoderOption>,
    pub encoder_locked: bool,
    pub downmix_warning: bool,
    pub bitrate_mode: BitrateMode,
    pub bitrate_kbps_min: u16,
    pub bitrate_kbps_max: u16,
    pub allowed_modes: Vec<BitrateModeKind>,
    pub faac_profiles: Vec<FaacProfile>,
    pub allowed_sample_rates: Vec<u32>,
    /// False when an explicit sample rate is kept that this encoder does not
    /// accept; the user must choose another before export.
    pub sample_rate_supported: bool,
    /// Total target kbps, or `None` when a quality setting owns the bitrate.
    pub estimate_kbps: Option<u16>,
}

fn is_opus(format: AudiobookFormat) -> bool {
    matches!(format, AudiobookFormat::M4aOpus | AudiobookFormat::MkaOpus)
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
        if let Some(settings) = request.output.encoder_settings() {
            choice.take_settings(settings);
        }
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
            if !caps.explicit_sample_rates.contains(&rate) {
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
        select_encode_on_same: bool,
    ) -> EditResult {
        let result = self.apply(edit, caps);
        let selects = match result {
            EditResult::Changed => true,
            EditResult::Same => select_encode_on_same,
            EditResult::Refused => false,
        };
        if selects && edit.selects_encode() {
            self.intent = AudioIntent::Encode;
        }
        result
    }

    fn apply(&mut self, edit: AudioEdit, caps: Option<&EncoderSettingsCapabilities>) -> EditResult {
        match edit {
            AudioEdit::Format(format) => {
                let same = self.format == format;
                self.format = format;
                if format == AudiobookFormat::Mp3 {
                    self.intent = AudioIntent::Preserve;
                }
                if same {
                    EditResult::Same
                } else {
                    EditResult::Changed
                }
            }
            AudioEdit::Intent(intent) => {
                if self.format == AudiobookFormat::Mp3 && intent == AudioIntent::Encode {
                    return EditResult::Refused;
                }
                set(&mut self.intent, intent)
            }
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
                if is_opus(self.format) {
                    set(&mut self.opus_bitrate_kbps, kbps)
                } else {
                    set(&mut self.aac_bitrate_kbps, kbps)
                }
            }
            AudioEdit::SampleRate(rate) => {
                if let SampleRateConfig::Explicit(hz) = rate {
                    if !self.allowed_sample_rates(caps).contains(&hz) {
                        return EditResult::Refused;
                    }
                }
                let offered = match (rate, caps) {
                    (SampleRateConfig::Auto, Some(caps)) => caps.sample_rate_auto,
                    (SampleRateConfig::Explicit(hz), Some(caps)) => {
                        caps.explicit_sample_rates.contains(&hz)
                    }
                    (SampleRateConfig::Auto, None) => true,
                    (SampleRateConfig::Explicit(_), None) => false,
                };
                set(
                    &mut self.sample_rate,
                    if offered {
                        rate
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
        if is_opus(self.format) {
            return EncoderType::Opus;
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
        !is_opus(self.format) && self.encoder == EncoderType::Faac
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

    fn bitrate_mode(&self, caps: Option<&EncoderSettingsCapabilities>) -> BitrateMode {
        if is_opus(self.format) {
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
        let opus = is_opus(self.format);
        EncoderSettings {
            encoder_type: if opus {
                EncoderType::Opus
            } else {
                self.encoder
            },
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
                if !caps.explicit_sample_rates.contains(&hz) =>
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
        AudioChoiceFacts {
            effective_encoder: self.effective_encoder(caps),
            encoder_options: caps.map_or_else(Vec::new, |caps| {
                caps.encoder_types
                    .iter()
                    .copied()
                    .filter(|encoder| {
                        if is_opus(self.format) {
                            *encoder == EncoderType::Opus
                        } else {
                            !matches!(encoder, EncoderType::Opus | EncoderType::Auto)
                        }
                    })
                    .map(|encoder| EncoderOption {
                        encoder,
                        available: encoder_available(encoder, Some(caps)),
                    })
                    .collect()
            }),
            encoder_locked: is_opus(self.format) || caps.is_none(),
            downmix_warning: false,
            bitrate_mode,
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
            sample_rate_supported: match self.sample_rate {
                SampleRateConfig::Explicit(hz) => {
                    caps.is_none() || allowed_sample_rates.contains(&hz)
                }
                SampleRateConfig::Auto => true,
            },
            allowed_sample_rates,
            estimate_kbps: match bitrate_mode {
                BitrateMode::Vbr(_) => None,
                _ => Some(self.settings(caps).bitrate_kbps),
            },
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
