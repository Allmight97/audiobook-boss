use super::*;

#[test]
fn an_update_carries_only_the_parts_that_changed() {
    let mut desk = Desk::open(
        &[("alpha", Some(alpha_tags())), ("beta", Some(beta_tags()))],
        &[0],
    );
    let before = desk.state.revision();

    desk.type_into(MetadataField::Genre, "Mystery");
    let update = desk.state.update_since(Some(before));
    assert!(update.metadata.is_some());
    assert!(update.titles.is_none() && update.selection.is_none() && update.lookup.is_none());

    let before = desk.state.revision();
    desk.state.settle();
    assert_eq!(
        desk.state.revision(),
        before,
        "nothing changed, nothing stamped"
    );

    desk.select(&[1]).expect("select beta");
    let update = desk.state.update_since(Some(before));
    assert!(update.selection.is_some() && update.metadata.is_some());
    assert!(
        update.titles.is_none(),
        "selecting does not resend the titles"
    );
}
