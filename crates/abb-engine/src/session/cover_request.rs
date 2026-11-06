//! Serves the covers the session offers, by the request a host passes on
//! unchanged (`cover_address.rs` owns the format).

use super::super::cover_address::{self, unknown, CoverRequest};
use super::super::preview::PreviewArtwork;
use super::Session;
use crate::cover_service::Cover;
use crate::errors::{AppError, Result};

impl Session {
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
