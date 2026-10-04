use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

fn png(size: u32) -> Vec<u8> {
    let image = image::DynamicImage::ImageRgb8(image::ImageBuffer::from_pixel(
        size,
        size,
        image::Rgb([12, 34, 56]),
    ));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .expect("encode fixture");
    bytes.into_inner()
}

fn counting(fetches: &Arc<AtomicUsize>, gate: Arc<tokio::sync::Notify>) -> CoverService {
    let fetches = Arc::clone(fetches);
    CoverService::new(Box::new(move |_url| {
        fetches.fetch_add(1, Ordering::SeqCst);
        let gate = Arc::clone(&gate);
        Box::pin(async move {
            gate.notified().await;
            crate::metadata::optimize_cover_art(&png(400))
        })
    }))
}

#[tokio::test]
async fn views_asking_for_one_cover_at_once_share_one_fetch() {
    let fetches = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(tokio::sync::Notify::new());
    let covers = counting(&fetches, Arc::clone(&gate));
    let url = "https://covers.test/book.jpg";

    let (small, full, opened) =
        tokio::join!(covers.remote_small(url), covers.remote_full(url), async {
            tokio::task::yield_now().await;
            gate.notify_waiters();
        });
    let (small, full) = (small.expect("small"), full.expect("full"));

    assert_eq!(fetches.load(Ordering::SeqCst), 1);
    let thumbnail = image::load_from_memory(&small).expect("thumbnail decodes");
    assert_eq!((thumbnail.width(), thumbnail.height()), (128, 128));
    assert!(full.len() > small.len());
    // Applying the cover later reuses the same download.
    covers.remote_full(url).await.expect("cached");
    assert_eq!(fetches.load(Ordering::SeqCst), 1);
    let () = opened;
}

#[tokio::test]
async fn a_failed_fetch_is_tried_again_on_the_next_request() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let covers = CoverService::new(Box::new({
        let attempts = Arc::clone(&attempts);
        move |_url| {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt == 0 {
                    Err(AppError::General("offline".into()))
                } else {
                    crate::metadata::optimize_cover_art(&png(64))
                }
            })
        }
    }));
    let url = "https://covers.test/flaky.jpg";

    assert!(covers.remote_full(url).await.is_err());
    assert!(covers.remote_full(url).await.is_ok());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[test]
fn the_cache_keeps_the_most_recently_used_covers() {
    let key = |index: usize| Key::RemoteFull(format!("https://covers.test/{index}.jpg"));
    let mut entries = Entries::default();
    for index in 0..CACHED_COVERS {
        entries.slot(key(index));
    }
    entries.slot(key(0));
    entries.slot(key(CACHED_COVERS));
    assert_eq!(entries.slots.len(), CACHED_COVERS);
    assert!(entries.slots.contains_key(&key(0)), "used again, so kept");
    assert!(
        !entries.slots.contains_key(&key(1)),
        "least recently used goes"
    );
}
