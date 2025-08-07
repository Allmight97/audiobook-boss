use audiobook_boss_lib::commands::{ping, echo, get_ffmpeg_version};

#[test]
fn test_ping() {
    let result = ping();
    assert!(result.is_ok());
    let value = result.expect("ping should succeed");
    assert_eq!(value, "pong");
}

#[test]
fn test_echo() {
    let test_string = "Hello, Tauri!".to_string();
    let result = echo(test_string.clone());
    assert!(result.is_ok());
    let value = result.expect("echo should succeed");
    assert_eq!(value, test_string);
}

#[test]
fn test_get_ffmpeg_version() {
    let result = get_ffmpeg_version();
    match result {
        Ok(v) => assert!(v.contains("ffmpeg version")),
        Err(e) => assert!(e.to_string().contains("not found")),
    }
}


