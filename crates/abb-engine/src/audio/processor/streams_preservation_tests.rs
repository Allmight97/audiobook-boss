use super::assess_preservation;
use abb_ffmpeg::CodecId as Id;

#[test]
fn unsupported_codec_or_container_cannot_preserve() {
    assert!(!assess_preservation("wav", "mov,mp4,m4a,3gp,3g2,mj2", Id::AAC,).can_preserve);
    assert!(!assess_preservation("wav", "wav", Id::MP3).can_preserve);
    assert!(!assess_preservation("mp3", "mp3", Id::AAC).can_preserve);
    assert!(!assess_preservation("mp3", "mp3", Id::MP3).can_preserve);
}

#[test]
fn a_source_keeps_its_codec_family_whether_or_not_it_can_be_copied() {
    use crate::audio::CodecFamily;
    let aac_in_mka = assess_preservation("mka", "matroska,webm", Id::AAC);
    assert!(!aac_in_mka.can_preserve);
    assert_eq!(aac_in_mka.codec_family, Some(CodecFamily::Aac));
    let opus_in_m4b = assess_preservation("m4b", "mov,mp4,m4a,3gp,3g2,mj2", Id::OPUS);
    assert!(opus_in_m4b.can_preserve);
    assert_eq!(opus_in_m4b.codec_family, Some(CodecFamily::Opus));
    assert_eq!(
        assess_preservation("mp3", "mp3", Id::MP3).codec_family,
        None
    );
}
