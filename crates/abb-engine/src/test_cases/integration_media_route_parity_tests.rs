//! One seeded set of tags, a cover and drifting chapters go through the
//! production planner on every output route and are read back from the file.

use super::media_fixtures::{
    assert_ffprobe_tag, audio_duration_ts, chapters_of, decode_pcm_f32, ffprobe_format_tags,
    mux_chapters,
};
use super::media_fixtures::{
    audio_packet_bytes, duration_ms, ffmpeg, minimal_jpg_bytes, picture_codecs, picture_payloads,
    preflight_payload, run_planned_title, single_title_payload, write_complete_frame_aac,
    write_two_picture_fixture, MINIMAL_JPG,
};
use super::support::encoders::{native_encoder_settings, opus_encoder_settings};
use crate::audio::{
    get_file_list_info, AacDecoder, AudioIntent, AudiobookFormat, OutputAudio, SampleRateConfig,
    TitleAudioRequest,
};
use crate::metadata::{
    read_metadata, save_metadata_intent, AlbumSortPatchOp, MetadataIntentPatch, PatchOp,
};
use ffmpeg_next::codec::Id;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const SEEDED_SERIES: &str = "Saga";
const SEEDED_PART: &str = "2.5";
const ASIN: &str = "B000000001";

enum Chapters {
    Planned,
    PlannedStarts,
    OnePerSource,
}

enum Audio {
    EncodedAac,
    EncodedOpus,
    Copied,
    CopiedOpus,
}

struct Route {
    name: &'static str,
    sources: Vec<PathBuf>,
    output: OutputAudio,
    intent: AudioIntent,
    expect_preserve: bool,
    pictures: Vec<Id>,
    chapters: Chapters,
    audio: Audio,
}

fn seed(path: &Path) {
    save_metadata_intent(
        path,
        &MetadataIntentPatch {
            series: Some(PatchOp::Set(SEEDED_SERIES.into())),
            series_part: Some(PatchOp::Set(SEEDED_PART.into())),
            album_sort: Some(AlbumSortPatchOp::Set("Stale sort".into())),
            ..Default::default()
        },
    )
    .expect("seed series and a stale album sort");
    assert_eq!(
        read_metadata(path).expect("reread seed").series.as_deref(),
        Some(SEEDED_SERIES),
        "seed premise: {}",
        path.display()
    );
}

fn quiet_ffmpeg(args: &[&str], output: &Path) {
    let output = output.to_str().expect("utf8 path");
    ffmpeg(
        ["-v", "error", "-y"]
            .into_iter()
            .chain(args.iter().copied())
            .chain([output]),
    );
}

fn drift_chapters(audio: &Path, source: &Path) {
    let end = duration_ms(audio);
    mux_chapters(
        audio,
        &[
            ("One", 0, 400),
            ("Two", 400, end + 30),
            ("Marker", 600, 600),
        ],
        source,
    );
    assert_eq!(
        chapters_of(source).len(),
        3,
        "chapter premise: {}",
        source.display()
    );
}

fn picture_source(dir: &Path, extension: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let audio = write_two_picture_fixture(dir, extension);
    let source = dir.join(format!("drifting.{extension}"));
    drift_chapters(&audio, &source);
    seed(&source);
    assert_eq!(picture_codecs(&source), [Id::MJPEG, Id::PNG], "{extension}");
    source
}

fn short_m4b_source(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let audio = dir.join("plain.m4a");
    quiet_ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=44100:duration=0.9",
            "-ac",
            "1",
            "-c:a",
            "aac",
        ],
        &audio,
    );
    let end = duration_ms(&audio);
    let source = dir.join("book.m4b");
    mux_chapters(&audio, &[("One", 0, 400), ("Two", 400, end - 60)], &source);
    let mut tag = mp4ameta::Tag::read_from_path(&source).expect("read fixture tags");
    tag.set_data(asin_ident(), mp4ameta::Data::Utf8(ASIN.into()));
    tag.write_with_path(
        &source,
        &mp4ameta::WriteConfig {
            write_meta_items: true,
            ..mp4ameta::WriteConfig::NONE
        },
    )
    .expect("plant ASIN");
    seed(&source);
    assert_eq!(planted_asin(&source), ASIN, "ASIN premise");
    let accepted_end = get_file_list_info(&[&source], AacDecoder::Auto)
        .expect("import short M4B")
        .files[0]
        .chapter_plan
        .clone()
        .expect("accepted chapter plan")
        .chapters
        .last()
        .map(|chapter| chapter.end_ms);
    assert_ne!(
        chapters_of(&source).last().map(|(_, _, end)| *end),
        accepted_end,
        "repair premise: the accepted plan moves only the last chapter end"
    );
    source
}

fn asin_ident() -> mp4ameta::ident::FreeformIdentStatic {
    mp4ameta::FreeformIdent::new_static("com.apple.iTunes", "ASIN")
}

fn planted_asin(path: &Path) -> String {
    let tag = mp4ameta::Tag::read_from_path(path).expect("read tags");
    let asin = tag
        .strings_of(&asin_ident())
        .next()
        .unwrap_or_default()
        .to_string();
    asin
}

fn opus_source(dir: &Path) -> PathBuf {
    fs::create_dir_all(dir).expect("source folder");
    let audio = dir.join("plain.mka");
    quiet_ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=0.9",
            "-c:a",
            "libopus",
        ],
        &audio,
    );
    let chaptered = dir.join("chaptered.mka");
    drift_chapters(&audio, &chaptered);
    let source = dir.join("cover.mka");
    quiet_ffmpeg(
        &[
            "-i",
            chaptered.to_str().expect("utf8 path"),
            "-map",
            "0",
            "-map_chapters",
            "0",
            "-c",
            "copy",
            "-attach",
            MINIMAL_JPG,
            "-metadata:s:t:0",
            "mimetype=image/jpeg",
            "-metadata:s:t:0",
            "filename=cover.jpg",
        ],
        &source,
    );
    seed(&source);
    assert_eq!(
        chapters_of(&source).len(),
        3,
        "mka chapters survive the seed"
    );
    assert_eq!(picture_codecs(&source), [Id::MJPEG], "mka");
    source
}

fn aac_stack(dir: &Path) -> Vec<PathBuf> {
    fs::create_dir_all(dir).expect("source folder");
    let sources = vec![dir.join("first.m4a"), dir.join("second.m4a")];
    write_complete_frame_aac(&sources[0], 440);
    write_complete_frame_aac(&sources[1], 880);
    seed(&sources[0]);
    sources
}

fn expected_album_sort(name: &str) -> String {
    format!("{SEEDED_SERIES} 02.5 - Route {name}")
}

fn route_payload(route: &Route, output_dir: &Path) -> crate::processing::ProcessPayload {
    let request = TitleAudioRequest {
        output: route.output.clone(),
        intent: route.intent,
        sample_rate: SampleRateConfig::Auto,
    };
    if let [only] = route.sources.as_slice() {
        return single_title_payload(only, output_dir, request);
    }
    fs::create_dir_all(output_dir).expect("output folder");
    let anchor = &route.sources[0];
    serde_json::from_value(serde_json::json!({
        "inputFiles": [anchor],
        "titleSources": {anchor.to_str().expect("utf8 path"): route.sources.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
        "audioRequests": [request], "outputDir": output_dir, "aacDecoder": "auto"
    }))
    .expect("grouped title request")
}

fn assert_planned_tags(route: &Route, output: &Path) {
    let name = route.name;
    let tags = read_metadata(output).expect("read output tags");
    let title = format!("Route {name}");
    assert_eq!(tags.title.as_deref(), Some(title.as_str()), "{name}");
    assert_eq!(tags.artist.as_deref(), Some("Parity Author"), "{name}");
    assert_eq!(tags.series.as_deref(), Some(SEEDED_SERIES), "{name}");
    assert_eq!(tags.series_part.as_deref(), Some(SEEDED_PART), "{name}");
    let sort = expected_album_sort(name);
    assert_eq!(tags.album_sort.as_deref(), Some(sort.as_str()), "{name}");
    let external = ffprobe_format_tags(output);
    assert_ffprobe_tag(&external, "title", &title);
    assert_ffprobe_tag(&external, "series", SEEDED_SERIES);
    assert_ffprobe_tag(&external, "series-part", SEEDED_PART);
    assert_ffprobe_tag(&external, "sort_album", &sort);
}

fn expected_pictures(route: &Route) -> Vec<(Id, Vec<u8>)> {
    let source = picture_payloads(&route.sources[0]);
    if route.intent == AudioIntent::Encode {
        source
            .into_iter()
            .find(|(codec, _)| *codec == Id::MJPEG)
            .into_iter()
            .collect()
    } else {
        source
    }
}

fn assert_chapters(route: &Route, output: &Path) {
    let name = route.name;
    let chapters = chapters_of(output);
    let titles: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.0.as_deref())
        .collect();
    match route.chapters {
        Chapters::OnePerSource => {
            assert_eq!(titles, [Some("first"), Some("second")], "{name}");
        }
        Chapters::Planned | Chapters::PlannedStarts => {
            assert_eq!(titles, [Some("One"), Some("Two")], "{name}");
            let starts: Vec<_> = chapters.iter().map(|chapter| chapter.1).collect();
            assert_eq!(starts, [0, 400], "{name}");
            if matches!(route.chapters, Chapters::Planned) {
                assert_eq!(chapters[0].2, 400, "{name}");
                let last_end = chapters[1].2;
                let source_end = duration_ms(&route.sources[0]);
                assert!(
                    (last_end - source_end).abs() <= 2,
                    "{name}: last chapter ends at {last_end} ms, source audio at {source_end} ms"
                );
            }
        }
    }
}

fn assert_audio(route: &Route, output: &Path) {
    let name = route.name;
    let source_samples: usize = route
        .sources
        .iter()
        .map(|source| decode_pcm_f32(source).len())
        .sum();
    let out_samples = decode_pcm_f32(output).len();
    match route.audio {
        Audio::EncodedAac => {
            assert_eq!(out_samples, source_samples, "{name}: decoded samples");
            assert_eq!(
                audio_duration_ts(output),
                source_samples as u64,
                "{name}: declared length"
            );
        }
        Audio::EncodedOpus => {
            let wanted = source_samples as f64 / 44_100.0;
            let got = out_samples as f64 / 48_000.0;
            assert!(
                (got - wanted).abs() < 0.002,
                "{name}: {got} s vs {wanted} s"
            );
        }
        Audio::Copied | Audio::CopiedOpus => {
            let packets: Vec<_> = route
                .sources
                .iter()
                .flat_map(|source| audio_packet_bytes(source))
                .collect();
            assert_eq!(audio_packet_bytes(output), packets, "{name}: packets");
            if matches!(route.audio, Audio::Copied) {
                assert_eq!(out_samples, source_samples, "{name}: decoded samples");
            } else {
                assert!(
                    out_samples.abs_diff(source_samples) <= 48,
                    "{name}: {out_samples} samples vs {source_samples}"
                );
            }
        }
    }
}

async fn check_route(route: &Route, work: &Path) -> PathBuf {
    let name = route.name;
    let anchor = &route.sources[0];
    let originals: Vec<_> = route
        .sources
        .iter()
        .map(|source| fs::read(source).expect("read source"))
        .collect();
    let payload = route_payload(route, &work.join(format!("out-{name}")));
    let intent = HashMap::from([(
        anchor.to_string_lossy().into_owned(),
        MetadataIntentPatch {
            title: Some(PatchOp::Set(format!("Route {name}"))),
            artist: Some(PatchOp::Set("Parity Author".into())),
            ..Default::default()
        },
    )]);
    let plan = preflight_payload(payload.clone(), Some(intent.clone()), false)
        .unwrap_or_else(|error| panic!("{name}: preflight: {error:?}"));
    assert_eq!(
        plan.audio_plans[0].handling.preserves(),
        route.expect_preserve,
        "{name}: planned handling"
    );
    let (output, _) = run_planned_title(payload, Some(intent), &work.join(format!("work-{name}")))
        .await
        .unwrap_or_else(|error| panic!("{name}: {error}"));

    assert_planned_tags(route, &output);
    assert_eq!(picture_codecs(&output), route.pictures, "{name}: pictures");
    let pictures = picture_payloads(&output);
    assert_eq!(
        pictures.iter().map(|(codec, _)| *codec).collect::<Vec<_>>(),
        route.pictures,
        "{name}: picture payloads"
    );
    assert_eq!(pictures, expected_pictures(route), "{name}: picture bytes");
    assert_chapters(route, &output);
    let imported = get_file_list_info(&[&output], AacDecoder::Auto).expect("reimport output");
    assert_eq!(imported.valid_count, 1, "{name}: output imports again");
    assert_audio(route, &output);
    for (source, original) in route.sources.iter().zip(originals) {
        assert_eq!(
            fs::read(source).expect("reread source"),
            original,
            "{name}: source untouched"
        );
    }
    output
}

#[tokio::test]
async fn every_encode_route_writes_the_planned_tags_cover_and_chapters() {
    let tmp = TempDir::new().expect("encode routes workspace");
    let source = picture_source(&tmp.path().join("mp3-pictures"), "mp3");
    let route = |name, output| Route {
        name,
        sources: vec![source.clone()],
        output,
        intent: AudioIntent::Encode,
        expect_preserve: false,
        pictures: vec![Id::MJPEG],
        chapters: Chapters::Planned,
        audio: if name == "e-m4b" {
            Audio::EncodedAac
        } else {
            Audio::EncodedOpus
        },
    };
    for row in [
        route(
            "e-m4b",
            OutputAudio::M4b {
                settings: native_encoder_settings(),
            },
        ),
        route(
            "e-m4a-opus",
            OutputAudio::new(AudiobookFormat::M4aOpus, opus_encoder_settings()),
        ),
        route(
            "e-mka-opus",
            OutputAudio::new(AudiobookFormat::MkaOpus, opus_encoder_settings()),
        ),
    ] {
        check_route(&row, tmp.path()).await;
    }
}

#[tokio::test]
async fn every_single_file_preserve_route_keeps_source_tags_pictures_and_audio() {
    let tmp = TempDir::new().expect("preserve routes workspace");
    let m4b = picture_source(&tmp.path().join("m4b-pictures"), "m4b");
    let short = short_m4b_source(&tmp.path().join("m4b-short"));
    let opus = opus_source(&tmp.path().join("mka-opus"));
    let preserve = |name, source: &PathBuf, output, pictures, chapters, audio| Route {
        name,
        sources: vec![source.clone()],
        output,
        intent: AudioIntent::Preserve,
        expect_preserve: true,
        pictures,
        chapters,
        audio,
    };
    let m4b_output = || OutputAudio::M4b {
        settings: native_encoder_settings(),
    };
    for row in [
        preserve(
            "p-m4b-remux",
            &m4b,
            m4b_output(),
            vec![Id::MJPEG, Id::PNG],
            Chapters::Planned,
            Audio::Copied,
        ),
        preserve(
            "p-m4b-copy",
            &short,
            m4b_output(),
            vec![],
            Chapters::PlannedStarts,
            Audio::Copied,
        ),
        preserve(
            "p-opus-to-m4a",
            &opus,
            OutputAudio::new(AudiobookFormat::M4aOpus, native_encoder_settings()),
            vec![Id::MJPEG],
            Chapters::Planned,
            Audio::CopiedOpus,
        ),
        preserve(
            "p-opus-mka",
            &opus,
            OutputAudio::new(AudiobookFormat::MkaOpus, native_encoder_settings()),
            vec![Id::MJPEG],
            Chapters::Planned,
            Audio::CopiedOpus,
        ),
    ] {
        let output = check_route(&row, tmp.path()).await;
        if row.name == "p-m4b-copy" {
            assert_eq!(planted_asin(&output), ASIN, "{}: ASIN kept", row.name);
        }
    }
}

#[tokio::test]
async fn every_preserve_merge_route_writes_the_planned_tags_and_joined_audio() {
    let tmp = TempDir::new().expect("merge routes workspace");
    let aac = aac_stack(&tmp.path().join("aac-stack"));
    let merge_aac = Route {
        name: "m-aac",
        sources: aac,
        output: OutputAudio::M4b {
            settings: native_encoder_settings(),
        },
        intent: AudioIntent::Preserve,
        expect_preserve: true,
        pictures: vec![],
        chapters: Chapters::OnePerSource,
        audio: Audio::Copied,
    };
    check_route(&merge_aac, tmp.path()).await;
}

#[tokio::test]
async fn a_preserved_merge_follows_the_users_order_and_takes_the_chosen_cover() {
    let tmp = TempDir::new().expect("ordered merge workspace");
    {
        let name = "aac";
        let sources = aac_stack(&tmp.path().join("aac-stack"));
        let output = OutputAudio::M4b {
            settings: native_encoder_settings(),
        };
        let intent = AudioIntent::Preserve;
        let extension = "m4b";
        let anchor = sources[0].clone();
        let title_sources = |order: &[&PathBuf]| -> crate::processing::ProcessPayload {
            let output_dir = tmp.path().join(format!("out-{name}"));
            fs::create_dir_all(&output_dir).expect("output folder");
            serde_json::from_value(serde_json::json!({
                "inputFiles": [&anchor],
                "titleSources": {anchor.to_str().expect("utf8 path"): order.iter().map(|path| serde_json::json!({"path": path})).collect::<Vec<_>>()},
                "audioRequests": [TitleAudioRequest {
                    output: output.clone(),
                    intent,
                    sample_rate: SampleRateConfig::Auto,
                }],
                "outputDir": output_dir, "aacDecoder": "auto"
            }))
            .expect("grouped title request")
        };
        let payload = title_sources(&[&sources[1], &sources[0]]);
        let natural = title_sources(&[&sources[0], &sources[1]]);
        let chosen = HashMap::from([(
            anchor.to_string_lossy().into_owned(),
            MetadataIntentPatch {
                title: Some(PatchOp::Set("Joined".into())),
                cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
                ..Default::default()
            },
        )]);
        let plan = preflight_payload(payload.clone(), Some(chosen.clone()), false)
            .unwrap_or_else(|error| panic!("{name}: preflight: {error:?}"));
        assert!(plan.audio_plans[0].handling.preserves(), "{name}");
        let natural_plan = preflight_payload(natural, Some(chosen.clone()), false)
            .unwrap_or_else(|error| panic!("{name}: natural preflight: {error:?}"));
        assert_ne!(
            plan.plan_signature, natural_plan.plan_signature,
            "{name}: review pins source order"
        );
        let originals: Vec<_> = sources
            .iter()
            .map(|source| fs::read(source).expect("read source"))
            .collect();
        let work = tmp.path().join(format!("work-{name}"));
        let (joined, _) = run_planned_title(payload, Some(chosen), &work)
            .await
            .unwrap_or_else(|error| panic!("{name}: {error}"));

        assert_eq!(joined.extension().and_then(|e| e.to_str()), Some(extension));
        let packets: Vec<_> = [&sources[1], &sources[0]]
            .into_iter()
            .flat_map(|source| audio_packet_bytes(source))
            .collect();
        assert_eq!(audio_packet_bytes(&joined), packets, "{name}: packets");
        let samples: usize = sources
            .iter()
            .map(|source| decode_pcm_f32(source).len())
            .sum();
        assert_eq!(decode_pcm_f32(&joined).len(), samples, "{name}: samples");
        let tags = read_metadata(&joined).expect("read joined tags");
        assert_eq!(tags.title.as_deref(), Some("Joined"), "{name}");
        assert_eq!(tags.cover_art, Some(minimal_jpg_bytes()), "{name}");
        assert_eq!(picture_codecs(&joined), [Id::MJPEG], "{name}: pictures");
        let chapters = chapters_of(&joined);
        let titles: Vec<_> = chapters.iter().map(|c| c.0.as_deref()).collect();
        assert_eq!(titles, [Some("second"), Some("first")], "{name}");
        assert!(
            (chapters[1].2 as f64 - samples as f64 / 44.1).abs() <= 2.0,
            "{name}: last chapter end {}",
            chapters[1].2
        );
        for (source, original) in sources.iter().zip(originals) {
            assert_eq!(
                fs::read(source).expect("reread source"),
                original,
                "{name}: source untouched"
            );
        }
        assert!(
            !work.exists() || fs::read_dir(&work).expect("workspace").next().is_none(),
            "{name}: no staging residue"
        );
    }
}
