use super::assess_preservation;
use ffmpeg_next as ff;

#[test]
fn unsupported_codec_or_container_cannot_preserve() {
    assert!(
        !assess_preservation("wav", "mov,mp4,m4a,3gp,3g2,mj2", ff::codec::Id::AAC,).can_preserve
    );
    assert!(!assess_preservation("wav", "wav", ff::codec::Id::MP3).can_preserve);
    assert!(!assess_preservation("mp3", "mp3", ff::codec::Id::AAC).can_preserve);
}
