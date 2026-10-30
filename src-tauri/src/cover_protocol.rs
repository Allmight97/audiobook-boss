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
#[path = "cover_protocol_tests.rs"]
mod tests;
