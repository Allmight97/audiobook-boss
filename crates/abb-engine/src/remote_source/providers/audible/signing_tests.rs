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

const TOO_SMALL_DEVICE_KEY_BASE64: [&str; 13] = [
    "MIICXgIBAAKBgQDNWkWtqoJ840Y4aPlhGJsL/n7M8bCfc1q2jfKlncSg9D5Wn6zl",
    "9R846XxwB5SOFQnAbUoTOn4m74Rp+1y1nvtAldH0wM9H7mY9D27CuNWQkM8r8IyA",
    "onxhif4uPNMOmZUihD19zX9okU4PZo58ngb6xr8+z1u7KyANIqz8C4cOFwIDAQAB",
    "AoGAOe4VagCH/yM+U/uJHlMKqj+JFde4s8Ke/JDeusnfBmjgjR9ObzDPNgboC9KX",
    "nd1het9Z3kLKA8Hd4OgAJ1xKFFSKFOXaS6D18aLy+Z9mn/XLKGA5B+duJTm2JoTk",
    "o+RW0XrnL2zqCG3ZMdLyu4dPJm0B6iz2Oxhk7CiVH0AFTEECQQD09jOyeF2KpsOB",
    "JazMT4AX83xP11/Dtdnn0QAZrKY7N99X/dDcy7X7v8/7zBKYHV/+v4AfE+sXRZ+j",
    "YsxaGsNDAkEA1psnFI3W2SWvDCwcNIYSvLhDdwltE6RYxyDhjMkDjNoisStCwu6O",
    "QHVT9rci4Ke5oTkjLTqE4XNYJvwXsc2anQJBAIb+Nrcfhc0m4Y6GSJe/KAQK9nK5",
    "adjcml0Z3EhJ0MHJpJdFFJjY2k0GA4PrjkOtzH0C44Q84c1NXRAZHix5mykCQQCT",
    "h5OFGMDzBGni50v4e5bT76RJZ8x5bUiuk5cajnGHWcnxm7jIgvB9tWqkN1+dqw/8",
    "mmfpRspkpkOagUDUtXAtAkEAm43fCBlra6Dtljcs0hRRAl0l/x9QYEx/HJKG6XOQ",
    "TktFmC/GuJyC9orfJCGt4DehFBJg1zcskQ/qlvyScrZJmQ==",
];

pub(in crate::remote_source::providers::audible) fn test_device_key_pem() -> String {
    pkcs1_pem(&TEST_DEVICE_KEY_BASE64)
}

fn pkcs1_pem(base64_lines: &[&str]) -> String {
    format!(
        "-----BEGIN RSA PRIVATE KEY-----\n{}\n-----END RSA PRIVATE KEY-----\n",
        base64_lines.join("\n")
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

#[test]
fn a_device_key_under_2048_bits_is_refused_with_the_reason() {
    let error = AdpSigner::new("adp-token", &pkcs1_pem(&TOO_SMALL_DEVICE_KEY_BASE64))
        .err()
        .expect("refused");

    let message = error.to_string();
    assert!(message.contains("shorter than 2048 bits"), "{message}");
    assert!(message.contains("Sign in to Audible again"), "{message}");
}
