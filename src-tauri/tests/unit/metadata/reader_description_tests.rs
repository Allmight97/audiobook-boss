use audiobook_boss_lib::metadata::reader::merge_tag_data;
use audiobook_boss_lib::metadata::AudiobookMetadata;
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};

#[test]
fn does_not_fallback_description_to_comment() {
    let mut tag = Tag::new(TagType::Mp4Ilst);
    tag.set_comment("Only a comment".into());

    let mut metadata = AudiobookMetadata::new();
    merge_tag_data(&tag, &mut metadata);

    assert!(metadata.description.is_none(), "description should not be populated from comment");
    assert!(metadata.comment.is_none(), "comment field should remain None");
}

#[test]
fn reads_description_when_present() {
    let mut tag = Tag::new(TagType::Mp4Ilst);
    tag.insert(TagItem::new(
        ItemKey::Description,
        ItemValue::Text("Actual description".into()),
    ));

    let mut metadata = AudiobookMetadata::new();
    merge_tag_data(&tag, &mut metadata);

    assert_eq!(metadata.description.as_deref(), Some("Actual description"));
}
