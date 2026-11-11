//! Audible API request signing. Ported from `audible_api` 0.1.0
//! `auth::auth_headers` (MIT OR Apache-2.0).
//! Copyright (c) 2024 Mark Hendrickson.

use std::time::{SystemTime, UNIX_EPOCH};

use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{RsaKeyPair, RSA_PKCS1_SHA256};
use base64::prelude::{Engine as _, BASE64_STANDARD};
use reqwest::header::{HeaderMap, HeaderValue};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::PrivatePkcs1KeyDer;

use super::provider_private_failure;
use crate::errors::{AppError, Result};

const SIGNING_ALGORITHM: &str = "SHA256withRSA:1.0";
const MIN_DEVICE_KEY_BITS: usize = 2048;

pub(super) struct AdpSigner {
    adp_token: HeaderValue,
    device_key: RsaKeyPair,
}

impl AdpSigner {
    pub(super) fn new(adp_token: &str, device_private_key_pem: &str) -> Result<Self> {
        let device_key = device_key_from_pem(device_private_key_pem)?;
        let adp_token = HeaderValue::from_str(adp_token)
            .map_err(|_| provider_private_failure("device token"))?;
        Ok(Self {
            adp_token,
            device_key,
        })
    }

    pub(super) fn key_bits(&self) -> usize {
        self.device_key.public_modulus_len() * 8
    }

    pub(super) fn headers(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        date: &str,
    ) -> Result<HeaderMap> {
        let mut signed = Vec::with_capacity(method.len() + path.len() + body.len() + 128);
        for part in [method.as_bytes(), path.as_bytes(), date.as_bytes(), body] {
            signed.extend_from_slice(part);
            signed.push(b'\n');
        }
        signed.extend_from_slice(self.adp_token.as_bytes());
        let mut signature = vec![0; self.device_key.public_modulus_len()];
        self.device_key
            .sign(
                &RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                &signed,
                &mut signature,
            )
            .map_err(|_| provider_private_failure("request signing"))?;
        let signature = format!("{}:{date}", BASE64_STANDARD.encode(signature));

        let signature = HeaderValue::from_str(&signature)
            .map_err(|_| provider_private_failure("request signing"))?;
        let mut headers = HeaderMap::new();
        headers.insert("x-adp-token", self.adp_token.clone());
        headers.insert("x-adp-alg", HeaderValue::from_static(SIGNING_ALGORITHM));
        headers.insert("x-adp-signature", signature);
        Ok(headers)
    }
}

fn device_key_from_pem(pem: &str) -> Result<RsaKeyPair> {
    let der = PrivatePkcs1KeyDer::from_pem_slice(pem.as_bytes())
        .map_err(|_| provider_private_failure("device key"))?;
    RsaKeyPair::from_der(der.secret_pkcs1_der()).map_err(|rejected| {
        if rejected.description_() == "TooSmall" {
            AppError::General(format!(
                "The saved Audible device key is shorter than {MIN_DEVICE_KEY_BITS} bits, \
                 so ABB cannot sign Audible requests with it. Sign in to Audible again."
            ))
        } else {
            provider_private_failure("device key")
        }
    })
}

/// The signed date Audible expects: UTC RFC 3339 with `+00:00`, then a
/// literal `Z`, as `audible_api` wrote it (`chrono` `to_rfc3339() + "Z"`).
pub(super) fn adp_date(now: SystemTime) -> String {
    let elapsed = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let seconds = elapsed.as_secs();
    let (year, month, day) = civil_date(seconds / 86_400);
    let time_of_day = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}{}+00:00Z",
        time_of_day / 3_600,
        time_of_day % 3_600 / 60,
        time_of_day % 60,
        fraction(elapsed.subsec_nanos()),
    )
}

fn fraction(nanos: u32) -> String {
    match nanos {
        0 => String::new(),
        n if n % 1_000_000 == 0 => format!(".{:03}", n / 1_000_000),
        n if n % 1_000 == 0 => format!(".{:06}", n / 1_000),
        n => format!(".{n:09}"),
    }
}

fn civil_date(days_since_epoch: u64) -> (u64, u64, u64) {
    let days = days_since_epoch + 719_468;
    let era = days / 146_097;
    let day_of_era = days % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
#[path = "signing_tests.rs"]
pub(super) mod tests;
