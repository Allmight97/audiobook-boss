//! The covers the session's views show, served by request. A host passes
//! the request path on unchanged: `<kind>/<size>/<revision>/<value>`,
//! percent-encoded as a whole and `value` again inside it. Only covers the
//! session handed out are served: a lookup result's or a library title's
//! URL, a listed source's embedded cover, the cover on screen, and the
//! running preview's artwork.

use std::path::PathBuf;

use percent_encoding::percent_decode_str;

use super::super::preview::PreviewArtwork;
use super::Session;
use crate::cover_service::Cover;
use crate::errors::{AppError, Result};

#[derive(Debug, PartialEq, Eq)]
enum CoverRequest {
    Remote { url: String, small: bool },
    Audio { path: PathBuf },
    Session { revision: u64 },
    Preview { run_id: String },
}

impl CoverRequest {
    fn parse(request: &str) -> Result<Self> {
        let decoded = decode(request.trim_start_matches('/'))?;
        let mut parts = decoded.splitn(4, '/');
        let (Some(kind), Some(size), Some(revision), Some(value)) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(unknown());
        };
        let value = decode(value)?;
        let small = match size {
            "small" => true,
            "full" => false,
            _ => return Err(unknown()),
        };
        match (kind, small) {
            ("remote", _) => Ok(Self::Remote { url: value, small }),
            ("audio", true) => Ok(Self::Audio {
                path: PathBuf::from(value),
            }),
            ("session", false) => Ok(Self::Session {
                revision: revision.parse().map_err(|_| unknown())?,
            }),
            ("preview", true) => Ok(Self::Preview { run_id: value }),
            _ => Err(unknown()),
        }
    }

    /// The request for a log line: its kind, and a remote cover's origin
    /// only, never a file path or a full address.
    fn describe(&self) -> String {
        match self {
            Self::Remote { url, small } => {
                let origin = reqwest::Url::parse(url)
                    .map(|url| url.origin().ascii_serialization())
                    .unwrap_or_else(|_| "invalid".into());
                let size = if *small { "small" } else { "full" };
                format!("remote_{size} origin={origin}")
            }
            Self::Audio { .. } => "audio_small".into(),
            Self::Session { .. } => "session_full".into(),
            Self::Preview { .. } => "preview_small".into(),
        }
    }
}

fn decode(text: &str) -> Result<String> {
    percent_decode_str(text)
        .decode_utf8()
        .map(std::borrow::Cow::into_owned)
        .map_err(|_| unknown())
}

fn unknown() -> AppError {
    AppError::InvalidInput("That cover is not one ABB is showing.".into())
}

impl Session {
    /// The cover `request` names; `None` when its source has no cover.
    pub(crate) async fn cover(&self, request: &str) -> Result<Option<Cover>> {
        let request = CoverRequest::parse(request).inspect_err(|_| {
            log::warn!("cover_load outcome=refused reason=malformed_address");
        })?;
        self.serve_cover(&request).await.inspect_err(|error| {
            log::info!(
                "cover_load kind={} outcome=failed reason={error}",
                request.describe()
            );
        })
    }

    async fn serve_cover(&self, request: &CoverRequest) -> Result<Option<Cover>> {
        let covers = &self.inner.network.covers;
        match request {
            CoverRequest::Remote { url, small } => {
                if !self.offers_remote_cover(url) {
                    return Err(unknown());
                }
                let cover = if *small {
                    covers.remote_small(url).await?
                } else {
                    covers.remote_full(url).await?
                };
                Ok(Some(cover))
            }
            CoverRequest::Audio { path } => {
                let path = crate::audio::validate_input_audio_path(path)?;
                if !self.lock().lists_source(&path) {
                    return Err(unknown());
                }
                covers.embedded_small(&path).await
            }
            CoverRequest::Session { revision } => {
                Ok(self.lock().displayed_cover_at(*revision).map(Cover::from))
            }
            CoverRequest::Preview { run_id } => self.preview_cover(run_id).await,
        }
    }

    fn offers_remote_cover(&self, url: &str) -> bool {
        self.lock().lookup_offers_cover(url) || self.inner.deps.remote.library_offers_cover(url)
    }

    async fn preview_cover(&self, run_id: &str) -> Result<Option<Cover>> {
        let artwork = {
            let state = self.lock();
            state
                .preview
                .matches(run_id)
                .then(|| state.preview.artwork.clone())
                .ok_or_else(unknown)?
        };
        match artwork {
            PreviewArtwork::None => Ok(None),
            PreviewArtwork::Source(path) => self.inner.network.covers.embedded_small(&path).await,
            PreviewArtwork::Bytes(bytes) => {
                #[expect(clippy::disallowed_methods, reason = "joined by preview_cover")]
                let rendered = tokio::task::spawn_blocking(move || {
                    crate::metadata::render_display_thumbnail(&bytes)
                })
                .await
                .map_err(|error| AppError::General(format!("Cover task failed: {error}")))?;
                rendered.map(|small| Some(Cover::from(small)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_decodes_once_around_the_whole_and_once_around_its_value() {
        let url = "https://covers.test/a b/c.jpg?x=1";
        let value = percent_encoding::utf8_percent_encode(url, percent_encoding::NON_ALPHANUMERIC);
        let whole = format!("remote/small/0/{value}");
        let request =
            percent_encoding::utf8_percent_encode(&whole, percent_encoding::NON_ALPHANUMERIC);
        assert_eq!(
            CoverRequest::parse(&format!("/{request}")).expect("parses"),
            CoverRequest::Remote {
                url: url.to_string(),
                small: true
            }
        );
    }

    #[test]
    fn only_known_kinds_and_sizes_parse() {
        for request in [
            "audio/full/0/x",
            "session/small/1/",
            "remote/huge/0/x",
            "elsewhere/small/0/x",
            "remote/small",
        ] {
            assert!(CoverRequest::parse(request).is_err(), "{request}");
        }
        assert_eq!(
            CoverRequest::parse("session/full/7/").expect("session cover"),
            CoverRequest::Session { revision: 7 }
        );
    }
}
