use super::*;

const SERIAL: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij0123";

const AUDIBLE_API_0_1_0_SAVED_SIGN_IN: &str = r#"{"locale":{"country_code":"us","domain":"com","market_place_id":"AF2M0KC94RCEA"},"device_registration":{"device_serial":"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij0123","client_id":"4142434445464748494a4b4c4d4e4f505152535455565758595a6162636465666768696a30313233234132435a4a5a474c4b324a4a564d","adp_token":"{enc:fake-adp}{key:fake}{iv:fake}{name:fake}{serial:Mg==}","device_private_key":"-----BEGIN RSA PRIVATE KEY-----\nfake\n-----END RSA PRIVATE KEY-----\n","access_token":"Atna|fake-access","refresh_token":"Atnr|fake-refresh","expires":1793003600,"website_cookies":{"session-id":"000-0000000-0000000"},"store_authentication_cookie":"fake-store-cookie","device_info":{"device_name":"Listener's Audible for iPhone","device_serial_number":"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij0123","device_type":"A2CZJZGLK2JJVM"},"customer_info":{"account_pool":"Amazon","given_name":"Fixture","home_region":"NA","name":"Fixture Listener","user_id":"amzn1.account.FAKE"}},"authorization_code":"ANfakecode","code_verifier":"fake-verifier"}"#;

#[test]
fn a_sign_in_saved_by_audible_api_0_1_0_reads_and_saves_unchanged() {
    let auth: Auth = serde_json::from_str(AUDIBLE_API_0_1_0_SAVED_SIGN_IN).expect("saved sign-in");

    let saved: Value = serde_json::from_str(AUDIBLE_API_0_1_0_SAVED_SIGN_IN).expect("json");
    assert_eq!(serde_json::to_value(&auth).expect("serialize"), saved);
    assert_eq!(auth.locale.domain, "com");
    assert_eq!(auth.device_registration.device_serial, SERIAL);
    assert_eq!(
        auth.device_registration.website_cookies["session-id"],
        "000-0000000-0000000"
    );
}

#[test]
fn a_new_sign_in_saves_the_us_locale_audible_api_0_1_0_saved() {
    let saved: Value = serde_json::from_str(AUDIBLE_API_0_1_0_SAVED_SIGN_IN).expect("json");

    assert_eq!(
        serde_json::to_value(Locale::united_states()).expect("serialize"),
        saved["locale"]
    );
}

#[test]
fn the_sign_in_url_equals_audible_api_0_1_0_for_the_same_verifier_and_serial() {
    assert_eq!(
        oauth_url("9xZ1W4VLYM59hr_aabYhTfo6pfI0LAZ0sN1Aylk3q1s", SERIAL),
        "https://www.amazon.com/ap/signin?openid.oa2.response_type=code&openid.oa2.code_challenge_method=S256&openid.oa2.code_challenge=4HJc70goUa6Fe3i1noBSwHvNvC_xK2Ba0qLm3usvKK4&openid.return_to=https%3A%2F%2Fwww.amazon.com%2Fap%2Fmaplanding&openid.assoc_handle=amzn_audible_ios_us&openid.identity=http%3A%2F%2Fspecs.openid.net%2Fauth%2F2.0%2Fidentifier_select&pageId=amzn_audible_ios&accountStatusPolicy=P1&openid.claimed_id=http%3A%2F%2Fspecs.openid.net%2Fauth%2F2.0%2Fidentifier_select&openid.mode=checkid_setup&openid.ns.oa2=http%3A%2F%2Fwww.amazon.com%2Fap%2Fext%2Foauth%2F2&openid.oa2.client_id=device%3A4142434445464748494a4b4c4d4e4f505152535455565758595a6162636465666768696a30313233234132435a4a5a474c4b324a4a564d&openid.ns.pape=http%3A%2F%2Fspecs.openid.net%2Fextensions%2Fpape%2F1.0&marketPlaceId=ATVPDKIKX0DER&openid.oa2.scope=device_auth_access&forceMobileLayout=true&openid.ns=http%3A%2F%2Fspecs.openid.net%2Fauth%2F2.0&openid.pape.max_auth_age=0"
    );
}

#[test]
fn the_client_id_equals_audible_api_0_1_0() {
    assert_eq!(build_client_id(SERIAL), "4142434445464748494a4b4c4d4e4f505152535455565758595a6162636465666768696a30313233234132435a4a5a474c4b324a4a564d");
}

#[test]
fn a_code_verifier_is_32_random_bytes_url_safe() {
    let verifier = new_code_verifier().expect("verifier");

    assert_eq!(verifier.len(), 43);
    assert_eq!(
        BASE64_URL_SAFE_NO_PAD
            .decode(&verifier)
            .expect("base64url")
            .len(),
        32
    );
    assert_ne!(verifier, new_code_verifier().expect("second verifier"));
}

#[test]
fn the_auth_code_comes_from_the_redirect_query() {
    assert_eq!(
        extract_auth_code(
            "https://www.amazon.com/ap/maplanding?openid.oa2.authorization_code=ANcode123&openid.mode=id_res"
        )
        .as_deref(),
        Some("ANcode123")
    );
    assert_eq!(
        extract_auth_code("https://www.amazon.com/ap/maplanding?openid.mode=id_res"),
        None
    );
    assert_eq!(extract_auth_code("not a url"), None);
}

#[test]
fn debug_output_names_no_secret() {
    let auth: Auth = serde_json::from_str(AUDIBLE_API_0_1_0_SAVED_SIGN_IN).expect("saved sign-in");

    let printed = format!("{auth:?}");

    for secret in [
        "fake-adp",
        "PRIVATE KEY",
        "fake-access",
        "fake-refresh",
        "fake-store-cookie",
        "ANfakecode",
    ] {
        assert!(!printed.contains(secret), "{printed}");
    }
}
