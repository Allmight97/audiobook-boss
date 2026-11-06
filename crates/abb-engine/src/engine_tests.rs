use super::*;

fn config(root: &Path) -> EngineConfig {
    EngineConfig {
        cache_dir: root.join("cache"),
        config_dir: root.join("config"),
        app_identifier: "com.audiobook-boss.test".to_string(),
        events: Arc::new(crate::DiscardEvents),
        aaxclean_helper: None,
    }
}

#[tokio::test]
async fn a_reset_is_refused_while_the_indexer_holds_its_connection_and_applies_after() {
    let root = tempfile::TempDir::new().expect("engine root");
    let engine = Engine::start(config(root.path())).expect("start headless engine");
    let revision = engine.settings_snapshot().await.revision;

    let held = engine
        .inner
        .remote_source
        .reset_lease()
        .expect("an idle Indexer");
    let refused = engine.settings_dispatch(SettingsIntent::Reset).await;
    drop(held);

    assert!(matches!(
        &refused.outcome,
        SettingsOutcome::Rejected { error } if error.message.contains("before resetting settings")
    ));
    assert_eq!(refused.snapshot.revision, revision);

    let applied = engine.settings_dispatch(SettingsIntent::Reset).await;
    assert_eq!(applied.outcome, SettingsOutcome::Applied);
    assert!(
        engine.inner.remote_source.reset_lease().is_ok(),
        "the Reset released its hold"
    );
}
