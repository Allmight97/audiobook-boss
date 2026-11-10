//! Failure paths that need no real media. Media-backed failure paths (wrong
//! key shape, stop after N packets) live in the engine beside its encrypted
//! fixtures.

use abb_ffmpeg::{Error, Input, Output, SecretOptions, Step, Tags};
use tempfile::TempDir;

fn open_error(path: &std::path::Path, options: &SecretOptions) -> Error {
    match Input::open(path, options) {
        Ok(_) => panic!("the input opened"),
        Err(error) => error,
    }
}

#[test]
fn a_missing_input_is_an_open_error() {
    let dir = TempDir::new().expect("dir");

    let error = open_error(&dir.path().join("missing.aax"), &SecretOptions::default());

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::OpenInput,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!error.to_string().contains('/'), "no path in {error}");
}

#[test]
fn an_input_that_is_not_media_is_an_open_error() {
    let dir = TempDir::new().expect("dir");
    let junk = dir.path().join("junk.aax");
    std::fs::write(&junk, vec![0x5a_u8; 4096]).expect("write junk");
    let mut options = SecretOptions::default();
    options
        .push("activation_bytes", "1ceb00da")
        .expect("option");

    let error = open_error(&junk, &options);

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::OpenInput,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(
        !error.to_string().contains("1ceb00da"),
        "no option value in {error}"
    );
}

#[test]
fn an_option_value_with_a_nul_is_refused_without_echoing_it() {
    let mut options = SecretOptions::default();

    let error = options
        .push("audible_key", "00\0secret")
        .expect_err("refused");

    assert_eq!(
        error,
        Error::NulInText {
            step: Step::OpenInput
        }
    );
    assert!(!error.to_string().contains("secret"));
}

#[test]
fn an_output_in_a_folder_that_cannot_exist_is_an_open_error_and_leaves_no_file() {
    let dir = TempDir::new().expect("dir");
    let not_a_folder = dir.path().join("file");
    std::fs::write(&not_a_folder, b"x").expect("write file");
    let output = not_a_folder.join("book.m4b");

    let error = match Output::create(&output, "mp4") {
        Ok(_) => panic!("the output opened"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::OpenOutputFile,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!output.exists());
}

#[test]
fn an_unknown_muxer_is_a_create_error_and_leaves_no_file() {
    let dir = TempDir::new().expect("dir");
    let output = dir.path().join("book.m4b");

    let error = match Output::create(&output, "no-such-muxer") {
        Ok(_) => panic!("the output opened"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::CreateOutput,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!output.exists());
}

#[test]
fn an_output_dropped_before_its_header_removes_its_file() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("book.m4b");
    let output = Output::create(&path, "mp4").expect("create");
    assert!(path.exists(), "FFmpeg opened the file");

    drop(output);

    assert!(!path.exists());
}

#[test]
fn a_header_with_no_streams_fails_and_removes_its_file() {
    let dir = TempDir::new().expect("dir");
    let path = dir.path().join("book.m4b");
    let output = Output::create(&path, "mp4").expect("create");

    let error = match output.write_header(&Tags::default()) {
        Ok(_) => panic!("a header with no streams was written"),
        Err(error) => error,
    };

    assert!(
        matches!(
            error,
            Error::Ffmpeg {
                step: Step::WriteHeader,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(!path.exists());
}
