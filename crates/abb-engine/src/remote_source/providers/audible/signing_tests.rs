use super::*;
use std::time::Duration;

const TEST_DEVICE_KEY_BASE64: [&str; 25] = [
    "MIIEowIBAAKCAQEAwg+dGtQztpswmalwFzQv5lAZP0xgWfLj1ROeYM+WFiQOEanv",
    "CRl9yrrPHxv5tIx+d1Nw/fPj85lgSDqgJcG/W0lvF7WoqgWhTF2RK+R5OoIshP3t",
    "SE0ErV5LJVgEcNRLJ1axleoRI9hQStyCwErmwmO8Lpnz/OKY4yW3vCTwxVYgLEPP",
    "KLwuTx1+i5GxVqiQaN+iiZNT9yxp4dQmeX7qIZ+VA97/1FbqqcGlWUj9exmu5/sd",
    "iSbeqcNzZdh2uTXWJcjnw5xELH+Y7qpkJfoSYNsRjI0wgtV9j5tEL+dWqsALMsvU",
    "PTWk8xgxWcxEC8aY+k47avTBllzhmAYdyNvh0wIDAQABAoIBAA/fFwpKsLAOoTrw",
    "EKrVKdW8V1RF+ttgciHLKPjiAXRuHnhzXYlw/EjIXog80X9ivjUkLemXON2Dfcxq",
    "fw6Xef8NUnit8s11CGwmeukMFp4iZ2G5It727ZsAS0USncpckoj1HjEUo7I6ztZQ",
    "kqEHfzgG0C0/iiT0o2xweUZ5/vckn5dJexfU9ddWec1cwhuWXQxEFmb6MlSDDsU8",
    "PNght98f5hi4/sLSmtJjc2qn5GkEwoL1+jx7+KaoTpAzZElPVhhnYL8m2QC79wNE",
    "qJZoZNoXORT+nREXM3WZpx2082Qw82AeP5ELiqKtqankdWRpDm8lEvAA+nOmfFkr",
    "6ZTEdgECgYEA0iEgabG9bvm34/Zc0D/5PqykELimRpRmGDd4MECiEC+RD9xWFYIB",
    "F+VfMcGO9xA3aEsmy88Tr7KE7Wilrfhg3SBQx+553CiVH4HuC+hnNbYYUIfRxkgu",
    "j7wy7wuRYvDeloiqLw0MXyMO7Wh9Xai6+9vqcPk/Mv8u70vEBMrq3OMCgYEA7GyF",
    "GPv1sDoP9hncQoFD6/lQEzK2Wd2M2UsTQ/WvAB0+af8To3Pk1ll7ZGZHcVvtMBUq",
    "CopmO9TPd9PW3k7L1Lw6jY/+3VY1wg7yzKGiXzkV5eE2WW5pk84yO5KQvcwDe327",
    "QqSM0oGxjzxJlbl4wWhY/ip0GjbZH2acI6hyalECgYEAhc3aWfN4cSZK1rqWN3XF",
    "PQeqIYuzNdZhfKI1wmQlzrC7aqIfnup9z72bdI+QffqxjZPtTHK8UnEW/CZa+gDj",
    "jWt6hRVyO6ML8lhGQQboUed2bDFQfzaPj6+Dr3xqQ9qloq270OI6Moqcgsi6q905",
    "1A8hpnbQq6SbOn3AgFzGWYkCgYBsNOLZ43+4T2q8vWtE+PrIRswybTRufusXDLhd",
    "k1WsHLZTNw6a8dEblcrW0cUDCgrNxfH+7wcJfUYTVt5x0uiRjjDSV8RTM0DZd5QC",
    "ujuhj3QJSYFhfxpNLsNo5pTeeUBixdHuF3foZLmKkBIxt7if9I0+jtmiodXP/Fuc",
    "Kl1rcQKBgGsqG1YDBZLbyt8FaB0qUd4rE4aIGVyL42QmP0NcydKxAaYIxSb3vCEe",
    "skfGJCvRp4YOipdeu6ZqgmBhynyb5xRFOrm4IBVAAo0Wa+oqXmQ9ma5dqzhUB4nN",
    "g2weyVuhQdCf8UlNP78M3fy1Iuq97VOgGjyk/iqR1/gr8PIxkNiV",
];

pub(in crate::remote_source::providers::audible) fn test_device_key_pem() -> String {
    format!(
        "-----BEGIN RSA PRIVATE KEY-----\n{}\n-----END RSA PRIVATE KEY-----\n",
        TEST_DEVICE_KEY_BASE64.join("\n")
    )
}

fn signer() -> AdpSigner {
    AdpSigner::new("adp-token", &test_device_key_pem()).expect("test device key")
}

#[test]
fn headers_equal_those_audible_api_0_1_0_wrote_for_the_same_request() {
    let cases = [
        (
            "GET",
            "/1.0/library?num_results=1000&sort_by=-PurchaseDate",
            "",
            "d9BhzxQjpyFARuiW/B/oWznhZpA2GbbvY9Wd5JMWX/lLczWa1yXpxI90syOgYjaOYwGr0NE04mVrfmy4yS2OyubXHYxZT3NfV9kpUsgyetT8vezlBzW/P4A9pmskga2Qva+It8i71b+UJoxp4FWW9qyPwIuOyAeVrgLjJfYksiliMBT8SSC3i+hHMXF4XsjOEVSfjXRaviRLldp6bWIx+90FOTJoP/eqOnKJnTLXw7jMfJ/FncxovnX8rq5/dv0NVhceZNLCzF46xevFwflWR4c2FI+DeXcM9M//G0z9ghEo4RhGqrIi9oEKinWqwunlHIH7iFh9GVQQj8QyiKs7rg==:2026-11-11T19:07:38.020136+00:00Z",
        ),
        (
            "POST",
            "/1.0/content/B0TEST/licenserequest",
            r#"{"quality":"High"}"#,
            "LVIG9LtYp3T3JpPUtoiS44tWLlUw1+cDXHp9jiOLn+6Tc1RsHX7LFUUukrfQ0mMJFzo2RBfww4SLJ2qvHWzsjDoIX0iLeOB19NRYqsUMlKHaWP1sKngtBq4qDi4fKWNhf5mRq0rx4lzNmadmTQPsLUvtdm5mUyHfzVdfaxnQknZyaQwv2ClmrmhauuT40K+FT/hIY2wi6Ojx6KbPgE08gDq3C9V3DvmmXTkAMf4KrauUXu7ZuUqFQtERx4eGt6kIDWGvX/P8te0/qjc0rfmylLq+uiQcKrhOKrgDC8PwrOjLWzADRPEZcv8KeCT/G5rPdFzI3vnsLojwXitkMdN/7g==:2026-11-11T19:07:38.045936+00:00Z",
        ),
    ];

    for (method, path, body, old_signature) in cases {
        let (_, date) = old_signature
            .split_once(':')
            .expect("signature carries its date");

        let headers = signer()
            .headers(method, path, body.as_bytes(), date)
            .expect("signed");

        assert_eq!(headers.len(), 3, "{method}");
        assert_eq!(headers["x-adp-token"], "adp-token", "{method}");
        assert_eq!(headers["x-adp-alg"], "SHA256withRSA:1.0", "{method}");
        assert_eq!(headers["x-adp-signature"], old_signature, "{method}");
    }
}

#[test]
fn the_signed_date_matches_chrono_rfc3339_with_a_trailing_z() {
    let cases = [
        (1793000000, 0, "2026-10-26T07:33:20+00:00Z"),
        (1793000000, 123000000, "2026-10-26T07:33:20.123+00:00Z"),
        (1793000000, 123456000, "2026-10-26T07:33:20.123456+00:00Z"),
        (
            1793000000,
            123456789,
            "2026-10-26T07:33:20.123456789+00:00Z",
        ),
        (951782400, 5, "2000-02-29T00:00:00.000000005+00:00Z"),
        (0, 0, "1970-01-01T00:00:00+00:00Z"),
        (
            1798761599,
            999999999,
            "2026-12-31T23:59:59.999999999+00:00Z",
        ),
        (1798761600, 0, "2027-01-01T00:00:00+00:00Z"),
        (1803859199, 1000000, "2027-02-28T23:59:59.001+00:00Z"),
        (1803859200, 0, "2027-03-01T00:00:00+00:00Z"),
        (1835395200, 0, "2028-02-29T00:00:00+00:00Z"),
        (1835481599, 0, "2028-02-29T23:59:59+00:00Z"),
    ];

    for (seconds, nanos, expected) in cases {
        let at = UNIX_EPOCH + Duration::new(seconds, nanos);
        assert_eq!(adp_date(at), expected, "{seconds}.{nanos:09}");
    }
}

#[test]
fn the_key_size_is_the_modulus_length_in_bits() {
    assert_eq!(signer().key_bits(), 2048);
}

#[test]
fn a_saved_key_that_is_not_pkcs1_pem_is_refused() {
    let error = AdpSigner::new("adp-token", "device-private-key")
        .err()
        .expect("refused");

    assert!(error.to_string().contains("withheld"), "{error}");
}
