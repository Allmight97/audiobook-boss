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

#include <assert.h>
#include <limits.h>
#include <string.h>

#include "sbr.h"
#include "sbr_internal.h"
#include "sbr_huff_tables.h"
#include "sbr_tables.h"
#include "bitstream.h"
#include "channels.h"
#include "util.h"
#include "faac_internal.h"
#include "stats.h"

static void put_header(const SBRInfo *sbr, BitStream *bs)
{
    /* sbr_header(), with extra_1 present: 21 bits, or 27 with extra_2 for the
     * limiter fields (bands 2, gains 3, interpol_freq 1, smoothing 1). A gain
     * limit can suppress the requested high-band envelope, hence gains 3. */
    int extra2 = sbr->numEnvFixFix > 1;
    uint32_t header = SBR_AMP_RES;
    header = (header << 4) | sbr->bs_start_freq;
    header = (header << 4) | sbr->bs_stop_freq;
    header = (header << 3) | SBR_XOVER_BAND;
    header = (header << 4) | 2 | extra2; /* reserved=0, extra_1=1 */
    header = (header << 2) | sbr->bs_freq_scale;
    header = (header << 1) | sbr->bs_alter_scale;
    header <<= 2; /* noise_bands=0 */
    if (extra2) header = (header << 6) | 0x2f;
    PutBit(bs, header, 21 + 6 * extra2);
}

static void put_grid(const SbrFrameData *fd, BitStream *bs)
{
    int num_env = fd->numEnvelopes;
    PutBit(bs, fd->frameClass, 2);
    if (fd->frameClass == SBR_FRAME_CLASS_VARFIX) {
        PutBit(bs, fd->tEnv[0], 2);
        PutBit(bs, num_env - 1, 2);
        for (int i = 0; i < num_env - 1; i++) PutBit(bs, (fd->tEnv[i + 1] - fd->tEnv[i] - 2) / 2, 2);
        /* VARFIX frames carry two envelopes and a two-bit pointer. */
        PutBit(bs, SBR_GRID_POINTER, 2);
        for (int i = 0; i < num_env; i++) PutBit(bs, SBR_FREQ_RES_HIGH, 1);
    } else {
        PutBit(bs, num_env >> 1, 2);
        PutBit(bs, SBR_FREQ_RES_HIGH, 1);
    }
}

/* Where each book sits in sbr_env_codes: its frequency-delta code, then its
 * time-delta one. Indexed [balance][fd->eff_amp_res]. */
typedef struct { uint16_t offset; uint8_t lav, start; } SbrDeltaBook;

static const SbrDeltaBook sbr_env_books[2][2] = {
    { { 0, 60, 7 }, { 134, 31, 6 } },
    { { 210, 24, 6 }, { 272, 12, 5 } },
};
_Static_assert(272 + (2 * 12 + 1) + (2 * T_HUFF_ENV_LAV + 1) == SBR_ENV_CODES_LEN, "sbr_env_books must tile sbr_env_codes");

/* Writes a run of envelope values, as time deltas against previous or as
 * frequency deltas along the run; -1 when one falls outside the book. */
static int code_deltas(const int *values, const int *previous, int count, int time,
                       const SbrDeltaBook *book, BitStream *bs)
{
    int lav = time ? T_HUFF_ENV_LAV : book->lav;
    const uint16_t *tab = sbr_env_codes + book->offset + time * (2 * book->lav + 1) + lav;
    /* Frequency deltas run against the band before, and open with the first band as is. */
    const int *ref = time ? previous : values;
    int first = !time;

    if (first) PutBit(bs, values[0], book->start);
    for (int i = first; i < count; i++) {
        int d = values[i] - ref[i - first];
        if (d < -lav || d > lav) return -1;
        /* PutBit keeps only the low length bits, including for short codes. */
        PutBit(bs, (tab[d] >> 5) | ~63U, tab[d] & 31);
    }
    return 0;
}

/* Coupled rendition of a pair from the channels' own levels: the level of
 * (L + R) / 2, and the balance L - R at half resolution around its center. */
static void couple_envelopes(SBRInfo *sbr, const SbrFrameData *fd, int ch0)
{
    int amp = fd->eff_amp_res;
    int pan = amp ? 12 : 24;
    const int *restrict left = fd->ch[ch0].envData[0], *restrict right = fd->ch[ch0 + 1].envData[0];
    int *restrict level = sbr->cpl[0].envData[0], *restrict balance = sbr->cpl[1].envData[0];

    /* Every slot rather than just the frame's bands: the fixed trip count
     * compiles to a fraction of the code, and the rest is never read. */
    for (int b = 0; b < SBR_MAX_ENVELOPES * SBR_MAX_BANDS; b++) {
        int l = left[b], r = right[b];
        int d = l > r ? l - r : r - l;
        /* the louder side plus log2((1 + 2^-d) / 2) in level steps, rounded */
        level[b] = (l > r ? l : r) - (d >= 2) - (!amp && d >= 5);
        balance[b] = (clamp_int(pan + l - r, 0, 2 * pan) + 1) >> 1;
    }
}

/* What envelope e of channel ch is coded against in time: the one before it,
 * or for the first the channel's last written one. */
static const int *env_ref(const SBRInfo *sbr, const SbrEnvData *env, int ch, int e)
{
    return e ? env->envData[e - 1] : sbr->ch[ch].ref[~sbr->frameCount & 1].env;
}

/* Each envelope takes the cheaper of frequency and time deltas, sized by
 * summing the code lengths (ties favor frequency). Bit e of the result is set
 * for time; -1 if some envelope can't be coded either way. */
static int choose_dt(const SBRInfo *sbr, const SbrFrameData *fd, const SbrEnvData *env, int ch,
                     int linked, const SbrDeltaBook *book)
{
    int dt = 0;

    for (int e = 0; e < fd->numEnvelopes; e++) {
        uint32_t best = UINT32_MAX;
        for (int t = 0; t <= (e || linked); t++) {
            int lav = t ? T_HUFF_ENV_LAV : book->lav;
            const uint16_t *tab = sbr_env_codes + book->offset + t * (2 * book->lav + 1) + lav;
            const int *values = env->envData[e];
            const int *ref = t ? env_ref(sbr, env, ch, e) : values;
            uint32_t bits = t ? 0 : book->start;
            int i = !t, nb = sbr->numBands;
            for (; i < nb; i++) {
                int d = values[i] - ref[i - !t];
                if (d < -lav || d > lav) break;
                bits += tab[d] & 31;
            }
            if (i == nb && bits < best) {
                best = bits;
                dt = (dt & ~(1 << e)) | t << e;
            }
        }
        if (best == UINT32_MAX) return -1;
    }
    return dt;
}

/* The channel data of one layout, coupled or not; -1 if some envelope can't
 * be coded in it. bs_df_env precedes the envelopes in the stream, so the
 * choices are all made first. They depend only on the reference slot this
 * call does not write, so every pass over a frame agrees. */
static int put_channels(SBRInfo *sbr, const SbrFrameData *fd, BitStream *bs, int nch, int ch0,
                        int sendHeader, int coupled)
{
    const SbrEnvRef *prev = &sbr->ch[ch0].ref[~sbr->frameCount & 1];
    int nb = sbr->numBands, n_env = fd->numEnvelopes, n_q = n_env > 1 ? 2 : 1;
    /* Time deltas need the previous frame's envelope in the same layout;
     * header frames stay self-contained so a decoder can start there. */
    int noiseLinked = !sendHeader && prev->nb && prev->coupled == coupled;
    int linked = noiseLinked && prev->nb == nb && prev->ampRes == fd->eff_amp_res;
    const SbrEnvData *env = coupled ? sbr->cpl : &fd->ch[ch0];
    const SbrDeltaBook *book[2] = { &sbr_env_books[0][fd->eff_amp_res], &sbr_env_books[coupled][fd->eff_amp_res] };
    int ngrid = nch >> coupled;
    int dt[2];

    for (int ch = 0; ch < nch; ch++)
        if ((dt[ch] = choose_dt(sbr, fd, &env[ch], ch0 + ch, linked, book[ch])) < 0) return -1;

    PutBit(bs, 0, 1); /* bs_data_extra */
    if (nch == 2) PutBit(bs, coupled, 1); /* bs_coupling */
    for (int ch = 0; ch < ngrid; ch++) put_grid(fd, bs);
    for (int ch = 0; ch < nch; ch++) {
        for (int e = 0; e < n_env; e++) PutBit(bs, dt[ch] >> e & 1, 1);
        for (int q = 0; q < n_q; q++) PutBit(bs, q | noiseLinked, 1);
    }
    for (int ch = 0; ch < ngrid; ch++) PutBit(bs, sbr->rich ? SBR_INVF_MODE_RICH : SBR_INVF_MODE, 2);
    /* Coupled pairs send envelope and noise per channel in turn; otherwise
     * all envelopes come first, then all noise floors. k walks that order:
     * its high bit picks the channel and its low bit envelope or noise when
     * coupled, and the other way round when not. */
    for (int k = 0; k < 2 * nch; k++) {
        int hi = k >> (nch - 1), lo = k & (nch - 1);
        int ch = coupled ? hi : lo;
        if (coupled ? lo : hi) {
            /* One band at a constant level: 5-bit absolute (the level, or for
             * a coupled balance channel the center 6), or the time delta 0,
             * whose code is the single bit 0. */
            for (int q = 0; q < n_q; q++) {
                int t = q | noiseLinked;
                PutBit(bs, t ? 0 : (coupled & ch) ? 6 : SBR_NOISE_LEVEL_DEFAULT, t ? 1 : 5);
            }
        } else {
            for (int e = 0; e < n_env; e++)
                code_deltas(env[ch].envData[e], env_ref(sbr, &env[ch], ch0 + ch, e), nb, dt[ch] >> e & 1, book[ch], bs);
        }
    }
    for (int ch = 0; ch < nch; ch++) PutBit(bs, 0, 1); /* bs_add_harmonic_flag */
    if (PsIsActive(sbr))
        PsWriteExtension(sbr, fd, bs);
    else
        PutBit(bs, 0, 1); /* bs_extended_data */

    for (int ch = 0; ch < nch; ch++) {
        SbrEnvRef *cur = &sbr->ch[ch0 + ch].ref[sbr->frameCount & 1];
        memcpy(cur->env, env[ch].envData[n_env - 1], nb * sizeof(int));
        cur->nb = nb;
        cur->ampRes = fd->eff_amp_res;
        cur->coupled = coupled;
    }
    return 0;
}

/* The extension_payload body for EXT_SBR_DATA: the 4-bit extension type, the
 * 1-bit header flag, the optional header, and the channel data. Returns its
 * bits, or INT_MAX if the layout can't be coded. */
static int put_payload(SBRInfo *sbr, const SbrFrameData *fd, BitStream *bs, int nch, int ch0,
                       int sendHeader, int coupled)
{
    uint32_t start = bs->currentBit;
    PutBit(bs, SBR_EXT_TYPE_SBR, 4); /* extension_type */
    PutBit(bs, sendHeader, 1);       /* bs_header_flag */
    if (sendHeader) put_header(sbr, bs);
    if (put_channels(sbr, fd, bs, nch, ch0, sendHeader, coupled)) return INT_MAX;
    return (int)(bs->currentBit - start);
}

static int SbrWrite(SBRInfo *sbr, const SbrFrameData *fd, BitStream *bs, int id_aac, int ch0)
{
    int sendHeader = sbr->sendHeaderThisFrame;
    int nch = (id_aac == ID_CPE) ? 2 : 1;
    int payloadBits = 0, coupled = 0;
    int fillBytes = 0;
    /* Room for 2 channels of 4 envelopes of 64 bands at 20 bits plus fields. */
    enum { SBR_DRY_RUN_CAPACITY = 2048 };
    BitStream dry;

    if (nch == 2) couple_envelopes(sbr, fd, ch0);

    /* The fill_element's cnt field must precede the payload in the bitstream,
     * so its size is needed before anything is written: each layout is sized
     * through the bounded counting stream (a pair is coupled when smaller,
     * ties favor coupled) and the last pass writes the winner. One call site
     * in a loop, so -O3 has no constant nch to clone on. */
    for (int pass = 0; pass <= nch; pass++) {
        int write = pass == nch;
        BitStream *out = bs;

        if (write) {
            fillBytes = (payloadBits + 7) / 8;

            /* The fill_element count escapes through an 8-bit field, so a single
             * extension_payload tops out at 15 + 255 - 1 = 269 bytes. A larger SBR
             * payload would silently truncate esc_count and corrupt the boundary. */
            assert(fillBytes <= 14 + 255);

            /* fill_element(): id, then 4-bit count with optional 8-bit escape.
             * The decoder reconstructs cnt = 15 + esc_count - 1, hence
             * esc_count = N - 14. */
            PutBit(bs, ID_FIL, 3);
            PutBit(bs, fillBytes < 15 ? fillBytes : 15, 4);
            if (fillBytes >= 15) PutBit(bs, fillBytes - 14, 8);
        } else {
            InitBitStream(&dry, NULL, SBR_DRY_RUN_CAPACITY);
            out = &dry;
        }
        int bits = put_payload(sbr, fd, out, nch, ch0, sendHeader, write ? coupled : pass);
        if (write) {
            PutBit(bs, 0, fillBytes * 8 - payloadBits);
        } else if (!pass || bits <= payloadBits) {
            payloadBits = bits;
            coupled = pass;
        }
    }
    return (fillBytes < 15 ? 7 : 15) + fillBytes * 8;
}

int SbrContextGetBits(SBRContext *sCtx, BitStream *bs, const AACElement *elem, int aacObjectType)
{
#if MAX_CHANNELS >= 6
    if (IsSBR(aacObjectType) && sCtx && elem->type != ID_LFE) {
#else
    if (IsSBR(aacObjectType) && sCtx) {
#endif
        if (sCtx->sbrInfo) {
            int id_aac = (elem->type == ID_CPE) ? ID_CPE : ID_SCE;
            /* One step past the newest slot is the oldest: the payload whose
             * audio this access unit's core carries. See SBR_FRAME_FIFO. */
            const SbrFrameData *fd = sCtx->frameFIFO[(sCtx->frameHead + 1) % SBR_FRAME_FIFO];
            SBRInfo *sbr = sCtx->sbrInfo;
            if (!sbr->headerDecided) {
                sbr->sendHeaderThisFrame = (sbr->frameCount++ % SBR_HEADER_PERIOD == 0);
                sbr->headerDecided = 1;
                PsCountFrame(sbr, fd, id_aac);
            }
            return SbrWrite(sbr, fd, bs, id_aac, elem->channels[0]);
        }
    }
    return 0;
}
