//! Loads a chosen cover and serves the covers the session offers, by the
//! request a host passes on unchanged (`cover_address.rs` owns the format).

use super::{Rest, Session};
use crate::cover_service::Cover;
use crate::errors::{AppError, Result};
use crate::session::cover_address::{self, unknown, CoverRequest};
use crate::session::intent::SessionOutcome;
use crate::session::preview::PreviewArtwork;
use crate::session::state::SessionState;

impl Session {
    pub(super) fn begin_cover_drop(&self, paths: Vec<String>) -> Rest {
        match crate::cover_source::dropped_cover_path(paths) {
            Some(path) => self.begin_cover_load(CoverSource::File(path)),
            None => Rest::Done(SessionOutcome::Applied),
        }
    }

    pub(super) fn begin_cover_load(&self, source: CoverSource) -> Rest {
        let from_url = matches!(source, CoverSource::Url(_));
        if let CoverSource::Url(url) = &source {
            let url = url.trim();
            if url.is_empty() {
                self.transition(SessionState::cover_url_required);
                return Rest::Done(SessionOutcome::CoverLoadFailed);
            }
            if !is_web_address(url) {
                self.transition(SessionState::cover_url_not_web_address);
                return Rest::Done(SessionOutcome::CoverLoadFailed);
            }
        }
        let started = self.transition(|state| {
            let request = state.begin_cover_request(from_url);
            (state.binding, request)
        });
        Rest::CoverLoad { source, started }
    }

    pub(super) async fn load_cover(
        &self,
        source: CoverSource,
        started: (u64, u64),
    ) -> SessionOutcome {
        let from_url = matches!(source, CoverSource::Url(_));
        if from_url {
            self.publish();
        }
        let result = match source {
            CoverSource::File(path) => crate::cover_source::load_cover_art_file(path).await,
            CoverSource::Url(url) => self
                .inner
                .network
                .covers
                .remote_full(url.trim())
                .await
                .map(|cover| cover.to_vec()),
        };
        self.transition(|state| {
            if state.binding != started.0 {
                return SessionOutcome::EditLost;
            }
            if state.cover_request() != started.1 {
                return SessionOutcome::Superseded;
            }
            let loaded = result.is_ok();
            state.cover_load_finished(from_url, result);
            if loaded {
                SessionOutcome::Applied
            } else {
                SessionOutcome::CoverLoadFailed
            }
        })
    }

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
            CoverRequest::Remote { url } => {
                if !self.offers_remote_cover(url) {
                    return Err(unknown());
                }
                Ok(Some(covers.remote_small(url).await?))
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
        let address = cover_address::remote(url);
        self.lock().lookup_offers_cover(url)
            || self.inner.deps.remote.library_offers_cover(&address)
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

fn is_web_address(text: &str) -> bool {
    reqwest::Url::parse(text).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

pub(super) enum CoverSource {
    File(String),
    Url(String),
}
