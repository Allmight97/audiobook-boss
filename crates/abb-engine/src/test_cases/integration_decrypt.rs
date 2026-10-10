//! Real AAX/AAXC decrypt through the published helper.
//!
//! Fixtures are synthesized at test time from a lavfi sine book. Compare the
//! helper output against that generated source (packets, duration, tags,
//! chapters), never against fixed hashes. A wrong AAX activation fails inside
//! the helper. A wrong AAXC key can still exit 0; materialization must reject
//! that output.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use aes::Aes128;
use cbc::cipher::{block_padding::NoPadding, BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use secrecy::SecretString;
use sha1::{Digest, Sha1};
use tempfile::TempDir;

use crate::errors::AppError;
use crate::remote_source::materializer::{
    AaxcleanLane, AaxcleanMaterializer, AaxcleanSecret, MaterializationRequest,
};
use crate::remote_source::{
    ProtectedMaterializationRun, RemoteAcquisitionFailureKind, RemoteSourceDiagnostic,
};

const AAXC_KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f";
const AAXC_IV_HEX: &str = "f0e0d0c0b0a090807060504030201000";
const AAX_ACTIVATION_HEX: &str = "1ceb00da";
const AAX_WRONG_ACTIVATION_HEX: &str = "deadbeef";
/// Public constant from FFmpeg libavformat/mov.c (`audible_fixed_key` default).
const AUDIBLE_FIXED_KEY: [u8; 16] = [
    0x77, 0x21, 0x4d, 0x4b, 0x19, 0x6a, 0x87, 0xcd, 0x52, 0x00, 0x45, 0xfd, 0x20, 0xa5, 0x1d, 0x67,
];

type Aes128CbcEnc = cbc::Encryptor<Aes128>;
type Aes128CbcDec = cbc::Decryptor<Aes128>;

struct DecryptBooks {
    _dir: TempDir,
    source: PathBuf,
    aax: PathBuf,
    aaxc: PathBuf,
}

fn books() -> &'static DecryptBooks {
    static BOOKS: OnceLock<DecryptBooks> = OnceLock::new();
    BOOKS.get_or_init(|| {
        let dir = TempDir::new().expect("decrypt fixture dir");
        let source = dir.path().join("source.m4b");
        write_source_book(&source);
        let plain = std::fs::read(&source).expect("read source book");
        let aaxc = dir.path().join("synthetic.aaxc");
        std::fs::write(&aaxc, encrypt_aaxc(&plain)).expect("write aaxc");
        let aax = dir.path().join("synthetic.aax");
        std::fs::write(&aax, encrypt_aax(&plain)).expect("write aax");
        DecryptBooks {
            source,
            aax,
            aaxc,
            _dir: dir,
        }
    })
}

fn write_source_book(path: &Path) {
    let dir = path.parent().expect("source parent");
    let meta = dir.join("meta.txt");
    let audio = dir.join("audio.m4a");
    let cover = dir.join("cover.jpg");
    std::fs::write(
        &meta,
        ";FFMETADATA1\n\
         title=ABB Synthetic Book\n\
         artist=ABB Test Author\n\
         album=ABB Synthetic Book\n\
         genre=Audiobook\n\
         comment=Synthetic AAX fixture generated from lavfi sine; public domain test data.\n\
         [CHAPTER]\nTIMEBASE=1/1000\nSTART=0\nEND=4000\ntitle=Chapter One\n\
         [CHAPTER]\nTIMEBASE=1/1000\nSTART=4000\nEND=8000\ntitle=Chapter Two\n\
         [CHAPTER]\nTIMEBASE=1/1000\nSTART=8000\nEND=12000\ntitle=Chapter Three\n",
    )
    .expect("write ffmetadata");

    ffmpeg(&[
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=44100:duration=12",
        "-i",
        meta.to_str().expect("meta path"),
        "-map",
        "0:a",
        "-map_metadata",
        "1",
        "-map_chapters",
        "1",
        "-c:a",
        "aac",
        "-b:a",
        "64k",
        "-ac",
        "2",
        "-use_editlist",
        "0",
        audio.to_str().expect("audio path"),
    ]);
    ffmpeg(&[
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=64x64:d=1",
        "-frames:v",
        "1",
        cover.to_str().expect("cover path"),
    ]);
    ffmpeg(&[
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-i",
        audio.to_str().expect("audio path"),
        "-i",
        cover.to_str().expect("cover path"),
        "-map",
        "0:a",
        "-map",
        "1:v",
        "-map_metadata",
        "0",
        "-map_chapters",
        "0",
        "-c",
        "copy",
        "-disposition:v",
        "attached_pic",
        "-use_editlist",
        "0",
        "-f",
        "ipod",
        path.to_str().expect("source path"),
    ]);
}

fn ffmpeg(args: &[&str]) {
    let binary = std::env::var("ABB_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string());
    let output = Command::new(&binary)
        .args(args)
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "ffmpeg must be on PATH or set via ABB_FFMPEG; spawning `{binary}` failed: {error}"
            )
        });
    assert!(
        output.status.success(),
        "ffmpeg failed: {}\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn encrypt_aaxc(src: &[u8]) -> Vec<u8> {
    let key = hex_16(AAXC_KEY_HEX);
    let iv = hex_16(AAXC_IV_HEX);
    encrypt_m4b(src, b"aaxc", &key, &iv, None)
}

fn encrypt_aax(src: &[u8]) -> Vec<u8> {
    let act = hex_bytes(AAX_ACTIVATION_HEX);
    let mut key_src = b"abb-synthetic-file-key".to_vec();
    key_src.extend_from_slice(&act);
    let key: [u8; 16] = sha1(&key_src)[..16].try_into().expect("file key");
    let (adrm, iv) = build_adrm(&act, &key);
    encrypt_m4b(src, b"aax ", &key, &iv, Some(&adrm))
}

fn encrypt_m4b(src: &[u8], brand: &[u8; 4], key: &[u8], iv: &[u8], adrm: Option<&[u8]>) -> Vec<u8> {
    let mut buf = src.to_vec();
    let top = boxes(&buf, 0, buf.len());
    let ftyp = *find_type(&top, b"ftyp");
    let moov = *find_type(&top, b"moov");
    let mdat = *find_type(&top, b"mdat");
    assert!(
        moov.pos > mdat.pos,
        "moov must follow mdat (do not use +faststart)"
    );
    let trak = audio_trak(&buf, moov);
    let chain = find_path(
        &buf,
        trak.pos + trak.hdr,
        trak.pos + trak.size,
        &[*b"mdia", *b"minf", *b"stbl"],
    )
    .expect("mdia/minf/stbl");
    let (ranges, stsd) = sample_ranges(&buf, chain[2]);
    let entry = stsd.pos + 16;
    assert_eq!(&buf[entry + 4..entry + 8], b"mp4a", "audio sample entry");

    for (off, size) in ranges {
        let n = size & !15;
        if n > 0 {
            let encrypted = aes128_cbc_encrypt(key, iv, &buf[off..off + n]);
            buf[off..off + n].copy_from_slice(&encrypted);
        }
    }

    buf[ftyp.pos + 8..ftyp.pos + 12].copy_from_slice(brand);
    buf[entry + 4..entry + 8].copy_from_slice(b"aavd");

    if let Some(adrm) = adrm {
        let insert_at = entry + 8 + 28;
        for pos in [
            entry,
            stsd.pos,
            chain[2].pos,
            chain[1].pos,
            chain[0].pos,
            trak.pos,
            moov.pos,
        ] {
            let size = read_u32(&buf, pos) + u32::try_from(adrm.len()).expect("adrm size");
            write_u32(&mut buf, pos, size);
        }
        buf.splice(insert_at..insert_at, adrm.iter().copied());
    }
    buf
}

fn build_adrm(activation_bytes: &[u8], file_key: &[u8]) -> (Vec<u8>, [u8; 16]) {
    let mut ik_src = Vec::from(AUDIBLE_FIXED_KEY);
    ik_src.extend_from_slice(activation_bytes);
    let ik = sha1(&ik_src);
    let mut iiv_src = Vec::from(AUDIBLE_FIXED_KEY);
    iiv_src.extend_from_slice(&ik);
    iiv_src.extend_from_slice(activation_bytes);
    let iiv = sha1(&iiv_src);
    let mut checksum_src = ik[..16].to_vec();
    checksum_src.extend_from_slice(&iiv[..16]);
    let checksum = sha1(&checksum_src);

    let mut plain = [0u8; 56];
    for i in 0..4 {
        plain[i] = activation_bytes[3 - i];
    }
    plain[8..24].copy_from_slice(file_key);
    let seed = sha1(b"abb-synthetic-iv-seed");
    plain[26..42].copy_from_slice(&seed[..16]);
    let mut blob = aes128_cbc_encrypt(&ik[..16], &iiv[..16], &plain[..48]);
    blob.extend_from_slice(&plain[48..]);

    let mut iv_src = plain[26..42].to_vec();
    iv_src.extend_from_slice(file_key);
    iv_src.extend_from_slice(&AUDIBLE_FIXED_KEY);
    let file_iv: [u8; 16] = sha1(&iv_src)[..16].try_into().expect("file iv");

    let mut body = vec![0u8; 8];
    body.extend_from_slice(&blob);
    body.extend_from_slice(&[0u8; 4]);
    body.extend_from_slice(&checksum);
    body.extend_from_slice(&[0u8; 4]);
    let mut box_bytes = Vec::with_capacity(8 + body.len());
    box_bytes.extend_from_slice(
        &u32::try_from(8 + body.len())
            .expect("adrm box")
            .to_be_bytes(),
    );
    box_bytes.extend_from_slice(b"adrm");
    box_bytes.extend_from_slice(&body);
    (box_bytes, file_iv)
}

#[derive(Clone, Copy)]
struct Mp4Box {
    pos: usize,
    size: usize,
    typ: [u8; 4],
    hdr: usize,
}

fn boxes(buf: &[u8], start: usize, end: usize) -> Vec<Mp4Box> {
    let mut found = Vec::new();
    let mut pos = start;
    while pos + 8 <= end {
        let mut size = read_u32(buf, pos) as usize;
        let mut typ = [0u8; 4];
        typ.copy_from_slice(&buf[pos + 4..pos + 8]);
        let mut hdr = 8usize;
        if size == 1 {
            if pos + 16 > end {
                break;
            }
            size =
                u64::from_be_bytes(buf[pos + 8..pos + 16].try_into().expect("wide size")) as usize;
            hdr = 16;
        } else if size == 0 {
            size = end - pos;
        }
        if size < hdr || pos + size > end {
            break;
        }
        found.push(Mp4Box {
            pos,
            size,
            typ,
            hdr,
        });
        pos += size;
    }
    found
}

fn find_type<'a>(top: &'a [Mp4Box], typ: &[u8; 4]) -> &'a Mp4Box {
    top.iter().find(|item| &item.typ == typ).expect("mp4 box")
}

fn find_path(
    buf: &[u8],
    mut start: usize,
    mut end: usize,
    path: &[[u8; 4]],
) -> Option<Vec<Mp4Box>> {
    let mut chain = Vec::new();
    for name in path {
        let found = boxes(buf, start, end)
            .into_iter()
            .find(|item| &item.typ == name)?;
        start = found.pos + found.hdr;
        end = found.pos + found.size;
        chain.push(found);
    }
    Some(chain)
}

fn audio_trak(buf: &[u8], moov: Mp4Box) -> Mp4Box {
    for trak in boxes(buf, moov.pos + moov.hdr, moov.pos + moov.size) {
        if trak.typ != *b"trak" {
            continue;
        }
        let Some(hdlr) = find_path(
            buf,
            trak.pos + trak.hdr,
            trak.pos + trak.size,
            &[*b"mdia", *b"hdlr"],
        ) else {
            continue;
        };
        let handler = hdlr.last().expect("hdlr");
        if &buf[handler.pos + 16..handler.pos + 20] == b"soun" {
            return trak;
        }
    }
    panic!("no sound track in synthetic book");
}

fn sample_ranges(buf: &[u8], stbl: Mp4Box) -> (Vec<(usize, usize)>, Mp4Box) {
    let start = stbl.pos + stbl.hdr;
    let end = stbl.pos + stbl.size;
    let tab: HashMap<[u8; 4], Mp4Box> = boxes(buf, start, end)
        .into_iter()
        .map(|item| (item.typ, item))
        .collect();
    let stsz = *tab.get(b"stsz").expect("stsz");
    let stsc = *tab.get(b"stsc").expect("stsc");
    let stsd = *tab.get(b"stsd").expect("stsd");
    let sizes = sample_sizes(buf, stsz);
    let offsets = chunk_offsets(buf, &tab);
    let stsc_entries = stsc_entries(buf, stsc);
    let mut ranges = Vec::new();
    let mut sample = 0usize;
    for (chunk_index, mut offset) in offsets.into_iter().enumerate() {
        let samples_per_chunk = stsc_entries
            .iter()
            .rev()
            .find(|entry| entry.0 <= (chunk_index + 1) as u32)
            .map(|entry| entry.1)
            .expect("stsc samples_per_chunk");
        for _ in 0..samples_per_chunk {
            let size = sizes[sample];
            ranges.push((offset, size));
            offset += size;
            sample += 1;
        }
    }
    assert_eq!(sample, sizes.len(), "stsc covered every sample");
    (ranges, stsd)
}

fn sample_sizes(buf: &[u8], stsz: Mp4Box) -> Vec<usize> {
    let pos = stsz.pos + 12;
    let fixed = read_u32(buf, pos);
    let count = read_u32(buf, pos + 4) as usize;
    if fixed != 0 {
        return vec![fixed as usize; count];
    }
    (0..count)
        .map(|index| read_u32(buf, pos + 8 + 4 * index) as usize)
        .collect()
}

fn chunk_offsets(buf: &[u8], tab: &HashMap<[u8; 4], Mp4Box>) -> Vec<usize> {
    if let Some(stco) = tab.get(b"stco") {
        let pos = stco.pos + 12;
        let count = read_u32(buf, pos) as usize;
        return (0..count)
            .map(|index| read_u32(buf, pos + 4 + 4 * index) as usize)
            .collect();
    }
    let co64 = tab.get(b"co64").expect("stco or co64");
    let pos = co64.pos + 12;
    let count = read_u32(buf, pos) as usize;
    (0..count)
        .map(|index| {
            let at = pos + 4 + 8 * index;
            u64::from_be_bytes(buf[at..at + 8].try_into().expect("co64")) as usize
        })
        .collect()
}

fn stsc_entries(buf: &[u8], stsc: Mp4Box) -> Vec<(u32, u32, u32)> {
    let pos = stsc.pos + 12;
    let count = read_u32(buf, pos) as usize;
    (0..count)
        .map(|index| {
            let at = pos + 4 + 12 * index;
            (
                read_u32(buf, at),
                read_u32(buf, at + 4),
                read_u32(buf, at + 8),
            )
        })
        .collect()
}

fn aes128_cbc_encrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len() % 16, 0);
    let mut buf = data.to_vec();
    let len = buf.len();
    Aes128CbcEnc::new_from_slices(key, iv)
        .expect("aes key/iv")
        .encrypt_padded::<NoPadding>(&mut buf, len)
        .expect("encrypt")
        .to_vec()
}

fn aes128_cbc_decrypt(key: &[u8], iv: &[u8], data: &[u8]) -> Vec<u8> {
    assert_eq!(data.len() % 16, 0);
    let mut buf = data.to_vec();
    Aes128CbcDec::new_from_slices(key, iv)
        .expect("aes key/iv")
        .decrypt_padded::<NoPadding>(&mut buf)
        .expect("decrypt")
        .to_vec()
}

/// AAXClean rejects a decrypted frame whose first 12 bits are all ones
/// (`AacValidateFilter`). Frames shorter than 16 bytes are checked as stored.
enum AaxcFrameCheck {
    Encrypted([u8; 16]),
    Clear(Vec<u8>),
}

fn aaxc_frame_checks(aaxc: &[u8]) -> Vec<AaxcFrameCheck> {
    let top = boxes(aaxc, 0, aaxc.len());
    let moov = *find_type(&top, b"moov");
    let trak = audio_trak(aaxc, moov);
    let chain = find_path(
        aaxc,
        trak.pos + trak.hdr,
        trak.pos + trak.size,
        &[*b"mdia", *b"minf", *b"stbl"],
    )
    .expect("mdia/minf/stbl");
    let (ranges, _) = sample_ranges(aaxc, chain[2]);
    let mut seen = HashSet::new();
    let mut frames = Vec::new();
    for (off, size) in ranges {
        if size >= 16 {
            let mut block = [0u8; 16];
            block.copy_from_slice(&aaxc[off..off + 16]);
            if seen.insert(block) {
                frames.push(AaxcFrameCheck::Encrypted(block));
            }
        } else {
            frames.push(AaxcFrameCheck::Clear(aaxc[off..off + size].to_vec()));
        }
    }
    frames
}

fn aaxclean_rejects_sync_word(frame: &[u8]) -> bool {
    frame.len() < 2 || (u16::from_be_bytes([frame[0], frame[1]]) & 0xfff0) == 0xfff0
}

fn aaxclean_frame_check_accepts(frames: &[AaxcFrameCheck], key: &[u8; 16], iv: &[u8; 16]) -> bool {
    frames.iter().all(|frame| match frame {
        AaxcFrameCheck::Clear(raw) => !aaxclean_rejects_sync_word(raw),
        AaxcFrameCheck::Encrypted(block) => {
            let plain = aes128_cbc_decrypt(key, iv, block);
            !aaxclean_rejects_sync_word(&plain)
        }
    })
}

fn increment_key(key: &mut [u8; 16]) {
    for byte in key.iter_mut().rev() {
        *byte = byte.wrapping_add(1);
        if *byte != 0 {
            return;
        }
    }
}

/// A key other than the fixture key for which no decrypted frame starts with
/// 12 one-bits, so AAXClean's frame check lets the helper exit 0.
fn wrong_aaxc_key_hex(aaxc: &[u8]) -> String {
    let frames = aaxc_frame_checks(aaxc);
    assert!(!frames.is_empty(), "synthetic AAXC has audio frames");
    let iv = hex_16(AAXC_IV_HEX);
    let right = hex_16(AAXC_KEY_HEX);
    let mut key = [0xff_u8; 16];
    for _ in 0..4096 {
        if key != right && aaxclean_frame_check_accepts(&frames, &key, &iv) {
            return hex_encode(&key);
        }
        increment_key(&mut key);
    }
    panic!("no wrong AAXC key passed AAXClean's frame check for this fixture");
}

fn hex_encode(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn sha1(data: &[u8]) -> [u8; 20] {
    Sha1::digest(data).into()
}

fn hex_bytes(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).expect("hex"))
        .collect()
}

fn hex_16(hex: &str) -> [u8; 16] {
    hex_bytes(hex).try_into().expect("16-byte hex")
}

fn read_u32(buf: &[u8], pos: usize) -> u32 {
    u32::from_be_bytes(buf[pos..pos + 4].try_into().expect("u32"))
}

fn write_u32(buf: &mut [u8], pos: usize, value: u32) {
    buf[pos..pos + 4].copy_from_slice(&value.to_be_bytes());
}

fn audio_packet_bytes(path: &Path) -> Vec<Vec<u8>> {
    let mut input = ffmpeg_next::format::input(path).expect("open for packets");
    let index = input
        .streams()
        .best(ffmpeg_next::media::Type::Audio)
        .expect("audio stream")
        .index();
    input
        .packets()
        .filter(|(stream, _)| stream.index() == index)
        .map(|(_, packet)| packet.data().expect("packet data").to_vec())
        .collect()
}

fn ffprobe_json(path: &Path, extra: &[&str]) -> serde_json::Value {
    let binary = std::env::var("ABB_FFPROBE").unwrap_or_else(|_| "ffprobe".to_string());
    let output = Command::new(&binary)
        .args(["-v", "quiet", "-print_format", "json"])
        .args(extra)
        .arg(path)
        .output()
        .unwrap_or_else(|error| {
            panic!("ffprobe must be on PATH or set via ABB_FFPROBE; spawning `{binary}` failed: {error}")
        });
    assert!(
        output.status.success(),
        "ffprobe failed for {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("ffprobe json")
}

fn tag_ci(tags: &serde_json::Map<String, serde_json::Value>, key: &str) -> Option<String> {
    tags.iter()
        .find(|(tag_key, _)| tag_key.eq_ignore_ascii_case(key))
        .and_then(|(_, value)| value.as_str().map(str::to_string))
}

fn assert_matches_source(output: &Path, source: &Path) {
    assert_eq!(
        audio_packet_bytes(output),
        audio_packet_bytes(source),
        "audio packets must match the generated source"
    );

    let out_format = ffprobe_json(output, &["-show_format", "-show_chapters"]);
    let src_format = ffprobe_json(source, &["-show_format", "-show_chapters"]);
    let out_duration: f64 = out_format["format"]["duration"]
        .as_str()
        .expect("output duration")
        .parse()
        .expect("output duration f64");
    let src_duration: f64 = src_format["format"]["duration"]
        .as_str()
        .expect("source duration")
        .parse()
        .expect("source duration f64");
    assert!(
        (out_duration - src_duration).abs() < 0.05,
        "duration {out_duration} vs {src_duration}"
    );

    let out_tags = out_format["format"]["tags"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    let src_tags = src_format["format"]["tags"]
        .as_object()
        .cloned()
        .unwrap_or_default();
    for key in ["title", "artist", "album", "genre"] {
        assert_eq!(tag_ci(&out_tags, key), tag_ci(&src_tags, key), "tag {key}");
    }

    let out_chapters = out_format["chapters"].as_array().expect("output chapters");
    let src_chapters = src_format["chapters"].as_array().expect("source chapters");
    assert_eq!(out_chapters.len(), src_chapters.len(), "chapter count");
    for (out_chapter, src_chapter) in out_chapters.iter().zip(src_chapters) {
        assert_eq!(
            tag_ci(
                out_chapter["tags"]
                    .as_object()
                    .unwrap_or(&serde_json::Map::new()),
                "title"
            ),
            tag_ci(
                src_chapter["tags"]
                    .as_object()
                    .unwrap_or(&serde_json::Map::new()),
                "title"
            )
        );
        assert_eq!(out_chapter["start_time"], src_chapter["start_time"]);
        assert_eq!(out_chapter["end_time"], src_chapter["end_time"]);
    }
}

/// Runs the helper through the materializer and returns whether it exited 0
/// with a non-empty result. The temp output is dropped with this call.
async fn helper_accepts(
    lane: AaxcleanLane,
    input: PathBuf,
    secret: AaxcleanSecret,
) -> Result<(), AppError> {
    let root = TempDir::new().expect("materialize root");
    let output = root.path().join("book.m4b");
    let partial = root.path().join("book.m4b.partial");
    let materializer = AaxcleanMaterializer::new_for_helper(None);
    materializer
        .materialize(
            MaterializationRequest {
                job_id: "decrypt-job".into(),
                operation_id: "decrypt-op".into(),
                lane,
                input_path: input,
                output_temp_path: partial,
                output_path: output,
                secret,
            },
            |_| {},
            || false,
        )
        .await
        .map(|_| ())
}

fn aaxc_secret() -> AaxcleanSecret {
    aaxc_secret_with(AAXC_KEY_HEX)
}

fn aaxc_secret_with(key_hex: &str) -> AaxcleanSecret {
    AaxcleanSecret::Aaxc {
        key_hex: SecretString::from(key_hex),
        iv_hex: SecretString::from(AAXC_IV_HEX),
    }
}

async fn materialize_protected(
    source: &Path,
    download_name: &str,
    secret: AaxcleanSecret,
) -> Result<
    (TempDir, ProtectedMaterializationRun),
    (RemoteSourceDiagnostic, TempDir, ProtectedMaterializationRun),
> {
    let root = TempDir::new().expect("protected download root");
    let input = root.path().join(download_name);
    std::fs::copy(source, &input).expect("copy protected download");
    match crate::remote_source::run_protected_materialization(&input, root.path(), &secret).await {
        Ok(run) => Ok((root, run)),
        Err((diagnostic, run)) => Err((diagnostic, root, run)),
    }
}

fn aax_secret(activation: &str) -> AaxcleanSecret {
    AaxcleanSecret::Aax {
        activation_bytes_hex: SecretString::from(activation),
    }
}

#[tokio::test]
async fn aax_decrypts_to_the_generated_source() {
    let books = books();
    let (_root, run) =
        materialize_protected(&books.aax, "source.aax", aax_secret(AAX_ACTIVATION_HEX))
            .await
            .unwrap_or_else(|(diagnostic, _, _)| {
                panic!("aax decrypt failed: {}", diagnostic.message)
            });
    assert_matches_source(&run.output, &books.source);
}

#[tokio::test]
async fn aaxc_decrypts_to_the_generated_source() {
    let books = books();
    let (_root, run) = materialize_protected(&books.aaxc, "source.aaxc", aaxc_secret())
        .await
        .unwrap_or_else(|(diagnostic, _, _)| panic!("aaxc decrypt failed: {}", diagnostic.message));
    assert_matches_source(&run.output, &books.source);
}

#[tokio::test]
async fn wrong_aax_activation_bytes_fail_and_leave_no_output() {
    let books = books();
    let (diagnostic, _root, run) = materialize_protected(
        &books.aax,
        "source.aax",
        aax_secret(AAX_WRONG_ACTIVATION_HEX),
    )
    .await
    .expect_err("wrong activation bytes must fail");
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::MaterializationFailed
    );
    assert!(
        diagnostic.message.contains("conversion_failed"),
        "wrong activation bytes must fail as conversion_failed, not a missing/stub helper: {}",
        diagnostic.message
    );
    assert!(!run.output.exists(), "no committed output");
    assert!(!run.partial.exists(), "no partial output");
}

#[tokio::test]
async fn wrong_aaxc_key_fails_at_materialization_when_helper_accepts_it() {
    let books = books();
    let aaxc = std::fs::read(&books.aaxc).expect("read aaxc fixture");
    let key = wrong_aaxc_key_hex(&aaxc);
    assert_ne!(
        key, AAXC_KEY_HEX,
        "the searched key must not be the real key"
    );

    // The helper still exits 0: AAXClean's frame check does not know this key is wrong.
    helper_accepts(
        AaxcleanLane::Aaxc,
        books.aaxc.clone(),
        aaxc_secret_with(&key),
    )
    .await
    .expect("helper must exit 0 for a wrong key its frame check accepts");

    let (diagnostic, _root, run) =
        materialize_protected(&books.aaxc, "source.aaxc", aaxc_secret_with(&key))
            .await
            .expect_err(
                "wrong AAXC key must fail at materialization, not return Ok for later validation",
            );
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::MaterializationFailed
    );
    assert!(
        diagnostic.message.contains("decrypt"),
        "message must name the decrypt: {}",
        diagnostic.message
    );
    assert!(
        diagnostic.message.contains("decodable audio"),
        "message must say the decrypt output is not audio: {}",
        diagnostic.message
    );
    assert!(
        !diagnostic.message.to_ascii_lowercase().contains(&key),
        "message must not include the key: {}",
        diagnostic.message
    );
    assert!(
        !diagnostic
            .message
            .to_ascii_lowercase()
            .contains(AAXC_IV_HEX),
        "message must not include the iv: {}",
        diagnostic.message
    );
    assert!(
        !diagnostic.message.contains('/'),
        "message must not include a path: {}",
        diagnostic.message
    );
    assert!(!run.output.exists(), "no committed m4b");
    assert!(!run.partial.exists(), "no partial");
}

#[tokio::test]
async fn cancel_kills_the_helper_and_leaves_no_output() {
    let books = books();
    let root = TempDir::new().expect("cancel root");
    let output = root.path().join("book.m4b");
    let partial = root.path().join("book.m4b.partial");
    let materializer = AaxcleanMaterializer::new_for_helper(None);
    let saw_progress = AtomicBool::new(false);
    let result = materializer
        .materialize(
            MaterializationRequest {
                job_id: "decrypt-cancel".into(),
                operation_id: "decrypt-cancel-op".into(),
                lane: AaxcleanLane::Aaxc,
                input_path: books.aaxc.clone(),
                output_temp_path: partial.clone(),
                output_path: output.clone(),
                secret: aaxc_secret(),
            },
            |_| {
                saw_progress.store(true, Ordering::SeqCst);
            },
            || saw_progress.load(Ordering::SeqCst),
        )
        .await;
    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "cancel should stop decrypt, got {result:?}"
    );
    assert!(!output.exists(), "no committed output");
    assert!(!partial.exists(), "no partial output");
}
