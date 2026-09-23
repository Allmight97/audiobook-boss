use crate::audio;
use crate::errors::Result;
use crate::processing::ProcessPayload;
use std::path::PathBuf;

use crate::audio::FileListInfo;

pub(super) fn inspect_and_validate_external_processing_contract(
    payload: &ProcessPayload,
) -> Result<FileListInfo> {
    payload.validate_title_sources()?;
    payload.validate_audio_requests()?;
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
    Ok(file_info)
}
