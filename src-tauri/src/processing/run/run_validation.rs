use crate::audio;
use crate::errors::Result;
use crate::processing::ProcessPayload;
use crate::processing::{AudioHandling, JobType};
use std::path::PathBuf;

use crate::audio::FileListInfo;

pub(crate) fn log_encoder_summary(payload: &ProcessPayload) {
    let Some(settings) = payload.settings.as_ref() else {
        log::info!("audio export: preserving original audio without an encoder");
        return;
    };
    log::info!(
        "encoder summary: encoder={:?} bitrate={}k bitrate_mode={:?} channels={:?} sample_rate={:?} afterburner={}",
        settings.encoder_type, settings.bitrate_kbps, settings.bitrate_mode,
        settings.channels, payload.sample_rate, settings.afterburner,
    );
}

pub(crate) fn resolve_sample_rate(payload: &ProcessPayload) -> Result<audio::SampleRateConfig> {
    if !payload
        .resolved_audio_handling()?
        .contains(&AudioHandling::Encode)
    {
        return Ok(audio::SampleRateConfig::Auto);
    }
    let sample_rate = payload
        .sample_rate
        .clone()
        .unwrap_or(audio::SampleRateConfig::Auto);
    audio::validate_sample_rate_config(&sample_rate)?;
    Ok(sample_rate)
}

pub(super) fn inspect_and_validate_external_processing_contract(
    payload: &ProcessPayload,
) -> Result<FileListInfo> {
    payload.resolved_audio_handling()?;
    let input_paths: Vec<PathBuf> = payload.input_files.iter().map(PathBuf::from).collect();
    for path in &input_paths {
        audio::validate_input_audio_path(path)?;
    }
    let mut file_info = audio::get_file_list_info(&input_paths)?;
    audio::apply_chapter_plans(
        &mut file_info,
        payload.chapter_plans.as_ref(),
        payload.job_type == Some(crate::processing::JobType::Merge)
            && payload.input_files.len() > 1,
    )?;
    validate_external_processing_contract_with_file_info(payload, &file_info)?;
    Ok(file_info)
}

pub(crate) fn validate_external_processing_contract_with_file_info(
    payload: &ProcessPayload,
    file_info: &FileListInfo,
) -> Result<()> {
    let handling = payload.resolved_audio_handling()?;
    if !handling.contains(&AudioHandling::Encode) {
        return Ok(());
    }
    let settings = payload.settings.as_ref().ok_or_else(|| {
        crate::errors::AppError::InvalidInput(
            "Encoder settings are required for encode processing.".to_string(),
        )
    })?;
    let merge = payload.job_type == Some(JobType::Merge);
    let filtered_info;
    let validation_info = if !handling.contains(&AudioHandling::Preserve) {
        file_info
    } else {
        let mut filtered = file_info.clone();
        let modes = &handling;
        filtered.files = file_info
            .files
            .iter()
            .enumerate()
            .filter(|(index, _)| modes.get(*index) != Some(&AudioHandling::Preserve))
            .map(|(_, file)| file.clone())
            .collect();
        filtered.selected_decoders = file_info
            .selected_decoders
            .iter()
            .enumerate()
            .filter(|(index, _)| modes.get(*index) != Some(&AudioHandling::Preserve))
            .map(|(_, decoder)| decoder.clone())
            .collect();
        filtered.valid_count = filtered.files.iter().filter(|file| file.is_valid).count();
        filtered.invalid_count = filtered.files.len().saturating_sub(filtered.valid_count);
        filtered.total_duration = filtered.files.iter().filter_map(|file| file.duration).sum();
        filtered.total_size = filtered.files.iter().filter_map(|file| file.size).sum();
        filtered_info = filtered;
        &filtered_info
    };
    audio::validate_audio_engine_inputs(
        settings,
        validation_info,
        &resolve_sample_rate(payload)?,
        merge,
    )?;
    Ok(())
}
