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
 * Designed for bare-metal, RTOS, and zero-allocation environments.
 *
 * Design summary:
 *   - Configuration is supplied once, up front, to faad_decoder_init() or
 *     faad_decoder_open(). PCM capacity is queryable immediately; format is
 *     discovered from ASC or decoded packets and may change with implicit SBR/PS.
 *   - Every fallible call returns a faad_status; faad_strerror() maps a status
 *     to a human-readable string.
 *   - Fixed-width integer types and width-pinned enums preserve the ABI within
 *     each target platform, including builds using -fshort-enums.
 *   - faad_config and the info structs grow only by appending named fields;
 *     callers MUST set struct_size (faad_config_init() does) so the library
 *     can reconcile versions. Existing offsets, meanings, enum values, and
 *     signatures and struct alignment never change within a released major ABI
 *     version. Explicit tail padding is never reused for new fields.
 *   - Independent handles may be used concurrently; one handle is owned by
 *     one thread at a time. Instrumented stats builds are process-global.
 */

#ifndef FAAD_H
#define FAAD_H

#ifdef __cplusplus
extern "C" {
#endif /* __cplusplus */

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#define FAAD_VERSION_MAJOR 3
#define FAAD_VERSION_MINOR 0
#define FAAD_VERSION_PATCH 0
#define FAAD_STR_(x) #x
#define FAAD_STR(x) FAAD_STR_(x)
#define FAAD_VERSION_STRING \
    FAAD_STR(FAAD_VERSION_MAJOR) "." FAAD_STR(FAAD_VERSION_MINOR) "." FAAD_STR(FAAD_VERSION_PATCH)
#define FAAD_VERSION_HEX \
    ((FAAD_VERSION_MAJOR << 16) | (FAAD_VERSION_MINOR << 8) | FAAD_VERSION_PATCH)

/* Export/visibility marker. */
#ifndef FAADAPI
# if defined(_WIN32)
#  define FAADAPI __declspec(dllexport)
# elif defined(__GNUC__) && (__GNUC__ >= 4)
#  define FAADAPI __attribute__((visibility("default")))
# else
#  define FAADAPI
# endif
#endif

/* Opaque decoder handle */
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
    FAAD_ERR_INSUFFICIENT_MEM = -3,  /* allocation failed or provided state block too small   */
    FAAD_ERR_OUTPUT_TOO_SMALL = -4,  /* provided PCM output buffer capacity too small        */
    FAAD_ERR_NEED_MORE_DATA   = -5,  /* input buffer doesn't contain a full frame            */
    FAAD_ERR_DECODE_FAILED    = -6,  /* bitstream corruption or DSP error                    */
    FAAD_ERR_SYNC_LOST        = -7,  /* lost frame sync (e.g., bad ADTS syncword)            */
    FAAD_STATUS_MAX           = 0x7fffffff
} faad_status;

/*
 * Library-global facts, independent of any decoder instance. These reflect
 * build-time options (max-channels, decoder-sbr, decoder-ps) that the header
 * alone cannot report, so they are queried at runtime. Set out->struct_size to
 * sizeof(faad_library_info) before the call; the struct grows only by
 * appending fields. version/copyright are static and library-owned; do not
 * free them.
 */
typedef struct faad_library_info {
    uint32_t                struct_size;    /* set by caller to sizeof(faad_library_info) */

    const char             *version;        /* library version string   (library-owned) */
    const char             *copyright;      /* library copyright string (library-owned) */

    uint32_t                max_channels;   /* highest channel count this build decodes */
    bool                    sbr_supported;  /* SBR compiled in                          */
    bool                    ps_supported;   /* Parametric Stereo compiled in            */
    uint8_t                 reserved[2];    /* explicit tail padding; always zero, never reuse */
} faad_library_info;

FAADAPI faad_status faad_get_library_info(faad_library_info *out);

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
    FAAD_OUTPUT_32BIT,               /* native-endian int24 in 32 bits           */
    FAAD_OUTPUT_FLOAT,               /* 32-bit float                             */
    FAAD_OUTPUT_MAX   = 0x7fffffff
};

enum faad_downmix_mode {
    FAAD_DOWNMIX_NONE   = 0,         /* preserve native channel layout           */
    FAAD_DOWNMIX_MONO   = 1,         /* downmix all channels to mono             */
    FAAD_DOWNMIX_STEREO = 2,         /* downmix surround to 2-channel stereo     */
    FAAD_DOWNMIX_MAX    = 0x7fffffff
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
} faad_config;

/*
 * Caller-owned decoder state must be aligned to this boundary. The size query
 * includes instance storage, not shared tables or caller-owned input/PCM.
 */
#define FAAD_STATE_ALIGNMENT 16u

/*
 * Describes the most recently emitted PCM; before any PCM, the ASC/ADTS snapshot. Set
 * struct_size to sizeof(faad_stream_info) before faad_decoder_get_info(); it is
 * updated to the bytes populated.
 *
 * Configured output order: mono FC; stereo FL FR; surround FL FR FC, then BC
 * (4), BL BR (5), LFE BL BR (6), or LFE BL BR SL SR (8). PCE surround element
 * order has no fixed WAVE layout and reports mask 0.
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
    bool                    format_known;     /* Usable rate/channel snapshot; later frames may change it */
    uint8_t                 reserved[3];      /* explicit tail padding; always zero, never reuse */
} faad_stream_info;

enum faad_frame_flag {
    FAAD_FRAME_FORMAT_CHANGED = 1u << 0, /* Emitted PCM format differs from the previous emitted frame;
                                         * always set on the first. Re-query faad_decoder_get_info(). */
    FAAD_FRAME_SBR            = 1u << 1, /* SBR applied */
    FAAD_FRAME_PS             = 1u << 2, /* Parametric Stereo applied */
    FAAD_FRAME_CONCEALED      = 1u << 3, /* Replacement audio for corrupt core data */
    FAAD_FRAME_DEGRADED       = 1u << 4, /* Recovered audio, e.g. damaged SBR with intact core */
    FAAD_FRAME_FLAG_MAX       = 0x7fffffff
};


/* Zero-initialize *cfg and fill in library defaults and struct_size. Pass
 * sizeof(*cfg) as caller_size; never writes past min(caller_size,
 * sizeof(faad_config)). Returns FAAD_ERR_INVALID_ARGUMENT if cfg is NULL or
 * caller_size is smaller than faad_config's baseline layout. */
FAADAPI faad_status faad_config_init(faad_config *cfg, uint32_t caller_size);


/*
 * Query the exact bytes of instance storage required to instantiate the decoder.
 * The size is independent of configuration. Embedded applications use this
 * to allocate static .bss/.dram memory.
 */
FAADAPI faad_status faad_get_state_size(uint32_t *state_bytes_out);

/*
 * Initialize the decoder in a caller-provided memory block, with zero internal
 * heap allocations (no malloc/free).
 *
 * mem_buf  - static memory block, aligned to FAAD_STATE_ALIGNMENT
 * mem_size - size of mem_buf, at least the size faad_get_state_size() returned
 * asc_buf  - valid, nonempty AudioSpecificConfig required for RAW streams;
 *            optional for ADTS. Configuration/ASC need not outlive this call.
 * asc_len  - length of asc_buf
 */
FAADAPI faad_status faad_decoder_init(void *mem_buf, uint32_t mem_size,
                                      const faad_config *cfg,
                                      const uint8_t *asc_buf, uint32_t asc_len,
                                      faad_decoder **out_dec);

/* Desktop convenience wrapper: allocates the state block with malloc(). */
FAADAPI faad_status faad_decoder_open(const faad_config *cfg,
                                        const uint8_t *asc_buf, uint32_t asc_len,
                                        faad_decoder **out_dec);

/*
 * Destroy a decoder. Pass the address of your handle; on success the handle is
 * set to NULL. Frees only memory allocated by faad_decoder_open(). A pointer to
 * a NULL handle is a no-op that returns FAAD_OK; a NULL pointer is invalid.
 */
FAADAPI faad_status faad_decoder_close(faad_decoder **dec);

/*
 * Query current stream metadata. Safe immediately after init. Before ADTS
 * discovery, format_known is false and format fields are zero/FAAD_OBJ_NULL.
 * ASC may supply a usable snapshot, but implicit SBR/PS can change it later;
 * frame metadata is authoritative for emitted PCM. Set
 * out_info->struct_size to sizeof(faad_stream_info); smaller baseline layouts
 * are rejected, larger layouts are accepted and only known bytes are written.
 * max_output_bytes is valid even before the first frame and across flushes;
 * ADTS/PCE use the build's channel capacity, fixed RAW layouts use their count
 * (at least two with PS support), and downmix reduces that bound.
 */
FAADAPI faad_status faad_decoder_get_info(const faad_decoder *dec, faad_stream_info *out_info);

/*
 * Discard audio history on seek/discontinuity, including concealment and
 * synthesis history. Preserves configuration, discovered format and capacity.
 * Does not drain PCM or generate missing packets. Reopen for a different stream.
 */
FAADAPI faad_status faad_decoder_flush(faad_decoder *dec);

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
FAADAPI faad_status faad_decode_frame(faad_decoder *dec,
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

#endif /* FAAD_H */
