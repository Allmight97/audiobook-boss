use std::fs;

use abb_engine::app_settings::{IndexerConnection, SettingsIntent, SettingsOutcome, SettingsUnit};
use abb_engine::remote_source::RemoteUiIntent;
use abb_engine::session::{SessionIntent, SessionOutcome};
use abb_engine::{Engine, ShutdownOutcome};
use tempfile::TempDir;

use super::integration_session_desk::{engine_config, Desk};

async fn save_categories(desk: &Desk, category_ids: Vec<u32>) {
    desk.send(SessionIntent::Remote {
        intent: RemoteUiIntent::EditConnection {
            base_url: None,
            category_ids: Some(category_ids),
            api_key: None,
        },
    })
    .await;
    let outcome = desk
        .send(SessionIntent::Remote {
            intent: RemoteUiIntent::SaveConnection,
        })
        .await;
    assert_eq!(outcome, SessionOutcome::RemoteSaved);
}

fn drafted(desk: &Desk) -> abb_engine::remote_source::IndexerDraftSnapshot {
    desk.engine
        .session_snapshot()
        .remote
        .expect("remote part")
        .connection
}

fn desk_with_config(files: &[(&str, &str)]) -> Desk {
    let root = TempDir::new().expect("engine root");
    let config = root.path().join("config");
    fs::create_dir_all(&config).expect("config folder");
    for (name, content) in files {
        fs::write(config.join(name), content).expect("seed config file");
    }
    let engine = Engine::start(engine_config(root.path())).expect("start headless engine");
    Desk { root, engine }
}

#[tokio::test]
async fn an_indexer_save_is_in_effect_after_quitting_and_relaunching() {
    let desk = Desk::new();
    save_categories(&desk, vec![3030]).await;
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);

    let desk = desk.relaunch();

    let snapshot = desk.engine.settings_snapshot().await;
    assert_eq!(snapshot.settings.indexer_connection.category_ids, [3030]);
    assert_eq!(snapshot.recovery, None);
    assert_eq!(drafted(&desk).category_ids, [3030]);
}

#[tokio::test]
async fn quitting_with_an_unwritable_folder_after_an_indexer_save_reports_it_and_a_relaunch_after_the_folder_returns_shows_it(
) {
    let desk = Desk::new();
    let config = desk.root.path().join("config");
    if config.is_dir() {
        fs::remove_dir_all(&config).expect("remove the config folder");
    }
    fs::write(&config, b"").expect("a regular file where the config folder was");

    save_categories(&desk, vec![3040]).await;
    assert_eq!(
        desk.engine
            .settings_snapshot()
            .await
            .settings
            .indexer_connection
            .category_ids,
        [3040],
        "the change is in effect although it is not on disk"
    );
    let ShutdownOutcome::SettingsUnsaved { error } = desk.engine.shutdown().await else {
        panic!("quitting with an unwritable config folder must say settings are unsaved");
    };
    assert!(!error.message.is_empty());

    fs::remove_file(&config).expect("remove the blocking file");
    fs::create_dir_all(&config).expect("restore the config folder");
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);

    let desk = desk.relaunch();
    assert_eq!(
        desk.engine
            .settings_snapshot()
            .await
            .settings
            .indexer_connection
            .category_ids,
        [3040]
    );
}

#[tokio::test]
async fn a_damaged_indexer_section_is_reported_and_left_alone_until_the_next_accepted_change() {
    let damaged =
        "[indexer]\nurl = \"https://user:secret@prowlarr.example.com\"\ncategories = [3030]\n";
    let stray = "url = \"https://stray.example.com\"\ncategories = [3040]\n";
    let desk = desk_with_config(&[("settings.toml", damaged), ("indexer.toml", stray)]);
    let settings_file = desk.root.path().join("config").join("settings.toml");
    let stray_file = desk.root.path().join("config").join("indexer.toml");

    let snapshot = desk.engine.settings_snapshot().await;

    let recovery = snapshot.recovery.expect("the reset is reported");
    assert_eq!(recovery.reset, [SettingsUnit::IndexerConnection]);
    assert_eq!(
        snapshot.settings.indexer_connection,
        IndexerConnection::default()
    );
    assert_eq!(drafted(&desk).base_url, "");
    assert_eq!(
        fs::read_to_string(&settings_file).expect("settings"),
        damaged
    );

    save_categories(&desk, vec![3030]).await;

    assert_eq!(desk.engine.settings_snapshot().await.recovery, None);
    let saved = fs::read_to_string(&settings_file).expect("settings");
    assert!(saved.contains("categories = [3030]"), "{saved}");
    assert!(!saved.contains("secret"), "{saved}");
    assert_eq!(fs::read_to_string(&stray_file).expect("stray file"), stray);
    let connection = drafted(&desk);
    assert_eq!(connection.base_url, "");
    assert_eq!(connection.category_ids, [3030]);
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}

#[tokio::test]
async fn resetting_all_settings_returns_the_remote_draft_to_the_default_connection() {
    let desk = Desk::new();
    save_categories(&desk, vec![3030]).await;
    assert_eq!(drafted(&desk).category_ids, [3030]);

    let reply = desk.engine.settings_dispatch(SettingsIntent::Reset).await;

    assert_eq!(reply.outcome, SettingsOutcome::Applied);
    let connection = drafted(&desk);
    assert_eq!(connection.category_ids, [3000, 3030]);
    assert_eq!(connection.base_url, "");
    assert!(!connection.api_key_configured);
    assert_eq!(
        reply.snapshot.settings.indexer_connection,
        IndexerConnection::default()
    );
}
