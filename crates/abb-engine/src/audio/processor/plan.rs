use std::path::PathBuf;

use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::{AacDecoder, SampleRateConfig};

/// Media processing plan that encapsulates inputs, outputs, and metadata for processing.
#[derive(Debug, Clone)]
pub struct MediaProcessingPlan {
    /// Output file path
    pub output_path: PathBuf,
    /// Encoder settings
    pub encoder_settings: EncoderSettings,
    /// Sample rate configuration (auto or explicit)
    pub sample_rate: SampleRateConfig,
    /// Input file paths for sample rate detection
    pub input_file_paths: Vec<PathBuf>,
    /// Total duration for progress tracking
    pub total_duration: f64,
    /// The decoder for AAC inputs.
    pub aac_decoder: AacDecoder,
}

impl MediaProcessingPlan {
    /// Creates a new media processing plan
    pub fn new(
        output_path: PathBuf,
        encoder_settings: EncoderSettings,
        sample_rate: SampleRateConfig,
        input_file_paths: Vec<PathBuf>,
        total_duration: f64,
        aac_decoder: AacDecoder,
    ) -> Self {
        Self {
            output_path,
            encoder_settings,
            sample_rate,
            input_file_paths,
            total_duration,
            aac_decoder,
        }
    }
}
