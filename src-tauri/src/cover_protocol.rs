//! Serves `abb-cover` addresses: the webview's `<img>` requests a cover, the
//! request path goes to `Engine::cover` unchanged, and the answer becomes the
//! HTTP response. The status mapping is the host's only decision.

use std::sync::Arc;

use abb_engine::{AppErrorCategory, AppErrorEnvelope};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::Manager;

pub(crate) const SCHEME: &str = "abb-cover";

pub(crate) fn handle<R: tauri::Runtime>(
    ctx: tauri::UriSchemeContext<'_, R>,
    request: Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    if ctx.webview_label() != "main" {
        return responder.respond(response(Ok(None)));
    }
    let app = ctx.app_handle().clone();
    let path = request.uri().path().to_string();
    // The handler runs on the webview's thread; the cover loads elsewhere.
    tauri::async_runtime::spawn(async move {
        let answer = match app.try_state::<abb_engine::Engine>() {
            Some(engine) => engine.cover(&path).await,
            None => Err(abb_engine::AppError::General(
                "The engine is not running.".into(),
            )),
        };
        responder.respond(response(answer));
    });
}

/// A cover is JPEG and never changes at its address; a missing or refused
/// one is not found, a failed one a bad gateway. Errors are never cached.
fn response(answer: abb_engine::Result<Option<Arc<[u8]>>>) -> Response<Vec<u8>> {
    let builder = Response::builder().header(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    let (builder, body) = match answer {
        Ok(Some(bytes)) => (
            builder
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "image/jpeg")
                .header(
                    header::CACHE_CONTROL,
                    "private, max-age=31536000, immutable",
                ),
            bytes.to_vec(),
        ),
        Ok(None) => (not_cached(builder, StatusCode::NOT_FOUND), Vec::new()),
        Err(error) => {
            let status = if AppErrorEnvelope::from(&error).category == AppErrorCategory::Validation
            {
                StatusCode::NOT_FOUND
            } else {
                log::warn!("A cover could not be loaded: {error}");
                StatusCode::BAD_GATEWAY
            };
            (not_cached(builder, status), Vec::new())
        }
    };
    builder
        .body(body)
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn not_cached(
    builder: tauri::http::response::Builder,
    status: StatusCode,
) -> tauri::http::response::Builder {
    builder
        .status(status)
        .header(header::CACHE_CONTROL, "no-store")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn covers_answer_as_cacheable_jpeg_and_failures_as_uncached_errors() {
        let found = response(Ok(Some(Arc::from(vec![1, 2, 3]))));
        assert_eq!(found.status(), StatusCode::OK);
        assert_eq!(found.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(found.body(), &[1, 2, 3]);

        let missing = response(Ok(None));
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        assert_eq!(missing.headers()[header::CACHE_CONTROL], "no-store");

        let refused = response(Err(abb_engine::AppError::InvalidInput("not shown".into())));
        assert_eq!(refused.status(), StatusCode::NOT_FOUND);

        let failed = response(Err(abb_engine::AppError::General("offline".into())));
        assert_eq!(failed.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(failed.headers()[header::CACHE_CONTROL], "no-store");
    }
}
