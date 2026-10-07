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

/* AudioSpecificConfig parser and builder for core AAC-LC with an implicit
 * SBR/PS sync extension. Header-only with internal linkage, so each library
 * and frontend compiles its own copy. */

#ifndef FAAC_ASC_CODEC_H
#define FAAC_ASC_CODEC_H

#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#define ASC_SYNC_EXTENSION_SBR 0x2b7u
#define ASC_SYNC_EXTENSION_PS  0x548u

/* ISO/IEC 14496-3 Table 1.16 sampling_frequency_index. */
static const uint32_t asc_codec_sample_rates[16] = {
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050,
    16000, 12000, 11025, 8000, 7350, 0, 0, 0
};

/* Table 1.16 index of a rate, or -1 when the table has none (an ASC can
 * carry any rate explicitly; an index-only field such as ADTS cannot). */
static inline int asc_codec_sr_idx(uint32_t rate)
{
    for (int i = 0; i < 13; i++) {
        if (asc_codec_sample_rates[i] == rate) return i;
    }
    return -1;
}

typedef struct {
    uint8_t  object_type;     /* core AOT (2 = AAC-LC) */
    uint32_t sample_rate;     /* core sample rate, Hz */
    uint8_t  num_channels;
    bool     sbr_present;
    uint32_t sbr_sample_rate; /* post-SBR (extension) rate, Hz; == 2*sample_rate if not explicitly signaled */
    bool     ps_present;
    bool     frame_length_flag; /* 0 = 1024 (standard), 1 = 960 (DRM / short frame) */
} AscInfo;

typedef struct { const uint8_t *buf; uint32_t len_bits; uint32_t pos; } asc_bitreader;

static inline uint32_t asc_br_get(asc_bitreader *br, uint32_t n)
{
    uint32_t val = 0;
    for (uint32_t i = 0; i < n; i++) {
        uint32_t bitpos = br->pos + i;
        uint32_t bit = 0;
        if (bitpos < br->len_bits) {
            bit = (br->buf[bitpos >> 3] >> (7 - (bitpos & 7))) & 1;
        }
        val = (val << 1) | bit;
    }
    br->pos += n;
    return val;
}

static inline uint32_t asc_br_remaining(const asc_bitreader *br)
{
    return br->pos < br->len_bits ? br->len_bits - br->pos : 0;
}

static inline uint32_t asc_parse_sample_rate(asc_bitreader *br)
{
    uint32_t idx = asc_br_get(br, 4);
    return idx == 15 ? asc_br_get(br, 24) : asc_codec_sample_rates[idx];
}

/*
 * Parses aot/sample_rate/channels, then either the "explicit" nested-AOT
 * HE-AAC form (top-level AOT==5) or, for a plain-LC ASC, walks
 * GASpecificConfig (frameLengthFlag/dependsOnCoreCoder[/coreCoderDelay]/
 * extensionFlag[/extensionFlag3]) before scanning for the "implicit"
 * trailing 0x2b7/0x548 sync-extension -- the form real MP4/M4A muxers
 * (including this project's own) actually write. asc_len is in bytes.
 *
 * PCE parsing (needed only when channelConfiguration==0, i.e. a
 * program_config_element carries the real channel layout) is intentionally
 * not implemented here; num_channels is left 0 and the implicit-extension
 * scan is skipped in that case.
 */
static inline void asc_codec_parse(const uint8_t *buf, uint32_t len, AscInfo *out)
{
    memset(out, 0, sizeof(*out));
    if (!buf || len < 2) return;

    asc_bitreader br = { buf, len * 8, 0 };

    uint32_t aot = asc_br_get(&br, 5);
    if (aot == 31) aot = 32 + asc_br_get(&br, 6);
    out->object_type = (uint8_t)aot;

    out->sample_rate = asc_parse_sample_rate(&br);
    out->num_channels = (uint8_t)asc_br_get(&br, 4);

    if (aot == 5 || aot == 29) {
        /* Explicit hierarchical form: the SBR (29: SBR+PS) object wraps the
         * core object and carries the output sampling frequency itself. */
        out->sbr_present = true;
        out->ps_present = (aot == 29);
        out->sbr_sample_rate = asc_parse_sample_rate(&br);
        uint32_t real_aot = asc_br_get(&br, 5);
        if (real_aot == 31) real_aot = 32 + asc_br_get(&br, 6);
        out->object_type = (uint8_t)real_aot;
        return;
    }

    out->sbr_sample_rate = out->sample_rate * 2;

    if (aot != 2) return; /* GASpecificConfig only walked here for AAC-LC */

    out->frame_length_flag = asc_br_get(&br, 1) != 0; /* frameLengthFlag */
    if (asc_br_get(&br, 1)) {
        asc_br_get(&br, 14); /* coreCoderDelay, iff dependsOnCoreCoder */
    }
    bool extension_flag = asc_br_get(&br, 1) != 0;
    if (out->num_channels == 0) return; /* PCE follows; not handled here */
    if (extension_flag) {
        asc_br_get(&br, 1); /* extensionFlag3 (reserved for AOT LC) */
    }

    if (asc_br_remaining(&br) >= 16 && asc_br_get(&br, 11) == ASC_SYNC_EXTENSION_SBR) {
        uint32_t ext_aot = asc_br_get(&br, 5);
        /* sbrPresentFlag is a real bit: an encoder may append the extension
         * to state explicitly that there is no SBR, and then
         * no sample-rate index follows. */
        if (ext_aot == 5 && asc_br_get(&br, 1)) {
            out->sbr_present = true;
            out->sbr_sample_rate = asc_parse_sample_rate(&br);
            if (asc_br_remaining(&br) >= 12 && asc_br_get(&br, 11) == ASC_SYNC_EXTENSION_PS) {
                out->ps_present = asc_br_get(&br, 1) != 0;
            }
        }
    }
}

typedef struct { uint8_t *buf; uint32_t cap_bits; uint32_t pos; } asc_bitwriter;

static inline void asc_bw_put(asc_bitwriter *bw, uint32_t val, uint32_t n)
{
    /* Whole byte-sized chunks rather than bit-at-a-time: the constant widths
     * at each call site made the compiler unroll a per-bit loop into
     * roughly 3 KB of straight-line code. */
    while (n > 0) {
        uint32_t byte_idx = bw->pos >> 3;
        uint32_t room = 8 - (bw->pos & 7);
        uint32_t take = n < room ? n : room;
        if ((byte_idx << 3) >= bw->cap_bits) {
            bw->pos += n;
            return;
        }
        uint32_t chunk = (val >> (n - take)) & ((1u << take) - 1);
        bw->buf[byte_idx] |= (uint8_t)(chunk << (room - take));
        bw->pos += take;
        n -= take;
    }
}

typedef struct {
    uint8_t object_type; /* core AOT, e.g. 2 = AAC-LC */
    uint8_t sr_idx;      /* Table 1.16 index for the core rate */
    uint8_t channels;
    bool    sbr_present;
    uint8_t sbr_sr_idx;  /* Table 1.16 index for the post-SBR (output) rate; ignored if !sbr_present */
    bool    ps_signaled; /* emit the PS sync-extension block at all (only meaningful if sbr_present) */
    bool    ps_present;  /* psPresentFlag value inside that block, if ps_signaled */
    bool    hierarchical; /* explicit hierarchical form (AOT 5/29 wrapping the core) instead of the sync-extension */
} AscBuildInfo;

/*
 * Builds a core-LC ASC, with an implicit SBR sync-extension (and, if
 * ps_signaled, a nested PS sync-extension) appended when sbr_present is
 * set. Returns the number of bytes written, or 0 if out_cap is too small.
 */
static inline uint32_t asc_codec_build(const AscBuildInfo *info, uint8_t *out, uint32_t out_cap)
{
    if (!info || !out || out_cap < 2) return 0;

    memset(out, 0, out_cap);
    asc_bitwriter bw = { out, out_cap * 8, 0 };

    if (info->sbr_present && info->hierarchical) {
        if (out_cap < 4) return 0;
        asc_bw_put(&bw, info->ps_signaled && info->ps_present ? 29 : 5, 5);
        asc_bw_put(&bw, info->sr_idx, 4);
        asc_bw_put(&bw, info->channels & 0x0F, 4);
        asc_bw_put(&bw, info->sbr_sr_idx, 4);
    }

    uint8_t obj = info->object_type;
    if (obj >= 32) {
        /* AOT-escape form, mirroring asc_codec_parse()'s aot==31 read above. */
        asc_bw_put(&bw, 31, 5);
        asc_bw_put(&bw, obj - 32, 6);
    } else {
        asc_bw_put(&bw, obj, 5);
    }
    if (!(info->sbr_present && info->hierarchical)) {
        asc_bw_put(&bw, info->sr_idx, 4);
        asc_bw_put(&bw, info->channels & 0x0F, 4);
    }
    asc_bw_put(&bw, 0, 3); /* frameLengthFlag, dependsOnCoreCoder, extensionFlag */

    if (info->sbr_present && !info->hierarchical) {
        asc_bw_put(&bw, ASC_SYNC_EXTENSION_SBR, 11);
        asc_bw_put(&bw, 5, 5); /* extensionAudioObjectType = SBR */
        asc_bw_put(&bw, 1, 1); /* sbrPresentFlag */
        asc_bw_put(&bw, info->sbr_sr_idx, 4);
        if (info->ps_signaled) {
            asc_bw_put(&bw, ASC_SYNC_EXTENSION_PS, 11);
            asc_bw_put(&bw, info->ps_present ? 1 : 0, 1);
        }
    }

    return (bw.pos + 7) / 8;
}

#endif /* FAAC_ASC_CODEC_H */
