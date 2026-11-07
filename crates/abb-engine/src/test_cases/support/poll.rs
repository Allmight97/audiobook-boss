use std::time::Duration;

const DEADLINE: Duration = Duration::from_secs(30);
const INTERVAL: Duration = Duration::from_millis(5);

pub(crate) async fn until<T>(what: &str, mut ready: impl FnMut() -> Option<T>) -> T {
    tokio::time::timeout(DEADLINE, async {
        loop {
            if let Some(found) = ready() {
                return found;
            }
            tokio::time::sleep(INTERVAL).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting until {what}"))
}

pub(crate) async fn until_true(what: &str, mut ready: impl FnMut() -> bool) {
    until(what, || ready().then_some(())).await;
}
