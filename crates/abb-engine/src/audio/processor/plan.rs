use std::path::PathBuf;

use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::SampleRateConfig;

#[derive(Debug, Clone)]
pub struct MediaProcessingPlan {
    pub output_path: PathBuf,
    pub encoder_settings: EncoderSettings,
    pub sample_rate: SampleRateConfig,
    pub input_file_paths: Vec<PathBuf>,
    pub total_duration: f64,
}

impl MediaProcessingPlan {
    pub fn new(
        output_path: PathBuf,
        encoder_settings: EncoderSettings,
        sample_rate: SampleRateConfig,
        input_file_paths: Vec<PathBuf>,
        total_duration: f64,
    ) -> Self {
        Self {
            output_path,
            encoder_settings,
            sample_rate,
            input_file_paths,
            total_duration,
        }
    }
}
