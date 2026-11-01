use super::*;
#[test]
fn sibling_cue_association_diagnostics_and_accepted_plan_isolation() {
    let tmp = tempfile::TempDir::new().expect("intake tempdir");
    let dir = tmp.path().canonicalize().expect("canonical fixture parent");
    let one = dir.join("one.mp3");
    let two = dir.join("two.mp3");
    std::fs::write(&one, b"one").expect("first audio identity");
    std::fs::write(&two, b"two").expect("second audio identity");
    std::fs::write(
        dir.join("one.cue"),
        "FILE \"stale.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"First\"\nINDEX 01 00:00:94",
    )
    .expect("nonstandard sidecar");
    std::fs::write(
        dir.join("two.cue"),
        "FILE \"two.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Second\"\nINDEX 01 00:01:00",
    )
    .expect("standard sidecar");
    let (first, cue) =
        inspect_chapter_source(&one, 2000, false, &mut Vec::new()).expect("first intake");
    let cue = cue.expect("first sidecar");
    assert_eq!(cue.status, CueStatus::NeedsConfirmation);
    assert!(cue.message.contains("stale.mp3"));
    let (second, cue) =
        inspect_chapter_source(&two, 2000, false, &mut Vec::new()).expect("second intake");
    assert_eq!(cue.expect("standard sidecar").status, CueStatus::Ready);
    std::fs::write(dir.join("one.cue"), "FILE \"../unsafe.mp3\" MP3")
        .expect("change sidecar after intake");
    let files = [&one, &two].map(|path| {
        let mut file = crate::audio::AudioFile::new(path.clone());
        file.is_valid = true;
        file.duration = Some(2.0);
        file.sample_rate = Some(44_100);
        file
    });
    let mut inputs = crate::audio::FileListInfo {
        files: files.to_vec(),
        total_duration: 4.0,
        total_size: 6.0,
        valid_count: 2,
        invalid_count: 0,
    };
    let accepted = std::collections::HashMap::from([
        (one.to_string_lossy().into_owned(), first.clone()),
        (two.to_string_lossy().into_owned(), second.clone()),
    ]);
    crate::audio::apply_chapter_plans(&mut inputs, Some(&accepted))
        .expect("job handoff retains accepted data despite changed CUE");
    assert_eq!(inputs.files[0].chapter_plan.as_ref(), Some(&first));
    assert_eq!(inputs.files[1].chapter_plan.as_ref(), Some(&second));
    assert_eq!(first.chapters[0].start_ms, 940);
    assert_eq!(second.chapters[0].start_ms, 1000);
    let grouped_title = crate::audio::TitleAudioRequest {
        output: crate::audio::OutputAudio::M4b {
            settings: crate::audio::EncoderSettings::default(),
        },
        intent: crate::audio::AudioIntent::Encode,
        sample_rate: crate::audio::SampleRateConfig::Auto,
    };
    assert!(
        crate::audio::resolve_title_audio(&grouped_title, &inputs, false)
            .expect_err("one title cannot compose CUE-bearing sources")
            .to_string()
            .contains("CUE-bearing")
    );
    assert_eq!(
        inspect_chapter_source(&one, 2000, false, &mut Vec::new())
            .expect("invalid diagnostic")
            .1
            .expect("sidecar")
            .status,
        CueStatus::Invalid
    );
    std::fs::write(&one, b"changed audio").expect("mutate audio");
    assert!(validate_chapter_plan(&one, 2000, &first).is_err());
    std::fs::write(dir.join("two.CUE"), "duplicate").expect("ambiguous sidecar");
    // On case-sensitive volumes these are two candidates; on insensitive
    // volumes the second write changes the single candidate to invalid text.
    assert_eq!(
        inspect_chapter_source(&two, 2000, false, &mut Vec::new())
            .expect("ambiguous diagnostic")
            .1
            .expect("sidecar")
            .status,
        CueStatus::Invalid
    );
    assert_eq!(
        inspect_chapter_source(&two, 2000, false, &mut second.chapters.clone())
            .expect("embedded precedence")
            .1
            .expect("sidecar")
            .status,
        CueStatus::EmbeddedPreferred
    );
}
