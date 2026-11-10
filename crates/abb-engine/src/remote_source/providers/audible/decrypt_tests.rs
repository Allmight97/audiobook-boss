use super::*;

#[test]
fn a_packet_reports_how_far_through_the_book_it_is() {
    let milliseconds = abb_ffmpeg::Rational::new(1, 1000);

    assert_eq!(packet_fraction(Some(3_000), milliseconds, 12.0), Some(0.25));
    assert_eq!(packet_fraction(Some(60_000), milliseconds, 12.0), Some(1.0));
}

#[test]
fn an_unknown_time_or_length_reports_no_fraction() {
    let milliseconds = abb_ffmpeg::Rational::new(1, 1000);

    assert_eq!(packet_fraction(None, milliseconds, 12.0), None);
    assert_eq!(packet_fraction(Some(3_000), milliseconds, 0.0), None);
    assert_eq!(packet_fraction(Some(3_000), milliseconds, -1.0), None);
}

#[expect(
    clippy::disallowed_methods,
    reason = "every test joins it through await_copy"
)]
fn worker_until_stopped(
    control: &CopyControl,
    fraction: f32,
    finished: &Arc<AtomicBool>,
) -> JoinHandle<Result<()>> {
    let (control, finished) = (control.clone(), Arc::clone(finished));
    tokio::task::spawn_blocking(move || {
        control.set_fraction(fraction);
        let give_up = std::time::Instant::now() + Duration::from_secs(4);
        while !control.stop_requested() && std::time::Instant::now() < give_up {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(30));
        finished.store(true, Ordering::SeqCst);
        Err(remote_acquisition_cancelled())
    })
}

#[tokio::test]
async fn a_cancel_stops_the_copy_and_waits_for_it_before_returning() {
    let control = CopyControl::default();
    let finished = Arc::new(AtomicBool::new(false));
    let worker = worker_until_stopped(&control, 0.0, &finished);

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        await_copy(worker, &control, &mut |_| {}, &|| true),
    )
    .await
    .expect("a cancel must stop the copy");

    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "{result:?}"
    );
    assert!(
        finished.load(Ordering::SeqCst),
        "the copy was joined, not left running"
    );
}

#[tokio::test]
async fn the_copy_fraction_is_reported_as_decryption_progress() {
    let control = CopyControl::default();
    let finished = Arc::new(AtomicBool::new(false));
    let worker = worker_until_stopped(&control, 0.5, &finished);
    let events = std::cell::RefCell::new(Vec::new());

    let result = tokio::time::timeout(
        Duration::from_secs(2),
        await_copy(
            worker,
            &control,
            &mut |event| events.borrow_mut().push(event),
            &|| !events.borrow().is_empty(),
        ),
    )
    .await
    .expect("the cancel after the first report must stop the copy");

    assert!(matches!(result, Err(AppError::Cancellation(_))));
    let events = events.into_inner();
    assert_eq!(events.len(), 1, "one report for one fraction");
    assert_eq!(
        events[0].percentage,
        acquisition_progress(AcquisitionStage::Decryption, Some(0.5), None, None).percentage
    );
}

#[tokio::test]
async fn a_failed_copy_returns_its_error() {
    let control = CopyControl::default();
    #[expect(clippy::disallowed_methods, reason = "joined by await_copy")]
    let worker = tokio::task::spawn_blocking(|| Err(AppError::General("copy failed".into())));

    let result = await_copy(worker, &control, &mut |_| {}, &|| false).await;

    assert!(matches!(result, Err(AppError::General(message)) if message == "copy failed"));
}

#[test]
fn dropping_the_guard_tells_the_copy_to_stop() {
    let control = CopyControl::default();

    drop(StopOnDrop(control.clone()));

    assert!(control.stop_requested());
}

#[cfg(unix)]
struct ReleasePipe<'a>(&'a Path);

#[cfg(unix)]
impl Drop for ReleasePipe<'_> {
    fn drop(&mut self) {
        let opened = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.0);
        assert!(opened.is_ok(), "release the worker blocked on the pipe");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn shutdown_waits_for_a_decrypt_worker_after_its_caller_is_aborted() {
    let dir = tempfile::TempDir::new().expect("temp dir");
    let input = dir.path().join("source.aaxc");
    let made = std::process::Command::new("mkfifo")
        .arg(&input)
        .status()
        .expect("run mkfifo");
    assert!(
        made.success(),
        "a pipe blocks the worker's open until the test releases it"
    );
    let release = ReleasePipe(&input);
    let output = dir.path().join("book.m4b");
    let tasks = EngineTasks::default();
    let caller = tasks.spawn({
        let (tasks, input, output) = (tasks.clone(), input.clone(), output.clone());
        async move {
            let material = AudibleDecryptionMaterial::Aaxc {
                key: secrecy::SecretBox::new(Box::new([0; 16])),
                iv: secrecy::SecretBox::new(Box::new([0; 16])),
            };
            decrypt_download(
                DecryptRequest {
                    input: &input,
                    output: &output,
                    material: &material,
                    job_id: "job",
                    title_id: "B0TEST",
                    tasks: &tasks,
                },
                &mut |_| {},
                &|| false,
            )
            .await
        }
    });
    tokio::task::yield_now().await;

    caller.abort();
    assert!(caller.await.expect_err("aborted").is_cancelled());
    tasks.close();

    assert!(
        tokio::time::timeout(Duration::from_millis(200), tasks.wait())
            .await
            .is_err(),
        "the worker still runs, so shutdown must still wait"
    );
    drop(release);
    tokio::time::timeout(Duration::from_secs(10), tasks.wait())
        .await
        .expect("shutdown returns once the worker has");
}
