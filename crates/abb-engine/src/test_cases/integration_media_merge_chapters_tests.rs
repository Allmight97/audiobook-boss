//! The chapters a preserved merge of chapterless sources carries: one per
//! source, titled by the source's own file name, each starting where the one
//! before ends.

use super::media_fixtures::{native_encoder_settings, run_planned_title};
use super::tag_interop_seed::seed_mergeable_m4a;
use crate::audio::{AudioIntent, OutputAudio, SampleRateConfig, TitleAudioRequest};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn a_preserved_merge_of_chapterless_sources_gets_one_chapter_per_source() {
    let dir = TempDir::new().expect("merge workspace");
    let sources = [
        seed_mergeable_m4a(&dir.path().join("a"), "first book"),
        seed_mergeable_m4a(&dir.path().join("b"), "second book"),
    ];
    let out_dir = dir.path().join("out");
    fs::create_dir_all(&out_dir).expect("output folder");
    let request = TitleAudioRequest {
        output: OutputAudio::M4b {
            settings: native_encoder_settings(),
        },
        intent: AudioIntent::Preserve,
        sample_rate: SampleRateConfig::Auto,
    };
    let anchor = &sources[0];
    let payload = serde_json::from_value(serde_json::json!({
        "inputFiles": [anchor],
        "titleSources": {anchor.to_str().expect("utf8 path"): sources.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
        "audioRequests": [request],
        "outputDir": out_dir, "aacDecoder": "auto"
    }))
    .expect("grouped title request");

    let (output, _) = run_planned_title(payload, None, &dir.path().join("work"))
        .await
        .expect("preserved merge");

    ffmpeg_next::init().expect("ffmpeg");
    let input = ffmpeg_next::format::input(&output).expect("open the merged file");
    let chapters = crate::metadata::read_chapters(&input);
    assert_eq!(chapters.len(), 2, "{chapters:?}");
    assert_eq!(chapters[0].title.as_deref(), Some("first book"));
    assert_eq!(chapters[1].title.as_deref(), Some("second book"));
    assert_eq!(chapters[0].start_ms, 0);
    assert_eq!(chapters[1].start_ms, chapters[0].end_ms);
    assert!(chapters[0].end_ms > 0 && chapters[1].end_ms > chapters[1].start_ms);
}
