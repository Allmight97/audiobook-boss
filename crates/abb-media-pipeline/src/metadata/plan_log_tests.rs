use super::field_decisions;
use crate::metadata::{AudiobookMetadata, MetadataWritePlan};

#[test]
fn the_plan_log_names_every_field_with_its_decision_in_a_fixed_order() {
    let plan = MetadataWritePlan::from_metadata(&AudiobookMetadata {
        title: Some("Title".to_string()),
        artist: Some(String::new()),
        series_part: Some("3".to_string()),
        track: Some((2, None)),
        disk: Some((0, None)),
        ..Default::default()
    });

    assert_eq!(
        field_decisions(&plan),
        "Title=set,Artist=clear,Album=preserve,Composer=preserve,Genre=preserve,Date=preserve,\
         Comment=preserve,Description=preserve,Series=preserve,SeriesPart=set,Track=set,Disk=clear"
    );
}
