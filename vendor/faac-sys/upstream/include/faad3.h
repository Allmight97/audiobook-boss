/*
 * FAAD - Freeware Advanced Audio Decoder
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
 * libfaad decoder API.
 *
 * Designed for bare-metal and RTOS environments with overridable allocators.
 *
 * Design summary:
 *   - Configuration is supplied once, up front, to faad_decoder_open().
 *     PCM capacity is queryable immediately; format is discovered from ASC or
 *     decoded packets and may change with implicit SBR/PS.
 *   - Every fallible call returns a faad_status; faad_strerror() maps a status
 *     to a human-readable string.
 *   - Fixed-width integer types and width-pinned enums preserve the ABI within
 *     each target platform, including builds using -fshort-enums.
 *   - faad_config and the info structs support size-based extension;
 *     callers MUST set struct_size (faad_config_init() does) so the library
 *     can reconcile versions. Existing offsets, meanings, enum values, and
 *     signatures and struct alignment never change within a released major ABI
 *     version. Appended fields start at or after the previous sizeof(struct);
 *     compiler padding is never reused and baseline sizes stay fixed.
 *     faad_config and faad_stream_info are 4-byte aligned: append only members
 *     of 4 bytes or less. Structs holding pointers take members at multiples
 *     of their size, in blocks of 8 bytes. Zero in a new field keeps the
 *     previous behavior. Callers check library support through the struct_size
 *     the library writes back.
 *   - Independent handles may be used concurrently; one handle is owned by
 *     one thread at a time. Instrumented stats builds are process-global.
 */

#ifndef FAAD3_H
#define FAAD3_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

#include <stddef.h>
#include <stdint.h>

#define FAAD_VERSION_MAJOR 3
#define FAAD_VERSION_MINOR 0
#define FAAD_VERSION_PATCH 0
#define FAAD_STR_(x) #x
#define FAAD_STR(x) FAAD_STR_(x)
#define FAAD_VERSION_STRING \
    FAAD_STR(FAAD_VERSION_MAJOR) "." FAAD_STR(FAAD_VERSION_MINOR) "." FAAD_STR(FAAD_VERSION_PATCH)
#define FAAD_VERSION_HEX \
    ((FAAD_VERSION_MAJOR << 16) | (FAAD_VERSION_MINOR << 8) | FAAD_VERSION_PATCH)

/* Windows exports from the DLL build only (FAAD_BUILDING). No dllimport: the API
 * has no data, so DLL and static consumers link the same way with no define. */
#ifndef FAADAPI
# if defined(_WIN32)
#  ifdef FAAD_BUILDING
#   define FAADAPI __declspec(dllexport)
#  else
#   define FAADAPI
#  endif
# elif defined(__GNUC__) && (__GNUC__ >= 4)
#  define FAADAPI __attribute__((visibility("default")))
# else
#  define FAADAPI
# endif
#endif

typedef struct faad_decoder faad_decoder;

/*
 * Result codes. All values are negative except FAAD_OK so a caller can test
 * `status < 0` for failure. The FAAD_STATUS_MAX sentinel pins the enum to a
 * 32-bit underlying type for a stable ABI under -fshort-enums.
 */
typedef enum faad_status {
    FAAD_OK                   = 0,
    FAAD_ERR_INVALID_ARGUMENT = -1,  /* NULL pointer or bad arguments                        */
    FAAD_ERR_UNSUPPORTED      = -2,  /* unsupported profile or bitstream feature             */
    FAAD_ERR_NO_MEMORY        = -3,  /* allocation failed or provided state block too small   */
    FAAD_ERR_OUTPUT_TOO_SMALL = -4,  /* provided PCM output buffer capacity too small        */
    FAAD_ERR_NEED_MORE_DATA   = -5,  /* input buffer doesn't contain a full frame            */
    FAAD_ERR_DECODE_FAILED    = -6,  /* bitstream corruption or DSP error                    */
    FAAD_ERR_SYNC_LOST        = -7,  /* lost frame sync (e.g., bad ADTS syncword)            */
    FAAD_STATUS_MAX           = 0x7fffffff
} faad_status;

enum faad_library_flag {
    FAAD_LIB_SBR = 1u << 0,
    FAAD_LIB_PS = 1u << 1,
    FAAD_LIB_DAB = 1u << 2,         /* Digital Audio Broadcasting: 960/120-sample frames */
    FAAD_LIB_DRC = 1u << 3,         /* Dynamic range control support is compiled in */
    FAAD_LIB_FLAG_MAX = 0x7fffffff
};

/*
 * Library-global facts, independent of any decoder instance. These reflect
 * build-time options (max-channels, decoder-sbr, decoder-ps, decoder-dab, decoder-drc) that the header
 * alone cannot report, so they are queried at runtime. Set out_info->struct_size to
 * sizeof(faad_library_info) before the call; the struct grows only by
 * appending fields. struct_size reports bytes populated. version/copyright
 * are static and library-owned; do not free them.
 */
typedef struct faad_library_info {
    uint32_t                struct_size;    /* set by caller to sizeof(faad_library_info) */

    const char             *version;        /* library version string   (library-owned) */
    const char             *copyright;      /* library copyright string (library-owned) */

    uint32_t                max_channels;   /* highest channel count this build decodes */
    uint32_t                flags;          /* FAAD_LIB_* */
} faad_library_info;

FAADAPI faad_status faad_get_library_info(faad_library_info *out_info);

/* AAC object type, numbered per the MPEG-4 Audio Object Type (AOT) registry. */
enum faad_object_type {
    FAAD_OBJ_NULL      = 0,          /* unset                                */
    FAAD_OBJ_LC        = 2,          /* AAC-LC (Low Complexity)              */
    FAAD_OBJ_HE_AAC_V1 = 5,          /* AAC-LC + SBR                         */
    FAAD_OBJ_HE_AAC_V2 = 29,         /* AAC-LC + SBR + PS                    */
    FAAD_OBJ_MAX       = 0x7fffffff
};

enum faad_stream_format {
    FAAD_STREAM_RAW  = 0,            /* raw AAC access units (needs out-of-band ASC) */
    FAAD_STREAM_ADTS = 1,            /* self-framing ADTS                             */
    FAAD_STREAM_MAX  = 0x7fffffff
};

/* Interpretation of the interleaved PCM produced by faad_decode_frame(). */
enum faad_output_format {
    FAAD_OUTPUT_NULL  = 0,           /* invalid / unset                          */
    FAAD_OUTPUT_16BIT,               /* native-endian int16                      */
    FAAD_OUTPUT_24BIT,               /* native-endian int24 packed in 3 bytes   */
    FAAD_OUTPUT_24IN32,              /* 24 significant bits in a 32-bit word     */
    FAAD_OUTPUT_FLOAT,               /* 32-bit float, unity full scale (+-1.0)   */
    FAAD_OUTPUT_MAX   = 0x7fffffff
};

enum faad_downmix_mode {
    FAAD_DOWNMIX_NONE       = 0,         /* preserve native channel layout           */
    FAAD_DOWNMIX_MONO       = 0x4,       /* downmix all channels to mono (front centre) */
    FAAD_DOWNMIX_STEREO     = 0x3,       /* downmix surround to stereo (front pair)  */
    FAAD_DOWNMIX_3_0        = 0x7,       /* 3.0 surround (FL FR FC)                  */
    FAAD_DOWNMIX_4_0        = 0x33,      /* 4.0 quad (FL FR BL BR)                   */
    FAAD_DOWNMIX_5_0        = 0x37,      /* 5.0 surround (FL FR FC BL BR)            */
    FAAD_DOWNMIX_5_1        = 0x3f,      /* 5.1 surround (FL FR FC LFE BL BR)        */
    FAAD_DOWNMIX_5_0_SIDE   = 0x607,     /* 5.0 side (FL FR FC SL SR)                */
    FAAD_DOWNMIX_5_1_SIDE   = 0x60f,     /* 5.1 side (FL FR FC LFE SL SR)            */
    FAAD_DOWNMIX_MAX        = 0x7fffffff
};

/*
 * Decoder configuration, supplied once at initialization.
 *
 * ALWAYS initialize with faad_config_init() before setting fields: it stamps
 * struct_size, which is how the library stays compatible as this struct grows.
 */
typedef struct faad_config {
    uint32_t                struct_size;   /* set by faad_config_init() */

    enum faad_stream_format stream_format; /* RAW or ADTS                   */
    enum faad_output_format output_format; /* PCM sample format             */
    enum faad_downmix_mode  downmix_mode;  /* channel downmixing strategy   */
    uint8_t                 drc_cut;          /* attenuation scale, 0..127 */
    uint8_t                 drc_boost;        /* boost scale, 0..127 */
    float                   drc_target_level; /* 0.0 off, else -31.75..-10.0 dBFS, snapped to 0.25 dB */
} faad_config;

enum faad_stream_flag {
    FAAD_STREAM_FORMAT_KNOWN  = 1u << 0,
    FAAD_STREAM_DRC_REF_LEVEL = 1u << 1, /* drc_ref_level carries a valid program reference level */
    FAAD_STREAM_FLAG_MAX      = 0x7fffffff
};

/*
 * Describes the most recently emitted PCM; before any PCM, the ASC/ADTS snapshot. Set
 * struct_size to sizeof(faad_stream_info) before faad_decoder_get_info(); it is
 * updated to the bytes populated.
 *
 * Configured output order (WAVE order, so it can be fed straight back to the
 * encoder as FAAC_LAYOUT_WAV): mono FC; stereo FL FR; surround FL FR FC, then BC
 * (4), BL BR (5), LFE BL BR (6), or LFE BL BR SL SR (8). Eight channels from
 * channelConfiguration 0 with an equivalent PCE are reported in WAVE order
 * with side and back pairs. FAAC signals front C/front pair, side pair,
 * back pair and LFE, unambiguously for Apple, FDK, FFmpeg and faad.
 * Configuration 7 streams are read as 7.1 wide: FL FR FC LFE BL BR FLC FRC;
 * configuration 12 is the side/back 7.1 and 11 is 6.1.
 * Other PCE layouts are placed by their elements' speakers; a layout that cannot be
 * placed retains element order and reports mask 0.
 */
typedef struct faad_stream_info {
    uint32_t                struct_size;      /* set by caller to sizeof(faad_stream_info) */

    uint32_t                sample_rate;      /* Resolved output sample rate in Hz */
    uint32_t                channels;         /* Resolved output channel count */
    enum faad_object_type   object_type;      /* Detected object type */
    uint32_t                decoder_delay;    /* Additional delay excluded from container priming,
                                               * in samples/channel at sample_rate (0 LC, 962 SBR) */
    uint32_t                frame_samples;    /* 1024, or 2048 with upsampled SBR output */
    uint32_t                max_output_bytes; /* Lifetime PCM capacity, including implicit SBR/PS */
    uint32_t                channel_mask;     /* WAVE speaker mask; 0 for unknown layout */
    uint32_t                flags;            /* FAAD_STREAM_* */
    float                   drc_ref_level;    /* program reference level in dBFS (<= 0; the LUFS value faac wrote);
                                               * valid only when flags has FAAD_STREAM_DRC_REF_LEVEL */
} faad_stream_info;

/* Offset just past the named field: the smallest struct size the library accepts. */
#define FAAD_STRUCT_END(type, field) \
    (offsetof(type, field) + sizeof(((type *)0)->field))
#define FAAD_CONFIG_BASELINE FAAD_STRUCT_END(faad_config, downmix_mode)
#define FAAD_LIBRARY_INFO_BASELINE FAAD_STRUCT_END(faad_library_info, flags)
#define FAAD_STREAM_INFO_BASELINE FAAD_STRUCT_END(faad_stream_info, flags)

enum faad_frame_flag {
    FAAD_FRAME_FORMAT_CHANGED = 1u << 0, /* Emitted PCM format differs from the previous emitted frame;
                                         * always set on the first. Re-query faad_decoder_get_info(). */
    FAAD_FRAME_SBR            = 1u << 1, /* SBR applied */
    FAAD_FRAME_PS             = 1u << 2, /* Parametric Stereo applied */
    FAAD_FRAME_CONCEALED      = 1u << 3, /* Replacement audio for corrupt core data */
    FAAD_FRAME_DEGRADED       = 1u << 4, /* Recovered audio, e.g. damaged SBR with intact core */
    FAAD_FRAME_DRC            = 1u << 5, /* Compression or normalization applied */
    FAAD_FRAME_DRC_INFO       = 1u << 6, /* The frame carried dynamic_range_info, applied or not */
    FAAD_FRAME_FLAG_MAX       = 0x7fffffff
};


/* Zero-initialize *config and fill in library defaults and struct_size. Pass
 * sizeof(*config) as caller_size; never writes past min(caller_size,
 * sizeof(faad_config)). Returns FAAD_ERR_INVALID_ARGUMENT if config is NULL or
 * caller_size is smaller than faad_config's baseline layout. */
FAADAPI faad_status faad_config_init(faad_config *config, uint32_t caller_size);


/*
 * Allocate decoder state in independent blocks.
 * asc_buf must contain a valid, nonempty AudioSpecificConfig for RAW streams;
 * it is optional for ADTS. Configuration and ASC need not outlive this call.
 */
FAADAPI faad_status faad_decoder_open(const faad_config *config,
                                        const uint8_t *asc_buf, uint32_t asc_len,
                                        faad_decoder **out);

/*
 * Destroy a decoder. Pass the address of your handle; on success the handle is
 * set to NULL and all instance allocations are freed. A pointer to
 * a NULL handle is a no-op that returns FAAD_OK; a NULL pointer is invalid.
 */
FAADAPI faad_status faad_decoder_close(faad_decoder **decoder);

/*
 * Query current stream metadata. Safe immediately after open. Before ADTS
 * discovery, FAAD_STREAM_FORMAT_KNOWN is clear in flags and format fields
 * are zero/FAAD_OBJ_NULL.
 * ASC may supply a usable snapshot, but implicit SBR/PS can change it later;
 * frame metadata is authoritative for emitted PCM. Set
 * out_info->struct_size to sizeof(faad_stream_info); smaller baseline layouts
 * are rejected, larger layouts are accepted and only known bytes are written.
 * max_output_bytes is valid even before the first frame and across flushes;
 * ADTS uses the build's channel capacity; RAW layouts (including ASC PCEs) use their count
 * (at least two with PS support), and downmix reduces that bound.
 */
FAADAPI faad_status faad_decoder_get_info(const faad_decoder *decoder, faad_stream_info *out_info);

/*
 * Discard audio history on seek/discontinuity, including concealment and
 * synthesis history. Preserves configuration, discovered format and capacity.
 * Does not drain PCM or generate missing packets. Reopen for a different stream.
 */
FAADAPI faad_status faad_decoder_flush(faad_decoder *decoder);

/*
 * Decode exactly ONE AAC access unit (packet/frame), zero-copy on the incoming
 * payload.
 *
 * in_buf         - a single ADTS frame or RAW access unit
 * in_bytes       - size of in_buf
 * bytes_consumed - returns the number of bitstream bytes parsed
 * out_pcm        - caller-allocated PCM output, formatted per output_format;
 *                  aligned for int16_t/int32_t/float (packed 24-bit: any alignment)
 * out_cap_bytes  - must be at least stream_info.max_output_bytes
 * bytes_written  - returns the number of PCM bytes generated
 * frame_flags    - optional; zeroed on entry. Callers must ignore unknown bits.
 *                  On FORMAT_CHANGED, re-query faad_decoder_get_info() before
 *                  using the PCM. Format comprises sample_rate, channels,
 *                  frame_samples, channel_mask and decoder_delay.
 *
 * Byte counts are zeroed on entry. Invalid arguments, insufficient PCM capacity,
 * and NEED_MORE_DATA consume/emit nothing and preserve decoder state, so retry
 * with the same input is safe. ADTS input can contain subsequent frames; only
 * the first is decoded. Sync loss reports skipped bytes (retaining a trailing
 * possible sync prefix). Complete unsupported/rejected frames report consumed
 * bytes and emit nothing; advance past them rather than retrying. RAW input
 * must contain one complete access unit; its framing is the caller's job.
 * Concealed/degraded PCM returns FAAD_OK with frame flags. With no emitted PCM,
 * frame_flags remains zero and the stream_info format is unchanged. Flush
 * preserves the last emitted format; it does not force FORMAT_CHANGED.
 */
FAADAPI faad_status faad_decode_frame(faad_decoder *decoder,
                                      const uint8_t *in_buf, uint32_t in_bytes,
                                      uint32_t *bytes_consumed,
                                      void *out_pcm, uint32_t out_cap_bytes,
                                      uint32_t *bytes_written,
                                      uint32_t *frame_flags);

/* Human-readable, static description of a status code. Never returns NULL. */
FAADAPI const char *faad_strerror(faad_status status);

#ifdef __cplusplus
}
#endif /* __cplusplus */

#endif /* FAAD3_H */
