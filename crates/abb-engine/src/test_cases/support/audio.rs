use abb_media_pipeline::audio::{
    AudioIntent, EncoderSettings, OutputAudio, SampleRateConfig, TitleAudioRequest,
};

pub(crate) fn default_m4b(intent: AudioIntent) -> TitleAudioRequest {
    TitleAudioRequest {
        output: OutputAudio::M4b {
            settings: EncoderSettings::default(),
        },
        intent,
        sample_rate: SampleRateConfig::Auto,
    }
}
