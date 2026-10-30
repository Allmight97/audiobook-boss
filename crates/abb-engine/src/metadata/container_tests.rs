use super::{
    classify_format_name, is_mp4_family_format, primary_format_name, remux_output_format_for,
    ContainerRoute,
};

#[test]
fn classifies_ffmpeg_mp4_family_format_name() {
    assert!(is_mp4_family_format("mov,mp4,m4a,3gp,3g2,mj2"));
}

#[test]
fn keeps_non_mp4_format_name_for_forced_remux_output() {
    assert!(!is_mp4_family_format("mp3"));
    assert_eq!(primary_format_name("matroska,webm"), "matroska");
}

#[test]
fn maps_demuxer_names_to_writable_remux_output_formats() {
    assert_eq!(remux_output_format_for("aac"), "adts");
    assert_eq!(remux_output_format_for("mp3"), "mp3");
    assert_eq!(remux_output_format_for("wav"), "wav");
    assert_eq!(remux_output_format_for("flac"), "flac");
}

#[test]
fn classifies_format_name_without_reopening_path() {
    assert_eq!(
        classify_format_name("mov,mp4,m4a,3gp,3g2,mj2"),
        ContainerRoute::Mp4Family
    );
    assert_eq!(
        classify_format_name("mp3"),
        ContainerRoute::Other {
            remux_output_format: "mp3".to_string()
        }
    );
}
