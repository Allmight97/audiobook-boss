use std::fs;

use abb_engine::session::{MetadataField, SessionIntent};

use super::integration_import_variants_tests::files_under;
use super::integration_session_desk::Desk;
use super::media_fixtures::{chapters_of, write_sine_mp3};

fn cue_sheet(tracks: u32, frames_per_track: u32) -> String {
    let mut sheet = String::from("FILE \"book.mp3\" MP3\n");
    for track in 0..tracks {
        let frames = track * frames_per_track;
        sheet.push_str(&format!(
            "TRACK {:02} AUDIO\nTITLE \"Chapter {}\"\nINDEX 01 {:02}:{:02}:{:02}\n",
            track + 1,
            track + 1,
            frames / (75 * 60),
            (frames / 75) % 60,
            frames % 75
        ));
    }
    sheet
}

#[tokio::test]
async fn five_hundred_cue_chapters_survive_export() {
    let desk = Desk::new();
    let source = desk.root.path().join("src");
    fs::create_dir_all(&source).expect("source folder");
    write_sine_mp3(&source.join("book.mp3"), 60.0, 440.0);
    fs::write(source.join("book.cue"), cue_sheet(500, 9)).expect("write the CUE");
    desk.import(&source.join("book.mp3")).await;
    desk.send(SessionIntent::SetField {
        field: MetadataField::Title,
        value: "Cue Heavy".to_string(),
    })
    .await;

    desk.export().await;
    desk.wait_until("the book is published", |desk| {
        !files_under(&desk.root.path().join("exports")).is_empty()
    })
    .await;
    let output = files_under(&desk.root.path().join("exports")).remove(0);

    let chapters = chapters_of(&output);
    assert_eq!(chapters.len(), 500);
    assert_eq!(chapters[0].0.as_deref(), Some("Chapter 1"));
    assert_eq!(chapters[499].0.as_deref(), Some("Chapter 500"));
    assert_eq!(chapters[499].1, 499 * 120);
}
