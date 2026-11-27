use super::*;

fn pdf(name: &str) -> SupplementalProcessingAsset {
    SupplementalProcessingAsset {
        path: PathBuf::from(format!("/books/{name}")),
        file_name: name.to_string(),
        size_bytes: 10,
        sha256: "00".to_string(),
    }
}

#[test]
fn companions_attach_export_retain_and_remove_by_path() {
    let mut companions = Companions::default();
    companions.attach("alpha", vec![pdf("guide.pdf")]);
    companions.attach("beta", Vec::new());

    let assets = companions
        .assets_for(["alpha", "beta", "unknown"])
        .expect("assets");
    assert_eq!(assets.keys().collect::<Vec<_>>(), ["alpha"]);
    assert!(companions.assets_for(["beta"]).is_none());
    assert_eq!(
        companions.names(),
        BTreeMap::from([("alpha".to_string(), vec!["guide.pdf".to_string()])])
    );

    companions.attach("gamma", vec![pdf("notes.pdf")]);
    companions.retain(&HashSet::from(["alpha", "gamma"]));
    assert_eq!(companions.names().len(), 2);

    companions.remove_paths(&[PathBuf::from("/books/guide.pdf")]);
    assert_eq!(
        companions.names(),
        BTreeMap::from([("gamma".to_string(), vec!["notes.pdf".to_string()])])
    );

    companions.retain(&HashSet::new());
    assert!(companions.names().is_empty());
}
