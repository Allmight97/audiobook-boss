use super::*;

#[test]
fn each_app_identifier_gets_its_own_vault() {
    let production = KeyringSecretVault::for_app_identifier("com.audiobook-boss");
    let first = KeyringSecretVault::for_app_identifier("com.audiobook-boss.dev.first");
    let second = KeyringSecretVault::for_app_identifier("com.audiobook-boss.dev.second");
    assert_eq!(production.service, "com.audiobook-boss.remote-source");
    assert_ne!(first.service, production.service);
    assert_ne!(first.service, second.service);
    assert_eq!(
        first.service,
        KeyringSecretVault::for_app_identifier("com.audiobook-boss.dev.first").service
    );
}

#[test]
fn ephemeral_store_tells_the_user_sign_in_will_not_be_remembered() {
    assert_eq!(
        account_message(Some("Connect Audible to load your library."), false).as_deref(),
        Some(concat!(
            "Connect Audible to load your library. ",
            "Sign-in won't be remembered after a restart."
        ))
    );
    assert_eq!(
        account_message(None, false).as_deref(),
        Some(SIGN_IN_NOT_REMEMBERED)
    );
    assert_eq!(
        account_message(Some("Connect Audible to load your library."), true).as_deref(),
        Some("Connect Audible to load your library.")
    );
    assert_eq!(account_message(None, true), None);
}

/// Headless `abb-dev` and the engine tests have no Secret Service. The probe
/// must fail closed so registration can use keyutils instead of hanging.
#[cfg(target_os = "linux")]
#[test]
fn secret_service_probe_fails_closed_without_a_session_bus() {
    let store = open_secret_service();
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        assert!(store.is_none());
    }
}
