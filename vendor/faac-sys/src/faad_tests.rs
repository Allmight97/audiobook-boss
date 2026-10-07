//! Review probes for the FAAD3 API in FAAC PR 29, driven by ABB's FAAC 2.2
//! output. Each test answers one question an integrator asks of `faad.h`.

use super::*;
use crate as faac;
use std::{ffi::CStr, mem::size_of, ptr};

const RATE: u32 = 44_100;
/// ABB's MP4 timing for its FAAC HE files (`he_timing.rs`).
const ABB_HE_CORE_PRIMING: usize = 2080;
const ABB_SBR_DELAY: usize = 962;

struct Encoded {
    asc: Vec<u8>,
    packets: Vec<Vec<u8>>,
    encoder_delay: usize,
}

struct Decoder(*mut faad_decoder);

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe { faad_decoder_close(&mut self.0) };
    }
}

struct Decoded {
    pcm: Vec<f64>,
    info: faad_stream_info,
    flags: Vec<u32>,
}

/// A non-periodic chirp per channel, so cross-correlation has one peak and a
/// channel swap shows up.
fn source(channels: u32, seconds: f64) -> Vec<f32> {
    let frames = (RATE as f64 * seconds) as usize;
    let mut out = Vec::with_capacity(frames * channels as usize);
    for i in 0..frames {
        let t = i as f64 / RATE as f64;
        for c in 0..channels {
            let (f0, f1) = if c == 0 {
                (150.0, 2500.0)
            } else {
                (2500.0, 300.0)
            };
            let phase = 2.0 * std::f64::consts::PI * (f0 * t + (f1 - f0) * t * t / (2.0 * seconds));
            out.push((phase.sin() * 8000.0) as f32);
        }
    }
    out
}

fn encode(
    profile: faac::faac_object_type,
    channels: u32,
    format: faac::faac_stream_format,
    pcm: &[f32],
) -> Encoded {
    let mut params = faac::faac_params::default();
    assert_eq!(
        unsafe { faac::faac_params_init(&mut params, size_of::<faac::faac_params>() as u32) },
        faac::FAAC_OK
    );
    params.sample_rate = RATE;
    params.num_channels = channels;
    params.object_type = profile;
    params.bit_rate = 64_000 / channels;
    params.output_format = format;
    params.input_format = faac::FAAC_INPUT_FLOAT;
    let mut handle = ptr::null_mut();
    assert_eq!(
        unsafe { faac::faac_encoder_open(&params, &mut handle) },
        faac::FAAC_OK
    );
    let mut info = faac::faac_encoder_info {
        struct_size: size_of::<faac::faac_encoder_info>() as u32,
        ..Default::default()
    };
    assert_eq!(
        unsafe { faac::faac_encoder_get_info(handle, &mut info) },
        faac::FAAC_OK
    );
    let (mut asc_ptr, mut asc_len) = (ptr::null(), 0);
    assert_eq!(
        unsafe { faac::faac_encoder_asc(handle, &mut asc_ptr, &mut asc_len) },
        faac::FAAC_OK
    );
    let asc = unsafe { std::slice::from_raw_parts(asc_ptr, asc_len as usize) }.to_vec();

    let chunk = (info.frame_samples * channels) as usize;
    let mut out = vec![0u8; info.max_output_bytes as usize];
    let mut packets = Vec::new();
    let mut push = |input: &[f32], packets: &mut Vec<Vec<u8>>| -> u32 {
        let mut written = 0;
        let data = if input.is_empty() {
            ptr::null()
        } else {
            input.as_ptr().cast()
        };
        assert_eq!(
            unsafe {
                faac::faac_encoder_encode(
                    handle,
                    data,
                    input.len() as u32,
                    out.as_mut_ptr(),
                    info.max_output_bytes,
                    &mut written,
                )
            },
            faac::FAAC_OK
        );
        if written > 0 {
            packets.push(out[..written as usize].to_vec());
        }
        written
    };
    for block in pcm.chunks(chunk) {
        push(block, &mut packets);
    }
    while push(&[], &mut packets) > 0 {}
    assert_eq!(
        unsafe { faac::faac_encoder_close(&mut handle) },
        faac::FAAC_OK
    );
    Encoded {
        asc,
        packets,
        encoder_delay: info.encoder_delay as usize,
    }
}

/// ABB decodes to float, the only output format it uses.
fn config(stream: faad_stream_format) -> faad_config {
    let mut cfg = faad_config::default();
    assert_eq!(
        unsafe { faad_config_init(&mut cfg, size_of::<faad_config>() as u32) },
        FAAD_OK
    );
    cfg.stream_format = stream;
    cfg.output_format = FAAD_OUTPUT_FLOAT;
    cfg
}

fn stream_info(dec: *const faad_decoder) -> faad_stream_info {
    let mut info = faad_stream_info {
        struct_size: size_of::<faad_stream_info>() as u32,
        ..Default::default()
    };
    assert_eq!(unsafe { faad_decoder_get_info(dec, &mut info) }, FAAD_OK);
    info
}

fn open(cfg: &faad_config, asc: &[u8]) -> Decoder {
    let mut dec = Decoder(ptr::null_mut());
    let asc_ptr = if asc.is_empty() {
        ptr::null()
    } else {
        asc.as_ptr()
    };
    assert_eq!(
        unsafe { faad_decoder_open(cfg, asc_ptr, asc.len() as u32, &mut dec.0) },
        FAAD_OK
    );
    dec
}

/// Decodes every packet and returns interleaved float PCM.
fn decode_with(dec: &Decoder, packets: &[Vec<u8>]) -> Decoded {
    let cap = stream_info(dec.0).max_output_bytes;
    let mut out = vec![0u8; cap as usize];
    let (mut pcm, mut flags) = (Vec::new(), Vec::new());
    for packet in packets {
        let (mut consumed, mut written, mut frame_flags) = (0, 0, 0);
        let status = unsafe {
            faad_decode_frame(
                dec.0,
                packet.as_ptr(),
                packet.len() as u32,
                &mut consumed,
                out.as_mut_ptr().cast(),
                cap,
                &mut written,
                &mut frame_flags,
            )
        };
        assert_eq!(status, FAAD_OK, "{}", strerror(status));
        assert_eq!(consumed as usize, packet.len());
        flags.push(frame_flags);
        pcm.extend(
            out[..written as usize]
                .chunks_exact(4)
                .map(|b| f32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as f64),
        );
    }
    Decoded {
        pcm,
        info: stream_info(dec.0),
        flags,
    }
}

fn decode(encoded: &Encoded) -> Decoded {
    decode_with(
        &open(&config(FAAD_STREAM_RAW), &encoded.asc),
        &encoded.packets,
    )
}

fn strerror(status: faad_status) -> String {
    unsafe { CStr::from_ptr(faad_strerror(status)) }
        .to_string_lossy()
        .into_owned()
}

fn channel(pcm: &[f64], channels: usize, index: usize) -> Vec<f64> {
    pcm.iter().skip(index).step_by(channels).copied().collect()
}

/// The lag of `decoded` against `reference` with the highest normalized
/// correlation over a one-second window; 0 means aligned.
fn best_lag(reference: &[f64], decoded: &[f64]) -> (i64, f64) {
    let (start, len, span) = (RATE as i64, 8192i64, 3000i64);
    let window = &reference[start as usize..(start + len) as usize];
    let energy = window.iter().map(|v| v * v).sum::<f64>().sqrt();
    (-span..=span)
        .map(|lag| {
            let other = &decoded[(start + lag) as usize..(start + lag + len) as usize];
            let dot: f64 = window.iter().zip(other).map(|(a, b)| a * b).sum();
            let norm = energy * other.iter().map(|v| v * v).sum::<f64>().sqrt();
            (lag, dot / norm)
        })
        .fold(
            (0, f64::MIN),
            |best, next| if next.1 > best.1 { next } else { best },
        )
}

#[test]
fn library_info_reports_version_and_build_options() {
    let mut info = faad_library_info {
        struct_size: size_of::<faad_library_info>() as u32,
        ..Default::default()
    };
    assert_eq!(unsafe { faad_get_library_info(&mut info) }, FAAD_OK);
    assert_eq!(
        unsafe { CStr::from_ptr(info.version) }.to_str(),
        Ok("3.0.0")
    );
    assert_eq!(info.max_channels, 2);
    assert!(info.sbr_supported && info.ps_supported);

    let mut unsized_info = faad_library_info::default();
    assert_eq!(
        unsafe { faad_get_library_info(&mut unsized_info) },
        FAAD_ERR_INVALID_ARGUMENT
    );
}

#[repr(C)]
#[derive(Default)]
struct Grown<T> {
    known: T,
    future: [u8; 16],
}

#[test]
fn struct_size_handshake_rejects_short_layouts_and_preserves_unknown_tails() {
    let mut short = faad_config::default();
    assert_eq!(
        unsafe { faad_config_init(&mut short, 4) },
        FAAD_ERR_INVALID_ARGUMENT
    );

    let mut grown = Grown::<faad_config> {
        future: [0xA5; 16],
        ..Default::default()
    };
    assert_eq!(
        unsafe { faad_config_init(&mut grown.known, size_of::<Grown<faad_config>>() as u32) },
        FAAD_OK
    );
    assert_eq!(
        grown.future, [0xA5; 16],
        "config_init wrote past the layout it knows"
    );
    assert_eq!(grown.known.struct_size, size_of::<faad_config>() as u32);

    let encoded = encode(
        faac::FAAC_OBJ_LOW,
        2,
        faac::FAAC_STREAM_RAW,
        &source(2, 0.1),
    );
    let dec = open(&config(FAAD_STREAM_RAW), &encoded.asc);
    let mut short_info = faad_stream_info {
        struct_size: 4,
        ..Default::default()
    };
    assert_eq!(
        unsafe { faad_decoder_get_info(dec.0, &mut short_info) },
        FAAD_ERR_INVALID_ARGUMENT
    );
    let mut grown_info = Grown::<faad_stream_info> {
        future: [0xA5; 16],
        ..Default::default()
    };
    grown_info.known.struct_size = size_of::<Grown<faad_stream_info>>() as u32;
    assert_eq!(
        unsafe { faad_decoder_get_info(dec.0, &mut grown_info.known) },
        FAAD_OK
    );
    assert_eq!(grown_info.future, [0xA5; 16]);
    assert_eq!(
        grown_info.known.struct_size,
        size_of::<faad_stream_info>() as u32
    );
}

#[test]
fn strerror_describes_every_status() {
    let known = [
        FAAD_OK,
        FAAD_ERR_INVALID_ARGUMENT,
        FAAD_ERR_UNSUPPORTED,
        FAAD_ERR_INSUFFICIENT_MEM,
        FAAD_ERR_OUTPUT_TOO_SMALL,
        FAAD_ERR_NEED_MORE_DATA,
        FAAD_ERR_DECODE_FAILED,
        FAAD_ERR_SYNC_LOST,
    ];
    let texts: Vec<_> = known.iter().map(|s| strerror(*s)).collect();
    assert!(texts.iter().all(|t| !t.is_empty()));
    let mut unique = texts.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), known.len(), "{texts:?}");
    assert!(!strerror(-1234).is_empty());
}

#[test]
fn lc_round_trip_is_aligned_after_container_priming_alone() {
    let pcm = source(2, 3.0);
    let encoded = encode(faac::FAAC_OBJ_LOW, 2, faac::FAAC_STREAM_RAW, &pcm);
    let decoded = decode(&encoded);
    assert_eq!(decoded.info.object_type, FAAD_OBJ_LC);
    assert_eq!((decoded.info.sample_rate, decoded.info.channels), (RATE, 2));
    assert_eq!(
        (decoded.info.decoder_delay, decoded.info.frame_samples),
        (0, 1024)
    );
    assert_eq!(decoded.pcm.len(), encoded.packets.len() * 1024 * 2);

    let trim = encoded.encoder_delay + decoded.info.decoder_delay as usize;
    assert_eq!(trim, 1024);
    for c in 0..2 {
        let src = channel(&pcm.iter().map(|v| *v as f64).collect::<Vec<_>>(), 2, c);
        let out = channel(&decoded.pcm, 2, c);
        let (lag, corr) = best_lag(&src, &out[trim..]);
        assert_eq!(lag, 0, "channel {c}");
        assert!(corr > 0.98, "channel {c} correlation {corr}");
    }
}

#[test]
fn he_round_trip_reports_the_sbr_delay_abb_already_applies() {
    let pcm = source(2, 3.0);
    let encoded = encode(faac::FAAC_OBJ_HE_AAC_V1, 2, faac::FAAC_STREAM_RAW, &pcm);
    assert_eq!(encoded.encoder_delay, ABB_HE_CORE_PRIMING);
    let decoded = decode(&encoded);
    assert_eq!(decoded.info.object_type, FAAD_OBJ_HE_AAC_V1);
    assert_eq!((decoded.info.sample_rate, decoded.info.channels), (RATE, 2));
    assert_eq!(decoded.info.frame_samples, 2048);
    assert_eq!(decoded.info.decoder_delay as usize, ABB_SBR_DELAY);
    assert!(decoded.flags.iter().all(|f| f & FAAD_FRAME_SBR != 0));

    let trim = encoded.encoder_delay + decoded.info.decoder_delay as usize;
    let src = channel(&pcm.iter().map(|v| *v as f64).collect::<Vec<_>>(), 2, 0);
    let (lag, corr) = best_lag(&src, &channel(&decoded.pcm, 2, 0)[trim..]);
    assert_eq!(lag, 0);
    assert!(corr > 0.9, "correlation {corr}");
}

#[test]
fn he_mono_decodes_to_the_stream_channel_count() {
    let pcm = source(1, 1.5);
    let encoded = encode(faac::FAAC_OBJ_HE_AAC_V1, 1, faac::FAAC_STREAM_RAW, &pcm);
    let decoded = decode(&encoded);
    assert_eq!(decoded.info.channels, 1);
    assert_eq!(decoded.info.channel_mask, 0x4, "mono is FC");
    assert!(decoded.flags.iter().all(|f| f & FAAD_FRAME_PS == 0));
}

#[test]
fn adts_discovers_format_on_the_first_frame_and_retries_partial_input() {
    let encoded = encode(
        faac::FAAC_OBJ_LOW,
        2,
        faac::FAAC_STREAM_ADTS,
        &source(2, 0.5),
    );
    let dec = open(&config(FAAD_STREAM_ADTS), &[]);
    let before = stream_info(dec.0);
    assert!(!before.format_known);
    assert!(before.max_output_bytes > 0);

    let first = &encoded.packets[0];
    let mut out = vec![0u8; before.max_output_bytes as usize];
    let (mut consumed, mut written, mut flags) = (1, 1, 1);
    let status = unsafe {
        faad_decode_frame(
            dec.0,
            first.as_ptr(),
            first.len() as u32 - 1,
            &mut consumed,
            out.as_mut_ptr().cast(),
            before.max_output_bytes,
            &mut written,
            &mut flags,
        )
    };
    assert_eq!(status, FAAD_ERR_NEED_MORE_DATA);
    assert_eq!((consumed, written, flags), (0, 0, 0));

    let status = unsafe {
        faad_decode_frame(
            dec.0,
            first.as_ptr(),
            first.len() as u32,
            &mut consumed,
            out.as_mut_ptr().cast(),
            before.max_output_bytes - 1,
            &mut written,
            &mut flags,
        )
    };
    assert_eq!(status, FAAD_ERR_OUTPUT_TOO_SMALL);
    assert_eq!((consumed, written), (0, 0));

    let decoded = decode_with(&dec, &encoded.packets);
    assert_eq!(
        decoded.flags[0] & FAAD_FRAME_FORMAT_CHANGED,
        FAAD_FRAME_FORMAT_CHANGED
    );
    assert!(decoded.flags[1..]
        .iter()
        .all(|f| f & FAAD_FRAME_FORMAT_CHANGED == 0));
    assert!(decoded.info.format_known);
    assert_eq!((decoded.info.sample_rate, decoded.info.channels), (RATE, 2));
}

/// FAAD's float is unity full scale, the scale of FFmpeg's float frames,
/// unlike FAAC's signed-16-scaled float input.
#[test]
fn float_output_is_unity_full_scale() {
    let pcm = source(2, 1.0);
    let encoded = encode(faac::FAAC_OBJ_LOW, 2, faac::FAAC_STREAM_RAW, &pcm);
    let decoded = decode(&encoded);
    let rms = |values: &mut dyn Iterator<Item = f64>| {
        let (sum, count) = values.fold((0.0, 0usize), |(s, n), v| (s + v * v, n + 1));
        (sum / count as f64).sqrt()
    };
    let source_rms = rms(&mut pcm.iter().map(|v| f64::from(*v) / 32768.0));
    let decoded_rms = rms(&mut decoded.pcm[encoded.encoder_delay * 2..].iter().copied());
    let ratio = decoded_rms / source_rms;
    assert!((0.9..1.1).contains(&ratio), "decoded/source level {ratio}");
}

#[test]
fn parallel_handles_decode_the_same_pcm_as_sequential_handles() {
    let encoded = encode(
        faac::FAAC_OBJ_HE_AAC_V1,
        2,
        faac::FAAC_STREAM_RAW,
        &source(2, 0.5),
    );
    let expected = decode(&encoded).pcm;
    let start = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|_| {
                scope.spawn(|| {
                    start.wait();
                    decode(&encoded).pcm
                })
            })
            .collect();
        for handle in handles {
            assert_eq!(handle.join().expect("parallel decoder"), expected);
        }
    });
}
