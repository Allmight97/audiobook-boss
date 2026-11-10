//! Real AAX/AAXC decrypt through the bundled FFmpeg.
//!
//! Fixtures are synthesized at test time from a lavfi sine book. Compare the
//! decrypt output against that generated source (packets, duration, tags,
//! chapters, cover), never against fixed hashes. A wrong AAX activation fails
//! at open. A wrong AAXC key opens and copies noise; materialization must
//! reject that output.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use aes::Aes128;
use cbc::cipher::{block_padding::NoPadding, BlockModeEncrypt, KeyIvInit};
use secrecy::SecretString;
use sha1::{Digest, Sha1};
use tempfile::TempDir;

use super::super::decrypt::{stream_copy, CopyControl};
use super::{
    materialize_protected_download, staged_materialized_path, AudibleAcquisitionError,
    TitleAcquisitionCtx, TitleProgressContext,
};
use crate::errors::AppError;
use crate::remote_source::{RemoteAcquisitionFailureKind, RemoteSourceDiagnostic};
use crate::test_cases::media_fixtures::{audio_packet_bytes, ffmpeg, ffprobe, ffprobe_tag_ci};
use abb_audible_core::AudibleDecryptionMaterial;

const AAXC_KEY_HEX: &str = "000102030405060708090a0b0c0d0e0f";
const AAXC_IV_HEX: &str = "f0e0d0c0b0a090807060504030201000";
const AAX_ACTIVATION_HEX: &str = "1ceb00da";
const AAX_WRONG_ACTIVATION_HEX: &str = "deadbeef";
const WRONG_AAXC_KEY_HEX: &str = "ffffffffffffffffffffffffffffffff";
/// Public constant from FFmpeg libavformat/mov.c (`audible_fixed_key` default).
const AUDIBLE_FIXED_KEY: [u8; 16] = [
    0x77, 0x21, 0x4d, 0x4b, 0x19, 0x6a, 0x87, 0xcd, 0x52, 0x00, 0x45, 0xfd, 0x20, 0xa5, 0x1d, 0x67,
];

type Aes128CbcEnc = cbc::Encryptor<Aes128>;

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

    ffmpeg([
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
    ffmpeg([
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
    ffmpeg([
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

fn ffprobe_json(path: &Path, extra: &[&str]) -> serde_json::Value {
    let output = ffprobe(
        ["-v", "quiet", "-print_format", "json"]
            .into_iter()
            .chain(extra.iter().copied())
            .chain([path.to_str().expect("utf8 path")]),
    );
    serde_json::from_slice(&output).expect("ffprobe json")
}

fn cover_bytes(path: &Path) -> Vec<u8> {
    let mut input = ffmpeg_next::format::input(path).expect("open book");
    let index = input
        .streams()
        .find(|stream| {
            stream
                .disposition()
                .contains(ffmpeg_next::format::stream::Disposition::ATTACHED_PIC)
        })
        .expect("cover stream")
        .index();
    input
        .packets()
        .find(|(stream, _)| stream.index() == index)
        .and_then(|(_, packet)| packet.data().map(<[u8]>::to_vec))
        .expect("cover packet")
}

fn assert_matches_source(output: &Path, source: &Path) {
    assert_eq!(
        audio_packet_bytes(output),
        audio_packet_bytes(source),
        "audio packets must match the generated source"
    );
    assert_eq!(
        cover_bytes(output),
        cover_bytes(source),
        "cover picture must match the generated source"
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
        assert_eq!(
            ffprobe_tag_ci(&out_tags, key),
            ffprobe_tag_ci(&src_tags, key),
            "tag {key}"
        );
    }

    let out_chapters = out_format["chapters"].as_array().expect("output chapters");
    let src_chapters = src_format["chapters"].as_array().expect("source chapters");
    assert_eq!(out_chapters.len(), src_chapters.len(), "chapter count");
    for (out_chapter, src_chapter) in out_chapters.iter().zip(src_chapters) {
        assert_eq!(
            ffprobe_tag_ci(
                out_chapter["tags"]
                    .as_object()
                    .unwrap_or(&serde_json::Map::new()),
                "title"
            ),
            ffprobe_tag_ci(
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

fn aax_material(activation_bytes_hex: &str) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aax {
        activation_bytes_hex: SecretString::from(activation_bytes_hex),
    }
}

fn aaxc_material(key_hex: &str) -> AudibleDecryptionMaterial {
    AudibleDecryptionMaterial::Aaxc {
        key_hex: SecretString::from(key_hex),
        iv_hex: SecretString::from(AAXC_IV_HEX),
    }
}

#[derive(Debug)]
struct ProtectedMaterializationRun {
    pub output: PathBuf,
    pub partial: PathBuf,
}

type MaterializationFailure = Box<(RemoteSourceDiagnostic, TempDir, ProtectedMaterializationRun)>;

async fn materialize_protected(
    source: &Path,
    download_name: &str,
    material: AudibleDecryptionMaterial,
) -> Result<(TempDir, ProtectedMaterializationRun), MaterializationFailure> {
    let root = TempDir::new().expect("protected download root");
    let input = root.path().join(download_name);
    std::fs::copy(source, &input).expect("copy protected download");
    match run_protected_materialization(&input, root.path(), &material, &|| false).await {
        (Ok(()), run) => Ok((root, run)),
        (Err(error), run) => Err(Box::new((
            AudibleAcquisitionError::materialization(error)
                .into_diagnostic(Some(TITLE_ID.to_string())),
            root,
            run,
        ))),
    }
}

const TITLE_ID: &str = "B0DECRYPT";

async fn run_protected_materialization(
    downloaded_path: &Path,
    item_dir: &Path,
    material: &AudibleDecryptionMaterial,
    is_cancelled: &impl Fn() -> bool,
) -> (Result<(), AppError>, ProtectedMaterializationRun) {
    let title_name = "Synthetic Book";
    let output = staged_materialized_path(item_dir, Some(title_name), TITLE_ID);
    let run = ProtectedMaterializationRun {
        partial: crate::remote_source::scoped_output::partial_sibling(&output),
        output: output.clone(),
    };
    let ctx = TitleAcquisitionCtx {
        job_id: "decrypt-job",
        title_id: TITLE_ID,
        item_dir,
        progress_context: TitleProgressContext {
            title_id: TITLE_ID,
            item_index: 1,
            total_items: 1,
        },
    };
    let result = materialize_protected_download(
        downloaded_path,
        Some(title_name),
        material,
        ctx,
        &mut |_| {},
        is_cancelled,
    )
    .await
    .map(|source| assert_eq!(source.path, output, "materialized path"));
    (result, run)
}

fn assert_message_holds_no_secret(message: &str, secrets: &[&str]) {
    let message = message.to_ascii_lowercase();
    for secret in secrets {
        assert!(!message.contains(secret), "message must not hold a secret");
    }
    assert!(!message.contains('/'), "message must not include a path");
}

#[test]
fn the_probe_reads_each_lane_brand_and_drm_box_from_the_encrypted_books() {
    let books = books();
    let read =
        crate::remote_source::providers::audible::protected_file_probe::read_protected_file_facts;

    let aax = read(&books.aax).expect("aax facts");
    let aaxc = read(&books.aaxc).expect("aaxc facts");

    assert_eq!(
        (
            aax.major_brand.as_str(),
            aax.adrm_present,
            aax.aavd_entry_present
        ),
        ("aax ", true, true)
    );
    assert_eq!(
        (
            aaxc.major_brand.as_str(),
            aaxc.adrm_present,
            aaxc.aavd_entry_present
        ),
        ("aaxc", false, true)
    );
}

#[tokio::test]
async fn aax_decrypts_to_the_generated_source() {
    let books = books();
    let (_root, run) =
        materialize_protected(&books.aax, "source.aax", aax_material(AAX_ACTIVATION_HEX))
            .await
            .unwrap_or_else(|failure| panic!("aax decrypt failed: {}", failure.0.message));
    assert_matches_source(&run.output, &books.source);
}

#[tokio::test]
async fn aaxc_decrypts_to_the_generated_source() {
    let books = books();
    let (_root, run) =
        materialize_protected(&books.aaxc, "source.aaxc", aaxc_material(AAXC_KEY_HEX))
            .await
            .unwrap_or_else(|failure| panic!("aaxc decrypt failed: {}", failure.0.message));
    assert_matches_source(&run.output, &books.source);
}

#[tokio::test]
async fn wrong_aax_activation_bytes_fail_and_leave_no_output() {
    let books = books();
    let (diagnostic, _root, run) = *materialize_protected(
        &books.aax,
        "source.aax",
        aax_material(AAX_WRONG_ACTIVATION_HEX),
    )
    .await
    .expect_err("wrong activation bytes must fail");
    assert_eq!(
        diagnostic.kind,
        RemoteAcquisitionFailureKind::MaterializationFailed
    );
    assert!(
        diagnostic.message.contains("opening the download"),
        "wrong activation bytes must be refused when the file opens: {}",
        diagnostic.message
    );
    assert_message_holds_no_secret(
        &diagnostic.message,
        &[AAX_WRONG_ACTIVATION_HEX, AAX_ACTIVATION_HEX],
    );
    assert!(!run.output.exists(), "no committed output");
    assert!(!run.partial.exists(), "no partial output");
}

#[tokio::test]
async fn wrong_aaxc_key_is_rejected_by_the_decodes_as_audio_gate() {
    let books = books();
    let (diagnostic, _root, run) = *materialize_protected(
        &books.aaxc,
        "source.aaxc",
        aaxc_material(WRONG_AAXC_KEY_HEX),
    )
    .await
    .expect_err("wrong AAXC key must fail at materialization, not return Ok for later validation");
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
    assert_message_holds_no_secret(
        &diagnostic.message,
        &[WRONG_AAXC_KEY_HEX, AAXC_KEY_HEX, AAXC_IV_HEX],
    );
    assert!(!run.output.exists(), "no committed m4b");
    assert!(!run.partial.exists(), "no partial");
}

#[tokio::test]
async fn a_download_that_does_not_match_the_license_lane_is_refused() {
    let books = books();
    let cases = [
        (&books.aax, "source.aax", aaxc_material(AAXC_KEY_HEX)),
        (&books.aaxc, "source.aaxc", aax_material(AAX_ACTIVATION_HEX)),
        (&books.source, "source.aaxc", aaxc_material(AAXC_KEY_HEX)),
    ];
    for (book, name, material) in cases {
        let (diagnostic, _root, run) = *materialize_protected(book, name, material)
            .await
            .expect_err("a lane mismatch must fail");
        assert_eq!(
            diagnostic.kind,
            RemoteAcquisitionFailureKind::MaterializationFailed
        );
        assert!(
            diagnostic.message.contains("does not match"),
            "message must say the lane does not match: {}",
            diagnostic.message
        );
        assert_message_holds_no_secret(
            &diagnostic.message,
            &[AAXC_KEY_HEX, AAXC_IV_HEX, AAX_ACTIVATION_HEX],
        );
        assert!(!run.output.exists(), "no committed output");
        assert!(!run.partial.exists(), "no partial output");
    }
}

#[tokio::test]
async fn a_malformed_key_is_refused_before_ffmpeg_opens_the_file() {
    let root = TempDir::new().expect("malformed key root");
    let missing = root.path().join("never-created.aaxc");
    let short_key = &AAXC_KEY_HEX[..30];
    let materials = [
        aaxc_material(short_key),
        aax_material(MALFORMED_ACTIVATION_HEX),
    ];

    for material in materials {
        let (result, run) =
            run_protected_materialization(&missing, root.path(), &material, &|| false).await;
        let message = result.expect_err("a malformed key must fail").to_string();

        assert!(
            message.contains("hexadecimal digits"),
            "the shape check must refuse it before the missing file is read: {message}"
        );
        assert_message_holds_no_secret(
            &message,
            &[short_key, MALFORMED_ACTIVATION_HEX, AAXC_IV_HEX],
        );
        assert!(!run.output.exists() && !run.partial.exists(), "no output");
    }
}

#[tokio::test]
async fn cancel_leaves_no_output_and_no_partial() {
    let books = books();
    let root = TempDir::new().expect("cancel root");
    let input = root.path().join("source.aaxc");
    std::fs::copy(&books.aaxc, &input).expect("copy protected download");
    let polls = std::cell::Cell::new(0_u32);
    let cancel_after_start = || {
        polls.set(polls.get() + 1);
        polls.get() > 1
    };

    let (result, run) = run_protected_materialization(
        &input,
        root.path(),
        &aaxc_material(AAXC_KEY_HEX),
        &cancel_after_start,
    )
    .await;

    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "cancel should stop decrypt, got {result:?}"
    );
    assert!(!run.output.exists(), "no committed output");
    assert!(!run.partial.exists(), "no partial output");
}

#[test]
fn the_stream_copy_stops_at_the_next_packet_when_told_to() {
    let books = books();
    let dir = TempDir::new().expect("stop dir");
    let output = dir.path().join("out.m4b");
    let control = CopyControl::default();
    control.request_stop();

    let result = stream_copy(&books.source, &output, &[], &control);

    assert!(
        matches!(result, Err(AppError::Cancellation(_))),
        "{result:?}"
    );
    assert_eq!(control.fraction(), 0.0, "no packet was copied");
    assert!(!output.exists(), "the stopped copy removes its own partial");
}

const LEAK_CHILD_ENV: &str = "ABB_DECRYPT_LEAK_CHILD";
const MALFORMED_ACTIVATION_HEX: &str = "zzzzzz";
const MALFORMED_AAXC_KEY_HEX: &str = "000102030405060708090a0b0c0d0eXX";

fn leak_child_test_name() -> String {
    let module = module_path!();
    let module = module.split_once("::").map_or(module, |(_, rest)| rest);
    format!("{module}::decrypt_cases_run_for_the_leak_check")
}

/// The child half of `decrypting_never_prints_a_secret_or_the_file_checksum`.
/// It does nothing unless that test started it, so the parent can read all
/// the output of this process, FFmpeg's included.
#[tokio::test]
async fn decrypt_cases_run_for_the_leak_check() {
    if std::env::var_os(LEAK_CHILD_ENV).is_none() {
        return;
    }
    crate::audio::limit_ffmpeg_logging();
    let books = books();
    let junk = TempDir::new().expect("junk dir");
    let not_mp4 = junk.path().join("not-mp4.bin");
    std::fs::write(&not_mp4, vec![0x5a_u8; 4096]).expect("write junk");
    let lanes = [
        (
            &books.aax,
            aax_material(AAX_ACTIVATION_HEX),
            aax_material(AAX_WRONG_ACTIVATION_HEX),
            aax_material(MALFORMED_ACTIVATION_HEX),
        ),
        (
            &books.aaxc,
            aaxc_material(AAXC_KEY_HEX),
            aaxc_material(WRONG_AAXC_KEY_HEX),
            aaxc_material(MALFORMED_AAXC_KEY_HEX),
        ),
    ];
    for (book, right, wrong, malformed) in lanes {
        let accepted = |outcome: Result<_, MaterializationFailure>| outcome.is_ok();
        assert!(accepted(
            materialize_protected(book, "book.enc", right).await
        ));
        assert!(!accepted(
            materialize_protected(book, "book.enc", wrong).await
        ));
        assert!(!accepted(
            materialize_protected(book, "book.enc", malformed).await
        ));
    }
    for material in [
        aax_material(AAX_ACTIVATION_HEX),
        aaxc_material(AAXC_KEY_HEX),
    ] {
        let outcome = materialize_protected(&not_mp4, "book.enc", material).await;
        assert!(outcome.is_err());
    }
}

#[test]
fn decrypting_never_prints_a_secret_or_the_file_checksum() {
    let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
        .args([
            "--exact",
            &leak_child_test_name(),
            "--nocapture",
            "--test-threads=1",
        ])
        .env(LEAK_CHILD_ENV, "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("run the decrypt child");
    let stdout = String::from_utf8_lossy(&output.stdout).to_ascii_lowercase();
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();

    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "the child ran its decrypt cases: {stdout} {stderr}"
    );
    let printed = format!("{stdout}{stderr}");
    for secret in [
        AAXC_KEY_HEX,
        AAXC_IV_HEX,
        WRONG_AAXC_KEY_HEX,
        MALFORMED_AAXC_KEY_HEX,
        AAX_ACTIVATION_HEX,
        AAX_WRONG_ACTIVATION_HEX,
        MALFORMED_ACTIVATION_HEX,
        "file checksum",
    ] {
        assert!(
            !printed.contains(&secret.to_ascii_lowercase()),
            "the child printed a secret or the AAX file checksum"
        );
    }
}
