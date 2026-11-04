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

/* AAC coupling channel syntax and reconstruction (ISO/IEC 14496-3,
 * Tables 4.8, 4.153 and 4.154). */
#include "faad_internal.h"

faad_status decode_cce(BitReader *bs, struct faad_decoder *dec)
{
    unsigned tag = bits_get(bs, 4);
    CouplingChannel *cc = dec->coupling;
    while (cc && cc->tag != tag) cc = cc->next;
    if (!cc) {
        cc = AllocMemory(sizeof(*cc));
        if (!cc) return FAAD_ERR_NO_MEMORY;
        memset(cc, 0, sizeof(*cc));
        cc->tag = (uint8_t)tag;
        cc->next = dec->coupling;
        dec->coupling = cc;
        dec->allocated_bytes += sizeof(*cc);
        if (sizeof(*cc) > dec->largest_allocation) dec->largest_allocation = sizeof(*cc);
    }
    if (cc->present) return FAAD_ERR_DECODE_FAILED; /* duplicate tag */
    cc->present = true;
    bool independent = bits_get(bs, 1);
    cc->targets = (uint8_t)(bits_get(bs, 3) + 1);
    unsigned lists = 0;
    for (unsigned i = 0; i < cc->targets; i++) {
        cc->target[i].pair = (uint8_t)bits_get(bs, 1);
        cc->target[i].tag = (uint8_t)bits_get(bs, 4);
        cc->target[i].select = cc->target[i].pair ? (uint8_t)bits_get(bs, 2) : 0;
        cc->target[i].gain = (uint8_t)lists++;
        if (cc->target[i].select == 3) lists++;
    }
    unsigned domain = bits_get(bs, 1);
    cc->point = independent ? 2 : (uint8_t)domain;
    if (independent && !cc->overlap) {
        cc->overlap = AllocMemory(sizeof(float[FRAME_LEN_LONG]));
        if (!cc->overlap) return FAAD_ERR_NO_MEMORY;
        memset(cc->overlap, 0, sizeof(float[FRAME_LEN_LONG]));
        dec->allocated_bytes += sizeof(float[FRAME_LEN_LONG]);
        if (sizeof(float[FRAME_LEN_LONG]) > dec->largest_allocation)
            dec->largest_allocation = sizeof(float[FRAME_LEN_LONG]);
    }
    bool sign = bits_get(bs, 1);
    float step = (float)(1u << bits_get(bs, 2)) / 8.0f;
    memset(&cc->ics, 0, sizeof(cc->ics));
    faad_status st = decode_ics(bs, dec, &cc->ics, cc->spec, false);
    if (st != FAAD_OK) return st;
    unsigned bands = cc->ics.num_window_groups * cc->ics.max_sfb;
    if (bands > 128) return FAAD_ERR_DECODE_FAILED;
    for (unsigned l = 0; l < lists; l++) {
        bool common = l == 0 || independent || bits_get(bs, 1);
        int gain = l && common ? decode_coupling_gain(bs) : 0;
        for (unsigned b = 0; b < (independent ? 1 : bands); b++) {
            unsigned g = cc->ics.max_sfb ? b / cc->ics.max_sfb : 0;
            unsigned sfb = cc->ics.max_sfb ? b % cc->ics.max_sfb : 0;
            if (!common && cc->ics.sfb_cb[g][sfb]) gain += decode_coupling_gain(bs);
            /* The sign bit belongs to the accumulated gain, not its delta.
             * Division rounds down for signed odd exponents. */
            int exponent = sign && !common ? (gain - (gain & 1)) / 2 : gain;
            float value = exp2f(-exponent * step);
            if (sign && !common && (gain & 1)) value = -value;
            if (!isfinite(value)) return FAAD_ERR_DECODE_FAILED;
            cc->gain[l][b] = value;
        }
    }
    if (bs->overread) return FAAD_ERR_DECODE_FAILED;
    apply_pns(&cc->ics, cc->spec, &dec->pns_seed);
    apply_tns(&cc->ics, cc->spec);
    return FAAD_OK;
}

static void add_spectrum(const CouplingChannel *cc, unsigned list, float *dst)
{
    const ICSInfo *ics = &cc->ics;
    unsigned window = 0, band = 0;
    unsigned stride = ics->window_sequence == EIGHT_SHORT_SEQUENCE ? ics->frame_len_long / 8 : 0;
    for (unsigned g = 0; g < ics->num_window_groups; g++) {
        for (unsigned sfb = 0; sfb < ics->max_sfb; sfb++, band++) {
            if (!ics->sfb_cb[g][sfb]) continue;
            float gain = cc->gain[list][band];
            for (unsigned w = 0; w < ics->window_group_length[g]; w++) {
                unsigned base = (window + w) * stride;
                for (unsigned i = ics->sfb_offsets[sfb]; i < ics->sfb_offsets[sfb + 1]; i++)
                    dst[base + i] += gain * cc->spec[base + i];
            }
        }
        window += ics->window_group_length[g];
    }
}

static void couple_channel(const CouplingChannel *cc, unsigned ch, float *dst, const float *pcm)
{
    for (unsigned t = 0; t < cc->targets; t++) {
        unsigned base = cc->target[t].ch, select = cc->target[t].select;
        unsigned list = cc->target[t].gain;
        bool left = ch == base && select != 1;
        bool right = cc->target[t].pair && ch == base + 1 && select != 2;
        if (!left && !right) continue;
        if (right && select == 3) list++;
        if (cc->point < 2) add_spectrum(cc, list, dst);
        else {
            float gain = cc->gain[list][0];
            for (unsigned i = 0; i < cc->ics.frame_len_long; i++) dst[i] += gain * pcm[i];
        }
    }
}

static void couple_spectrum(struct faad_decoder *dec, unsigned ch, unsigned point)
{
    for (CouplingChannel *cc = dec->coupling; cc; cc = cc->next)
        if (cc->present && cc->point == point)
            couple_channel(cc, ch, dec->spec[ch], NULL);
}

faad_status coupling_spectrum(struct faad_decoder *dec, CouplingTarget *targets, unsigned count)
{
    for (CouplingChannel *cc = dec->coupling; cc; cc = cc->next) {
        if (!cc->present) continue;
        for (unsigned t = 0; t < cc->targets; t++) {
            unsigned i;
            for (i = 0; i < count; i++)
                if (targets[i].tag == cc->target[t].tag
                    && targets[i].type == (cc->target[t].pair ? ID_CPE : ID_SCE)) break;
            if (i == count) return FAAD_ERR_DECODE_FAILED;
            cc->target[t].ch = targets[i].ch;
        }
    }
    uint32_t seed = dec->pns_seed;
#ifdef FAAD_STATS
    faadDecStats stats = g_faadStats;
#endif
    faad_status result = FAAD_OK;
    /* Only coupled frames take this pass. Reconstruct the pre-TNS spectra
     * from saved readers; this also handles CCEs preceding their targets.
     * Restore the original PNS seeds and leave frame statistics unchanged. */
    for (unsigned t = 0; t < count; t++) {
        CouplingTarget *target = &targets[t];
        CPEInfo *cpe = &dec->scratch->cpe;
        memset(cpe, 0, sizeof(*cpe));
        BitReader bs = target->bits;
        unsigned ch = target->ch, nch = target->type == ID_CPE ? 2 : 1;
        dec->pns_seed = target->seed;
        result = nch == 2 ? decode_cpe(&bs, dec, cpe, ch)
                          : decode_sce(&bs, dec, &cpe->ics[0], ch);
        if (result != FAAD_OK) break;
        for (unsigned c = 0; c < nch; c++) apply_pns(&cpe->ics[c], dec->spec[ch + c], &dec->pns_seed);
        if (nch == 2) {
            apply_ms_stereo(cpe, dec->spec[ch], dec->spec[ch + 1]);
            apply_is_stereo(cpe, dec->spec[ch], dec->spec[ch + 1]);
        }
        for (unsigned c = 0; c < nch; c++) {
            ICSInfo *ics = &cpe->ics[c];
            for (CouplingChannel *cc = dec->coupling; cc; cc = cc->next) {
                if (!cc->present || cc->point == 2) continue;
                for (unsigned i = 0; i < cc->targets; i++) {
                    unsigned base = cc->target[i].ch, sel = cc->target[i].select;
                    if (!((ch + c == base && sel != 1)
                        || (cc->target[i].pair && ch + c == base + 1 && sel != 2))) continue;
                    if (ics->window_sequence != cc->ics.window_sequence
                        || ics->window_shape != cc->ics.window_shape) {
                        result = FAAD_ERR_DECODE_FAILED;
                        goto done;
                    }
                }
            }
            couple_spectrum(dec, ch + c, 0);
            apply_tns(ics, dec->spec[ch + c]);
            couple_spectrum(dec, ch + c, 1);
        }
    }
done:
    dec->pns_seed = seed;
#ifdef FAAD_STATS
    g_faadStats = stats;
#endif
    return result;
}

void coupling_pcm(struct faad_decoder *dec)
{
    for (CouplingChannel *cc = dec->coupling; cc; cc = cc->next) {
        if (!cc->present || cc->point != 2) {
            if (cc->overlap) memset(cc->overlap, 0, sizeof(float[FRAME_LEN_LONG]));
            cc->prev_shape = 0;
            continue;
        }
        /* Core synthesis and concealment history have consumed spec already;
         * SBR has not started, so one channel holds temporary coupling PCM. */
        float *pcm = dec->spec[0];
        imdct_window(dec, cc->ics.window_sequence, cc->ics.window_shape,
                     cc->prev_shape, cc->overlap, cc->spec, pcm);
        cc->prev_shape = cc->ics.window_shape;
        for (unsigned ch = 0; ch < dec->num_channels; ch++)
            couple_channel(cc, ch, dec->pcm + ch * FRAME_LEN_LONG, pcm);
    }
}
