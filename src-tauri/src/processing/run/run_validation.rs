use crate::audio;
use crate::errors::Result;
use crate::processing::ProcessPayload;
use std::path::PathBuf;

use crate::audio::FileListInfo;

pub(crate) fn log_encoder_summary(payload: &ProcessPayload) {
    log::info!(
        "encoder summary: encoder={:?} bitrate={}k bitrate_mode={:?} channels={:?} sample_rate={:?} afterburner={}",
        payload.settings.encoder_type,
        payload.settings.bitrate_kbps,
        payload.settings.bitrate_mode,
        payload.settings.channels,
        payload.sample_rate,
        payload.settings.afterburner
    );
}

pub(crate) fn resolve_sample_rate(payload: &ProcessPayload) -> Result<audio::SampleRateConfig> {
    let sample_rate = payload
        .sample_rate
        .clone()
        .unwrap_or(audio::SampleRateConfig::Auto);
    audio::validate_sample_rate_config(&sample_rate)?;
    Ok(sample_rate)
}

pub(crate) fn validate_external_processing_contract(payload: &ProcessPayload) -> Result<()> {
    let input_paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    let file_info = audio::get_file_list_info(&input_paths)?;
    validate_external_processing_contract_with_file_info(payload, &file_info)
}

pub(crate) fn validate_external_processing_contract_with_file_info(
    payload: &ProcessPayload,
    file_info: &FileListInfo,
) -> Result<()> {
    audio::validate_audio_engine_inputs(&payload.settings, file_info)?;
    Ok(())
}
