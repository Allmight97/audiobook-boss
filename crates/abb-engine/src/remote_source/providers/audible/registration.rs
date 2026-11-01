//! Device registration with the current Audible for iPhone profile. Ported
//! from `audible_api` 0.1.0 `auth::register` (MIT OR Apache-2.0).

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use audible_api::auth::oauth::build_client_id;
use audible_api::auth::register::Registration;
use serde_json::{json, Value};

use super::{provider_private_failure, AUDIBLE_IOS_DEVICE_TYPE};
use crate::errors::Result;

const APP_VERSION: &str = "4.60";
const OS_VERSION: &str = "26.2";
const SOFTWARE_VERSION: &str = "46000856";
const DEVICE_SERIAL_LEN: usize = 40;
const SERIAL_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

pub(super) fn new_device_serial() -> String {
    let mut serial = String::with_capacity(DEVICE_SERIAL_LEN);
    while serial.len() < DEVICE_SERIAL_LEN {
        let uuid = uuid::Uuid::new_v4();
        // Bytes 6 and 8 carry the UUID version and variant bits; the rest are random.
        // 248 is the largest multiple of 62 below 256, so every symbol is equally likely.
        let random = uuid
            .as_bytes()
            .iter()
            .enumerate()
            .filter(|(index, byte)| *index != 6 && *index != 8 && **byte < 248);
        for (_, byte) in random {
            if serial.len() == DEVICE_SERIAL_LEN {
                break;
            }
            serial.push(SERIAL_ALPHABET[usize::from(*byte) % SERIAL_ALPHABET.len()] as char);
        }
    }
    serial
}

pub(super) async fn register_device(
    authorization_code: &str,
    code_verifier: &str,
    domain: &str,
    device_serial: &str,
) -> Result<Registration> {
    let reply = reqwest::Client::new()
        .post(format!("https://api.amazon.{domain}/auth/register"))
        .json(&registration_request_body(
            authorization_code,
            code_verifier,
            domain,
            device_serial,
        ))
        .send()
        .await
        .map_err(|_| provider_private_failure("device registration"))?;
    if !reply.status().is_success() {
        log::warn!(
            "remote_source audible stage=device_registration http_status={}",
            reply.status().as_u16()
        );
        return Err(provider_private_failure("device registration"));
    }
    let reply: Value = reply
        .json()
        .await
        .map_err(|_| provider_private_failure("device registration response parse"))?;
    registration_from_reply(&reply, device_serial)
}

fn registration_request_body(
    authorization_code: &str,
    code_verifier: &str,
    domain: &str,
    device_serial: &str,
) -> Value {
    json!({
        "requested_token_type": [
            "bearer",
            "mac_dms",
            "website_cookies",
            "store_authentication_cookie"
        ],
        "cookies": {
            "website_cookies": [],
            "domain": format!(".amazon.{domain}")
        },
        "registration_data": {
            "domain": "Device",
            "app_version": APP_VERSION,
            "device_serial": device_serial,
            "device_type": AUDIBLE_IOS_DEVICE_TYPE,
            "device_name": "%FIRST_NAME%%FIRST_NAME_POSSESSIVE_STRING%%DUPE_STRATEGY_1ST%Audible for iPhone",
            "os_version": OS_VERSION,
            "software_version": SOFTWARE_VERSION,
            "device_model": "iPhone",
            "app_name": "Audible"
        },
        "auth_data": {
            "client_id": build_client_id(device_serial),
            "authorization_code": authorization_code,
            "code_verifier": code_verifier,
            "code_algorithm": "SHA-256",
            "client_domain": "DeviceLegacy"
        },
        "requested_extensions": ["device_info", "customer_info"]
    })
}

// Complexity: one required-token `?` per field of the saved device.
fn registration_from_reply(reply: &Value, device_serial: &str) -> Result<Registration> {
    let success = &reply["response"]["success"];
    let tokens = &success["tokens"];
    let token = |value: &Value| {
        value
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| provider_private_failure("device registration response"))
    };
    let expires_in = match &tokens["bearer"]["expires_in"] {
        Value::String(text) => text.parse::<i64>().ok(),
        value => value.as_i64(),
    }
    .ok_or_else(|| provider_private_failure("device registration response"))?;
    let website_cookies = tokens["website_cookies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|cookie| {
            Some((
                cookie["Name"].as_str()?.to_string(),
                cookie["Value"].as_str()?.replace('"', ""),
            ))
        })
        .collect::<HashMap<_, _>>();

    Ok(Registration {
        device_serial: device_serial.to_string(),
        client_id: build_client_id(device_serial),
        adp_token: token(&tokens["mac_dms"]["adp_token"])?,
        device_private_key: token(&tokens["mac_dms"]["device_private_key"])?,
        access_token: token(&tokens["bearer"]["access_token"])?,
        refresh_token: token(&tokens["bearer"]["refresh_token"])?,
        expires: unix_now() + expires_in,
        website_cookies,
        store_authentication_cookie: token(&tokens["store_authentication_cookie"]["cookie"])?,
        device_info: success["extensions"]["device_info"].clone(),
        customer_info: success["extensions"]["customer_info"].clone(),
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
#[path = "registration_tests.rs"]
mod tests;
