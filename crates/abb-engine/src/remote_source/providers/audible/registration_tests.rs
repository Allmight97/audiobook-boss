use super::*;

#[test]
fn new_serials_are_forty_random_letters_and_digits() {
    let first = new_device_serial();
    let second = new_device_serial();

    assert_eq!(first.len(), 40);
    assert!(first.chars().all(|c| c.is_ascii_alphanumeric()));
    assert_ne!(first, second);
}

#[test]
fn registration_announces_the_current_iphone_app() {
    let body = registration_request_body("code", "verifier", "com", "S".repeat(40).as_str());
    let device = &body["registration_data"];

    assert_eq!(device["app_version"], "4.60");
    assert_eq!(device["os_version"], "26.2");
    assert_eq!(device["software_version"], "46000856");
    assert_eq!(device["device_type"], "A2CZJZGLK2JJVM");
    assert_eq!(device["device_serial"], "S".repeat(40));
    assert_eq!(
        body["auth_data"]["client_id"],
        build_client_id(&"S".repeat(40))
    );
    assert_eq!(body["cookies"]["domain"], ".amazon.com");
}

#[test]
fn registration_reply_becomes_the_saved_device() {
    let reply = json!({"response": {"success": {
        "tokens": {
            "mac_dms": {"adp_token": "adp", "device_private_key": "key"},
            "bearer": {"access_token": "access", "refresh_token": "refresh", "expires_in": "3600"},
            "store_authentication_cookie": {"cookie": "store"},
            "website_cookies": [{"Name": "session-id", "Value": "\"abc\""}]
        },
        "extensions": {"device_info": {"device_type": "A2CZJZGLK2JJVM"}, "customer_info": {"user_id": "u"}}
    }}});

    let registration = registration_from_reply(&reply, "serial").expect("registration");

    assert_eq!(registration.adp_token, "adp");
    assert_eq!(registration.device_private_key, "key");
    assert_eq!(registration.refresh_token, "refresh");
    assert_eq!(registration.website_cookies["session-id"], "abc");
    assert_eq!(registration.customer_info["user_id"], "u");
    assert!(registration.expires > unix_now());
}

#[test]
fn registration_reply_without_signing_key_is_refused() {
    let reply = json!({"response": {"success": {"tokens": {"bearer": {"expires_in": 3600}}}}});

    assert!(registration_from_reply(&reply, "serial").is_err());
}
