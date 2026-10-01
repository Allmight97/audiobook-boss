use abb_remote_source_core::{
    acquisition_progress, acquisition_progress_for_current_title, AcquisitionProgress,
    AcquisitionStage,
};

#[derive(Clone, Copy)]
pub(in crate::remote_source::providers::audible) struct TitleProgressContext<'a> {
    pub(super) title_id: &'a str,
    pub(super) item_index: u32,
    pub(super) total_items: u32,
}

pub(in crate::remote_source::providers::audible) fn title_progress(
    context: TitleProgressContext<'_>,
    stage: AcquisitionStage,
    fraction: Option<f32>,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
) -> AcquisitionProgress {
    with_title_progress(
        acquisition_progress(stage, fraction, bytes_downloaded, bytes_total),
        context,
    )
}

pub(in crate::remote_source::providers::audible) fn with_title_progress(
    progress: AcquisitionProgress,
    context: TitleProgressContext<'_>,
) -> AcquisitionProgress {
    acquisition_progress_for_current_title(
        progress,
        context.title_id,
        context.item_index,
        context.total_items,
    )
}
