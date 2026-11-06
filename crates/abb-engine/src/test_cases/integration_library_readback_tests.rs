use std::fs;
use std::path::{Path, PathBuf};

use abb_engine::audio::{AudioIntent, AudiobookFormat, EncoderType};
use abb_engine::session::{
    AudioEdit, CueChoice, MetadataField, SessionIntent, SubmissionStatus, TitlePlan,
};
use abb_engine::work_runtime::WorkOperationStatus;
use abb_engine::{AudiobookMetadata, MetadataIntentPatch, PatchOp, ShutdownOutcome};

use super::integration_session_desk::{finished, submission, Desk};
use super::media_fixtures::{
    assert_ffprobe_tag, chapters_of, decode_pcm_f32, ffprobe_format_tags, write_sine_mp3,
    SAMPLE_RATE,
};
use super::media_fixtures::{minimal_jpg_bytes, picture_codecs, timed_text_chapter_track};

fn encode_m4b() -> [AudioEdit; 3] {
    [
        AudioEdit::Format(AudiobookFormat::M4b),
        AudioEdit::Intent(AudioIntent::Encode),
        AudioEdit::Encoder(EncoderType::NativeAac),
    ]
}

fn preserve(format: AudiobookFormat) -> [AudioEdit; 2] {
    [
        AudioEdit::Format(format),
        AudioEdit::Intent(AudioIntent::Preserve),
    ]
}

fn tags(path: &Path) -> AudiobookMetadata {
    abb_engine::read_metadata(path).expect("read tags back")
}

async fn plan_settled(desk: &Desk) -> TitlePlan {
    desk.wait_until("the title plan leaves pending", |desk| {
        desk.plan() != TitlePlan::Pending
    })
    .await;
    desk.plan()
}

async fn export_to(desk: &Desk, folder: &str, edits: &[AudioEdit]) -> PathBuf {
    let directory = desk.root.path().join(folder);
    fs::create_dir_all(&directory).expect("create export folder");
    desk.send(SessionIntent::SetOutputDirectory {
        directory: directory.to_string_lossy().into_owned(),
    })
    .await;
    let title_ids: Vec<_> = desk
        .engine
        .session_snapshot()
        .titles
        .expect("titles")
        .files
        .into_iter()
        .map(|file| file.input_id)
        .collect();
    for edit in edits {
        desk.send(SessionIntent::SetTitleAudio {
            title_ids: title_ids.clone(),
            edit: *edit,
        })
        .await;
    }
    let wants_preserve = edits.contains(&AudioEdit::Intent(AudioIntent::Preserve));
    match plan_settled(desk).await {
        TitlePlan::Resolved { plan } => assert_eq!(plan.handling.preserves(), wants_preserve),
        other => panic!("the chosen audio settings do not plan: {other:?}"),
    }
    desk.send(SessionIntent::Submit).await;
    let Some(SubmissionStatus::Submitted { operation_id, .. }) = submission(desk) else {
        panic!("export not accepted: {:?}", submission(desk));
    };
    desk.wait_until("the export finishes", |desk| {
        finished(desk.export_status(&operation_id))
    })
    .await;
    assert_eq!(
        desk.export_status(&operation_id),
        WorkOperationStatus::Completed
    );
    desk.output_of(&operation_id)
}

#[tokio::test]
async fn an_edited_grouped_title_exports_every_field_and_the_chosen_cover() {
    let desk = Desk::new();
    let root = desk.root.path().to_path_buf();
    let first = desk.audiobook(&root.join("src/first.m4b"), 1.0).await;
    let second = desk.audiobook(&root.join("src/second.m4b"), 0.6).await;
    let first_bytes = fs::read(&first).expect("read first source");
    let second_bytes = fs::read(&second).expect("read second source");
    let png = root.join("cover.png");
    image::RgbImage::from_pixel(1, 1, image::Rgb([180, 20, 90]))
        .save(&png)
        .expect("write cover image");

    desk.import(&first).await;
    desk.import(&second).await;
    desk.send(SessionIntent::SelectAll).await;
    desk.send(SessionIntent::GroupSelected).await;
    assert_eq!(
        desk.engine
            .session_snapshot()
            .audio
            .expect("audio part")
            .titles
            .len(),
        1
    );
    for (field, value) in [
        (MetadataField::Title, "Golden Edit"),
        (MetadataField::Author, "Edit Author"),
        (MetadataField::Narrator, "Edit Narrator"),
        (MetadataField::Series, "Edit Saga"),
        (MetadataField::SeriesPart, "3"),
        (MetadataField::Subseries, "Edit Arc"),
        (MetadataField::SubseriesPart, "2"),
        (MetadataField::Genre, "Edited Genre"),
        (MetadataField::Date, "2019"),
        (MetadataField::Description, "Edited blurb"),
    ] {
        desk.send(SessionIntent::SetField {
            field,
            value: value.to_string(),
        })
        .await;
    }
    desk.send(SessionIntent::LoadCoverFromFile {
        path: png.to_string_lossy().into_owned(),
    })
    .await;
    desk.wait_until("the chosen cover shows", |desk| {
        let cover = desk.metadata().cover;
        !cover.loading && cover.custom && cover.image.is_some()
    })
    .await;

    let out = export_to(&desk, "exports", &encode_m4b()).await;

    let read = tags(&out);
    assert_eq!(read.title.as_deref(), Some("Golden Edit"));
    assert_eq!(read.album.as_deref(), Some("Golden Edit"));
    assert_eq!(read.artist.as_deref(), Some("Edit Author"));
    assert_eq!(read.composer.as_deref(), Some("Edit Narrator"));
    assert_eq!(read.series.as_deref(), Some("Edit Saga"));
    assert_eq!(read.series_part.as_deref(), Some("3"));
    assert_eq!(read.subseries.as_deref(), Some("Edit Arc"));
    assert_eq!(read.subseries_part.as_deref(), Some("2"));
    assert_eq!(read.date.as_deref(), Some("2019"));
    assert_eq!(read.description.as_deref(), Some("Edited blurb"));
    assert_eq!(read.genre.as_deref(), Some("Edited Genre"));
    let library = ffprobe_format_tags(&out);
    assert_ffprobe_tag(&library, "composer", "Edit Narrator");
    assert_ffprobe_tag(&library, "genre", "Edited Genre");
    assert_ffprobe_tag(&library, "SERIES", "Edit Saga; Edit Arc");
    assert_ffprobe_tag(&library, "SERIES-PART", "3; 2");
    assert_ffprobe_tag(&library, "sort_album", "Edit Saga 03 - Golden Edit");
    assert_eq!(picture_codecs(&out), vec![ffmpeg_next::codec::Id::MJPEG]);
    assert_eq!(
        read.cover_art,
        Some(
            crate::metadata::optimize_cover_art(&fs::read(&png).expect("read cover image"))
                .expect("optimize cover")
        )
    );
    let chapters = chapters_of(&out);
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.0.as_deref())
        .collect();
    assert_eq!(names, [Some("first"), Some("second")]);
    assert_eq!(chapters[0].1, 0);
    let first_length_ms = decode_pcm_f32(&first).len() as i64 * 1000 / i64::from(SAMPLE_RATE);
    assert!(
        (chapters[1].1 - first_length_ms).abs() <= 2,
        "second chapter starts at {} ms, first source is {first_length_ms} ms",
        chapters[1].1
    );
    assert_eq!(fs::read(&first).expect("reread first"), first_bytes);
    assert_eq!(fs::read(&second).expect("reread second"), second_bytes);
    assert_eq!(desk.engine.shutdown().await, ShutdownOutcome::Settled);
}

#[derive(Clone, Copy, Debug)]
enum SourceKind {
    M4b,
    Mp3,
}

async fn cleared_cover_export(
    desk: &Desk,
    kind: SourceKind,
    folder: &str,
    edits: &[AudioEdit],
) -> (PathBuf, PathBuf, Vec<u8>) {
    let src = desk.root.path().join("src").join(match kind {
        SourceKind::M4b => "book.m4b",
        SourceKind::Mp3 => "book.mp3",
    });
    match kind {
        SourceKind::M4b => {
            desk.audiobook(&src, 0.5).await;
        }
        SourceKind::Mp3 => {
            fs::create_dir_all(src.parent().expect("source folder")).expect("source folder");
            write_sine_mp3(&src, 0.5, 440.0);
        }
    }
    abb_engine::save_metadata_intent(
        &src,
        &MetadataIntentPatch {
            cover_art: Some(PatchOp::Set(minimal_jpg_bytes())),
            ..Default::default()
        },
    )
    .expect("give the source a cover");
    let before = fs::read(&src).expect("read source");
    desk.import(&src).await;
    desk.wait_until("the source cover shows", |desk| {
        desk.metadata().cover.image.is_some()
    })
    .await;
    assert_eq!(picture_codecs(&src), vec![ffmpeg_next::codec::Id::MJPEG]);
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Bare Cover".to_string(),
    })
    .await;
    desk.send(SessionIntent::ClearCover).await;
    let cover = desk.metadata().cover;
    assert!(cover.removal_requested && cover.image.is_none());
    let out = export_to(desk, folder, edits).await;
    (src, out, before)
}

#[tokio::test]
async fn a_cleared_cover_stays_off_the_export_on_every_route() {
    let cases: [(SourceKind, &str, Vec<AudioEdit>); 4] = [
        (SourceKind::M4b, "encoded", encode_m4b().to_vec()),
        (
            SourceKind::M4b,
            "kept",
            preserve(AudiobookFormat::M4b).to_vec(),
        ),
        (SourceKind::Mp3, "encoded", encode_m4b().to_vec()),
        (
            SourceKind::Mp3,
            "kept",
            preserve(AudiobookFormat::Mp3).to_vec(),
        ),
    ];
    for (kind, folder, edits) in cases {
        let desk = Desk::new();
        let (src, out, before) = cleared_cover_export(&desk, kind, folder, &edits).await;
        let route = format!("{kind:?} {folder}");
        assert!(picture_codecs(&out).is_empty(), "{route}: cover came back");
        let read = tags(&out);
        assert_eq!(read.cover_art, None, "{route}");
        assert_eq!(read.title.as_deref(), Some("Bare Cover"), "{route}");
        assert_eq!(fs::read(&src).expect("reread source"), before, "{route}");
        assert_eq!(
            picture_codecs(&src),
            vec![ffmpeg_next::codec::Id::MJPEG],
            "{route}"
        );
    }
}

const CUE_SHEET: &str = "FILE \"stale.mp3\" MP3\nTRACK 01 AUDIO\nTITLE \"Opening\"\nINDEX 01 00:00:00\nTRACK 02 AUDIO\nTITLE \"Near end\"\nINDEX 01 00:01:36\n";

async fn desk_with_cue_mp3() -> Desk {
    let desk = Desk::new();
    let src = desk.root.path().join("src");
    fs::create_dir_all(&src).expect("source folder");
    write_sine_mp3(&src.join("book.mp3"), 1.5, 440.0);
    fs::write(src.join("book.cue"), CUE_SHEET).expect("write CUE");
    desk.import(&src.join("book.mp3")).await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Cue Book".to_string(),
    })
    .await;
    desk
}

#[tokio::test]
async fn ignoring_a_cue_sheet_keeps_its_chapters_out_of_the_export() {
    let kept = desk_with_cue_mp3().await;
    let out = export_to(&kept, "with-cue", &encode_m4b()).await;
    let chapters = chapters_of(&out);
    assert_eq!(chapters.len(), 2, "{chapters:?}");
    assert_eq!(chapters[0].0.as_deref(), Some("Opening"));
    assert_eq!(chapters[0].1, 0);
    assert_eq!(chapters[1].0.as_deref(), Some("Near end"));
    assert_eq!(chapters[1].1, 1_480);
    assert!(chapters[1].2 > 1_480, "short last chapter remains");
    let (track_seconds, _, _) = timed_text_chapter_track(&out);
    assert!(
        track_seconds > 1.48 && track_seconds < 1.6,
        "chapter track spans the audio: {track_seconds}"
    );
    assert!(kept.residual_workspace_dirs().is_empty());

    let ignored = desk_with_cue_mp3().await;
    let input_id = ignored
        .engine
        .session_snapshot()
        .titles
        .expect("titles")
        .files[0]
        .input_id
        .clone();
    ignored
        .send(SessionIntent::SetTitleAudio {
            title_ids: vec![input_id.clone()],
            edit: AudioEdit::Format(AudiobookFormat::Mp3),
        })
        .await;
    ignored
        .wait_until("an accepted CUE refuses MP3 output", |desk| {
            matches!(desk.plan(), TitlePlan::Failed { .. })
        })
        .await;
    ignored
        .send(SessionIntent::ChooseCue {
            input_id,
            choice: CueChoice::Ignore,
        })
        .await;
    ignored
        .wait_until("ignoring the CUE permits MP3 pass-through", |desk| {
            matches!(desk.plan(), TitlePlan::Resolved { ref plan } if plan.handling.preserves())
        })
        .await;
    let out = export_to(&ignored, "ignored", &encode_m4b()).await;
    assert!(chapters_of(&out).is_empty());
}

#[tokio::test]
async fn an_m4b_with_only_uppercase_series_atoms_reads_and_exports_its_series() {
    use mp4ameta::{Data, FreeformIdent, Tag};
    let desk = Desk::new();
    let book = desk
        .audiobook(&desk.root.path().join("library/alpha.m4b"), 1.0)
        .await;
    let mut tag = Tag::read_from_path(&book).expect("read atoms");
    tag.set_data(
        FreeformIdent::new_static("com.apple.iTunes", "SERIES"),
        Data::Utf8("Dune".to_string()),
    );
    tag.set_data(
        FreeformIdent::new_static("com.apple.iTunes", "SERIES-PART"),
        Data::Utf8("2".to_string()),
    );
    tag.write_to_path(&book).expect("write atoms");
    desk.import(&book).await;
    assert_eq!(desk.shown(MetadataField::Series), "Dune");
    assert_eq!(desk.shown(MetadataField::SeriesPart), "2");
    assert_eq!(tags(&book).series.as_deref(), Some("Dune"));
    assert_eq!(tags(&book).series_part.as_deref(), Some("2"));

    let output = export_to(&desk, "uppercase", &encode_m4b()).await;
    let written = Tag::read_from_path(&output).expect("read output atoms");
    for (name, value) in [
        ("series", "Dune"),
        ("SERIES", "Dune"),
        ("series-part", "2"),
        ("SERIES-PART", "2"),
    ] {
        let ident = FreeformIdent::new_static("com.apple.iTunes", name);
        assert_eq!(written.strings_of(&ident).next(), Some(value), "{name}");
    }
}
