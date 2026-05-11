use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;

use crate::audio::settings_encoder::EncoderSettings;
use crate::audio::AudioFile;
use crate::audio::SampleRateConfig;
use crate::errors::Result;
use crate::processing::ProcessingContext;

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
}

impl MediaProcessingPlan {
    /// Creates a new media processing plan
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

    /// Helper function to calculate total duration from AudioFile list
    /// Handles Option<f64> duration fields properly
    pub fn calculate_total_duration(files: &[AudioFile]) -> f64 {
        files.iter().filter_map(|f| f.duration).sum()
    }

    /// Executes the processing plan with context-based progress tracking
    pub async fn execute_with_context(
        &self,
        context: &ProcessingContext,
        metadata: Option<&crate::metadata::AudiobookMetadata>,
        passthrough: Option<&crate::metadata::passthrough::PassthroughMetadata>,
    ) -> Result<()> {
        let processor = crate::audio::processor::FfmpegNextProcessor;
        processor
            .execute(self, context, metadata, passthrough)
            .await
    }
}

/// Trait defining a media processor boundary for executing processing plans.
///
/// This provides a stable interface for media processing implementations.
/// The default implementation is the native ffmpeg-next path; command-level
/// encoder selection routes external FDK work through the processor adapter
/// boundary before execution reaches this plan.
pub trait MediaProcessor {
    fn execute<'a>(
        &'a self,
        plan: &'a MediaProcessingPlan,
        context: &'a ProcessingContext,
        metadata: Option<&'a crate::metadata::AudiobookMetadata>,
        passthrough: Option<&'a crate::metadata::passthrough::PassthroughMetadata>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;
}
