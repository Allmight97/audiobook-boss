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

/* Tags in config-7 native order: center, front, side, back, LFE.
 * Unrecognized PCEs retain only their channel count. */
typedef struct {
    uint8_t channels;
    uint8_t profile, sr_index;
    uint8_t tags[5];
    bool wave71;
    /* Where the fields sit in the parsed buffer, so the element can be
     * rewritten into another one: start bit, length before the byte
     * alignment, and comment bytes. */
    uint16_t pos, body_bits;
    uint8_t comment_bytes;
} AscPce;

typedef struct {
    uint8_t  object_type;     /* core AOT (2 = AAC-LC) */
    uint32_t sample_rate;     /* core sample rate, Hz */
    uint8_t  num_channels;
    uint8_t  channel_config;
    AscPce   pce;
    bool     sbr_present;
    uint32_t sbr_sample_rate; /* post-SBR (extension) rate, Hz; == 2*sample_rate if not explicitly signaled */
    bool     ps_present;
    bool     frame_length_flag; /* 0 = 1024 (standard), 1 = 960 (DRM / short frame) */
} AscInfo;

typedef struct { const uint8_t *buf; uint32_t len_bits; uint32_t pos; } asc_bitreader;

static inline uint32_t asc_br_get(asc_bitreader *br, uint32_t n)
{
    uint32_t val = 0;
    for (; n; n--, br->pos++) {
        uint32_t bit = br->pos < br->len_bits ? (br->buf[br->pos >> 3] >> (7 - (br->pos & 7))) & 1 : 0;
        val = (val << 1) | bit;
    }
    return val;
}

static inline uint32_t asc_br_remaining(const asc_bitreader *br)
{
    return br->pos < br->len_bits ? br->len_bits - br->pos : 0;
}

/* Some muxers (Audible's among them) set dependsOnCoreCoder on a plain AAC-LC
 * config yet leave out the 14-bit coreCoderDelay, ending the ASC right after
 * extensionFlag. A conforming ASC always has room for the delay plus
 * extensionFlag, so the delay is read only when those 15 bits are present. */
static inline void asc_skip_core_coder_delay(asc_bitreader *br)
{
    if (asc_br_remaining(br) >= 15) asc_br_get(br, 14);
}

static inline uint32_t asc_parse_sample_rate(asc_bitreader *br)
{
    uint32_t idx = asc_br_get(br, 4);
    return idx == 15 ? asc_br_get(br, 24) : asc_codec_sample_rates[idx];
}

/* Two back pairs use the rear-surround convention: first side, then back.
 * Pair tags must be distinct, since tags select elements, not positions. */
static inline bool asc_pce_wave71(const AscPce *pce, uint32_t front,
                                uint32_t side, uint32_t back, uint32_t lfe,
                                bool pairs)
{
    return pairs && front == 2 && lfe == 1
        && ((side == 0 && back == 2) || (side == 1 && back == 1))
        && pce->tags[1] != pce->tags[2] && pce->tags[1] != pce->tags[3]
        && pce->tags[2] != pce->tags[3];
}

/* PCE lists carry element tags rather than PCM positions. Count only audio
 * elements, but skip every optional field so SBR starts at the correct bit. */
static inline uint8_t asc_parse_pce(asc_bitreader *br, AscPce *pce)
{
    memset(pce, 0, sizeof(*pce));
    uint32_t start = br->pos;
    if (start > UINT16_MAX || asc_br_remaining(br) < 34) return 0;
    asc_br_get(br, 4);
    pce->profile = (uint8_t)asc_br_get(br, 2);
    pce->sr_index = (uint8_t)asc_br_get(br, 4);
    uint32_t front = asc_br_get(br, 4);
    uint32_t side = asc_br_get(br, 4);
    uint32_t back = asc_br_get(br, 4);
    uint32_t lfe = asc_br_get(br, 2);
    uint32_t assoc = asc_br_get(br, 3);
    uint32_t cc = asc_br_get(br, 4);
    bool pairs = true;
    if (asc_br_get(br, 1)) asc_br_get(br, 4);
    if (asc_br_get(br, 1)) asc_br_get(br, 4);
    if (asc_br_get(br, 1)) asc_br_get(br, 3);
    uint32_t channels = lfe, elements = front + side + back;
    if (br->pos > br->len_bits || asc_br_remaining(br) < elements * 5 + lfe * 4 + assoc * 4 + cc * 5)
        return 0;
    for (uint32_t i = 0; i < elements; i++) {
        uint32_t pair = asc_br_get(br, 1);
        uint8_t tag = (uint8_t)asc_br_get(br, 4);
        channels += 1 + pair;
        if (i < 4) pce->tags[i] = tag;
        pairs = pairs && pair == (i != 0);
    }
    for (uint32_t i = 0; i < lfe; i++) {
        uint8_t tag = (uint8_t)asc_br_get(br, 4);
        if (i == 0) pce->tags[4] = tag;
    }
    br->pos += assoc * 4 + cc * 5;
    pce->body_bits = (uint16_t)(br->pos - start);
    asc_br_get(br, (8 - (br->pos & 7)) & 7);
    if (asc_br_remaining(br) < 8) return 0;
    uint32_t comment_bits = asc_br_get(br, 8) * 8;
    if (asc_br_remaining(br) < comment_bits) return 0;
    br->pos += comment_bits;
    pce->pos = (uint16_t)start;
    pce->comment_bytes = (uint8_t)(comment_bits / 8);
    pce->channels = (uint8_t)channels;
    pce->wave71 = asc_pce_wave71(pce, front, side, back, lfe, pairs);
    return pce->channels;
}

/*
 * Parses aot/sample_rate/channels, then either the "explicit" nested-AOT
 * HE-AAC form (top-level AOT==5) or, for a plain-LC ASC, walks
 * GASpecificConfig (frameLengthFlag/dependsOnCoreCoder[/coreCoderDelay]/
 * extensionFlag[/extensionFlag3]) before scanning for the "implicit"
 * trailing 0x2b7/0x548 sync-extension -- the form real MP4/M4A muxers
 * (including this project's own) actually write. asc_len is in bytes.
 *
 * PCE audio lists supply the channel count when channelConfiguration is zero;
 * optional fields and comments are skipped before the sync-extension scan.
 */
static inline void asc_codec_parse(const uint8_t *buf, uint32_t len, AscInfo *out)
{
    memset(out, 0, sizeof(*out));
    if (!buf || len < 2 || len > UINT32_MAX / 8) return;

    asc_bitreader br = { buf, len * 8, 0 };

    uint32_t aot = asc_br_get(&br, 5);
    if (aot == 31) aot = 32 + asc_br_get(&br, 6);
    out->object_type = (uint8_t)aot;

    out->sample_rate = asc_parse_sample_rate(&br);
    out->channel_config = out->num_channels = (uint8_t)asc_br_get(&br, 4);

    if (aot == 5 || aot == 29) {
        /* Explicit hierarchical form: the SBR (29: SBR+PS) object wraps the
         * core object and carries the output sampling frequency itself. */
        out->sbr_present = true;
        out->ps_present = (aot == 29);
        out->sbr_sample_rate = asc_parse_sample_rate(&br);
        uint32_t real_aot = asc_br_get(&br, 5);
        if (real_aot == 31) real_aot = 32 + asc_br_get(&br, 6);
        out->object_type = (uint8_t)real_aot;
        aot = real_aot;
    }

    if (!out->sbr_present) out->sbr_sample_rate = out->sample_rate * 2;

    if (aot != 2) return; /* GASpecificConfig only walked here for AAC-LC */

    out->frame_length_flag = asc_br_get(&br, 1) != 0; /* frameLengthFlag */
    if (asc_br_get(&br, 1)) asc_skip_core_coder_delay(&br); /* coreCoderDelay */
    bool extension_flag = asc_br_get(&br, 1) != 0;
    if (out->num_channels == 0) {
        out->num_channels = asc_parse_pce(&br, &out->pce);
        if (!out->num_channels) return;
    }
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

/* Copies a parsed PCE's fields and comment from src, the buffer it was parsed
 * from. The byte alignment inside the element is relative to the start of the
 * ASC or of the raw data block, which differ, so it is rewritten for bw. */
static inline void asc_pce_write(asc_bitwriter *bw, const uint8_t *src, const AscPce *pce)
{
    asc_bitreader br = { src, UINT32_MAX, pce->pos };
    for (uint32_t i = 0; i < pce->body_bits; i++)
        asc_bw_put(bw, asc_br_get(&br, 1), 1);
    asc_bw_put(bw, 0, (8 - (bw->pos & 7)) & 7);
    asc_bw_put(bw, pce->comment_bytes, 8);
    br.pos = ((pce->pos + pce->body_bits + 7) & ~7u) + 8;
    for (uint32_t i = 0; i < pce->comment_bytes; i++)
        asc_bw_put(bw, asc_br_get(&br, 8), 8);
}

/* The PCE of an ASC as the element a raw data block opens with: element id,
 * fields, alignment and comment. Returns its bytes, or 0 if out_cap is short. */
static inline uint32_t asc_pce_element(const uint8_t *asc, const AscPce *pce, uint8_t *out, uint32_t out_cap)
{
    uint32_t bytes = ((3 + pce->body_bits + 7) >> 3) + 1 + pce->comment_bytes;
    if (!pce->channels || out_cap < bytes) return 0;
    memset(out, 0, bytes);
    asc_bitwriter bw = { out, bytes * 8, 0 };
    asc_bw_put(&bw, 5, 3); /* ID_PCE */
    asc_pce_write(&bw, asc, pce);
    return bytes;
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
    const AscPce *pce;    /* with channels == 0: the layout, parsed from pce_src */
    const uint8_t *pce_src;
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
    if (info->pce) asc_pce_write(&bw, info->pce_src, info->pce);

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
