mod client;
mod stream;

pub(in crate::remote_source::providers::audible) use client::{audio_download_client, no_redirect_client};
pub(in crate::remote_source::providers::audible) use stream::stream_response_chunks;
