use audiobook_boss_lib::metadata::writer::update_tag_data;
use audiobook_boss_lib::metadata::AudiobookMetadata;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{ItemKey, ItemValue, Tag, TagItem, TagType};

#[test]
fn removes_fields_and_cover_art_when_absent() {
    let mut tag = Tag::new(TagType::Mp4Ilst);
    tag.set_title("Old Title".into());
    tag.set_artist("Old Author".into());
    tag.insert(TagItem::new(ItemKey::Movement, ItemValue::Text("Series".into())));
    tag.insert(TagItem::new(ItemKey::AlbumTitleSortOrder, ItemValue::Text("Sort".into())));

    let picture = Picture::new_unchecked(PictureType::CoverFront, Some(MimeType::Jpeg), None, vec![1, 2, 3]);
    tag.push_picture(picture);

    let mut metadata = AudiobookMetadata::new();
    metadata.cover_art = Some(Vec::new());

    update_tag_data(&mut tag, &metadata).expect("update should succeed");

    assert!(tag.title().is_none());
    assert!(tag.artist().is_none());
    assert!(tag.get(&ItemKey::Movement).is_none());
    assert!(tag.get(&ItemKey::AlbumTitleSortOrder).is_none());
    assert!(tag.pictures().is_empty(), "cover art should be removed");
}

#[test]
fn writes_description_without_comment_coupling() {
    let mut tag = Tag::new(TagType::Mp4Ilst);

    let mut metadata = AudiobookMetadata::new();
    metadata.description = Some("A tale".into());

    update_tag_data(&mut tag, &metadata).expect("update should succeed");

    let description_item = tag
        .get(&ItemKey::Description)
        .and_then(|item| item.value().text())
        .map(|s| s.to_string());

    assert_eq!(description_item.as_deref(), Some("A tale"));
    assert!(tag.comment().is_none(), "comment should remain untouched");
}
