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
