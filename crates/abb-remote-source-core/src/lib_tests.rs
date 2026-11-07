use super::*;

#[test]
fn progress_plan_uses_truthful_stage_bands() {
    let licensing = acquisition_progress(AcquisitionStage::License, Some(0.5), None, None);
    let downloading =
        acquisition_progress(AcquisitionStage::Download, Some(0.5), Some(50), Some(100));
    let decrypting = acquisition_progress(AcquisitionStage::Decryption, Some(0.5), None, None);
    let validating = acquisition_progress(AcquisitionStage::Validation, Some(0.5), None, None);
    let importing = acquisition_progress(AcquisitionStage::ImportHandoff, Some(0.5), None, None);

    assert!((0.0..=15.0).contains(&licensing.percentage));
    assert!((15.0..=65.0).contains(&downloading.percentage));
    assert!((65.0..=90.0).contains(&decrypting.percentage));
    assert!((90.0..=97.0).contains(&validating.percentage));
    assert!((97.0..=100.0).contains(&importing.percentage));
    assert_eq!(downloading.bytes_downloaded, Some(50));
    assert_eq!(downloading.bytes_total, Some(100));
    assert!(decrypting.message.to_lowercase().contains("decrypt"));

    let scoped_progress = acquisition_progress_for_current_title(downloading, "B000000001", 2, 3);

    assert_eq!(
        scoped_progress.current_title_id.as_deref(),
        Some("B000000001")
    );
    assert_eq!(scoped_progress.current_item_index, Some(2));
    assert_eq!(scoped_progress.total_items, Some(3));
    assert_eq!(scoped_progress.stage, AcquisitionStage::Download);
}

#[test]
fn multi_title_progress_is_one_batch_bar() {
    let halfway = || acquisition_progress(AcquisitionStage::Download, Some(0.5), None, None);

    let first = acquisition_progress_for_current_title(halfway(), "B1", 1, 2);
    let second = acquisition_progress_for_current_title(halfway(), "B2", 2, 2);
    let only = acquisition_progress_for_current_title(halfway(), "B1", 1, 1);

    assert!((first.percentage - 20.0).abs() < 0.001);
    assert!((second.percentage - 70.0).abs() < 0.001);
    assert!((only.percentage - 40.0).abs() < 0.001);
}

#[test]
fn progress_plan_rejects_non_finite_fraction_at_its_owner() {
    let progress = acquisition_progress(AcquisitionStage::Download, Some(f32::NAN), None, None);

    assert_eq!(progress.percentage, 15.0);
    assert!(progress.percentage.is_finite());
}
