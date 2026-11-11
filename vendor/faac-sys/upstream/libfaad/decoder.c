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

#include "faad_internal.h"

#ifdef FAAD_STATS
#include <stdio.h>
#endif

#include "atomic.h"
#include "bits.h"
#include "endian.h"

/* Baselines end at the last field so appended fields remain optional. */
#define CONFIG_BASELINE_SIZE ((uint32_t)FAAD_CONFIG_BASELINE)
#define LIBRARY_INFO_BASELINE_SIZE ((uint32_t)FAAD_LIBRARY_INFO_BASELINE)
#define STREAM_INFO_BASELINE_SIZE ((uint32_t)FAAD_STREAM_INFO_BASELINE)

/* Append-only fields must not increase the alignment an older caller supplies. */
_Static_assert(_Alignof(faad_config) == _Alignof(uint32_t), "config ABI alignment");
_Static_assert(_Alignof(faad_stream_info) == _Alignof(uint32_t), "stream ABI alignment");
_Static_assert(_Alignof(faad_library_info) == _Alignof(const char *), "library ABI alignment");

static uint32_t bounded_size(uint32_t caller_size, size_t library_size)
{
    return caller_size < library_size ? caller_size : (uint32_t)library_size;
}

/* channel_configuration (ISO/IEC 14496-3 Table 1.19); 0 takes its layout from a PCE. */
enum channel_config { CFG_PCE, CFG_MONO, CFG_STEREO, CFG_3_0, CFG_4_0, CFG_5_0, CFG_5_1, CFG_7_1,
                      CFG_6_1 = 11, CFG_7_1_SIDE_BACK = 12, CFG_COUNT };

/* Channels a channel_configuration carries (7 is 7.1 wide); 0, a PCE, is
 * learnt from the first decoded frame, so start from stereo. */
static uint32_t config_channels(uint32_t channel_config)
{
    return channel_config == CFG_7_1 || channel_config == CFG_7_1_SIDE_BACK ? 8
        : channel_config == CFG_6_1 ? 7
        : (channel_config >= CFG_MONO && channel_config <= CFG_5_1)
        ? channel_config : (MAX_CHANNELS < 2 ? 1 : 2);
}

static uint32_t asc_channels(const AudioSpecificConfig *asc)
{
    return asc->pce.channels ? asc->pce.channels : config_channels(asc->num_channels);
}

/* WAVE_FORMAT_EXTENSIBLE speakers, by bit position; a layout outputs in this order. */
enum speaker_pos { POS_FL, POS_FR, POS_FC, POS_LFE, POS_BL, POS_BR, POS_FLC, POS_FRC, POS_BC, POS_SL, POS_SR,
                   POS_COUNT };
enum speaker {
    SPK_FL = 1 << POS_FL, SPK_FR = 1 << POS_FR, SPK_FC = 1 << POS_FC, SPK_LFE = 1 << POS_LFE,
    SPK_BL = 1 << POS_BL, SPK_BR = 1 << POS_BR, SPK_FLC = 1 << POS_FLC, SPK_FRC = 1 << POS_FRC,
    SPK_BC = 1 << POS_BC, SPK_SL = 1 << POS_SL, SPK_SR = 1 << POS_SR,
    SPK_FRONT = SPK_FL | SPK_FR, SPK_BACK = SPK_BL | SPK_BR, SPK_SIDE = SPK_SL | SPK_SR
};

#if MAX_CHANNELS >= 3
#define EL(cat, cpe) (uint8_t)((cat) << 5 | (cpe) << 4)
/* Table 1.19: each channel_configuration as the element list a PCE would give.
 * Configuration 7 is 7.1 wide: centre, the two front pairs from the centre
 * outward, then the surrounds. Apple, FDK and the specification read it so; the
 * side/back 7.1 that older encoders also wrote as 7 cannot be told from it. */
static const uint8_t config_elements[CFG_COUNT][6] = {
    [CFG_MONO] = { EL(ASC_PCE_FRONT, 0), 0xff },
    [CFG_STEREO] = { EL(ASC_PCE_FRONT, 1), 0xff },
    [CFG_3_0] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), 0xff },
    [CFG_4_0] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_BACK, 0), 0xff },
    [CFG_5_0] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_BACK, 1), 0xff },
    [CFG_5_1] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_BACK, 1), EL(ASC_PCE_LFE, 0), 0xff },
    [CFG_7_1] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_BACK, 1),
                  EL(ASC_PCE_LFE, 0) },
    /* The 2013 amendment: 6.1 with a back centre, and the side/back 7.1 that
     * configuration 7 cannot name. Only a MP4 or raw ASC carries them, as an ADTS
     * header has three bits for the configuration. */
    [CFG_6_1] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_BACK, 1), EL(ASC_PCE_BACK, 0),
                  EL(ASC_PCE_LFE, 0) },
    [CFG_7_1_SIDE_BACK] = { EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_SIDE, 1), EL(ASC_PCE_BACK, 1),
                  EL(ASC_PCE_LFE, 0) },
};

/* A configuration 0 stream whose PCE has not arrived yet comes from a writer
 * that signals layouts by PCE, whose 7.1 is front, side, back. */
static const uint8_t pce_side_back_7_1[5] = {
    EL(ASC_PCE_FRONT, 0), EL(ASC_PCE_FRONT, 1), EL(ASC_PCE_SIDE, 1), EL(ASC_PCE_BACK, 1), EL(ASC_PCE_LFE, 0) };

/* Gives each element its speakers (a WAVE mask bit per channel) and orders
 * the output by position. A layout whose elements cannot be placed, or two of
 * which would share a speaker, stays in element order with no mask. Two back
 * pairs without a side pair read as side then back, as 7.1 does. */
static void layout_build(struct faad_decoder *dec, const uint8_t *el, uint32_t n, bool by_tag)
{
    static const uint16_t speakers[4][2] = { { SPK_FC, SPK_FRONT }, { 0, SPK_SIDE }, { SPK_BC, SPK_BACK }, { SPK_LFE, 0 } };
    struct faad_layout *L = &dec->layout;
    uint16_t spk[MAX_CHANNELS];
    uint32_t mask = 0, ch = 0, front_pairs = 0, back_pairs = 0, side = 0, front_total = 0;
    memset(L, 0, sizeof(*L));
    L->lfe = 0xff;
    for (uint32_t i = 0; i < n; i++) {
        side += el[i] >> 5 == ASC_PCE_SIDE;
        back_pairs += el[i] >> 4 == (ASC_PCE_BACK << 1 | 1);
        front_total += el[i] >> 4 == (ASC_PCE_FRONT << 1 | 1);
    }
    bool rear_pairs = back_pairs == 2 && !side;
    for (uint32_t i = 0; i < n; i++) {
        uint32_t cat = el[i] >> 5, pair = el[i] >> 4 & 1, bits = speakers[cat][pair];
        if (cat == ASC_PCE_FRONT && pair) {
            /* Front pairs are listed from the centre outward: with two, the inner one is FLC/FRC. */
            if (front_total > 2) bits = 0;
            else if (front_total == 2 && !front_pairs++) bits = SPK_FLC | SPK_FRC;
        }
        if (rear_pairs && cat == ASC_PCE_BACK && pair && !(mask & SPK_SIDE)) bits = SPK_SIDE;
        if (!bits || (mask & bits) || ch + 1 + pair > MAX_CHANNELS) return;
        mask |= bits;
        L->el_key[i] = (uint8_t)((cat == ASC_PCE_LFE ? ID_LFE : pair) << 4 | (el[i] & 0xf));
        /* Tags select the elements, so two of one kind cannot share one. */
        for (uint32_t j = 0; by_tag && j < i; j++)
            if (L->el_key[j] == L->el_key[i]) return;
        L->el_slot[i] = (uint8_t)ch;
        spk[ch] = (uint16_t)(bits & (0u - bits));
        if (pair) spk[ch + 1] = (uint16_t)(bits & (bits - 1));
        ch += 1 + pair;
    }
    if (!ch) return;
    for (uint32_t i = 0; i < ch; i++) {
        uint32_t rank = faac_popcount32(mask & (spk[i] - 1u)); /* speakers before this one */
        L->map[rank] = (uint8_t)i;
        if (spk[i] == SPK_LFE) L->lfe = (uint8_t)i;
    }
    L->mask = (uint16_t)mask;
    L->n = (uint8_t)ch;
    L->n_el = by_tag ? (uint8_t)n : 0;
}

void faad_layout_update(struct faad_decoder *dec)
{
    struct faad_layout *L = &dec->layout;
    uint32_t cfg = dec->asc.num_channels;
    if (cfg) {
        if (cfg == L->cfg && L->n) return;
        if (cfg < CFG_COUNT) {
            uint32_t n = 0;
            while (n < 5 && config_elements[cfg][n] != 0xff) n++;
            layout_build(dec, config_elements[cfg], n, false);
        } else {
            memset(L, 0, sizeof(*L));
        }
    } else {
        if (L->cfg) memset(L, 0, sizeof(*L)); /* a new layout starts, with no PCM in it yet */
        if (!L->frozen && dec->asc.pce.n_el)
            layout_build(dec, dec->asc.pce.el, dec->asc.pce.n_el, true);
    }
    L->cfg = (uint8_t)cfg;
}

/* A configuration 0 stream may only get its PCE some frames in; the elements
 * of its first frame stand for the common configuration they match. */
void faad_layout_infer(struct faad_decoder *dec, uint32_t seq)
{
    for (uint32_t cfg = CFG_7_1; cfg >= CFG_3_0; cfg--) {
        uint32_t want = 1, n = 0;
        for (; n < 5 && config_elements[cfg][n] != 0xff; n++) {
            uint32_t e = config_elements[cfg][n];
            want = want << 2 | ((e >> 5) == ASC_PCE_LFE ? ID_LFE : (e >> 4) & 1 ? ID_CPE : ID_SCE);
        }
        if (want == seq) {
            layout_build(dec, cfg == CFG_7_1 ? pce_side_back_7_1 : config_elements[cfg], n, false);
            return;
        }
    }
}

/* Tags put the elements of a PCE layout in their slots wherever they sit in the
 * stream. A tag the PCE does not list means the writer did not number them as its
 * PCE does; if the elements so far are in order, the stream is read in element order. */
static uint32_t element_slot(faad_decoder *dec, uint32_t type, uint32_t tag, uint32_t ch_idx, uint32_t occupied)
{
    struct faad_layout *L = &dec->layout;
    if (!L->n_el) return ch_idx;
    for (uint32_t i = 0; i < L->n_el; i++)
        if (L->el_key[i] == (type << 4 | tag)) return L->el_slot[i];
    if (occupied != (1u << ch_idx) - 1) return MAX_CHANNELS; /* an earlier element is out of order */
    L->n_el = 0;
    return ch_idx;
}
#define SLOTS_BY_TAG(dec) ((dec)->layout.n_el != 0)
#else
static uint32_t element_slot(faad_decoder *dec, uint32_t type, uint32_t tag, uint32_t ch_idx, uint32_t occupied)
{
    (void)dec; (void)type; (void)tag; (void)occupied;
    return ch_idx;
}
#define SLOTS_BY_TAG(dec) 0
#endif

static bool downmix_mode_is_valid(enum faad_downmix_mode mode)
{
    uint32_t n = faac_popcount32((uint32_t)mode);
    return mode == FAAD_DOWNMIX_NONE || (!((uint32_t)mode >> POS_COUNT) && n <= 8);
}

/* Channels out of a downmix to mode, from channels in. */
static uint32_t downmix_out_chs(enum faad_downmix_mode mode, uint32_t channels)
{
    uint32_t n = faac_popcount32((uint32_t)mode);
    return mode != FAAD_DOWNMIX_NONE && channels > n ? n : channels;
}

/* Speakers the downmix weighs, in output order; a stream of unknown
 * positions is taken to follow the WAV order of its channel count. */
static uint32_t downmix_mask(const faad_decoder *dec, uint32_t num_chs)
{
    static const uint16_t canon[] = {
        SPK_FRONT, SPK_FRONT, SPK_FRONT, SPK_FRONT | SPK_FC, SPK_FRONT | SPK_FC | SPK_BC,
        SPK_FRONT | SPK_FC | SPK_BACK, SPK_FRONT | SPK_FC | SPK_LFE | SPK_BACK,
        SPK_FRONT | SPK_FC | SPK_LFE | SPK_BC | SPK_SIDE,
        SPK_FRONT | SPK_FC | SPK_LFE | SPK_BACK | SPK_SIDE };
    uint32_t mask = faad_layout_mask(dec, num_chs);
    return mask ? mask : canon[num_chs <= 8 ? num_chs : 0];
}

#define DM_K 0.70710678f
/* A stereo target needs two rows even when the build keeps one channel. */
#define DM_CH (MAX_CHANNELS < 2 ? 2 : MAX_CHANNELS)
/* Weighs input channel in_c into the output that carries speaker spk, if the
 * target layout has one. */
static void dm_add(float w[][DM_CH], uint32_t tm, uint32_t in_c, uint32_t spk, float v)
{
    if (tm & spk) w[faac_popcount32(tm & (spk - 1))][in_c] += v;
}

/* Time-domain downmix of channels src[], ordered as the speakers of mask, to
 * the speakers of the mode: surround to stereo as Lo = L + 0.707 (C + Ls),
 * Ro = R + 0.707 (C + Rs), a lone rear channel at half that, LFE dropped,
 * each output scaled so a full-scale input cannot clip; mono as (Lo + Ro) / 2.
 * The outputs go to dst[] (one frame_samples run each). Returns the output
 * channel count, num_chs itself when there is nothing to mix. */
static uint32_t downmix_pcm(enum faad_downmix_mode mode, const float *src[], uint32_t num_chs,
                            uint32_t mask, uint32_t frame_samples, float *dst[])
{
    bool mono = mode == FAAD_DOWNMIX_MONO;
    uint32_t tm = mono ? SPK_FRONT : (uint32_t)mode, tn = faac_popcount32(tm);
    if (mode == FAAD_DOWNMIX_NONE || num_chs < 2 || (!mono && num_chs <= tn))
        return num_chs;

    float w[DM_CH][DM_CH] = {{0}};
    for (uint32_t p = 0, in = 0; p < POS_COUNT && in < num_chs; p++) {
        if (!(mask >> p & 1)) continue;
        uint32_t c = in++, spk = 1u << p;
        if (tm & spk) {
            dm_add(w, tm, c, spk, 1.0f);
            continue;
        }
        switch (p) {
        case POS_FLC: case POS_FRC:
            dm_add(w, tm, c, p == POS_FLC ? SPK_FL : SPK_FR, DM_K);
            dm_add(w, tm, c, SPK_FC, DM_K);
            break;
        case POS_FC:
            dm_add(w, tm, c, SPK_FL, DM_K);
            dm_add(w, tm, c, SPK_FR, DM_K);
            break;
        case POS_BC: {
            uint32_t pair = (tm & SPK_BACK) == SPK_BACK ? SPK_BACK
                          : (tm & SPK_SIDE) == SPK_SIDE ? SPK_SIDE : SPK_FRONT;
            dm_add(w, tm, c, pair & -pair, 0.5f * DM_K);
            dm_add(w, tm, c, pair & (pair - 1), 0.5f * DM_K);
            break;
        }
        case POS_BL: case POS_BR: case POS_SL: case POS_SR: {
            bool right = p == POS_BR || p == POS_SR, back = p == POS_BL || p == POS_BR;
            uint32_t alt = back ? (right ? SPK_SR : SPK_SL) : (right ? SPK_BR : SPK_BL);
            if (tm & alt) dm_add(w, tm, c, alt, 1.0f);
            else dm_add(w, tm, c, right ? SPK_FR : SPK_FL, DM_K);
            break;
        }
        default:
            break;
        }
    }

    uint32_t on = mono ? 1 : tn;
    for (uint32_t o = 0; o < tn; o++) {
        float sum = 0.0f;
        for (uint32_t c = 0; c < num_chs; c++) sum += w[o][c];
        float g = sum > 1.0f ? 1.0f / sum : 1.0f;
        for (uint32_t c = 0; c < num_chs; c++) w[o][c] *= g;
    }
    if (mono)
        for (uint32_t c = 0; c < num_chs; c++) w[0][c] = 0.5f * (w[0][c] + w[1][c]);

    /* Channel by channel over a block, in output order; dst may alias src,
     * which is only written once the block has been read. */
    enum { BLOCK = 64 };
    for (uint32_t b = 0; b < frame_samples; b += BLOCK) {
        uint32_t n = frame_samples - b < BLOCK ? frame_samples - b : BLOCK;
        float ob[DM_CH][BLOCK] = {{0}};
        for (uint32_t o = 0; o < on; o++)
            for (uint32_t c = 0; c < num_chs; c++)
                if (w[o][c] != 0.0f)
                    for (uint32_t i = 0; i < n; i++) ob[o][i] += w[o][c] * src[c][b + i];
        for (uint32_t o = 0; o < on; o++) memcpy(dst[o] + b, ob[o], n * sizeof(float));
    }
    return on;
}

static bool spectral_downmix_windows_match(struct faad_decoder *dec, uint32_t num_chs)
{
    if (num_chs == 7 && !faad_layout_mask(dec, 7)) return false;
    const uint8_t *map = faad_layout_map(dec, num_chs);
    uint32_t mask = downmix_mask(dec, num_chs), first = map ? map[0] : 0, lfe = UINT32_MAX;
    if (mask & SPK_LFE) {
        uint32_t out = (mask & SPK_FL ? 1 : 0) + (mask & SPK_FR ? 1 : 0) + (mask & SPK_FC ? 1 : 0);
        lfe = map ? map[out] : out;
    }
    for (uint32_t ch = 0; ch < num_chs; ch++) {
        if (ch == first || ch == lfe) continue;
        if (dec->win_seq[ch] != dec->win_seq[first]
            || dec->win_shape[ch] != dec->win_shape[first]
            || dec->prev_window_shape[ch] != dec->prev_window_shape[first])
            return false;
    }
    return true;
}

/* A spectral downmix keeps only the mixed overlap in the output-channel slots.
 * Rebuild source-channel overlaps from the preceding spectra when leaving that
 * path; the right-hand IMDCT overlap depends only on that frame's spectrum and
 * window, not on the older overlap that contributed to its emitted samples. */
static void restore_channel_overlaps(struct faad_decoder *dec, uint32_t channels,
                                     uint32_t prior_errors)
{
    memset(dec->overlap, 0, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    float fade = (prior_errors > 0 && prior_errors <= 5) ? powf(0.8f, (float)prior_errors)
        : prior_errors > 5 ? 0.0f : 1.0f;
    if (fade == 0.0f) return;
    float *prev_frame_spec = dec->pcm + 2 * FRAME_LEN_LONG;
    float *discard_pcm = dec->pcm + FRAME_LEN_LONG;
    for (uint32_t ch = 0; ch < channels; ch++) {
        for (int i = 0; i < FRAME_LEN_LONG; i++)
            prev_frame_spec[i] = dec->prev_spec[ch][i] * fade;
        imdct_and_window(dec, ch, dec->prev_window_seq[ch], dec->prev_window_shape[ch],
                         prev_frame_spec, discard_pcm);
    }
}

/* Combine spectra before IMDCT when every contributing channel has the same
 * window state. The two output overlap slots then hold the downmixed history;
 * callers restore channel overlaps before a window mismatch or SBR frame. */
static bool try_spectral_downmix(struct faad_decoder *dec, uint32_t num_chs,
                                 uint32_t *out_num_chs, float *pcm, bool save_history)
{
    enum faad_downmix_mode mode = dec->config.downmix_mode;
    if ((mode != FAAD_DOWNMIX_MONO && mode != FAAD_DOWNMIX_STEREO)
        || num_chs <= 2 || !spectral_downmix_windows_match(dec, num_chs))
        return false;

    if (num_chs < 3 || num_chs > 8) return false;
    const uint8_t *map = faad_layout_map(dec, num_chs);
    uint32_t mask = downmix_mask(dec, num_chs);
    uint32_t nl = map ? map[0] : 0, nr = map ? map[1] : 1;
    if (!dec->spectral_downmix_active) {
        const float *src[MAX_CHANNELS];
        float *dst[2] = { pcm, pcm + FRAME_LEN_LONG };
        for (uint32_t ch = 0; ch < num_chs; ch++)
            src[ch] = dec->overlap[map ? map[ch] : ch];
        uint32_t out_channels = downmix_pcm(mode, src, num_chs, mask, FRAME_LEN_LONG, dst);
        memcpy(dec->overlap[nl], dst[0], FRAME_LEN_LONG * sizeof(float));
        if (out_channels == 2)
            memcpy(dec->overlap[nr], dst[1], FRAME_LEN_LONG * sizeof(float));
    }

    /* Preserve source spectra for concealment and overlap restoration before
     * the in-place mix replaces the left/right channel spectra. */
    if (save_history)
        memcpy(dec->prev_spec, dec->spec, sizeof(dec->spec[0]) * num_chs);

    const float *src[MAX_CHANNELS];
    float *dst[2] = { dec->spec[nl], dec->spec[nr] };
    for (uint32_t ch = 0; ch < num_chs; ch++)
        src[ch] = dec->spec[map ? map[ch] : ch];
    downmix_pcm(mode, src, num_chs, mask, FRAME_LEN_LONG, dst);

    uint8_t seq = dec->win_seq[nl], shape = dec->win_shape[nl];
    imdct_and_window(dec, nl, seq, shape, dec->spec[nl], pcm);
    if (mode == FAAD_DOWNMIX_MONO) {
        *out_num_chs = 1;
    } else {
        imdct_and_window(dec, nr, seq, shape, dec->spec[nr], pcm + FRAME_LEN_LONG);
        *out_num_chs = 2;
    }
    for (uint32_t ch = 0; ch < num_chs; ch++) {
        dec->prev_window_shape[ch] = dec->win_shape[ch];
        dec->prev_window_seq[ch] = dec->win_seq[ch];
    }
    dec->spectral_downmix_active = true;
    dec->spectral_downmix_channels = num_chs;
    dec->spectral_downmix_layout = dec->asc.num_channels;
    return true;
}


FAADAPI faad_status faad_get_library_info(faad_library_info *out_info)
{
    if (!out_info || out_info->struct_size < LIBRARY_INFO_BASELINE_SIZE) {
        return FAAD_ERR_INVALID_ARGUMENT;
    }
    faad_library_info info;
    memset(&info, 0, sizeof(info));
    info.struct_size = bounded_size(out_info->struct_size, sizeof(info));
    info.version = FAAD_VERSION_STRING;
    info.copyright =
        "FAAD - Freeware Advanced Audio Decoder (https://freewareadvancedaudio.github.io)\n"
        " Copyright (C) 2026, Nils Schimmelmann\n";
    info.max_channels = MAX_CHANNELS;
#ifndef FAAD_DISABLE_SBR
    info.flags |= FAAD_LIB_SBR;
#endif
#ifndef FAAD_DISABLE_PS
    info.flags |= FAAD_LIB_PS;
#endif
#ifndef FAAD_DISABLE_DAB
    info.flags |= FAAD_LIB_DAB;
#endif
#ifndef FAAD_DISABLE_DRC
    info.flags |= FAAD_LIB_DRC;
#endif

    memcpy(out_info, &info, info.struct_size);
    return FAAD_OK;
}

FAADAPI faad_status faad_config_init(faad_config *config, uint32_t caller_size)
{
    if (!config || caller_size < CONFIG_BASELINE_SIZE) {
        return FAAD_ERR_INVALID_ARGUMENT;
    }
    faad_config tmp;
    uint32_t n = caller_size < sizeof(tmp) ? caller_size : (uint32_t)sizeof(tmp);
    memset(&tmp, 0, sizeof(tmp));
    tmp.struct_size = n;
    tmp.stream_format = FAAD_STREAM_ADTS;
    tmp.output_format = FAAD_OUTPUT_16BIT;
    tmp.downmix_mode = FAAD_DOWNMIX_NONE;
    memcpy(config, &tmp, n);
    return FAAD_OK;
}

static uint32_t output_sample_bytes(enum faad_output_format f)
{
    return f == FAAD_OUTPUT_16BIT ? 2 : f == FAAD_OUTPUT_24BIT ? 3 : 4;
}

static uint32_t output_capacity(const faad_decoder *dec)
{
    uint32_t channels = MAX_CHANNELS;
    if (dec->config.stream_format == FAAD_STREAM_RAW && dec->asc_parsed && (dec->asc.num_channels || dec->asc.pce.channels))
        channels = asc_channels(&dec->asc);
#ifndef FAAD_DISABLE_PS
    if (channels < 2) channels = 2;
#endif
    channels = downmix_out_chs(dec->config.downmix_mode, channels);
    uint32_t samples = FRAME_IS_960(dec) ? 960 : 1024;
#ifndef FAAD_DISABLE_SBR
    samples *= 2;
#endif
    return samples * channels * output_sample_bytes(dec->config.output_format);
}

static uint32_t channel_mask(const faad_decoder *dec, uint32_t channels)
{
    if (channels < dec->num_channels) /* mixed: the stream is in the target layout */
        return (uint32_t)dec->config.downmix_mode;
    static const uint32_t masks[] = { 0, SPK_FC, SPK_FRONT };
    return channels <= 2 ? masks[channels] : faad_layout_mask(dec, channels);
}

#ifndef FAAD_DISABLE_DRC
/* A private cursor makes truncated metadata harmless to the audio parser. */
static void read_drc(faad_decoder *dec, BitReader input, uint32_t end)
{
    uint8_t top[16], ctl[16], sign[16];
    bool excluded[MAX_CHANNELS] = { false };
    unsigned bands = 1, ref = 0;
    bool has_ref;
#define DRC_READ(n) (bits_get_consumed(&input) <= end && (n) <= end - bits_get_consumed(&input) ? bits_get(&input, (n)) : (bad = true, 0u))
    bool bad = false;
    if (DRC_READ(1)) { (void)DRC_READ(4); (void)DRC_READ(4); }
    if (DRC_READ(1)) {
        unsigned channel = 0;
        bool more;
        do {
            for (unsigned i = 0; i < 7; i++, channel++) {
                bool omit = DRC_READ(1) != 0;
                if (channel < MAX_CHANNELS) excluded[channel] = omit;
            }
            more = DRC_READ(1) != 0;
        } while (more && !bad);
    }
    bool banded = DRC_READ(1) != 0;
    if (banded) {
        bands += DRC_READ(4) & 0xfu;
        (void)DRC_READ(4);
        for (unsigned b = 0; b < bands && b < sizeof(top); b++) {
            top[b] = (uint8_t)DRC_READ(8);
            if (b && top[b] <= top[b - 1]) bad = true;
        }
    } else top[0] = 255;
    has_ref = DRC_READ(1) != 0;
    if (has_ref) { ref = DRC_READ(7); (void)DRC_READ(1); }
    for (unsigned b = 0; b < bands; b++) {
        sign[b] = (uint8_t)DRC_READ(1);
        ctl[b] = (uint8_t)DRC_READ(7);
    }
#undef DRC_READ
    if (bad) return;
    dec->drc.present = true;
    dec->drc.bands = (uint8_t)bands;
    memcpy(dec->drc.top, top, bands);
    memcpy(dec->drc.ctl, ctl, bands);
    memcpy(dec->drc.sign, sign, bands);
    memcpy(dec->drc.excluded, excluded, sizeof(excluded));
    if (has_ref) { dec->drc.ref = (uint8_t)ref; dec->drc.ref_present = true; }
}

static void apply_drc(faad_decoder *dec)
{
    float gain[16];
    for (unsigned c = 0; c < MAX_CHANNELS; c++) dec->drc.sbr_gain[c] = 1.0f;
    for (unsigned b = 0; b < dec->drc.bands; b++) {
        float scale = dec->drc.sign[b] ? -(float)dec->config.drc_cut : (float)dec->config.drc_boost;
        /* One control step is 1/24 octave, the step the encoders that emit
         * these gains assume; plain 0.25 dB would drift by 0.3 %. */
        gain[b] = exp2f(dec->drc.ctl[b] * scale / (127.0f * 24.0f));
    }
    for (unsigned c = 0; c < dec->num_channels; c++) {
        if (dec->drc.excluded[c]) continue;
        unsigned windows = dec->win_seq[c] == EIGHT_SHORT_SEQUENCE ? 8 : 1;
        unsigned length = (FRAME_IS_960(dec) ? 960 : 1024) / windows;
        unsigned start = 0;
        for (unsigned b = 0; b < dec->drc.bands; b++) {
            unsigned end = 4u * (dec->drc.top[b] + 1u) / windows;
            if (end > length) end = length;
            if (gain[b] != 1.0f && end > start) {
                dec->drc.applied = true;
                if (gain[b] > 1.0f) dec->drc.boost = true;
                for (unsigned w = 0; w < windows; w++)
                    for (unsigned i = start; i < end; i++) dec->spec[c][w * length + i] *= gain[b];
            }
            start = end;
        }
        /* The HF envelope is absolute energy, independent of core amplitude. */
        dec->drc.sbr_gain[c] = gain[dec->drc.bands - 1] * gain[dec->drc.bands - 1];
    }
}

/* Keep all PCM arithmetic out of the disabled path. */
static bool drc_output(faad_decoder *dec, const float *src[], uint32_t num_chs,
                       uint32_t frame_samples, void *out_pcm)
{
    float target_gain = dec->config.drc_target_level != 0.0f && dec->drc.ref_present
        ? exp2f(((float)dec->drc.ref + 4.0f * dec->config.drc_target_level) / 24.0f) : 1.0f;
    float previous_gain = dec->drc.previous_norm ? dec->drc.previous_norm : 1.0f;
    if (target_gain != 1.0f || previous_gain != 1.0f || (dec->drc.boost && dec->config.output_format != FAAD_OUTPUT_FLOAT)) {
        bool floating = dec->config.output_format == FAAD_OUTPUT_FLOAT;
        bool limit = target_gain > 1.0f || previous_gain > 1.0f || dec->drc.boost;
        float norm = target_gain / 32768.0f;
        float ramp = (target_gain - previous_gain) / frame_samples;
        for (uint32_t i = 0; i < frame_samples; i++) {
            float gain = ramp == 0.0f ? norm : (previous_gain + ramp * (i + 1)) / 32768.0f;
            for (uint32_t c = 0; c < num_chs; c++) {
                float sample = src[c][i] * gain;
                if (floating) ((float *)out_pcm)[i * num_chs + c] = sample;
                else {
                    float magnitude = fabsf(sample);
                    if (limit && magnitude > 0.89f)
                        sample = copysignf(0.89f + 0.11f * (1.0f - 1.0f / (1.0f + (magnitude - 0.89f) / 0.11f)), sample);
                    ((float *)src[c])[i] = sample * 32768.0f;
                }
            }
        }
        if (target_gain != 1.0f || previous_gain != 1.0f) dec->drc.applied = true;
        if (floating) {
            dec->drc.previous_norm = target_gain;
            return true;
        }
    }
    dec->drc.previous_norm = target_gain;
    return false;
}

#endif

static faad_status resolve_config(const faad_config *cfg, faad_config *resolved)
{
    faad_config_init(resolved, sizeof(*resolved));
    if (!cfg) return FAAD_OK;
    if (cfg->struct_size < CONFIG_BASELINE_SIZE) return FAAD_ERR_INVALID_ARGUMENT;
    /* Absent or partially supplied fields keep defaults.
     * Never memcpy a partially supplied scalar or read beyond the caller. */
#define COPY_CONFIG_FIELD(field) \
    if (cfg->struct_size >= offsetof(faad_config, field) + sizeof(cfg->field)) \
        resolved->field = cfg->field
    COPY_CONFIG_FIELD(stream_format);
    COPY_CONFIG_FIELD(output_format);
    COPY_CONFIG_FIELD(downmix_mode);
#ifndef FAAD_DISABLE_DRC
    COPY_CONFIG_FIELD(drc_cut);
    COPY_CONFIG_FIELD(drc_boost);
    COPY_CONFIG_FIELD(drc_target_level);
    if (resolved->drc_cut > 127 || resolved->drc_boost > 127
        || (resolved->drc_target_level != 0.0f
            && !(resolved->drc_target_level >= -31.75f && resolved->drc_target_level <= -10.0f)))
        return FAAD_ERR_INVALID_ARGUMENT;
    resolved->drc_target_level = roundf(4.0f * resolved->drc_target_level) / 4.0f;
#endif
#undef COPY_CONFIG_FIELD
    return (resolved->stream_format == FAAD_STREAM_RAW || resolved->stream_format == FAAD_STREAM_ADTS)
        && (resolved->output_format == FAAD_OUTPUT_16BIT || resolved->output_format == FAAD_OUTPUT_24BIT
            || resolved->output_format == FAAD_OUTPUT_24IN32 || resolved->output_format == FAAD_OUTPUT_FLOAT)
        && downmix_mode_is_valid(resolved->downmix_mode) ? FAAD_OK : FAAD_ERR_INVALID_ARGUMENT;
}

static void faad_init_global_tables_impl(void)
{
    extern void init_dequant_tables(void);
    extern void init_huffman_luts(void);
    extern void init_windows(void);
#ifndef FAAD_DISABLE_SBR
    extern void init_qmf_twiddles(void);
#endif
    extern void init_is_tables(void);
#ifndef FAAD_DISABLE_PS
    extern void init_ps_tables(void);
#endif

    init_dequant_tables();
    init_huffman_luts();
    init_windows();
#ifndef FAAD_DISABLE_SBR
    init_qmf_twiddles();
#endif
    init_is_tables();
#ifndef FAAD_DISABLE_PS
    init_ps_tables();
#endif
}

void faad_init_global_tables(void)
{
    static faac_once_t once = FAAC_ONCE_INIT;
    faac_once_run(&once, faad_init_global_tables_impl);
}

static faad_status init_decoder(faad_decoder *dec, const faad_config *resolved,
                                const uint8_t *asc_buf, uint32_t asc_len,
                                faad_decoder **out_dec)
{
    dec->config = *resolved;

    dec->pns_seed = 0x12345678;

    if (asc_buf && asc_len > 0) {
        BitReader bs;
        bits_init(&bs, asc_buf, asc_len);
        faad_status st = asc_decode(&bs, &dec->asc);
        if (st != FAAD_OK) return st;

        dec->num_channels = asc_channels(&dec->asc);
        if (dec->num_channels > MAX_CHANNELS) return FAAD_ERR_UNSUPPORTED;
        dec->core_sample_rate = dec->asc.sample_rate ? dec->asc.sample_rate : 44100;
        if (get_sr_index(dec->core_sample_rate) < 0) return FAAD_ERR_UNSUPPORTED;
        dec->sample_rate = dec->asc.is_sbr ? dec->asc.sbr_sample_rate : dec->core_sample_rate;
        uint32_t core_samples = FRAME_IS_960(dec) ? 960 : 1024;
        dec->frame_samples = dec->asc.is_sbr ? (core_samples * 2) : core_samples;
        dec->asc_parsed = true;
        faad_layout_update(dec);
        dec->format_known = dec->asc.num_channels != 0 || dec->asc.pce.channels != 0;
        dec->sbr_seen = dec->asc.is_sbr;
        dec->ps_seen = dec->asc.is_ps;
    } else {
        dec->num_channels = MAX_CHANNELS < 2 ? 1 : 2;
        dec->sample_rate = 44100;
        dec->core_sample_rate = 44100;
        dec->frame_samples = 1024;
    }
    dec->core_channels = dec->num_channels;
    dec->max_output_bytes = output_capacity(dec);

#ifdef FAAD_STATS
    if (g_faadStats.dumpFile) fclose(g_faadStats.dumpFile);
    memset(&g_faadStats, 0, sizeof(g_faadStats));
#endif
    *out_dec = dec;
    return FAAD_OK;
}

static void free_split_state(faad_decoder *dec)
{
    if (!dec) return;
#define FREE_PART(field) do { if (dec->field) FreeMemory(dec->field); } while (0)
#if MAX_CHANNELS >= 3
    while (dec->coupling) {
        CouplingChannel *next = dec->coupling->next;
        if (dec->coupling->overlap) FreeMemory(dec->coupling->overlap);
        FreeMemory(dec->coupling);
        dec->coupling = next;
    }
#endif
    FREE_PART(spec);
    FREE_PART(overlap);
    FREE_PART(prev_spec);
#ifndef FAAD_DISABLE_SBR
    FREE_PART(sbr);
    FREE_PART(sbr_el);
    FREE_PART(sbr_scratch.y);
#endif
#ifndef FAAD_DISABLE_PS
    FREE_PART(ps);
#endif
#if defined(FAAD_DISABLE_SBR) || MAX_CHANNELS < 2
    FREE_PART(scratch);
#endif
    FREE_PART(pcm);
    FreeMemory(dec);
#undef FREE_PART
}

#ifndef FAAD_DISABLE_SBR
static faad_status alloc_optional(faad_decoder *dec, void **ptr, size_t size)
{
    if (*ptr) return FAAD_OK;
    *ptr = AllocMemory(size);
    if (!*ptr) return FAAD_ERR_NO_MEMORY;
    memset(*ptr, 0, size);
    dec->allocated_bytes += size;
    if (size > dec->largest_allocation) dec->largest_allocation = size;
    return FAAD_OK;
}

faad_status faad_ensure_sbr(faad_decoder *dec)
{
    if (alloc_optional(dec, (void **)&dec->sbr, sizeof(SBRChannel[MAX_CHANNELS])) != FAAD_OK
        || alloc_optional(dec, (void **)&dec->sbr_el, sizeof(SBRElement[MAX_CHANNELS])) != FAAD_OK
        || alloc_optional(dec, (void **)&dec->sbr_scratch.y,
                          sizeof(float[SBR_MAX_BANDS][SBR_BUF_SLOTS][2])) != FAAD_OK)
        return FAAD_ERR_NO_MEMORY;
    dec->sbr_scratch.x_low = dec->sbr_scratch.y;
    return FAAD_OK;
}

#ifndef FAAD_DISABLE_PS
faad_status faad_ensure_ps(faad_decoder *dec)
{
    return alloc_optional(dec, (void **)&dec->ps, sizeof(PSState));
}
#endif
#endif

#define ALLOC_PART(field, size) do { \
    dec->field = AllocMemory(size); \
    if (!dec->field) { free_split_state(dec); return FAAD_ERR_NO_MEMORY; } \
    memset(dec->field, 0, size); \
    dec->allocated_bytes += (size); \
    if ((size) > dec->largest_allocation) dec->largest_allocation = (size); \
} while (0)

FAADAPI faad_status faad_decoder_open(const faad_config *config,
                                      const uint8_t *asc_buf, uint32_t asc_len,
                                      faad_decoder **out)
{
    if (!out) return FAAD_ERR_INVALID_ARGUMENT;
    *out = NULL;
    faad_config resolved;
    if (resolve_config(config, &resolved) != FAAD_OK
        || ((asc_buf == NULL) != (asc_len == 0))
        || (resolved.stream_format == FAAD_STREAM_RAW && !asc_len))
        return FAAD_ERR_INVALID_ARGUMENT;
    faad_init_global_tables();
    faad_decoder *dec = AllocMemory(sizeof(*dec));
    if (!dec) return FAAD_ERR_NO_MEMORY;
    memset(dec, 0, sizeof(*dec));
    dec->allocated_bytes = sizeof(*dec);
    dec->largest_allocation = sizeof(*dec);
    ALLOC_PART(spec, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    ALLOC_PART(overlap, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    ALLOC_PART(prev_spec, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    ALLOC_PART(pcm, sizeof(float[MAX_CHANNELS * FRAME_SAMPLES_MAX]));
#if !defined(FAAD_DISABLE_SBR) && MAX_CHANNELS >= 2
    /* Core PCM is packed into the first half until every IMDCT finishes. */
    dec->scratch = (FrameScratch *)(dec->pcm + MAX_CHANNELS * FRAME_LEN_LONG);
#else
    ALLOC_PART(scratch, sizeof(FrameScratch));
#endif
    faad_status st = init_decoder(dec, &resolved, asc_buf, asc_len, out);
    if (st != FAAD_OK) { free_split_state(dec); return st; }
#ifndef FAAD_DISABLE_SBR
    if (dec->asc.is_sbr && faad_ensure_sbr(dec) != FAAD_OK) st = FAAD_ERR_NO_MEMORY;
#ifndef FAAD_DISABLE_PS
    if (st == FAAD_OK && dec->asc.is_ps && faad_ensure_ps(dec) != FAAD_OK)
        st = FAAD_ERR_NO_MEMORY;
#endif
#endif
    if (st != FAAD_OK) { *out = NULL; free_split_state(dec); }
    return st;
}
#undef ALLOC_PART

#ifdef FAAD_STATS
faadDecStats g_faadStats;

static void faad_print_stats(void)
{
    const faadDecStats *s = &g_faadStats;
    if (s->totalFrames == 0) return;

    double tns_pct = s->icsCount > 0 ? 100.0 * s->tnsActiveFrames / s->icsCount : 0.0;
    double sbr_pct = s->totalFrames > 0 ? 100.0 * s->sbrActiveFrames / s->totalFrames : 0.0;
    double short_pct = s->icsCount > 0 ? 100.0 * s->shortBlockIcsCount / s->icsCount : 0.0;
    double pad_avg = s->fillElementCount > 0 ? (double)s->fillElementPadBitsSum / s->fillElementCount : 0.0;
    /* Percentages of scalefactor bands, counted per channel slot. */
    double ms_pct = s->totalBands > 0 ? 100.0 * s->msBands / s->totalBands : 0.0;
    double is_pct = s->totalBands > 0 ? 100.0 * s->isBands / s->totalBands : 0.0;
    double pns_pct = s->totalBands > 0 ? 100.0 * s->pnsBands / s->totalBands : 0.0;

    fprintf(stderr, "\n--- Decoder Diagnostics ---\n");
    fprintf(stderr, " Frames              : %u (non-END termination: %u)\n",
            s->totalFrames, s->nonEndTermination);
    fprintf(stderr, " Elements            : SCE=%u CPE=%u CCE=%u LFE=%u DSE=%u PCE=%u FIL=%u END=%u\n",
            s->elementCounts[0], s->elementCounts[1], s->elementCounts[2], s->elementCounts[3],
            s->elementCounts[4], s->elementCounts[5], s->elementCounts[6], s->elementCounts[7]);
    if (s->minChannels == s->maxChannels) {
        fprintf(stderr, " Channels            : %u (constant, 0 changes)\n", s->minChannels);
    } else {
        fprintf(stderr, " Channels            : varied %u-%u (%u changes)\n",
                s->minChannels, s->maxChannels, s->channelCountChanges);
    }
    fprintf(stderr, " TNS                 : active in %u/%u ics (%.1f%%)\n",
            s->tnsActiveFrames, s->icsCount, tns_pct);
    fprintf(stderr, " SBR                 : active in %u/%u frames (%.1f%%)\n",
            s->sbrActiveFrames, s->totalFrames, sbr_pct);
    fprintf(stderr, " SBR payloads        : %u headers, %u envelopes\n",
            s->sbrHeaderCount, s->sbrEnvelopeSum);
    fprintf(stderr, " Parametric stereo   : active in %u/%u frames\n",
            s->psActiveFrames, s->totalFrames);
    fprintf(stderr, " Short blocks        : %u/%u ics (%.1f%%)\n",
            s->shortBlockIcsCount, s->icsCount, short_pct);
    fprintf(stderr, " M/S, IS, PNS bands  : %.1f%% / %.1f%% / %.1f%% of %lu total\n",
            ms_pct, is_pct, pns_pct, s->totalBands);

    fprintf(stderr, " ESCBOOK magnitude   : %u values needed the >=16 escape path\n",
            s->escbookMagnitudeEscapes);
    fprintf(stderr, " Fill-elt pad-align  : %u elements, avg %.1f bits, max %u bits\n",
            s->fillElementCount, pad_avg, s->fillElementMaxPad);
    fprintf(stderr, " Error concealment   : %u frames\n", s->errorConcealmentFrames);
    fprintf(stderr, "---------------------------\n");
}
#endif

FAADAPI faad_status faad_decoder_close(faad_decoder **decoder)
{
    if (!decoder) return FAAD_ERR_INVALID_ARGUMENT;
    faad_decoder *dec = *decoder;
    if (!dec) return FAAD_OK;
    *decoder = NULL;
#ifdef FAAD_STATS
    faad_print_stats();
    if (g_faadStats.dumpFile) fclose(g_faadStats.dumpFile);
    g_faadStats.dumpFile = NULL;
    g_faadStats.dumpOpenTried = false;
#endif
    free_split_state(dec);
    return FAAD_OK;
}

FAADAPI faad_status faad_decoder_get_info(const faad_decoder *decoder, faad_stream_info *out_info)
{
    if (!decoder || !out_info || out_info->struct_size < STREAM_INFO_BASELINE_SIZE)
        return FAAD_ERR_INVALID_ARGUMENT;
    faad_stream_info info;
    memset(&info, 0, sizeof(info));
    info.struct_size = bounded_size(out_info->struct_size, sizeof(info));
    info.max_output_bytes = decoder->max_output_bytes;
    if (decoder->format_known) info.flags |= FAAD_STREAM_FORMAT_KNOWN;
#ifndef FAAD_DISABLE_DRC
    info.drc_ref_level = -0.25f * (float)decoder->drc.ref;
    if (decoder->drc.ref_present) info.flags |= FAAD_STREAM_DRC_REF_LEVEL;
#endif
    if (!(info.flags & FAAD_STREAM_FORMAT_KNOWN)) {
        memcpy(out_info, &info, info.struct_size);
        return FAAD_OK;
    }
    bool sbr = decoder->asc.is_sbr || decoder->sbr_seen;
    info.sample_rate = decoder->core_sample_rate;
    uint32_t core_samples = FRAME_IS_960(decoder) ? 960 : 1024;
    info.frame_samples = core_samples;
#ifndef FAAD_DISABLE_SBR
    if (sbr) { info.sample_rate *= 2; info.frame_samples = core_samples * 2; }
#endif
    info.channels = decoder->num_channels;
#ifndef FAAD_DISABLE_PS
    if (decoder->ps_seen && info.channels == 1) info.channels = 2;
#endif
    info.channels = downmix_out_chs(decoder->config.downmix_mode, info.channels);
    info.object_type = (decoder->asc.is_ps || decoder->ps_seen) ? FAAD_OBJ_HE_AAC_V2
        : sbr ? FAAD_OBJ_HE_AAC_V1 : FAAD_OBJ_LC;
#ifndef FAAD_DISABLE_SBR
    info.decoder_delay = sbr ? 2 * FAAD_SBR_CORE_DELAY : 0;
#endif
    info.channel_mask = channel_mask(decoder, info.channels);
    if (decoder->pcm_emitted) {
        info.sample_rate = decoder->emitted_format.sample_rate;
        info.channels = decoder->emitted_format.channels;
        info.frame_samples = decoder->emitted_format.frame_samples;
        info.channel_mask = decoder->emitted_format.channel_mask;
        info.decoder_delay = decoder->emitted_format.decoder_delay;
    }
    memcpy(out_info, &info, info.struct_size);

    return FAAD_OK;
}

FAADAPI faad_status faad_decoder_flush(faad_decoder *decoder)
{
    if (!decoder) return FAAD_ERR_INVALID_ARGUMENT;

    memset(decoder->overlap, 0, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    memset(decoder->spec, 0, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    memset(decoder->prev_spec, 0, sizeof(float[MAX_CHANNELS][FRAME_LEN_LONG]));
    memset(decoder->prev_window_shape, 0, sizeof(decoder->prev_window_shape));
    memset(decoder->prev_window_seq, 0, sizeof(decoder->prev_window_seq));
    memset(decoder->win_shape, 0, sizeof(decoder->win_shape));
    memset(decoder->win_seq, 0, sizeof(decoder->win_seq));
#ifndef FAAD_DISABLE_SBR
    memset(decoder->elem_nch, 0, sizeof(decoder->elem_nch));
#endif
    memset(decoder->pcm, 0, sizeof(float[MAX_CHANNELS * FRAME_SAMPLES_MAX]));
    decoder->spectral_downmix_active = false;
    decoder->spectral_downmix_channels = 0;
    decoder->spectral_downmix_layout = 0;
#ifndef FAAD_DISABLE_DRC
    memset(&decoder->drc, 0, sizeof(decoder->drc));
#endif
    decoder->consecutive_errors = 0;
    decoder->pns_seed = 0x12345678;
#if MAX_CHANNELS >= 3
    for (CouplingChannel *cc = decoder->coupling; cc; cc = cc->next) {
        if (cc->overlap) memset(cc->overlap, 0, sizeof(float[FRAME_LEN_LONG]));
        cc->prev_shape = 0;
        cc->present = false;
    }
#endif
#ifndef FAAD_DISABLE_SBR
    if (decoder->sbr) memset(decoder->sbr, 0, sizeof(SBRChannel[MAX_CHANNELS]));
    if (decoder->sbr_el) memset(decoder->sbr_el, 0, sizeof(SBRElement[MAX_CHANNELS]));
#endif
#ifndef FAAD_DISABLE_PS
    if (decoder->ps) memset(decoder->ps, 0, sizeof(PSState));
#endif
    decoder->sbr_present = false;
    decoder->ps_present = false;

    return FAAD_OK;
}


#ifdef FAAD_STATS
/* Per-frame record of what the encoder chose, appended to the file named by
 * FAAD_DUMP, so streams from different encoders can be diffed decision by
 * decision. One line per record, first field the type, second the frame:
 *   C frame ch bits win_seq max_sfb groups global_gain | cb:sf:nnz:ms ... / ...
 *       one per ICS; bits is the whole element (both channels of a CPE, and
 *       the second channel of a CPE carries ms_mask_present there instead)
 *   H frame nch amp_res start stop xover freq_scale alter_scale noise_bands
 *     limiter_bands limiter_gains interpol smoothing reset kx M n_low n_high n_q e1 e2
 *   F frame ch class L_E L_Q freq_res amp_res invf harm_flag n_harm coupling -1 E_dB Q_dB
 *   G frame ch class L_E pointer t_E...
 *   P frame iid icc num_env
 *   PM frame iid_mode icc_mode phase iid_count icc_count phase_count frame_class
 *       mode is -1 when disabled; IID modes 0..2 are coarse and 3..5 fine
 *   PE frame env border iid... | icc... | ipd... | opd...
 *       each vector contains only its enabled parameter count from PM
 *   B frame total hdr sect sf spec aux sbr ps fill
 *       bits by syntax part; sect/sf/spec/aux (pulse, TNS) sum over the ics,
 *       sbr excludes ps, fill is pad after the SBR payload, and the
 *       remainder of total is element and ics headers */
FILE *faad_dump_file(void)
{
    faadDecStats *st = &g_faadStats;
    if (!st->dumpOpenTried) {
        st->dumpOpenTried = true;
        const char *path = getenv("FAAD_DUMP");
        if (path && *path) st->dumpFile = fopen(path, "a");
    }
    return st->dumpFile;
}

static void core_dump_ics(int ch, const ICSInfo *ics, const float *spec,
                          const uint8_t (*ms)[MAX_SFB], unsigned bits)
{
    FILE *df = faad_dump_file();
    if (!df) return;
    fprintf(df, "C %u %d %u %u %u %u %u |", g_faadStats.totalFrames, ch, bits, ics->window_sequence,
            ics->max_sfb, ics->num_window_groups, ics->global_gain);
    int wo = 0;
    for (int g = 0; g < ics->num_window_groups && g < 8; g++) {
        for (int sfb = 0; sfb < ics->max_sfb && sfb < MAX_SFB; sfb++) {
            int nnz = 0;
            for (int w = 0; w < ics->window_group_length[g]; w++)
                for (int k = ics->sfb_offsets[sfb]; k < ics->sfb_offsets[sfb + 1] && k < 1024; k++)
                    nnz += spec[(wo + w) * 128 + k] != 0.0f;
            fprintf(df, " %u:%d:%d:%d", ics->sfb_cb[g][sfb], ics->scalefactors[g][sfb], nnz,
                    ms ? ms[g][sfb] : 0);
        }
        fprintf(df, " /");
        wo += ics->window_group_length[g];
    }
    fprintf(df, "\n");
}
#endif

/* Clamp and round to nearest: truncating toward zero costs the output half
 * an LSB of error against any rounding decoder, on every sample. */
static inline int16_t pcm_to_s16(float v)
{
    /* Clamp, then let the FPU round: adding 1.5*2^23 leaves the rounded
     * integer in the low mantissa bits. Straight-line float ops and one
     * narrowing, so the loops vectorise where copysign + convert did not. */
    v = v < 32767.0f ? v : 32767.0f;
    v = v > -32768.0f ? v : -32768.0f;
    v += 12582912.0f;
    /* A union, not memcpy: with -fno-builtin (ESP-IDF on Xtensa) memcpy becomes a call per sample. */
    union { float f; int32_t i; } u;
    u.f = v;
    return (int16_t)u.i;
}

/* Both integer 24-bit containers share scaling, rounding and clipping. */
static inline int32_t pcm_to_s24(float v)
{
    v *= 256.0f;
    v = v < 8388607.0f ? v : 8388607.0f;
    v = v > -8388608.0f ? v : -8388608.0f;
    return (int32_t)lrintf(v);
}

FAADAPI faad_status faad_decode_frame(faad_decoder *decoder,
                                      const uint8_t *in_buf, uint32_t in_bytes,
                                      uint32_t *bytes_consumed,
                                      void *out_pcm, uint32_t out_cap_bytes,
                                      uint32_t *bytes_written,
                                      uint32_t *frame_flags)
{
    if (bytes_consumed) *bytes_consumed = 0;
    if (bytes_written) *bytes_written = 0;
    if (frame_flags) *frame_flags = 0;
    if (!decoder || !in_buf || !bytes_consumed || !out_pcm || !bytes_written)
        return FAAD_ERR_INVALID_ARGUMENT;
    /* RAW lengths enter bit-count arithmetic directly. ADTS lengths are
     * bounded by the 13-bit frame header before payload parsing. */
    if (decoder->config.stream_format == FAAD_STREAM_RAW && in_bytes > UINT32_MAX / 8)
        return FAAD_ERR_INVALID_ARGUMENT;
    uint32_t sample_bytes = output_sample_bytes(decoder->config.output_format);
    if (sample_bytes != 3 && ((uintptr_t)out_pcm & (sample_bytes - 1)))
        return FAAD_ERR_INVALID_ARGUMENT;
    if (out_cap_bytes < decoder->max_output_bytes) return FAAD_ERR_OUTPUT_TOO_SMALL;
    if (!in_bytes) return FAAD_ERR_NEED_MORE_DATA;
    const uint32_t prior_channels = decoder->num_channels;
    const uint32_t prior_errors = decoder->consecutive_errors;

    BitReader bs;
    bits_init(&bs, in_buf, in_bytes);
    uint32_t adts_frame_len = in_bytes;

    if (decoder->config.stream_format == FAAD_STREAM_ADTS) {
        /* Validate framing without touching the instance. Preserve a split
         * syncword rather than silently eating its first byte. */
        if (in_buf[0] == 0xff && in_bytes == 1) return FAAD_ERR_NEED_MORE_DATA;
        if (in_bytes < 2 || in_buf[0] != 0xff || (in_buf[1] & 0xf6) != 0xf0) {
            uint32_t skip = 1;
            while (skip + 1 < in_bytes && !(in_buf[skip] == 0xff
                && (in_buf[skip + 1] & 0xf6) == 0xf0)) skip++;
            if (skip + 1 >= in_bytes) skip = in_bytes - (in_buf[in_bytes - 1] == 0xff);
            *bytes_consumed = skip;
            return FAAD_ERR_SYNC_LOST;
        }
        uint32_t header_bytes = (in_buf[1] & 1) ? 7 : 9;
        if (in_bytes < header_bytes) return FAAD_ERR_NEED_MORE_DATA;
        adts_frame_len = ((uint32_t)(in_buf[3] & 3) << 11)
            | ((uint32_t)in_buf[4] << 3) | (in_buf[5] >> 5);
        if (adts_frame_len < header_bytes) {
            *bytes_consumed = 1;
            return FAAD_ERR_DECODE_FAILED;
        }
        if (adts_frame_len > in_bytes) return FAAD_ERR_NEED_MORE_DATA;
        AudioSpecificConfig asc;
        faad_status st = adts_decode_header(&bs, &asc, &adts_frame_len);
        if (st != FAAD_OK || config_channels(asc.num_channels) > MAX_CHANNELS) {
            /* An invalid header is a false sync, so its length field cannot be
             * trusted to skip over real frames behind it. */
            *bytes_consumed = st == FAAD_ERR_DECODE_FAILED ? 1 : adts_frame_len;
            return st != FAAD_OK ? st : FAAD_ERR_UNSUPPORTED;
        }
        if (!asc.num_channels) asc.pce = decoder->asc.pce;
        decoder->asc = asc;
        faad_layout_update(decoder);
        /* Without a configuration or PCE a frame starts from the channel count of
         * the frames before it, not from a stereo guess. */
        decoder->core_channels = (!asc.num_channels && !asc.pce.channels && decoder->pcm_emitted)
            ? decoder->core_channels : asc_channels(&asc);
        decoder->core_sample_rate = asc.sample_rate;
        bs.len = adts_frame_len;
    }
    *bytes_consumed = adts_frame_len;
    decoder->num_channels = decoder->core_channels;
#ifndef FAAD_DISABLE_DRC
    decoder->drc.present = false;
#endif
    bool decode_success = true;
    bool degraded = false;
#ifdef FAAD_STATS
    g_faadStats.totalFrames++;
#endif
    uint32_t active_chs = decoder->num_channels;
    if (active_chs > MAX_CHANNELS) active_chs = MAX_CHANNELS;
    for (uint32_t c = 0; c < active_chs; c++)
        memset(decoder->spec[c], 0, sizeof(decoder->spec[c]));
    decoder->sbr_present = false;
    decoder->ps_present = false;

#ifdef FAAD_STATS
    g_faadStats.frameSectBits = g_faadStats.frameSfBits = g_faadStats.frameSpecBits = 0;
    g_faadStats.frameAuxBits = g_faadStats.frameSbrBits = g_faadStats.framePsBits = 0;
    g_faadStats.frameFillBits = 0;
    unsigned frame_hdr_bits = bits_get_consumed(&bs);
#endif
#if MAX_CHANNELS >= 3
    CouplingTarget coupling_targets[MAX_CHANNELS];
    unsigned coupling_target_count = 0;
    for (CouplingChannel *cc = decoder->coupling; cc; cc = cc->next) cc->present = false;
#endif
    bool have_coupling = false;
    uint32_t ch_idx = 0;
    uint32_t last_elem_type = ID_SCE;
    uint32_t last_elem_ch = 0;
    uint32_t occupied = 0;
    uint32_t seq = 1; /* element types, two bits each, behind a leading 1 */
#ifndef FAAD_DISABLE_SBR
    uint8_t elem_nch[MAX_CHANNELS] = {0};
#endif

    bool saw_end = false;
    if (decode_success) {
        /* Fill elements (SBR) follow the last channel element, so the loop
         * runs to END even once every channel slot is taken. */
        while (bits_get_consumed(&bs) + 3 <= bs.len * 8) {
            uint32_t syntax_id = bits_get(&bs, 3);
#ifdef FAAD_STATS
            g_faadStats.elementCounts[syntax_id]++;
#endif
            if (syntax_id == ID_END) {
                saw_end = true;
                break;
            } else if (syntax_id == ID_SCE || syntax_id == ID_LFE) {
                seq = seq << 2 | syntax_id;
                uint32_t elem_ch = element_slot(decoder, syntax_id, bits_show(&bs, 4), ch_idx, occupied);
                if (elem_ch >= MAX_CHANNELS
                    || (SLOTS_BY_TAG(decoder) && (occupied & (1u << elem_ch)))) {
                    decode_success = false; break;
                }
                occupied |= 1u << elem_ch;
#ifndef FAAD_DISABLE_SBR
                elem_nch[elem_ch] = 1;
#endif
                last_elem_ch = elem_ch;
#if MAX_CHANNELS >= 3
                coupling_targets[coupling_target_count++] = (CouplingTarget){
                    bs, decoder->pns_seed, (uint8_t)syntax_id, (uint8_t)bits_show(&bs, 4), (uint8_t)elem_ch };
#endif
                last_elem_type = syntax_id;
#ifdef FAAD_STATS
                unsigned b0 = bits_get_consumed(&bs);
#endif
                ICSInfo *ics = &decoder->scratch->cpe.ics[0]; /* cleared by decode_sce */
                if (decode_sce(&bs, decoder, ics, elem_ch) != FAAD_OK) {
                    decode_success = false; /* the rest of the payload is out of step */
                    break;
                }
#ifdef FAAD_STATS
                core_dump_ics(elem_ch, ics, decoder->spec[elem_ch], NULL,
                              bits_get_consumed(&bs) - b0);
#endif
                decoder->win_seq[elem_ch] = ics->window_sequence;
                decoder->win_shape[elem_ch] = ics->window_shape;
                apply_pns(ics, decoder->spec[elem_ch], &decoder->pns_seed);
                apply_tns(ics, decoder->spec[elem_ch]);
                ch_idx += 1;
            } else if (syntax_id == ID_CPE) {
#if MAX_CHANNELS < 2
                decode_success = false; /* a pair cannot fit in the channel ceiling */
                break;
#else
                seq = seq << 2 | ID_CPE;
                uint32_t elem_ch = element_slot(decoder, syntax_id, bits_show(&bs, 4), ch_idx, occupied);
                if (elem_ch + 1 >= MAX_CHANNELS
                    || (SLOTS_BY_TAG(decoder) && (occupied & (3u << elem_ch)))) {
                    decode_success = false; break;
                }
                occupied |= 3u << elem_ch;
#ifndef FAAD_DISABLE_SBR
                elem_nch[elem_ch] = 2;
#endif
                last_elem_ch = elem_ch;
#if MAX_CHANNELS >= 3
                coupling_targets[coupling_target_count++] = (CouplingTarget){
                    bs, decoder->pns_seed, ID_CPE, (uint8_t)bits_show(&bs, 4), (uint8_t)elem_ch };
#endif
                last_elem_type = ID_CPE;
                CPEInfo *cpe = &decoder->scratch->cpe;
                memset(cpe, 0, sizeof(*cpe));
#ifdef FAAD_STATS
                unsigned b0 = bits_get_consumed(&bs);
#endif
                if (decode_cpe(&bs, decoder, cpe, elem_ch) != FAAD_OK) {
                    decode_success = false;
                    break;
                }
#ifdef FAAD_STATS
                {
                    unsigned nb = bits_get_consumed(&bs) - b0;
                    /* The all-one mask sends no per-band flags. */
                    if (cpe->ms_mask_present == MS_MASK_ALL)
                        memset(cpe->ms_used, 1, sizeof(cpe->ms_used));
                    const uint8_t (*ms)[MAX_SFB] = cpe->ms_mask_present != MS_MASK_NONE ? (const uint8_t (*)[MAX_SFB])cpe->ms_used : NULL;
                    core_dump_ics(elem_ch, &cpe->ics[0], decoder->spec[elem_ch], ms, nb);
                    core_dump_ics(elem_ch + 1, &cpe->ics[1], decoder->spec[elem_ch + 1], ms, cpe->ms_mask_present);
                }
#endif
                for (uint32_t i = 0; i < 2; i++) {
                    decoder->win_seq[elem_ch + i] = cpe->ics[i].window_sequence;
                    decoder->win_shape[elem_ch + i] = cpe->ics[i].window_shape;
                }

                apply_pns(&cpe->ics[0], decoder->spec[elem_ch], &decoder->pns_seed);
                apply_pns(&cpe->ics[1], decoder->spec[elem_ch + 1], &decoder->pns_seed);
                apply_ms_stereo(cpe, decoder->spec[elem_ch], decoder->spec[elem_ch + 1]);
                apply_is_stereo(cpe, decoder->spec[elem_ch], decoder->spec[elem_ch + 1]);
                apply_tns(&cpe->ics[0], decoder->spec[elem_ch]);
                apply_tns(&cpe->ics[1], decoder->spec[elem_ch + 1]);

                ch_idx += 2;
#endif
            } else if (syntax_id == ID_CCE) {
#if MAX_CHANNELS >= 3
                faad_status st = decode_cce(&bs, decoder);
                if (st == FAAD_ERR_NO_MEMORY) return st;
                if (st != FAAD_OK) { decode_success = false; break; }
                have_coupling = true;
#else
                return FAAD_ERR_UNSUPPORTED;
#endif
            } else if (syntax_id == ID_DSE) {
                decode_dse(&bs);
            } else if (syntax_id == ID_PCE) {
                if (decode_pce(&bs, decoder) != FAAD_OK) { decode_success = false; break; }
            } else if (syntax_id == ID_FIL) {
                uint32_t count = bits_get(&bs, 4);
                if (count == 15) count += bits_get(&bs, 8) - 1;
                if (count > 0) {
                    uint32_t fill_end = bits_get_consumed(&bs) + count * 8;
                    uint32_t ext_type = bits_get(&bs, 4);
                    if (ext_type == SBR_EXTENSION_DATA || ext_type == SBR_EXTENSION_DATA_CRC) {
                        uint32_t ch0 = last_elem_ch;
#ifdef FAAD_STATS
                        unsigned sbr_mark = bits_get_consumed(&bs);
                        unsigned ps_mark = g_faadStats.framePsBits;
#endif
                        /* Repack the unaligned fill into a reader whose last bit
                         * is exactly the declared payload boundary. */
                        uint8_t payload[269]; /* maximum fill count: 15 + 255 - 1 */
                        BitReader source = bs, fill;
                        source.byte_pos = (fill_end - count * 8) / 8;
                        source.bit_pos = (fill_end - count * 8) % 8;
                        for (uint32_t i = 0; i < count; i++) payload[i] = (uint8_t)bits_get(&source, 8);
                        if (source.overread) {
                            decode_success = false;
                            break;
                        }
                        bits_init(&fill, payload, count);
                        bits_skip(&fill, 4); /* extension type already read */
                        faad_status sbr_st = sbr_decode_extension(decoder, &fill, ch0, last_elem_type, ext_type == SBR_EXTENSION_DATA_CRC);
                        bits_skip(&bs, bits_get_consumed(&fill) - 4);
#ifndef FAAD_DISABLE_SBR
                        if (sbr_st == FAAD_ERR_NO_MEMORY) return sbr_st;
                        if (sbr_st != FAAD_OK) {
                            /* The core is intact: play it band-limited rather than
                             * conceal it, and keep the half-read payload out of
                             * the envelope history. */
                            uint32_t nch = (last_elem_type == ID_CPE) ? 2 : 1;
                            for (uint32_t c = ch0; c < ch0 + nch && c < MAX_CHANNELS; c++) decoder->sbr[c].have_frame = false;
                            decoder->sbr_present = true;
                            degraded = true;
                        }
#else
                        (void)sbr_st;
#endif
                        uint32_t consumed = bits_get_consumed(&bs);
#ifdef FAAD_STATS
                        g_faadStats.fillElementCount++;
                        g_faadStats.frameSbrBits += consumed - sbr_mark - (g_faadStats.framePsBits - ps_mark);
#endif
                        if (consumed < fill_end) {
                            uint32_t pad = fill_end - consumed;
                            bits_skip(&bs, pad);
#ifdef FAAD_STATS
                            g_faadStats.fillElementPadBitsSum += pad;
                            g_faadStats.frameFillBits += pad;
                            if (pad > g_faadStats.fillElementMaxPad) {
                                g_faadStats.fillElementMaxPad = pad;
                            }
#endif
                        }
                    } else {
#ifndef FAAD_DISABLE_DRC
                        if (ext_type == 11) {
                            uint32_t end = fill_end;
                            if (end > bs.len * 8) end = bs.len * 8;
                            read_drc(decoder, bs, end);
                        }
#endif
                        bits_skip(&bs, (count - 1) * 8 + 4);
                    }
                }
            }
        }
#ifdef FAAD_STATS
        if (!saw_end) {
            g_faadStats.nonEndTermination++;
        }
        FILE *df = faad_dump_file();
        if (df) {
            const faadDecStats *s = &g_faadStats;
            fprintf(df, "B %u %u %u %u %u %u %u %u %u %u\n", s->totalFrames, bits_get_consumed(&bs),
                    frame_hdr_bits, s->frameSectBits, s->frameSfBits, s->frameSpecBits, s->frameAuxBits,
                    s->frameSbrBits, s->framePsBits, s->frameFillBits);
        }
#endif
    }

    if (!saw_end || bs.overread) decode_success = false;
#if MAX_CHANNELS >= 3
    if (decode_success && have_coupling
        && coupling_spectrum(decoder, coupling_targets, coupling_target_count) != FAAD_OK)
        decode_success = false;
    if (!decode_success)
        for (CouplingChannel *cc = decoder->coupling; cc; cc = cc->next) cc->present = false;
#endif
#if MAX_CHANNELS >= 3
    if (SLOTS_BY_TAG(decoder) && occupied != (1u << decoder->layout.n) - 1) decode_success = false;
#endif
    if (decode_success && !faad_layout_frozen(decoder) && !decoder->asc.num_channels && !decoder->asc.pce.channels)
        faad_layout_infer(decoder, seq);
    /* A fixed RAW layout also fixes the lifetime output capacity. */
    if (decoder->config.stream_format == FAAD_STREAM_RAW && (decoder->asc.num_channels || decoder->asc.pce.channels)
        && ch_idx > asc_channels(&decoder->asc))
        return FAAD_ERR_DECODE_FAILED;

    if (decode_success && ch_idx > 0) {
        decoder->consecutive_errors = 0;
        decoder->num_channels = ch_idx;
        decoder->core_channels = ch_idx;
#ifndef FAAD_DISABLE_SBR
        /* A concealed frame keeps the layout of the frame it repeats. */
        memcpy(decoder->elem_nch, elem_nch, sizeof(elem_nch));
#endif
#ifdef FAAD_STATS
        if (!g_faadStats.haveLastChannels) {
            g_faadStats.haveLastChannels = true;
            g_faadStats.lastChannels = ch_idx;
            g_faadStats.minChannels = ch_idx;
            g_faadStats.maxChannels = ch_idx;
        } else {
            if (ch_idx != g_faadStats.lastChannels) {
                g_faadStats.channelCountChanges++;
                g_faadStats.lastChannels = ch_idx;
            }
            if (ch_idx < g_faadStats.minChannels) g_faadStats.minChannels = ch_idx;
            if (ch_idx > g_faadStats.maxChannels) g_faadStats.maxChannels = ch_idx;
        }
#endif
    } else {
#ifdef FAAD_STATS
        g_faadStats.errorConcealmentFrames++;
#endif
        decoder->consecutive_errors++;
        float fade = 0.0f;
        if (decoder->consecutive_errors <= 5) {
            fade = powf(0.8f, (float)decoder->consecutive_errors);
        }
        for (uint32_t c = 0; c < decoder->num_channels; c++) {
            for (int i = 0; i < FRAME_LEN_LONG; i++) {
                decoder->spec[c][i] = decoder->prev_spec[c][i] * fade;
            }
            /* Repeat the last block's windowing; a start block's spectrum
             * follows its own short-window overlap as a stop block. */
            static const uint8_t next_seq[4] = { ONLY_LONG_SEQUENCE, LONG_STOP_SEQUENCE,
                                                 EIGHT_SHORT_SEQUENCE, ONLY_LONG_SEQUENCE };
            decoder->win_seq[c] = next_seq[decoder->prev_window_seq[c] & 3];
            decoder->win_shape[c] = decoder->prev_window_shape[c];
        }
    }

    /* One buffer, one frame_samples-long run per channel: SBR analyzes a
     * channel's whole core frame before it synthesises that channel, so it
     * works in place over the core output at the start of each run. */
    if (decoder->sbr_present) decoder->sbr_seen = true;
#ifndef FAAD_DISABLE_PS
    if (decoder->ps_present || decoder->asc.is_ps) decoder->ps_seen = true;
#endif
    bool sbr_frame = decoder->asc.is_sbr || decoder->sbr_present || decoder->sbr_seen;
#ifdef FAAD_DISABLE_SBR
    sbr_frame = false;
#endif
    uint32_t core_samples = FRAME_IS_960(decoder) ? 960 : 1024;
    decoder->frame_samples = sbr_frame ? (core_samples * 2) : core_samples;
#ifndef FAAD_DISABLE_DRC
    decoder->drc.applied = decoder->drc.boost = false;
    if (decode_success && decoder->drc.present && (decoder->config.drc_cut || decoder->config.drc_boost))
        apply_drc(decoder);
#endif
    float *pcm_final = decoder->pcm;
    uint32_t num_chs = decoder->num_channels;
    bool spectral_downmixed = false;
    if (decoder->spectral_downmix_active
        && (sbr_frame || have_coupling || decoder->num_channels <= 2
            || decoder->spectral_downmix_channels != decoder->num_channels
            || decoder->spectral_downmix_layout != decoder->asc.num_channels
            || !spectral_downmix_windows_match(decoder, decoder->num_channels))) {
        restore_channel_overlaps(decoder, prior_channels, prior_errors);
        decoder->spectral_downmix_active = false;
    }
    if (!sbr_frame && !have_coupling)
        spectral_downmixed = try_spectral_downmix(decoder, decoder->num_channels, &num_chs, pcm_final,
                                                     decode_success && ch_idx > 0);
    if (!spectral_downmixed) {
        uint32_t lfe_channel = faad_downmix_lfe_channel(decoder, decoder->num_channels);
        for (uint32_t c = 0; c < decoder->num_channels; c++) {
            if (c != lfe_channel)
                imdct_and_window(decoder, c, decoder->win_seq[c], decoder->win_shape[c], decoder->spec[c], pcm_final + c * FRAME_LEN_LONG);
        }
    }
    if (!spectral_downmixed && decode_success && ch_idx > 0) {
        if (ch_idx == MAX_CHANNELS) {
            /* IMDCT has consumed this spectrum; rotate before SBR reuses spec[0]. */
            float (*tmp)[FRAME_LEN_LONG] = decoder->prev_spec;
            decoder->prev_spec = decoder->spec;
            decoder->spec = tmp;
        } else {
            /* A later ADTS header can expand concealment before that channel decodes. */
            memcpy(decoder->prev_spec, decoder->spec, sizeof(decoder->spec[0]) * ch_idx);
        }
    }
#if MAX_CHANNELS >= 3
    if (decoder->coupling) coupling_pcm(decoder);
#endif
    if (sbr_frame) {
        /* Move backwards after IMDCT scratch is dead, making room for each
         * channel's doubled output without overwriting another core frame. */
        for (uint32_t c = decoder->num_channels; c-- > 1; )
            memmove(pcm_final + c * 2048, pcm_final + c * FRAME_LEN_LONG,
                    sizeof(float[FRAME_LEN_LONG]));
        sbr_apply(decoder, decoder->num_channels, pcm_final);
    }

    uint32_t frame_samples = decoder->frame_samples;
    /* Channel runs sit at a fixed stride: the IMDCT and SBR buffers are sized
     * for 1024- and 2048-sample frames whatever the core frame length. */
    uint32_t pcm_stride = sbr_frame ? 2 * FRAME_LEN_LONG : FRAME_LEN_LONG;

    /* Output in the WAV / SMPTE order (FL FR FC LFE BL BR SL SR), taken
     * from the element order the channel configuration implies. */
    const float *src[MAX_CHANNELS < 2 ? 2 : MAX_CHANNELS];
    if (!spectral_downmixed) {
        num_chs = decoder->num_channels;
        const uint8_t *map = faad_layout_map(decoder, num_chs);
        for (uint32_t c = 0; c < num_chs; c++)
            src[c] = pcm_final + (map ? map[c] : c) * pcm_stride;
        float *dst[DM_CH];
        for (uint32_t c = 0; c < DM_CH; c++)
            dst[c] = pcm_final + c * pcm_stride;
        uint32_t mixed = downmix_pcm(decoder->config.downmix_mode, src, num_chs,
                                     downmix_mask(decoder, num_chs), frame_samples, dst);
        if (mixed != num_chs)
            for (num_chs = mixed; mixed--; ) src[mixed] = dst[mixed];
    }
    if (num_chs <= 2)
        for (uint32_t c = 0; c < num_chs; c++) src[c] = pcm_final + c * pcm_stride;

    uint32_t required_bytes = frame_samples * num_chs * output_sample_bytes(decoder->config.output_format);
    /* The entry check guarantees capacity before any history advances. A
     * lifetime-bound violation is a rejected frame, never a retryable buffer
     * error after synthesis has already consumed the packet. */
    if (required_bytes > decoder->max_output_bytes) return FAAD_ERR_DECODE_FAILED;

#ifndef FAAD_DISABLE_DRC
    if ((decoder->config.drc_target_level != 0.0f || decoder->drc.boost)
        && drc_output(decoder, src, num_chs, frame_samples, out_pcm)) goto output_done;
#endif
    if (decoder->config.output_format == FAAD_OUTPUT_16BIT) {
        int16_t * restrict out_int16 = (int16_t *)out_pcm;
        if (num_chs == 2) {
            const float * restrict pcm_l = src[0];
            const float * restrict pcm_r = src[1];
            for (uint32_t i = 0; i < frame_samples; i++) {
                out_int16[2 * i]     = pcm_to_s16(pcm_l[i]);
                out_int16[2 * i + 1] = pcm_to_s16(pcm_r[i]);
            }
        } else if (num_chs == 1) {
            const float * restrict pcm_m = src[0];
            for (uint32_t i = 0; i < frame_samples; i++)
                out_int16[i] = pcm_to_s16(pcm_m[i]);
        } else {
            for (uint32_t i = 0; i < frame_samples; i++)
                for (uint32_t c = 0; c < num_chs; c++)
                    out_int16[i * num_chs + c] = pcm_to_s16(src[c][i]);
        }
    } else if (decoder->config.output_format == FAAD_OUTPUT_24BIT) {
        /* Packed in host byte order like the wider formats; byte-wise stores assume no alignment of out_pcm. */
        uint8_t * restrict out_u8 = (uint8_t *)out_pcm;
        for (uint32_t i = 0; i < frame_samples; i++)
            for (uint32_t c = 0; c < num_chs; c++, out_u8 += 3)
                write_pcm24(out_u8, pcm_to_s24(src[c][i]), FAAC_BIG_ENDIAN);
    } else if (decoder->config.output_format == FAAD_OUTPUT_24IN32) {
        int32_t * restrict out_int32 = (int32_t *)out_pcm;
        for (uint32_t i = 0; i < frame_samples; i++)
            for (uint32_t c = 0; c < num_chs; c++)
                out_int32[i * num_chs + c] = pcm_to_s24(src[c][i]);
    } else {
        /* The core reconstructs at 16-bit full scale; float output is unity full scale. */
        const float norm = 1.0f / 32768.0f;
        float * restrict out_f32 = (float *)out_pcm;
        if (num_chs == 2) {
            const float * restrict pcm_l = src[0];
            const float * restrict pcm_r = src[1];
            for (uint32_t i = 0; i < frame_samples; i++) {
                out_f32[2 * i] = pcm_l[i] * norm;
                out_f32[2 * i + 1] = pcm_r[i] * norm;
            }
        } else if (num_chs == 1) {
            const float * restrict pcm_m = src[0];
            for (uint32_t i = 0; i < frame_samples; i++)
                out_f32[i] = pcm_m[i] * norm;
        } else {
            for (uint32_t i = 0; i < frame_samples; i++)
                for (uint32_t c = 0; c < num_chs; c++)
                    out_f32[i * num_chs + c] = src[c][i] * norm;
        }
    }

#ifndef FAAD_DISABLE_DRC
output_done:
    ;
#endif
    *bytes_consumed = adts_frame_len;
    *bytes_written = required_bytes;

    bool sbr_active = sbr_frame;
#ifdef FAAD_STATS
    if (sbr_active) {
        g_faadStats.sbrActiveFrames++;
    }
#endif

    decoder->format_known = decoder->format_known || decoder->asc.num_channels != 0
        || (decode_success && ch_idx > 0);
    decoder->sample_rate = sbr_active ? 2 * decoder->core_sample_rate : decoder->core_sample_rate;
    uint32_t mask = channel_mask(decoder, num_chs);
    uint32_t delay = sbr_active ? 2 * FAAD_SBR_CORE_DELAY : 0;
    uint32_t flags = 0;
#ifndef FAAD_DISABLE_DRC
    if (decoder->drc.applied) flags |= FAAD_FRAME_DRC;
    if (decoder->drc.present) flags |= FAAD_FRAME_DRC_INFO;
#endif
    if (!decoder->pcm_emitted || decoder->emitted_format.sample_rate != decoder->sample_rate
        || decoder->emitted_format.channels != num_chs
        || decoder->emitted_format.frame_samples != frame_samples
        || decoder->emitted_format.channel_mask != mask
        || decoder->emitted_format.decoder_delay != delay)
        flags |= FAAD_FRAME_FORMAT_CHANGED;
    if (sbr_active) flags |= FAAD_FRAME_SBR;
#ifndef FAAD_DISABLE_PS
    if (sbr_active && decoder->ps_seen && decoder->core_channels == 1) flags |= FAAD_FRAME_PS;
#endif
    if (!decode_success || ch_idx == 0) flags |= FAAD_FRAME_CONCEALED;
    if (degraded) flags |= FAAD_FRAME_DEGRADED;
    if (required_bytes) {
        decoder->emitted_format.sample_rate = decoder->sample_rate;
        decoder->emitted_format.channels = num_chs;
        decoder->emitted_format.frame_samples = frame_samples;
        decoder->emitted_format.channel_mask = mask;
        decoder->emitted_format.decoder_delay = delay;
        decoder->pcm_emitted = true;
        faad_layout_freeze(decoder);
        decoder->format_known = true;
        if (frame_flags) *frame_flags = flags;
    }

    return FAAD_OK;
}

FAADAPI const char *faad_strerror(faad_status status)
{
    switch (status) {
        case FAAD_OK:                   return "Success";
        case FAAD_ERR_INVALID_ARGUMENT: return "Invalid argument";
        case FAAD_ERR_UNSUPPORTED:      return "Unsupported configuration";
        case FAAD_ERR_NO_MEMORY:        return "Insufficient memory allocated";
        case FAAD_ERR_OUTPUT_TOO_SMALL: return "Output buffer too small";
        case FAAD_ERR_NEED_MORE_DATA:   return "Need more input data";
        case FAAD_ERR_DECODE_FAILED:    return "Decoding failed";
        case FAAD_ERR_SYNC_LOST:        return "Lost syncword alignment";
        default:                        return "Unknown status";
    }
}
