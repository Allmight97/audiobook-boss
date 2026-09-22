use crate::audio;
use crate::errors::Result;
use crate::processing::ProcessPayload;
use crate::processing::{AudioHandling, JobType};
use std::path::{Path, PathBuf};

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
    payload.validate_title_sources()?;
    payload.resolved_audio_handling()?;
    let input_paths: Vec<PathBuf> = (0..payload.input_files.len())
        .flat_map(|index| payload.sources_for(index))
        .map(|source| PathBuf::from(source.path))
        .collect();
    let input_paths = input_paths
        .iter()
        .map(|path| audio::validate_input_audio_path(path))
        .collect::<Result<Vec<_>>>()?;
    if input_paths
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len()
        != input_paths.len()
    {
        return Err(crate::errors::AppError::InvalidInput(
            "An audio source can belong to only one output title.".into(),
        ));
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
    if payload.job_type == Some(JobType::Merge) {
        return audio::validate_audio_engine_inputs(
            settings,
            std::slice::from_ref(file_info),
            &resolve_sample_rate(payload)?,
        );
    }
    let mut titles = Vec::new();
    for (index, mode) in handling.iter().enumerate() {
        if *mode == AudioHandling::Preserve {
            continue;
        }
        let sources = payload.sources_for(index);
        let paths = sources
            .iter()
            .map(|source| audio::validate_input_audio_path(Path::new(&source.path)))
            .collect::<Result<Vec<_>>>()?;
        titles.push(super::super::plan::title_file_info(file_info, &paths)?);
    }
    audio::validate_audio_engine_inputs(settings, &titles, &resolve_sample_rate(payload)?)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn batch_validation_probes_external_toolchain_once_for_all_titles() {
        let temp = tempfile::tempdir().expect("create isolated toolchain directory");
        let script = temp.path().join("ffmpeg");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$(dirname "$0")/calls"
case "$*" in
  *-version*) echo 'ffmpeg version fixture' ;;
  *-encoders*) echo ' A..... libfdk_aac AAC encoder' ;;
  *-decoders*) echo ' A..... libfdk_aac AAC decoder' ;;
esac
"#,
        )
        .expect("write FFmpeg probe fixture");
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
            .expect("make probe fixture executable");
        struct ResetToolchain;
        impl Drop for ResetToolchain {
            fn drop(&mut self) {
                audio::set_user_external_ffmpeg_path(None);
            }
        }
        let _reset = ResetToolchain;
        audio::set_user_external_ffmpeg_path(Some(script));
        let files: Vec<_> = ["one.wav", "two.wav"]
            .into_iter()
            .map(|name| {
                let path = temp.path().join(name);
                std::fs::write(&path, b"path-validation fixture")
                    .expect("write source path fixture");
                let mut file =
                    audio::AudioFile::new(path.canonicalize().expect("canonicalize source path"));
                file.is_valid = true;
                file.channels = Some(1);
                file.sample_rate = Some(44_100);
                file
            })
            .collect();
        let info = FileListInfo {
            files,
            selected_decoders: vec![None, None],
            total_duration: 0.0,
            total_size: 0.0,
            valid_count: 2,
            invalid_count: 0,
        };
        for encoder in ["auto", "fdk_he_aac"] {
            let payload: ProcessPayload = serde_json::from_value(serde_json::json!({
                "inputFiles": info.files.iter().map(|file| &file.path).collect::<Vec<_>>(),
                "outputDir": temp.path(), "jobType": "batch",
                "settings": { "encoderType": encoder, "bitrateKbps": 64,
                    "bitrateMode": { "mode": "vbr", "value": 3 }, "channels": "mono", "afterburner": false,
                    "nativeAacSpeed": 0, "faacProfile": "auto" }
            }))
            .expect("decode processing fixture");
            std::fs::write(temp.path().join("calls"), "").expect("reset probe log");
            validate_external_processing_contract_with_file_info(&payload, &info)
                .expect("validate title batch");
            let calls = std::fs::read_to_string(temp.path().join("calls")).expect("read probe log");
            assert_eq!(
                calls
                    .lines()
                    .filter(|line| line.contains("-version"))
                    .count(),
                1
            );
            assert_eq!(
                calls
                    .lines()
                    .filter(|line| line.contains("-encoders"))
                    .count(),
                1
            );
            assert_eq!(
                calls
                    .lines()
                    .filter(|line| line.contains("-decoders"))
                    .count(),
                1
            );
        }
    }
}
