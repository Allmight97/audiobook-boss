/*
 * FAAC - Freeware Advanced Audio Coder
 * Copyright (C) 2026 Nils Schimmelmann
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 */

/*
 * libfaac encoder API.
 *
 * This is the only public surface for the encoder. The legacy faacEnc* API
 * (faacEncOpen/faacEncGetCurrentConfiguration/faacEncSetConfiguration/
 * faacEncEncode/faacEncClose) has been removed; see docs/libfaac.md for a
 * porting guide.
 *
 * Design summary (see docs/libfaac.md for the full narrative):
 *   - Parameters are supplied once, up front, to faac_encoder_open(), so the
 *     encoder never exists in a half-configured state and all derived values
 *     (frame size, output-buffer bound, resolved object type) are known and
 *     queryable the instant open() returns.
 *   - Every fallible call returns a faac_status; faac_strerror() maps a status
 *     to a human-readable string.
 *   - Fixed-width integer types and width-pinned enums keep the ABI identical
 *     across 64-bit platforms (LP64 and LLP64), regardless of -fshort-enums.
 *     32-bit targets have their own layout because of pointer fields.
 *   - faac_params and faac_encoder_info are 4-byte aligned: append only members
 *     of 4 bytes or less. faac_library_info holds pointers, so appended members
 *     sit at multiples of their size, in blocks of 8 bytes.
 *   - faac_params grows only additively: new releases append named fields and
 *     grow sizeof(faac_params); callers MUST zero-initialize via
 *     faac_params_init() so struct_size lets the library reconcile versions.
 *   - Independent handles may be opened, driven and closed from different
 *     threads concurrently without external synchronization; the tables the
 *     library shares between handles are built once per process. A single
 *     handle is still owned by one thread at a time.
 */

#ifndef FAAC_H
#define FAAC_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

#include <stddef.h>
#include <stdint.h>

/*
 * The project release version, which every library reports; SONAMEs are
 * versioned separately. The removed legacy faacEnc* header never defined it,
 * so callers can tell this faac_* API apart by its presence:
 *
 *   #if defined(FAAC_VERSION_MAJOR)
 */
#define FAAC_VERSION_MAJOR 3
#define FAAC_VERSION_MINOR 0
#define FAAC_VERSION_PATCH 0
#define FAAC_VERSION_HEX \
    ((FAAC_VERSION_MAJOR << 16) | (FAAC_VERSION_MINOR << 8) | FAAC_VERSION_PATCH)

/* Windows exports from the DLL build only (FAAC_BUILDING). No dllimport: the API
 * has no data, so DLL and static consumers link the same way with no define. */
#ifndef FAACAPI
# if defined(_WIN32)
#  ifdef FAAC_BUILDING
#   define FAACAPI __declspec(dllexport)
#  else
#   define FAACAPI
#  endif
# elif defined(__GNUC__) && (__GNUC__ >= 4)
#  define FAACAPI __attribute__((visibility("default")))
# else
#  define FAACAPI
# endif
#endif

/* Opaque encoder handle */
typedef struct faac_encoder faac_encoder;

/*
 * Result codes. All values are negative except FAAC_OK so a caller can test
 * `status < 0` for failure. The FAAC_STATUS_MAX sentinel pins the enum to a
 * 32-bit underlying type for a stable ABI under -fshort-enums.
 */
typedef enum faac_status {
    FAAC_OK                   = 0,
    FAAC_ERR_INVALID_ARGUMENT = -1,  /* NULL pointer, bad struct_size, or bad enum/field value */
    FAAC_ERR_UNSUPPORTED      = -2,  /* object type left out of the build, more channels than it supports, or MPEG-2 ASC requested */
    FAAC_ERR_NO_MEMORY        = -3,  /* allocation failed */
    FAAC_ERR_OUTPUT_TOO_SMALL = -4,  /* output buffer smaller than max_output_bytes */
    FAAC_ERR_INPUT_OVERFLOW   = -5,  /* in_samples exceeded the per-call frame capacity */
    FAAC_ERR_INTERNAL         = -6,  /* unexpected failure inside the encoder core */
    FAAC_STATUS_MAX           = 0x7fffffff
} faac_status;

/*
 * AAC object type, numbered per the MPEG-4 Audio Object Type (AOT) registry so
 * a single collision-free namespace covers current and future profiles.
 * FAAC_OBJ_AUTO lets the library pick AAC-LC or HE-AAC by bitrate.
 */
enum faac_object_type {
    FAAC_OBJ_AUTO      = 0,          /* let the library choose LC or HE-AAC by bitrate */
    FAAC_OBJ_LC        = 2,          /* AAC-LC                                       */
    FAAC_OBJ_HE_AAC_V1 = 5,          /* AAC-LC + SBR                                 */
    FAAC_OBJ_HE_AAC_V2 = 29,         /* AAC-LC + SBR + PS  (requires encoder-ps build option) */
    /* AOT 23 (LD), 39 (ELD), 42 (xHE-AAC/USAC) are intentionally NOT reserved
     * here; each would need its own additive configuration surface. */
    FAAC_OBJ_MAX       = 0x7fffffff
};

enum faac_stereo_mode {
    FAAC_STEREO_LR   = 0,            /* independent (L/R) stereo         */
    FAAC_STEREO_MS,                  /* mid/side                          */
    FAAC_STEREO_IS,                  /* intensity                         */
    FAAC_STEREO_AUTO,                /* per-band mix of M/S and intensity */
    FAAC_STEREO_MAX = 0x7fffffff
};

enum faac_shortctl_mode {
    FAAC_SHORTCTL_NORMAL  = 0,       /* let block switching decide       */
    FAAC_SHORTCTL_NOSHORT,           /* force long blocks only           */
    FAAC_SHORTCTL_NOLONG,            /* force short blocks only           */
    FAAC_SHORTCTL_MAX = 0x7fffffff
};

enum faac_stream_format {
    FAAC_STREAM_RAW  = 0,            /* raw AAC frames (needs out-of-band ASC) */
    FAAC_STREAM_ADTS = 1,            /* self-framing MPEG-4 ADTS                */
    FAAC_STREAM_ADTS_MPEG2 = 2,      /* self-framing MPEG-2 ADTS; HE-AAC resolves to MPEG-4 */
    FAAC_STREAM_MAX  = 0x7fffffff
};

/* How bit_rate is held. ABR: an average, frames bounded only by the 6144
 * bits/ch ceiling. CBR: a bit reservoir bounds every frame to the decoder
 * buffer, stuffs frames that would overflow it, and declares the fill. */
enum faac_rate_control {
    FAAC_RC_ABR = 0,                 /* average whole-stream bit_rate            */
    FAAC_RC_VBR,                     /* quality drives; bit_rate must be 0        */
    FAAC_RC_CBR,                     /* bit reservoir; whole-stream bit_rate      */
    FAAC_RC_MAX  = 0x7fffffff
};

/* Interpretation of the interleaved PCM handed to faac_encoder_encode(). */
enum faac_input_format {
    FAAC_INPUT_NULL  = 0,            /* invalid / unset                          */
    FAAC_INPUT_16BIT,                /* native-endian int16                      */
    FAAC_INPUT_24BIT,                /* native-endian int24 in 24 bits           */
    FAAC_INPUT_24IN32,               /* 24 significant bits in a 32-bit word     */
    FAAC_INPUT_FLOAT,                /* 32-bit float, unity full scale (+-1.0)   */
    FAAC_INPUT_MAX = 0x7fffffff
};

/* Order of the interleaved input channels. Eight channels carry a program
 * config element (PCE) that lists the pairs as front, side, back, so Apple,
 * FDK, FFmpeg and libfaad read the same 7.1; AAC channel configuration 7
 * would declare 7.1 wide. */
enum faac_channel_layout {
    /* AAC element order: FC, front L/R, then BC (4 ch), back L/R (5/6 ch),
     * or side L/R then back L/R (8 ch), with LFE last (6/8 ch). */
    FAAC_LAYOUT_AAC = 0,
    /* WAVE order: FL FR FC, then BC (4 ch), BL BR (5 ch), LFE BL BR (6 ch),
     * or LFE BL BR SL SR (8 ch). The library reorders to AAC element order. */
    FAAC_LAYOUT_WAV = 1,
    FAAC_LAYOUT_MAX = 0x7fffffff
};

enum faac_flag {
    FAAC_FLAG_TNS = 1u << 0,         /* temporal noise shaping                   */
    FAAC_FLAG_PNS = 1u << 1,         /* perceptual noise substitution            */
    FAAC_FLAG_MAX = 0x7fffffff
};

enum faac_drc_profile {
    FAAC_DRC_OFF         = 0,        /* no dynamic_range_info element (default)   */
    FAAC_DRC_FILM        = 1,        /* film standard: +6 dB boost, wide cut      */
    FAAC_DRC_FILM_LIGHT  = 2,        /* film light: wide null band, gentler cut   */
    FAAC_DRC_MUSIC       = 3,        /* music standard: +12 dB boost, slow release */
    FAAC_DRC_MUSIC_LIGHT = 4,        /* music light: wide null band, +12 dB boost */
    FAAC_DRC_SPEECH      = 5,        /* speech: +15 dB boost, fast release        */
    FAAC_DRC_MAX         = 0x7fffffff
};

/*
 * Encoder parameters, supplied once to faac_encoder_open().
 *
 * ALWAYS initialize with faac_params_init() before setting fields: it zeroes
 * the whole struct (including padding) and stamps struct_size, which is how the
 * library stays compatible as this struct grows. Construct one from scratch and
 * open() will reject it. New storage is added by appending named fields.
 *
 * Maintainers: appended fields must start at or after the previous sizeof(struct),
 * never in its tail padding: struct_size cannot distinguish a new field there
 * from padding supplied by an older caller. Explicit reserved fields must be zero
 * on input and are zero on output until assigned a future meaning. They may be
 * reused with their storage and offsets preserved and zero retaining the previous
 * behavior. Callers must check library support before requesting new behavior.
 * Existing field offsets, meanings and struct alignment, and baseline sizes stay fixed.
 */
typedef struct faac_params {
    uint32_t                struct_size;   /* set by faac_params_init() */

    uint32_t                sample_rate;   /* input/output sample rate in Hz (required)      */
    uint32_t                channels;      /* channel count, 1..max_channels (required)       */

    enum faac_object_type   object_type;
    enum faac_stereo_mode   stereo_mode;
    uint32_t                flags;         /* FAAC_FLAG_*; unknown bits are rejected */

    uint32_t                bit_rate;      /* whole-stream bits/sec; 0 = 64 kbit/s per channel in ABR/CBR, must be 0 in VBR */
    uint32_t                bandwidth;     /* cutoff in Hz; 0 = from bit_rate, the curve's top in VBR (full band above 48 kHz at quality 100 or higher) */
    uint32_t                quality;       /* VBR quality 1..5000, required in VBR; must be 0 in ABR/CBR */

    enum faac_stream_format stream_format;
    enum faac_input_format  input_format;
    enum faac_shortctl_mode short_control;

    enum faac_channel_layout channel_layout; /* input channel order; 6 and 8 channels carry an LFE */

    /* Ceiling on any single frame, for packet-oriented transports that cannot
     * fragment one -- a frame that overruns the link MTU is dropped, not split.
     * Like bit_rate this is a whole-stream rate. It follows from the payload a
     * packet can carry or standard ISO/IEC 14496-3 limits (6144 bits/channel
     * per frame):
     *
     *     peak_bit_rate = payload_bytes * 8 * sample_rate / 1024
     *
     * Must be >= bit_rate (a peak below the average is
     * unsatisfiable) and <= FAAC_MAX_PEAK_BIT_RATE; open() rejects anything else.
     * More headroom means fewer retries.
     *
     * Best-effort: enforced by backing off quality over bounded retries, which
     * cannot beat the irreducible per-frame cost of side info and scalefactors.
     * Measured on transient-heavy stereo, the cap holds for every frame at and
     * above ~64 kbit/s; below that the floor starts to show (~48 kbit/s
     * overshoots on a few percent of frames). */
    uint32_t                peak_bit_rate;  /* whole-stream peak bits/sec; 0 = unlimited */

    enum faac_rate_control  rate_control;  /* see enum; zero selects ABR */

    enum faac_drc_profile   drc_profile;   /* FAAC_DRC_OFF (default) or a line-mode profile */
    float                   drc_ref_level; /* program reference level of the content in LUFS
                                              [-31.75, 0.0]; faac_params_init() sets -23.0. The
                                              profile's null band is centered on it, and it is
                                              written as prog_ref_level, whose field is nominally
                                              dBFS */
} faac_params;

/* Upper bound on faac_params.peak_bit_rate: far above any real stream rate, and
 * low enough that converting it to a per-frame bit budget cannot overflow. */
#define FAAC_MAX_PEAK_BIT_RATE ((uint32_t)100000000)

/*
 * Resolved encoder properties, filled by faac_encoder_get_info(). All values are
 * final: the object type is resolved (FAAC_OBJ_AUTO becomes a concrete type),
 * auto-derived rate-control defaults are filled in, and buffer sizes account for
 * the object type (e.g. HE-AAC's 2048-sample frame). Set struct_size to
 * sizeof(faac_encoder_info) before the call; the struct grows only by appending
 * fields, so a newer library stays compatible with an older caller's smaller
 * struct.
 */
typedef struct faac_encoder_info {
    uint32_t                struct_size;      /* set by caller to sizeof(faac_encoder_info) */

    uint32_t                frame_samples;    /* samples/channel per full frame (1024 LC, 2048 HE) */
    uint32_t                max_output_bytes; /* upper bound on one encode() call's output          */
    enum faac_object_type   object_type;      /* concrete type in effect (AUTO resolved)            */
    enum faac_stream_format stream_format;    /* resolved format (HE-AAC forces MPEG-4)              */

    uint32_t                bit_rate;         /* resolved whole-stream bits/sec (0 in VBR)           */
    uint32_t                bandwidth;        /* resolved cutoff in Hz                               */
    uint32_t                quality;          /* resolved quantizer quality                          */

    /* Priming delay in samples/channel at the output rate, as gapless tags
     * (iTunSMPB) declare it. An MP4 edit list counts in the track timescale,
     * which for HE-AAC is the core rate, so the frontend's writer halves this
     * value there. Not the same as frame_samples for HE-AAC, and excludes the
     * SBR decoder delay, which decoders add themselves. */
    uint32_t                encoder_delay;

    /* AudioSpecificConfig for raw streams and containers, valid as soon as
     * open() returns; copy the bytes if they are needed after this struct.
     * Up to 14 bytes: the eight-channel PCE and the HE-AAC extension add to
     * the 2 to 7 bytes of the other layouts. ADTS MPEG-2 has none (asc_size
     * is 0). */
    uint32_t                asc_size;
    uint8_t                 asc[14];

    /* Appended after asc so the layout an older caller compiled against is
     * unchanged. */
    enum faac_drc_profile   drc_profile;      /* resolved DRC profile                              */
    float                   drc_ref_level;    /* resolved program reference level in LUFS          */
} faac_encoder_info;

/*
 * Library-global facts, independent of any encoder instance: the compiled
 * channel ceiling, the version/copyright strings, and the SBR analysis density.
 * These reflect build-time options (max-channels, sbr-decimation) that the
 * header alone cannot report, so they are queried at runtime. Set
 * out->struct_size to sizeof(faac_library_info) before the call; the struct
 * grows only by appending fields, so a newer library stays compatible with an
 * older caller's smaller struct. version/copyright are static and library-owned;
 * do not free them.
 *
 * Maintainers: appended fields must start at or after the previous sizeof(struct),
 * never in its tail padding: struct_size cannot distinguish a new field there
 * from padding in the older layout.
 */
enum faac_library_flag {
    FAAC_LIB_SBR = 1u << 0,         /* HE-AAC v1 (SBR) is compiled in */
    FAAC_LIB_PS  = 1u << 1,         /* HE-AAC v2 (Parametric Stereo) is compiled in */
    FAAC_LIB_DRC = 1u << 2,         /* Dynamic Range Control support is compiled in */
    FAAC_LIB_FLAG_MAX = 0x7fffffff
};

typedef struct faac_library_info {
    uint32_t                struct_size;    /* set by caller to sizeof(faac_library_info) */

    const char             *version;        /* library version string   (library-owned) */
    const char             *copyright;      /* library copyright string (library-owned) */

    uint32_t                max_channels;   /* highest channels this build accepts */
    uint32_t                sbr_decimation; /* HE-AAC SBR analysis density: every Nth slot
                                             * is analyzed (1 = full quality). Build-time. */
    uint32_t                flags;          /* FAAC_LIB_* */
} faac_library_info;

/* Offset just past the named field: the smallest struct size the library accepts. */
#define FAAC_STRUCT_END(type, field) \
    (offsetof(type, field) + sizeof(((type *)0)->field))
#define FAAC_PARAMS_BASELINE FAAC_STRUCT_END(faac_params, drc_ref_level)
#define FAAC_ENCODER_INFO_BASELINE FAAC_STRUCT_END(faac_encoder_info, drc_ref_level)
#define FAAC_LIBRARY_INFO_BASELINE FAAC_STRUCT_END(faac_library_info, sbr_decimation)

FAACAPI faac_status faac_get_library_info(faac_library_info *out_info);

/* Zero-initialize *params and fill in library defaults and struct_size. Pass
 * sizeof(*params) as caller_size; never writes past min(caller_size,
 * sizeof(faac_params)), so a caller stays safe even if the loaded library's
 * faac_params has grown since the caller was built. Returns
 * FAAC_ERR_INVALID_ARGUMENT if params is NULL or caller_size is smaller than
 * faac_params's minimum layout, through rate_control.
 */
FAACAPI faac_status faac_params_init(faac_params *params, uint32_t caller_size);

/*
 * Create an encoder from a fully-specified faac_params. On success writes the
 * new handle to *out and returns FAAC_OK. On failure *out is set to NULL and a
 * negative status describes why (e.g. FAAC_ERR_UNSUPPORTED for an object type
 * this build does not implement). The faac_params is consumed by this call and
 * need not outlive it.
 */
FAACAPI faac_status faac_encoder_open(const faac_params *config, faac_encoder **out);

/*
 * Destroy an encoder. Pass the address of your handle; on success the handle is
 * set to NULL to help guard against use-after-free. Passing a pointer to a NULL
 * handle is a no-op that returns FAAC_OK.
 */
FAACAPI faac_status faac_encoder_close(faac_encoder **encoder);

/*
 * Query the resolved encoder properties. Set out->struct_size to
 * sizeof(faac_encoder_info) first; the library fills the fields (writing at most
 * out->struct_size bytes, so a newer library is safe against an older caller's
 * struct) and updates struct_size to the number of bytes populated. Returns
 * FAAC_ERR_INVALID_ARGUMENT if enc or out is NULL, or struct_size is too small.
 * Use these values to size the input/output buffers and to report the effective
 * configuration back to the user.
 */
FAACAPI faac_status faac_encoder_get_info(const faac_encoder *encoder, faac_encoder_info *out_info);

/*
 * Return the AudioSpecificConfig (a.k.a. decoder-specific info) for this
 * encoder via *buf / *len. The buffer is library-owned and valid until the
 * encoder is closed; do NOT free it. Returns FAAC_ERR_UNSUPPORTED for
 * configurations that have no ASC (e.g. MPEG-2).
 */
FAACAPI faac_status faac_encoder_asc(const faac_encoder *encoder,
                                     const uint8_t **buf, uint32_t *len);

/*
 * Encode. `in` points to `in_samples` interleaved PCM samples (total across all
 * channels, i.e. samples-per-channel * channels), formatted per
 * input_format; it may be any count from 0 to frame_samples*channels. The
 * library buffers internally and emits at most one frame per call, writing up
 * to out_cap bytes to `out` and the byte count to *bytes_written (which may be
 * 0 while priming, accumulating, or flushing). Pass in_samples == 0 to flush at
 * end of stream; keep calling until *bytes_written is 0.
 */
FAACAPI faac_status faac_encoder_encode(faac_encoder *encoder,
                                        const void *in, uint32_t in_samples,
                                        uint8_t *out, uint32_t out_cap,
                                        uint32_t *bytes_written);

/* Human-readable, static description of a status code. Never returns NULL. */
FAACAPI const char *faac_strerror(faac_status status);

#ifdef __cplusplus
}
#endif /* __cplusplus */

#endif /* FAAC_H */
