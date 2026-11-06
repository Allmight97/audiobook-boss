//! The address of every cover the session offers, and the request it parses
//! back to. A snapshot carries the address as an opaque string; the host
//! passes it to its image scheme, so what reaches `parse` is the address
//! percent-encoded as a whole: `<kind>/<revision>/<value>`, with `value`
//! percent-encoded again inside it. Only covers the session handed out are
//! served: a lookup result's or a library title's URL, a listed source's
//! embedded cover, the cover on screen, and the running preview's artwork.

use std::path::{Path, PathBuf};

use percent_encoding::{percent_decode_str, utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};

use crate::errors::{AppError, Result};

const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

#[derive(Debug, PartialEq, Eq)]
pub(super) enum CoverRequest {
    Remote { url: String },
    Audio { path: PathBuf },
    Session { revision: u64 },
    Preview { run_id: String },
}

impl CoverRequest {
    pub(super) fn parse(request: &str) -> Result<Self> {
        let decoded = decode(request.trim_start_matches('/'))?;
        let mut parts = decoded.splitn(3, '/');
        let (Some(kind), Some(revision), Some(value)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(unknown());
        };
        let value = decode(value)?;
        match kind {
            "remote" => Ok(Self::Remote { url: value }),
            "audio" => Ok(Self::Audio {
                path: PathBuf::from(value),
            }),
            "session" => Ok(Self::Session {
                revision: revision.parse().map_err(|_| unknown())?,
            }),
            "preview" => Ok(Self::Preview { run_id: value }),
            _ => Err(unknown()),
        }
    }

    /// The request for a log line: its kind, and a remote cover's origin
    /// only, never a file path or a full address.
    pub(super) fn describe(&self) -> String {
        match self {
            Self::Remote { url } => {
                let origin = reqwest::Url::parse(url)
                    .map(|url| url.origin().ascii_serialization())
                    .unwrap_or_else(|_| "invalid".into());
                format!("remote_small origin={origin}")
            }
            Self::Audio { .. } => "audio_small".into(),
            Self::Session { .. } => "session_full".into(),
            Self::Preview { .. } => "preview_small".into(),
        }
    }
}

pub(crate) fn remote(url: &str) -> String {
    address("remote", 0, url)
}

/// `revision` advances whenever a Save writes a cover into the source, so
/// the address of a source's cover names its current image.
pub(crate) fn audio(path: &Path, revision: u64) -> String {
    address("audio", revision, &path.to_string_lossy())
}

pub(crate) fn session(revision: u64) -> String {
    address("session", revision, "")
}

pub(crate) fn preview(run_id: &str) -> String {
    address("preview", 0, run_id)
}

fn address(kind: &str, revision: u64, value: &str) -> String {
    format!(
        "{kind}/{revision}/{}",
        utf8_percent_encode(value, COMPONENT)
    )
}

fn decode(text: &str) -> Result<String> {
    percent_decode_str(text)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| unknown())
}

pub(super) fn unknown() -> AppError {
    AppError::InvalidInput("That cover is not one ABB is showing.".into())
}

#[cfg(test)]
#[path = "cover_address_tests.rs"]
pub(super) mod tests;
