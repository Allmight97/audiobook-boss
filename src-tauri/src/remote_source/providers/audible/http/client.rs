use reqwest::redirect::Policy;

/// Build a client that follows redirects with an HTTPS-only guard and redirect cap.
pub(in crate::remote_source::providers::audible) fn audio_download_client(
    max_redirects: usize,
) -> Result<reqwest::Client, ()> {
    let redirect_policy = Policy::custom(move |attempt| {
        if attempt.previous().len() >= max_redirects {
            return attempt.error("remote source download exceeded redirect limit");
        }
        if attempt.url().scheme() != "https" {
            return attempt.error("remote source download redirect must use https");
        }
        attempt.follow()
    });
    reqwest::Client::builder()
        .redirect(redirect_policy)
        .build()
        .map_err(|_| ())
}

/// Build a client that does not follow redirects; callers handle redirect loops manually.
pub(in crate::remote_source::providers::audible) fn no_redirect_client() -> Result<reqwest::Client, ()> {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .map_err(|_| ())
}
