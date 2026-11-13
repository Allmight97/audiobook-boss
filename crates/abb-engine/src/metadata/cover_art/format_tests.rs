use super::*;

fn png_header() -> Vec<u8> {
    vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
}

#[test]
fn detects_only_jpeg_and_png_cover_bytes() {
    assert_eq!(
        detect_cover_art_format(&[0xFF, 0xD8, 0xFF]),
        Some(CoverFormat::Jpeg)
    );
    assert_eq!(
        detect_cover_art_format(&png_header()),
        Some(CoverFormat::Png)
    );
    assert_eq!(detect_cover_art_format(b"GIF89a"), None);
    assert_eq!(detect_cover_art_format(b"RIFF\x00\x00\x00\x00WEBP"), None);
}

fn encoded(format: image::ImageFormat) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::DynamicImage::new_rgb8(5, 3)
        .write_to(&mut std::io::Cursor::new(&mut bytes), format)
        .expect("encode fixture");
    bytes
}

fn bmp(header_size: i32, size: &[u8]) -> Vec<u8> {
    let mut bytes = b"BM".to_vec();
    bytes.resize(14, 0);
    bytes.extend(header_size.to_le_bytes());
    bytes.extend(size);
    bytes
}

fn tiff(big_endian: bool, value_type: u16) -> Vec<u8> {
    let u16_bytes = |value: u16| {
        if big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        }
    };
    let u32_bytes = |value: u32| {
        if big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        }
    };
    let mut bytes = if big_endian {
        b"MM\0*".to_vec()
    } else {
        b"II*\0".to_vec()
    };
    bytes.extend(u32_bytes(8));
    bytes.extend(u16_bytes(3));
    for (tag, value) in [(259, 1), (256, 5), (257, 3)] {
        bytes.extend(u16_bytes(tag));
        bytes.extend(u16_bytes(value_type));
        bytes.extend(u32_bytes(1));
        if value_type == 3 {
            bytes.extend(u16_bytes(value as u16));
            bytes.extend([0, 0]);
        } else {
            bytes.extend(u32_bytes(value));
        }
    }
    bytes
}

fn pictures_of(codec: &str) -> Option<Vec<Vec<u8>>> {
    Some(match codec {
        "MJPEG" => vec![encoded(image::ImageFormat::Jpeg)],
        "PNG" => vec![encoded(image::ImageFormat::Png)],
        "WEBP" => vec![encoded(image::ImageFormat::WebP)],
        "GIF" => vec![[b"GIF89a".as_slice(), &[5, 0, 3, 0]].concat()],
        "BMP" => vec![
            bmp(40, &[5_i32.to_le_bytes(), (-3_i32).to_le_bytes()].concat()),
            bmp(12, &[5, 0, 3, 0]),
        ],
        "TIFF" => vec![tiff(false, 3), tiff(false, 4), tiff(true, 3), tiff(true, 4)],
        _ => return None,
    })
}

const UNREAD_PICTURE_CODECS: [&str; 1] = ["JPEGXL"];

/// `ff_id3v2_mime_tags` (`libavformat/id3v2/reader.c`) maps an ID3 or FLAC
/// picture's MIME type to its codec.
fn ffmpeg_id3_picture_codecs() -> Vec<String> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let pin = std::fs::read_to_string(
        manifest.join("../../vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision"),
    )
    .expect("read ffmpeg-revision");
    let exe = std::env::current_exe().expect("test binary path");
    let cache = exe
        .ancestors()
        .map(|dir| dir.join("abb-ffmpeg-cache"))
        .find(|dir| dir.is_dir())
        .expect("no abb-ffmpeg-cache beside the test binary; build with bundled-ffmpeg");
    let source = std::fs::read_dir(&cache)
        .expect("list abb-ffmpeg-cache")
        .flatten()
        .flat_map(|tree| {
            std::fs::read_dir(tree.path())
                .into_iter()
                .flatten()
                .flatten()
        })
        .map(|entry| entry.path())
        .find(|dir| {
            std::fs::read_to_string(dir.join(".git/HEAD"))
                .is_ok_and(|head| head.trim() == pin.trim())
        })
        .unwrap_or_else(|| {
            panic!(
                "no FFmpeg checkout of {} in {}",
                pin.trim(),
                cache.display()
            )
        });
    let id3v2 =
        std::fs::read_to_string(source.join("libavformat/id3v2/reader.c")).expect("id3v2/reader.c");
    let table = id3v2
        .split("ff_id3v2_mime_tags[] = {")
        .nth(1)
        .and_then(|rest| rest.split("};").next())
        .expect("ff_id3v2_mime_tags in id3v2/reader.c");
    let mut codecs: Vec<String> = table
        .split("AV_CODEC_ID_")
        .skip(1)
        .filter_map(|rest| rest.split(|c: char| !c.is_ascii_alphanumeric()).next())
        .filter(|codec| *codec != "NONE")
        .map(str::to_string)
        .collect();
    codecs.sort();
    codecs.dedup();
    codecs
}

#[test]
fn reads_width_then_height_of_every_picture_type_an_id3_or_flac_tag_names() {
    let codecs = ffmpeg_id3_picture_codecs();
    assert!(codecs.contains(&"MJPEG".to_string()), "{codecs:?}");
    for codec in codecs {
        if UNREAD_PICTURE_CODECS.contains(&codec.as_str()) {
            continue;
        }
        let pictures = pictures_of(&codec).unwrap_or_else(|| {
            panic!(
                "FFmpeg's ID3 picture table names {codec}: teach picture_dimensions its \
                 header, or list it in UNREAD_PICTURE_CODECS, CHANGELOG and ABB-PROVENANCE.md \
                 as a cover that blocks a Save"
            )
        });
        for (variant, bytes) in pictures.iter().enumerate() {
            assert_eq!(
                picture_dimensions(bytes),
                Some((5, 3)),
                "{codec} variant {variant}"
            );
        }
    }
}

#[test]
fn a_truncated_or_unknown_picture_has_no_size() {
    let mut truncated_tiff = tiff(false, 3);
    truncated_tiff.truncate(truncated_tiff.len() - 4);
    let mut tiff_past_its_end = tiff(true, 4);
    tiff_past_its_end[4..8].copy_from_slice(&u32::MAX.to_be_bytes());
    for (name, bytes) in [
        ("gif with no width", b"GIF89a\0\0\x03\0".to_vec()),
        ("truncated tiff", truncated_tiff),
        ("tiff ifd past the end", tiff_past_its_end),
        ("unknown", b"not a picture".to_vec()),
    ] {
        assert_eq!(picture_dimensions(&bytes), None, "{name}");
    }
}
