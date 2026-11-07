use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AcquisitionStage {
    License,
    Download,
    Decryption,
    Validation,
    ImportHandoff,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum AcquisitionStrategy {
    DownloadImportReady,
    DownloadThenDecryptAax,
    DownloadThenDecryptAaxc,
    DownloadThenDecryptDash,
    ProtectedUnsupported,
    ProviderProtocolFailed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionProgress {
    pub stage: AcquisitionStage,
    #[specta(type = specta_typescript::Number)]
    pub percentage: f32,
    pub message: String,
    pub bytes_downloaded: Option<u64>,
    pub bytes_total: Option<u64>,
    pub current_title_id: Option<String>,
    pub current_item_index: Option<u32>,
    pub total_items: Option<u32>,
    pub terminal: bool,
}

pub fn acquisition_progress(
    stage: AcquisitionStage,
    fraction: Option<f32>,
    bytes_downloaded: Option<u64>,
    bytes_total: Option<u64>,
) -> AcquisitionProgress {
    let fraction = fraction
        .filter(|value| value.is_finite())
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let (start, end, message, terminal) = match stage {
        AcquisitionStage::License => (0.0, 15.0, "Requesting download license.", false),
        AcquisitionStage::Download => (15.0, 65.0, "Downloading audiobook.", false),
        AcquisitionStage::Decryption => (65.0, 90.0, "Decrypting audiobook.", false),
        AcquisitionStage::Validation => (90.0, 97.0, "Validating acquired audiobook.", false),
        AcquisitionStage::ImportHandoff => (97.0, 100.0, "Importing acquired audiobook.", false),
        AcquisitionStage::Failed => (100.0, 100.0, "Acquisition failed.", true),
        AcquisitionStage::Cancelled => (100.0, 100.0, "Acquisition cancelled.", true),
    };

    AcquisitionProgress {
        stage,
        percentage: start + ((end - start) * fraction),
        message: message.to_string(),
        bytes_downloaded,
        bytes_total,
        current_title_id: None,
        current_item_index: None,
        total_items: None,
        terminal,
    }
}

/// Scopes a title's stage progress to its batch. `item_index` is 1-based; the
/// percentage covers the whole batch so a multi-title bar never restarts.
pub fn acquisition_progress_for_current_title(
    mut progress: AcquisitionProgress,
    title_id: impl Into<String>,
    item_index: u32,
    total_items: u32,
) -> AcquisitionProgress {
    if !progress.terminal && total_items > 0 {
        let completed_titles = item_index.clamp(1, total_items) - 1;
        progress.percentage =
            (completed_titles as f32 * 100.0 + progress.percentage) / total_items as f32;
    }
    progress.current_title_id = Some(title_id.into());
    progress.current_item_index = Some(item_index);
    progress.total_items = Some(total_items);
    progress
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
