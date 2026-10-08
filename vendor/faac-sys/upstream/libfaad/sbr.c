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
#include "sbr_tables.h"
#include "fft.h"

#ifndef FAAD_DISABLE_SBR
#define SBR_NOISE_FLOOR_OFFSET 6

/* Limiter gains (§4.6.18.7.5), amplitude domain: -3 dB, 0 dB, +3 dB, none. */
static const float sbr_lim_gain[4] = { 0.70794578f, 1.0f, 1.41253754f, 1e10f };

/* Gain smoothing window (§4.6.18.7.6). */
static const float sbr_h_smooth[5] = {
    0.33333333333333f, 0.30150283239582f, 0.21816949906249f,
    0.11516383427084f, 0.03183050093751f
};

/* Twiddles for the FFT-based QMF banks */

static float ana_pre_c[64], ana_pre_s[64];   /* exp(+j*pi*n/64) */
static float ana_post_c[32], ana_post_s[32]; /* exp(-j*pi*(k+1/2)/128) */
static float syn_rot_c[32], syn_rot_s[32];   /* exp(-j*pi*(n+1/8)/64) */
static float syn_a_c[32], syn_a_s[32];       /* the same, over 128 */
static float syn_b_c[32], syn_b_s[32];       /* the same times exp(+j*2*pi*k/64), over 128 */
void init_qmf_twiddles(void)
{
    fft_init();
    for (int n = 0; n < 64; n++) {
        ana_pre_c[n] = (float)cos(M_PI * n / 64.0);
        ana_pre_s[n] = (float)sin(M_PI * n / 64.0);
    }
    for (int k = 0; k < 32; k++) {
        ana_post_c[k] = (float)cos(-0.5 * M_PI * (k + 0.5) / 64.0);
        ana_post_s[k] = (float)sin(-0.5 * M_PI * (k + 0.5) / 64.0);
    }
    for (int n = 0; n < 32; n++) {
        double a = -M_PI * (n + 0.125) / 64.0, b = a + 2.0 * M_PI * n / 64.0;
        syn_rot_c[n] = (float)cos(a);
        syn_rot_s[n] = (float)sin(a);
        syn_a_c[n] = (float)(cos(a) / 128.0);
        syn_a_s[n] = (float)(sin(a) / 128.0);
        syn_b_c[n] = (float)(cos(b) / 128.0);
        syn_b_s[n] = (float)(sin(b) / 128.0);
    }
}

/* QMF banks (§4.6.18.4, §4.6.18.8) */

/* Push one 32-sample block into the analysis delay line and window it:
 * u(n) = sum_j x(n + 64j) c(2(n + 64j)), newest sample first. The line is
 * a ring of ten 32-sample blocks kept twice over, so the five 64-sample
 * runs never wrap. */
static void qmf_analysis_window(SBRChannel *ch, const float *in, float u[64])
{
    int pos = ch->qmf_x_pos - 32;
    if (pos < 0) pos += 320;
    ch->qmf_x_pos = pos;
    float *x = ch->qmf_x + ch->qmf_x_pos;
    for (int n = 0; n < 32; n++) x[n] = x[320 + n] = in[31 - n];
    for (int n = 0; n < 64; n++) {
        float acc = 0.0f;
        for (int j = 0; j < 5; j++) acc += x[n + 64 * j] * qmf_c[2 * (n + 64 * j)];
        u[n] = acc;
    }
}

/* 32-band analysis of two consecutive slots (64 new samples). The
 * decimated bank is the lower half of the 64-band one, so its phase origin
 * sits a quarter sample in: X(k) = 2 sum_n u(n) exp(j*pi/64*(k+1/2)(2n-1/2)),
 * the conjugate of a 64-point DFT of u(n)*exp(-j*pi*n/64), rotated; the
 * factor 2 restores the passband gain the decimated prototype c(2n)
 * halves. u is real, so that DFT is conjugate-symmetric about bin 63/2 and
 * one transform of u0 + j*u1 carries both slots. */
static void qmf_analysis_pair(SBRChannel *ch, const float *in, float out0[32][2], float out1[32][2])
{
    float u0[64], u1[64];
    qmf_analysis_window(ch, in, u0);
    qmf_analysis_window(ch, in + 32, u1);

    float z[128], w[128];
    for (int n = 0; n < 64; n++) {
        z[n]      = u0[n] * ana_pre_c[n] + u1[n] * ana_pre_s[n];
        z[64 + n] = u1[n] * ana_pre_c[n] - u0[n] * ana_pre_s[n];
    }
    fft(z, w, 6);
    const float *re = w, *im = w + 64;
    for (int k = 0; k < 32; k++) {
        /* 2 conj(W0(k)) and 2 conj(W1(k)) from W(k) and W(63 - k) */
        float ar = re[k] + re[63 - k], ai = im[63 - k] - im[k];
        float br = im[k] + im[63 - k], bi = re[k] - re[63 - k];
        out0[k][0] = ar * ana_post_c[k] - ai * ana_post_s[k];
        out0[k][1] = ar * ana_post_s[k] + ai * ana_post_c[k];
        out1[k][0] = br * ana_post_c[k] - bi * ana_post_s[k];
        out1[k][1] = br * ana_post_s[k] + bi * ana_post_c[k];
    }
}

/* Separate restrict pointers so the tap loop vectorises. */
static inline void mac64(float * restrict acc, const float * restrict x, const float * restrict c)
{
    for (int n = 0; n < 64; n++) acc[n] += x[n] * c[n];
}

/* 64-band synthesis of one slot: 64 output samples.
 * v(n) = 1/64 sum_k Re{X(k) exp(j*pi/64*(k+1/2)(n-127.5))}, n = 0..127,
 * which is C(n-128) - S(n-128) for C the DCT-IV of Re X and S the DST-IV
 * of Im X; their symmetries give all 128 samples from the 64 of each. The
 * DST-IV is a DCT-IV of the reversed input with alternate signs, and each
 * DCT-IV of 64 is a 32-point complex FFT between two rotations, so both
 * FFTs ride one 64-point transform: the first on the even inputs, the
 * second on the odd, separated by one butterfly. The delay line is a ring
 * of ten 128-sample blocks; the newest block starts at qmf_v_pos. */
static void qmf_synthesis_slot(SBRChannel *ch, float X[64][2], float *out)
{
    float z[128], w[128];
    for (int n = 0; n < 32; n++) {
        float c = syn_rot_c[n], s = syn_rot_s[n];
        float ar = X[2 * n][0], ai = X[63 - 2 * n][0];
        float br = X[63 - 2 * n][1], bi = X[2 * n][1];
        z[2 * n]          = ar * c - ai * s;
        z[64 + 2 * n]     = ar * s + ai * c;
        z[2 * n + 1]      = br * c - bi * s;
        z[64 + 2 * n + 1] = br * s + bi * c;
    }
    fft(z, w, 6);

    int vpos = ch->qmf_v_pos - 128;
    if (vpos < 0) vpos += 1280;
    ch->qmf_v_pos = vpos;
    float *v = ch->qmf_v + ch->qmf_v_pos;
    const float *wr = w, *wi = w + 64;
    for (int k = 0; k < 32; k++) {
        float pr = wr[k] + wr[k + 32], pi = wi[k] + wi[k + 32];
        float mr = wr[k] - wr[k + 32], mi = wi[k] - wi[k + 32];
        float cr = pr * syn_a_c[k] - pi * syn_a_s[k], ci = pr * syn_a_s[k] + pi * syn_a_c[k];
        float dr = mr * syn_b_c[k] - mi * syn_b_s[k], di = mr * syn_b_s[k] + mi * syn_b_c[k];
        v[2 * k]       = dr - cr;
        v[127 - 2 * k] = cr + dr;
        v[63 - 2 * k]  = ci + di;
        v[64 + 2 * k]  = di - ci;
    }

    /* Ten 64-tap runs, each inside one 128-sample block, so no run wraps. */
    const float *run[10];
    for (int i = 0; i < 5; i++) {
        int pos0 = ch->qmf_v_pos + 256 * i;
        pos0 -= (pos0 >= 1280) ? 1280 : 0;
        int pos1 = pos0 + 192;
        pos1 -= (pos1 >= 1280) ? 1280 : 0;
        run[2 * i]     = ch->qmf_v + pos0;
        run[2 * i + 1] = ch->qmf_v + pos1;
    }
    for (int n = 0; n < 64; n++) out[n] = run[0][n] * qmf_c[n];
    for (int i = 1; i < 10; i++) mac64(out, run[i], qmf_c + 64 * i);
}


/* Frequency band tables (§4.6.18.3.2) and patches (§4.6.18.6.3) */

static int cmp_u8(const void *a, const void *b) { return (int)*(const uint8_t *)a - (int)*(const uint8_t *)b; }

static int sbr_round(double v) { return (int)floor(v + 0.5); }

static int sbr_start_band(uint32_t sr, int start_freq)
{
    int row = (sr <= 16000) ? 0 : (sr <= 22050) ? 1 : (sr <= 24000) ? 2 : (sr <= 32000) ? 3 : (sr <= 64000) ? 4 : 5;
    int temp = (sr < 32000) ? 3000 : (sr < 64000) ? 4000 : 5000;
    int start_min = (int)(((temp << 7) + (sr >> 1)) / sr);
    int k0 = start_min + sbr_offset[row][start_freq & 15];
    return k0 < 1 ? 1 : k0 > 63 ? 63 : k0;
}

static int sbr_stop_band(uint32_t sr, int k0, int stop_freq)
{
    if (stop_freq == 15) return k0 * 3 < 64 ? k0 * 3 : 64;
    if (stop_freq == 14) return k0 * 2 < 64 ? k0 * 2 : 64;
    int temp = (sr < 32000) ? 3000 : (sr < 64000) ? 4000 : 5000;
    int stop_min = (int)(((temp << 8) + (sr >> 1)) / sr);
    uint8_t dk[13];
    int prev = stop_min;
    for (int i = 0; i < 13; i++) {
        int cur = sbr_round(stop_min * pow(64.0 / stop_min, (i + 1) / 13.0));
        dk[i] = (uint8_t)(cur - prev);
        prev = cur;
    }
    qsort(dk, 13, 1, cmp_u8);
    int k2 = stop_min;
    for (int i = 0; i < stop_freq; i++) k2 += dk[i];
    return k2 < 64 ? k2 : 64;
}

/* Master table for bs_freq_scale > 0: log-spaced bands, two regions above
 * a 2.2449 ratio, the upper one warped by alter_scale. */
static int sbr_master_log(SBRElement *el, int k0, int k2)
{
    int bands = 14 - 2 * el->freq_scale; /* 12, 10, 8 */
    double warp = el->alter_scale ? 1.3 : 1.0;
    int two_regions = ((double)k2 / k0 > 2.2449);
    int k1 = two_regions ? 2 * k0 : k2;

    int nb0 = 2 * sbr_round(bands * log2((double)k1 / k0) / 2.0);
    if (nb0 < 1 || nb0 > SBR_MAX_BANDS) return -1;
    uint8_t dk0[SBR_MAX_BANDS];
    for (int i = 0; i < nb0; i++)
        dk0[i] = (uint8_t)(sbr_round(k0 * pow((double)k1 / k0, (i + 1.0) / nb0)) - sbr_round(k0 * pow((double)k1 / k0, (double)i / nb0)));
    qsort(dk0, nb0, 1, cmp_u8);
    if (dk0[0] == 0) return -1;
    el->f_master[0] = (uint8_t)k0;
    for (int i = 0; i < nb0; i++) el->f_master[i + 1] = (uint8_t)(el->f_master[i] + dk0[i]);
    int n = nb0;

    if (two_regions) {
        int nb1 = 2 * sbr_round(bands * log2((double)k2 / k1) / (2.0 * warp));
        if (nb1 < 1 || n + nb1 > SBR_MAX_BANDS) return -1;
        uint8_t dk1[SBR_MAX_BANDS];
        for (int i = 0; i < nb1; i++)
            dk1[i] = (uint8_t)(sbr_round(k1 * pow((double)k2 / k1, (i + 1.0) / nb1)) - sbr_round(k1 * pow((double)k2 / k1, (double)i / nb1)));
        qsort(dk1, nb1, 1, cmp_u8);
        if (dk1[0] < dk0[nb0 - 1]) {
            int change = dk0[nb0 - 1] - dk1[0];
            int half = (dk1[nb1 - 1] - dk1[0]) / 2;
            if (change > half) change = half;
            dk1[0] = (uint8_t)(dk1[0] + change);
            dk1[nb1 - 1] = (uint8_t)(dk1[nb1 - 1] - change);
        }
        qsort(dk1, nb1, 1, cmp_u8);
        if (dk1[0] == 0) return -1;
        for (int i = 0; i < nb1; i++) el->f_master[n + i + 1] = (uint8_t)(el->f_master[n + i] + dk1[i]);
        n += nb1;
    }
    return n;
}

/* Master table for bs_freq_scale == 0: uniform spacing, the remainder folded
 * into the outermost bands. */
static int sbr_master_linear(SBRElement *el, int k0, int k2)
{
    int dk = el->alter_scale ? 2 : 1;
    int nb = el->alter_scale ? 2 * sbr_round((k2 - k0) / 4.0) : 2 * ((k2 - k0) / 2);
    if (nb < 1 || nb > SBR_MAX_BANDS) return -1;
    int diff = k2 - (k0 + nb * dk);
    uint8_t vdk[SBR_MAX_BANDS];
    for (int i = 0; i < nb; i++) vdk[i] = (uint8_t)dk;
    int k = diff < 0 ? 0 : nb - 1, incr = diff < 0 ? 1 : -1;
    while (diff != 0) {
        vdk[k] = (uint8_t)(vdk[k] - incr);
        k += incr;
        diff += incr;
    }
    el->f_master[0] = (uint8_t)k0;
    for (int i = 0; i < nb; i++) el->f_master[i + 1] = (uint8_t)(el->f_master[i] + vdk[i]);
    return nb;
}

static bool sbr_build_patches(SBRElement *el, uint32_t sr)
{
    int k0 = el->k0, kx = el->kx, M = el->M;
    int msb = k0, usb = kx, np = 0;
    int goal_sb = (int)((2048000 + (sr >> 1)) / sr);
    int k;
    if (goal_sb < kx + M) {
        for (k = 0; k < el->n_master && el->f_master[k] < goal_sb; k++) ;
    } else {
        k = el->n_master;
    }
    int sb;
    /* Passes that add no patch only reset msb, so a malformed table can
     * revisit the same state forever; a valid one needs few passes. */
    for (int pass = 0;; pass++) {
        if (pass >= 4 * SBR_MAX_PATCHES) return false;
        int j = k + 1, odd;
        do {
            j--;
            sb = el->f_master[j];
            odd = (sb - 2 + k0) & 1;
        } while (sb > k0 - 1 + msb - odd && j > 0);

        if (np >= SBR_MAX_PATCHES) return false;
        int num = sb - usb > 0 ? sb - usb : 0;
        el->patch_num[np] = (uint8_t)num;
        el->patch_start[np] = (uint8_t)(k0 - odd - num);
        if (num > 0) {
            usb = sb;
            msb = sb;
            np++;
        } else {
            msb = kx;
        }
        if (el->f_master[k] - sb < 3) k = el->n_master;
        if (sb == kx + M) break;
    }

    if (np > 1 && el->patch_num[np - 1] < 3) np--;
    el->num_patches = (uint8_t)np;
    return np > 0;
}

static bool sbr_build_limiter(SBRElement *el)
{
    if (el->limiter_bands == 0) {
        el->f_lim[0] = el->f_low[0];
        el->f_lim[1] = el->f_low[el->n_low];
        el->n_lim = 1;
        return true;
    }
    static const double lim_bands[3] = { 1.2, 2.0, 3.0 };
    double lb = lim_bands[el->limiter_bands - 1];

    uint8_t borders[SBR_MAX_PATCHES + 1];
    int nb = 0;
    borders[nb++] = el->kx;
    for (int i = 0; i < el->num_patches; i++) borders[nb] = (uint8_t)(borders[nb - 1] + el->patch_num[i]), nb++;

    uint8_t lim[SBR_MAX_LIM + 1];
    int n = 0;
    for (int i = 0; i <= el->n_low; i++) lim[n++] = el->f_low[i];
    for (int i = 0; i < nb; i++) lim[n++] = borders[i];
    qsort(lim, n, 1, cmp_u8);

    int k = 1;
    while (k < n) {
        double oct = log2((double)lim[k] / lim[k - 1]);
        if (oct * lb < 0.49) {
            /* Too narrow: drop the border that is not a patch boundary;
             * two adjacent patch boundaries both stay. */
            int rm;
            if (lim[k] == lim[k - 1]) rm = k;
            else {
                bool kb = false, kb1 = false;
                for (int i = 0; i < nb; i++) { kb |= (borders[i] == lim[k]); kb1 |= (borders[i] == lim[k - 1]); }
                rm = !kb ? k : !kb1 ? k - 1 : -1;
            }
            if (rm < 0) { k++; continue; }
            memmove(lim + rm, lim + rm + 1, (size_t)(n - rm - 1));
            n--;
        } else {
            k++;
        }
    }
    el->n_lim = (uint8_t)(n - 1);
    memcpy(el->f_lim, lim, (size_t)n);
    return el->n_lim >= 1;
}

static bool sbr_build_tables(SBRElement *el, uint32_t sr)
{
    /* the band tables divide by sr; an explicit ASC rate can be anything */
    if (sr < 8000 || sr > 192000) return false;
    int k0 = sbr_start_band(sr, el->start_freq);
    int k2 = sbr_stop_band(sr, k0, el->stop_freq);
    int max_span = (sr <= 32000) ? 48 : (sr <= 44100) ? 35 : 32;
    if (k2 <= k0 || k2 - k0 > max_span) return false;
    el->k0 = (uint8_t)k0;
    el->k2 = (uint8_t)k2;

    int n = el->freq_scale ? sbr_master_log(el, k0, k2) : sbr_master_linear(el, k0, k2);
    if (n < 1 || el->xover_band >= n) return false;
    el->n_master = (uint8_t)n;

    el->n_high = (uint8_t)(n - el->xover_band);
    for (int i = 0; i <= el->n_high; i++) el->f_high[i] = el->f_master[el->xover_band + i];
    el->kx = el->f_high[0];
    el->M = (uint8_t)(el->f_high[el->n_high] - el->kx);
    if (el->kx > 32 || el->kx + el->M > 64) return false;

    el->n_low = (uint8_t)((el->n_high + 1) / 2);
    int odd = el->n_high & 1;
    el->f_low[0] = el->f_high[0];
    for (int i = 1; i <= el->n_low; i++) el->f_low[i] = el->f_high[2 * i - odd];

    int nq = el->noise_bands ? sbr_round(el->noise_bands * log2((double)k2 / el->kx)) : 1;
    if (nq < 1) nq = 1;
    if (nq > SBR_MAX_NQ) nq = SBR_MAX_NQ;
    el->n_q = (uint8_t)nq;
    el->f_noise[0] = el->f_low[0];
    int idx = 0;
    for (int k = 1; k <= nq; k++) {
        idx += (el->n_low - idx) / (nq + 1 - k);
        el->f_noise[k] = el->f_low[idx];
    }

    if (!sbr_build_patches(el, sr)) return false;
    return sbr_build_limiter(el);
}

static void sbr_reset_channel(SBRChannel *ch)
{
    memset(ch->x_low_tail, 0, sizeof(ch->x_low_tail));
    memset(ch->y_tail, 0, sizeof(ch->y_tail));
    memset(ch->bw_array, 0, sizeof(ch->bw_array));
    memset(ch->invf_mode_prev, 0, sizeof(ch->invf_mode_prev));
    memset(ch->E_prev, 0, sizeof(ch->E_prev));
    memset(ch->Q_prev, 0, sizeof(ch->Q_prev));
    memset(ch->s_index_prev, 0, sizeof(ch->s_index_prev));
    memset(ch->g_hist, 0, sizeof(ch->g_hist));
    memset(ch->q_hist, 0, sizeof(ch->q_hist));
    ch->freq_res_prev = 1;
    ch->l_A_prev = -1;
    ch->L_E_prev = 0;
    ch->t_E_end_prev = 0;
    ch->index_noise = 0;
    ch->index_sine = 0;
    ch->have_frame = false;
    ch->primed = false;
    ch->kx_prev = 0;
    ch->M_prev = 0;
}

/* Bitstream: header, grid, envelopes (§4.6.18.3) */

/* Canonical codewords of each length form a contiguous range. */
int sbr_huff_decode(BitReader *bs, const SBRHuffBook *book)
{
    uint32_t code = 0, first_code = 0;
    for (int len = 1; len <= HB_MAX_LEN; len++) {
        code = (code << 1) | bits_get_1(bs);
        int first = book->first[len], end = book->first[len + 1];
        if (!book->tab) {
            uint32_t index = code - first_code;
            if (index < (uint32_t)(end - first))
                return book->order[first + index] - book->offset;
            first_code = (first_code + (uint32_t)(end - first)) << 1;
        } else {
            for (int i = first; i < end; i++) {
                uint32_t c = book->tab[book->order[i]].code;
                if (c == code) return book->order[i] - book->offset;
                if (c > code) break;
            }
        }
    }
    return 0;
}

static const uint8_t sbr_ceil_log2[] = { 0, 1, 2, 2, 3, 3 };

static bool sbr_read_grid(BitReader *bs, SBRChannel *ch)
{
    ch->frame_class = (uint8_t)bits_get(bs, 2);
    int L_E;
    switch (ch->frame_class) {
    case 0: { /* FIXFIX */
        int v = (int)(bits_get(bs, 2) & 3);
        L_E = 1 << v;
        if (L_E > 4) return false;
        uint8_t fr = (uint8_t)bits_get(bs, 1);
        ch->t_E[0] = 0;
        for (int i = 1; i <= L_E; i++) ch->t_E[i] = (uint8_t)(i * 16 / L_E);
        for (int i = 0; i < L_E; i++) ch->freq_res[i] = fr;
        ch->bs_pointer = 0;
        break;
    }
    case 1: { /* FIXVAR */
        int end = 16 + (int)(bits_get(bs, 2) & 3);
        L_E = (int)(bits_get(bs, 2) & 3) + 1;
        if (L_E > 4) return false;
        ch->t_E[0] = 0;
        ch->t_E[L_E] = (uint8_t)end;
        for (int i = 0; i < L_E - 1; i++) {
            int rel = 2 * (int)(bits_get(bs, 2) & 3) + 2;
            ch->t_E[L_E - 1 - i] = (uint8_t)(ch->t_E[L_E - i] - rel);
        }
        ch->bs_pointer = (uint8_t)bits_get(bs, sbr_ceil_log2[L_E]);
        for (int i = 0; i < L_E; i++) ch->freq_res[L_E - 1 - i] = (uint8_t)bits_get(bs, 1);
        break;
    }
    case 2: { /* VARFIX */
        int start = (int)(bits_get(bs, 2) & 3);
        L_E = (int)(bits_get(bs, 2) & 3) + 1;
        if (L_E > 4) return false;
        ch->t_E[0] = (uint8_t)start;
        for (int i = 0; i < L_E - 1; i++) {
            int rel = 2 * (int)(bits_get(bs, 2) & 3) + 2;
            ch->t_E[i + 1] = (uint8_t)(ch->t_E[i] + rel);
        }
        ch->t_E[L_E] = 16;
        ch->bs_pointer = (uint8_t)bits_get(bs, sbr_ceil_log2[L_E]);
        for (int i = 0; i < L_E; i++) ch->freq_res[i] = (uint8_t)bits_get(bs, 1);
        break;
    }
    default: { /* VARVAR */
        int start = (int)(bits_get(bs, 2) & 3);
        int end = 16 + (int)(bits_get(bs, 2) & 3);
        int rel0 = (int)(bits_get(bs, 2) & 3);
        int rel1 = (int)(bits_get(bs, 2) & 3);
        L_E = rel0 + rel1 + 1;
        if (L_E > SBR_MAX_ENV) return false;
        ch->t_E[0] = (uint8_t)start;
        for (int i = 0; i < rel0; i++) {
            int rel = 2 * (int)(bits_get(bs, 2) & 3) + 2;
            ch->t_E[i + 1] = (uint8_t)(ch->t_E[i] + rel);
        }
        ch->t_E[L_E] = (uint8_t)end;
        for (int i = 0; i < rel1; i++) {
            int rel = 2 * (int)(bits_get(bs, 2) & 3) + 2;
            ch->t_E[L_E - 1 - i] = (uint8_t)(ch->t_E[L_E - i] - rel);
        }
        ch->bs_pointer = (uint8_t)bits_get(bs, sbr_ceil_log2[L_E]);
        for (int i = 0; i < L_E; i++) ch->freq_res[i] = (uint8_t)bits_get(bs, 1);
        break;
    }
    }
    ch->L_E = (uint8_t)L_E;
    for (int i = 0; i < L_E; i++)
        if (ch->t_E[i + 1] <= ch->t_E[i] || ch->t_E[i + 1] > 16 + 3) return false;
    if (ch->bs_pointer > L_E) return false;

    /* Transient envelope l_A and the noise floor borders (§4.6.18.3.3). */
    int p = ch->bs_pointer, mid;
    switch (ch->frame_class) {
    case 0:  ch->l_A = -1; mid = L_E / 2; break;
    case 2:  ch->l_A = (int8_t)(p ? p - 1 : -1); mid = (p == 0) ? 1 : (p == 1) ? L_E - 1 : p - 1; break;
    default: ch->l_A = (int8_t)(p ? L_E + 1 - p : -1); mid = (p > 1) ? L_E + 1 - p : L_E - 1; break;
    }
    if (mid < 0) mid = 0;
    ch->L_Q = (uint8_t)(L_E > 1 ? 2 : 1);
    ch->t_Q[0] = ch->t_E[0];
    if (ch->L_Q == 1) {
        ch->t_Q[1] = ch->t_E[L_E];
    } else {
        ch->t_Q[1] = ch->t_E[mid];
        ch->t_Q[2] = ch->t_E[L_E];
    }
    return true;
}

static bool sbr_read_envelope(BitReader *bs, const SBRElement *el, SBRChannel *ch, bool balance)
{
    /* 3.0 dB resolution: 5-bit start (6 for the balance channel); 1.5 dB: 7 (6). */
    int start_bits = ch->amp_res ? (balance ? 5 : 6) : (balance ? 6 : 7);
    const SBRHuffBook *t_book = &sbr_books[ch->amp_res ? (balance ? HB_T_ENV_BAL_30 : HB_T_ENV_30)
                                                       : (balance ? HB_T_ENV_BAL_15 : HB_T_ENV_15)];
    const SBRHuffBook *f_book = &sbr_books[ch->amp_res ? (balance ? HB_F_ENV_BAL_30 : HB_F_ENV_30)
                                                       : (balance ? HB_F_ENV_BAL_15 : HB_F_ENV_15)];

    /* The balance channel is coded at half resolution: its values count double. */
    int scale = balance ? 2 : 1;
    for (int l = 0; l < ch->L_E; l++) {
        int nb = ch->freq_res[l] ? el->n_high : el->n_low;
        if (ch->df_env[l] == 0) {
            ch->E[l][0] = (int16_t)(scale * (int)bits_get(bs, start_bits));
            for (int k = 1; k < nb; k++) {
                int d = scale * sbr_huff_decode(bs, f_book);
                ch->E[l][k] = (int16_t)(ch->E[l][k - 1] + d);
            }
        } else {
            /* Time deltas against the previous envelope, mapped across a
             * resolution change (§4.6.18.6.2). */
            const int16_t *prev = (l == 0) ? ch->E_prev : ch->E[l - 1];
            int r_prev = (l == 0) ? ch->freq_res_prev : ch->freq_res[l - 1];
            for (int k = 0; k < nb; k++) {
                int d = scale * sbr_huff_decode(bs, t_book);
                int ref;
                if (r_prev == ch->freq_res[l]) {
                    ref = prev[k];
                } else if (ch->freq_res[l]) {
                    int i = 0;
                    while (i + 1 < el->n_low && el->f_high[k] >= el->f_low[i + 1]) i++;
                    ref = prev[i];
                } else {
                    int i = 0;
                    while (i < el->n_high && el->f_high[i] != el->f_low[k]) i++;
                    ref = prev[i < el->n_high ? i : el->n_high - 1];
                }
                ch->E[l][k] = (int16_t)(ref + d);
            }
        }
        for (int k = 0; k < nb; k++) {
            if (ch->E[l][k] < -128 || ch->E[l][k] > 256) return false;
        }
    }
    return true;
}

static bool sbr_read_noise(BitReader *bs, const SBRElement *el, SBRChannel *ch, bool balance)
{
    const SBRHuffBook *t_book = &sbr_books[balance ? HB_T_NOISE_BAL_30 : HB_T_NOISE_30];
    const SBRHuffBook *f_book = &sbr_books[balance ? HB_F_ENV_BAL_30 : HB_F_ENV_30];

    int scale = balance ? 2 : 1;
    for (int l = 0; l < ch->L_Q; l++) {
        if (ch->df_noise[l] == 0) {
            ch->Q[l][0] = (int16_t)(scale * (int)bits_get(bs, 5));
            for (int k = 1; k < el->n_q; k++) {
                int d = scale * sbr_huff_decode(bs, f_book);
                ch->Q[l][k] = (int16_t)(ch->Q[l][k - 1] + d);
            }
        } else {
            const int16_t *prev = (l == 0) ? ch->Q_prev : ch->Q[l - 1];
            for (int k = 0; k < el->n_q; k++) {
                int d = scale * sbr_huff_decode(bs, t_book);
                ch->Q[l][k] = (int16_t)(prev[k] + d);
            }
        }
        for (int k = 0; k < el->n_q; k++) {
            if (ch->Q[l][k] < -60 || ch->Q[l][k] > 60) return false;
        }
    }
    return true;
}

faad_status sbr_decode_extension(struct faad_decoder *dec, BitReader *bs, uint32_t ch0, uint32_t syntax_id, bool crc)
{
    int nch = (syntax_id == ID_CPE) ? 2 : 1;
    if (ch0 + nch > MAX_CHANNELS) return FAAD_ERR_INVALID_ARGUMENT;
    SBRElement *el = &dec->sbr_el[ch0];
    if (crc) bits_skip(bs, 10); /* bs_sbr_crc_bits, not verified */
    uint32_t sbr_sr = dec->asc.sbr_sample_rate > 0 ? dec->asc.sbr_sample_rate : 2 * dec->core_sample_rate;

    if (bits_get(bs, 1)) { /* bs_header_flag */
        SBRElement h = *el;
        h.amp_res    = bits_get(bs, 1);
        h.start_freq = (uint8_t)bits_get(bs, 4);
        h.stop_freq  = (uint8_t)bits_get(bs, 4);
        h.xover_band = (uint8_t)bits_get(bs, 3);
        bits_skip(bs, 2);
        bool extra1 = bits_get(bs, 1);
        bool extra2 = bits_get(bs, 1);
        if (extra1) {
            h.freq_scale  = (uint8_t)bits_get(bs, 2);
            h.alter_scale = (uint8_t)bits_get(bs, 1);
            h.noise_bands = (uint8_t)bits_get(bs, 2);
        } else {
            h.freq_scale = 2; h.alter_scale = 1; h.noise_bands = 2;
        }
        if (extra2) {
            h.limiter_bands  = (uint8_t)bits_get(bs, 2);
            h.limiter_gains  = (uint8_t)bits_get(bs, 2);
            h.interpol_freq  = (uint8_t)bits_get(bs, 1);
            h.smoothing_mode = (uint8_t)bits_get(bs, 1);
        } else {
            h.limiter_bands = 2; h.limiter_gains = 2; h.interpol_freq = 1; h.smoothing_mode = 1;
        }
#ifdef FAAD_STATS
        g_faadStats.sbrHeaderCount++;
#endif
        /* A change to anything the tables depend on resets the element. */
        bool reset = !el->header_present || h.start_freq != el->start_freq || h.stop_freq != el->stop_freq ||
                     h.xover_band != el->xover_band || h.freq_scale != el->freq_scale ||
                     h.alter_scale != el->alter_scale || h.noise_bands != el->noise_bands;
        *el = h;
        el->header_present = true;
        if (reset) {
            if (!sbr_build_tables(el, sbr_sr)) {
                el->header_present = false;
                return FAAD_ERR_DECODE_FAILED;
            }
            for (int c = 0; c < nch; c++) sbr_reset_channel(&dec->sbr[ch0 + c]);
        }
#ifdef FAAD_STATS
        {
            FILE *df = faad_dump_file();
            if (df) {
                fprintf(df, "H %u %d %d %u %u %u %u %u %u %u %u %u %u %d %u %u %u %u %u %d %d\n",
                        g_faadStats.totalFrames, nch, (int)el->amp_res, el->start_freq, el->stop_freq,
                        el->xover_band, el->freq_scale, el->alter_scale, el->noise_bands,
                        el->limiter_bands, el->limiter_gains, el->interpol_freq, el->smoothing_mode,
                        (int)reset, el->kx, el->M, el->n_low, el->n_high, el->n_q,
                        (int)extra1, (int)extra2);
            }
        }
#endif
    }
    if (!el->header_present) return FAAD_OK; /* nothing to reconstruct against yet */

    SBRChannel *chs[2] = { &dec->sbr[ch0], &dec->sbr[ch0 + (nch > 1 ? 1 : 0)] };
    el->nch = (uint8_t)nch;
    /* a frame that fails part-way has half-written grids, so it must not reach HF */
    for (int c = 0; c < nch; c++) chs[c]->have_frame = false;

    /* sbr_single_channel_element / sbr_channel_pair_element (§4.6.18.3.4) */
    bool data_extra = bits_get(bs, 1);
    if (data_extra) bits_skip(bs, 4 * (uint32_t)nch); /* bs_reserved, ahead of bs_coupling */
    el->coupling = (nch == 2) ? bits_get(bs, 1) : false;

    if (!sbr_read_grid(bs, chs[0])) return FAAD_ERR_DECODE_FAILED;
    if (nch == 2) {
        if (el->coupling) {
            SBRChannel *a = chs[0], *b = chs[1];
            b->frame_class = a->frame_class; b->L_E = a->L_E; b->L_Q = a->L_Q;
            b->bs_pointer = a->bs_pointer; b->l_A = a->l_A;
            memcpy(b->t_E, a->t_E, sizeof(b->t_E));
            memcpy(b->t_Q, a->t_Q, sizeof(b->t_Q));
            memcpy(b->freq_res, a->freq_res, sizeof(b->freq_res));
        } else if (!sbr_read_grid(bs, chs[1])) {
            return FAAD_ERR_DECODE_FAILED;
        }
    }
    for (int c = 0; c < nch; c++) {
        SBRChannel *ch = chs[c];
        /* a single FIXFIX envelope is always coded at 1.5 dB */
        ch->amp_res = (ch->frame_class == 0 && ch->L_E == 1) ? false : el->amp_res;
    }
    for (int c = 0; c < nch; c++) {
        SBRChannel *ch = chs[c];
        for (int l = 0; l < ch->L_E && l < SBR_MAX_ENV; l++) ch->df_env[l] = (uint8_t)bits_get(bs, 1);
        for (int l = 0; l < ch->L_Q && l < 2; l++) ch->df_noise[l] = (uint8_t)bits_get(bs, 1);
    }
    for (int k = 0; k < el->n_q && k < SBR_MAX_NQ; k++) chs[0]->invf_mode[k] = (uint8_t)bits_get(bs, 2);
    if (nch == 2) {
        if (el->coupling) memcpy(chs[1]->invf_mode, chs[0]->invf_mode, sizeof(chs[0]->invf_mode));
        else for (int k = 0; k < el->n_q && k < SBR_MAX_NQ; k++) chs[1]->invf_mode[k] = (uint8_t)bits_get(bs, 2);
    }
    if (el->coupling) {
        if (!sbr_read_envelope(bs, el, chs[0], false)) return FAAD_ERR_DECODE_FAILED;
        if (!sbr_read_noise(bs, el, chs[0], false)) return FAAD_ERR_DECODE_FAILED;
        if (!sbr_read_envelope(bs, el, chs[1], true)) return FAAD_ERR_DECODE_FAILED;
        if (!sbr_read_noise(bs, el, chs[1], true)) return FAAD_ERR_DECODE_FAILED;
    } else {
        for (int c = 0; c < nch; c++) {
            if (!sbr_read_envelope(bs, el, chs[c], false)) return FAAD_ERR_DECODE_FAILED;
        }
        for (int c = 0; c < nch; c++) {
            if (!sbr_read_noise(bs, el, chs[c], false)) return FAAD_ERR_DECODE_FAILED;
        }
    }
    for (int c = 0; c < nch; c++) {
        SBRChannel *ch = chs[c];
        ch->add_harmonic_flag = bits_get(bs, 1);
        memset(ch->add_harmonic, 0, sizeof(ch->add_harmonic));
        if (ch->add_harmonic_flag)
            for (int k = 0; k < el->n_high; k++) ch->add_harmonic[k] = (uint8_t)bits_get(bs, 1);
    }
#ifdef FAAD_STATS
    g_faadStats.sbrEnvelopeSum += chs[0]->L_E;
#endif

    if (bits_get(bs, 1)) { /* bs_extended_data */
        uint32_t cnt = bits_get(bs, 4);
        if (cnt == 15) cnt += bits_get(bs, 8);
        uint32_t end = bits_get_consumed(bs) + cnt * 8;
        while (bits_get_consumed(bs) + 7 < end) {
            uint32_t id = bits_get(bs, 2);
#ifndef FAAD_DISABLE_PS
            if (id == PS_EXTENSION_DATA) {
                uint32_t here = bits_get_consumed(bs);
                ps_read_data(dec, bs, end > here ? end - here : 0);
#ifdef FAAD_STATS
                g_faadStats.framePsBits += bits_get_consumed(bs) - here;
#endif
                break;
            }
#endif
            (void)id;
            break;
        }
        uint32_t consumed = bits_get_consumed(bs);
        if (consumed < end) bits_skip(bs, end - consumed);
    }

    for (int c = 0; c < nch; c++) chs[c]->have_frame = true;
    dec->sbr_present = true;
    return FAAD_OK;
}

/* HF generation (§4.6.18.6) */

/* Chirp factors from the inverse filtering modes (§4.6.18.6.1). */
static void sbr_chirp(const SBRElement *el, SBRChannel *ch)
{
    static const float new_bw_tab[4] = { 0.0f, 0.75f, 0.9f, 0.98f };
    for (int k = 0; k < el->n_q; k++) {
        /* a switch between OFF and LOW in either direction lands on 0.6 */
        float nb = (ch->invf_mode[k] + ch->invf_mode_prev[k] == 1) ? 0.6f : new_bw_tab[ch->invf_mode[k] & 3];
        float prev = ch->bw_array[k];
        float t = (nb < prev) ? 0.75f * nb + 0.25f * prev : 0.90625f * nb + 0.09375f * prev;
        if (t < 0.015625f) t = 0.0f;
        if (t >= 0.99609375f) t = 0.99609375f;
        ch->bw_array[k] = t;
        ch->invf_mode_prev[k] = ch->invf_mode[k];
    }
}

/* Second-order covariance LPC of one low band over the whole buffer. */
static void sbr_lpc(float x[SBR_BUF_SLOTS][2], float a0[2], float a1[2])
{
    float p01r = 0, p01i = 0, p02r = 0, p02i = 0, p11 = 0, p12r = 0, p12i = 0, p22 = 0;
    for (int n = SBR_T_HFADJ; n < SBR_BUF_SLOTS; n++) {
        float x0r = x[n][0], x0i = x[n][1];
        float x1r = x[n - 1][0], x1i = x[n - 1][1];
        float x2r = x[n - 2][0], x2i = x[n - 2][1];
        /* phi(i,j) = sum x(n-i) conj(x(n-j)) */
        p01r += x0r * x1r + x0i * x1i; p01i += x0i * x1r - x0r * x1i;
        p02r += x0r * x2r + x0i * x2i; p02i += x0i * x2r - x0r * x2i;
        p12r += x1r * x2r + x1i * x2i; p12i += x1i * x2r - x1r * x2i;
        p11 += x1r * x1r + x1i * x1i;
        p22 += x2r * x2r + x2i * x2i;
    }
    float d = p22 * p11 - (p12r * p12r + p12i * p12i) * (1.0f / 1.000001f);
    float a1r = 0, a1i = 0, a0r = 0, a0i = 0;
    if (d != 0.0f) {
        /* alpha1 = (phi01 phi12 - phi02 phi11) / d */
        float inv = 1.0f / d;
        a1r = (p01r * p12r - p01i * p12i - p02r * p11) * inv;
        a1i = (p01r * p12i + p01i * p12r - p02i * p11) * inv;
    }
    if (p11 != 0.0f) {
        /* alpha0 = -(phi01 + alpha1 conj(phi12)) / phi11 */
        float inv = 1.0f / p11;
        a0r = -(p01r + a1r * p12r + a1i * p12i) * inv;
        a0i = -(p01i + a1i * p12r - a1r * p12i) * inv;
    }
    if (a0r * a0r + a0i * a0i >= 16.0f || a1r * a1r + a1i * a1i >= 16.0f) {
        a0r = a0i = a1r = a1i = 0.0f;
    }
    a0[0] = a0r; a0[1] = a0i;
    a1[0] = a1r; a1[1] = a1i;
}

static void sbr_hf_generate(const SBRElement *el, SBRChannel *ch, SBRScratch *sc)
{
    /* predictor per source band, computed the first time a patch uses it */
    float a0[32][2], a1[32][2];
    bool have_lpc[32] = { false };

    int start = 2 * ch->t_E[0] + SBR_T_HFADJ;
    int end = 2 * ch->t_E[ch->L_E] + SBR_T_HFADJ;
    int k = el->kx, g = 0;
    for (int i = 0; i < el->num_patches; i++) {
        for (int x = 0; x < el->patch_num[i]; x++, k++) {
            int p = el->patch_start[i] + x;
            if (!have_lpc[p]) {
                sbr_lpc(sc->x_low[p], a0[p], a1[p]);
                have_lpc[p] = true;
            }
            while (g < el->n_q && k >= el->f_noise[g + 1]) g++;
            float bw = ch->bw_array[g], bw2 = bw * bw;
            float c0r = a0[p][0] * bw, c0i = a0[p][1] * bw;
            float c1r = a1[p][0] * bw2, c1i = a1[p][1] * bw2;
            float (*xl)[2] = sc->x_low[p];
            float (*xh)[2] = sc->y[k];
            for (int n = start; n < end; n++) {
                xh[n][0] = xl[n][0] + c0r * xl[n - 1][0] - c0i * xl[n - 1][1] + c1r * xl[n - 2][0] - c1i * xl[n - 2][1];
                xh[n][1] = xl[n][1] + c0r * xl[n - 1][1] + c0i * xl[n - 1][0] + c1r * xl[n - 2][1] + c1i * xl[n - 2][0];
            }
        }
    }
}

/* Envelope adjustment (§4.6.18.7) */

/* 2^(n/2) built from the float exponent field; the envelope and noise
 * scalefactors are integers, so no pow() is needed. */
static inline float sbr_pow2_half(int n)
{
    union { uint32_t u; float f; } v;
    int e = n >> 1; /* floor(n/2), also for negative n */
    if (e < -126) return 0.0f;
    if (e > 127) e = 127;
    v.u = (uint32_t)(e + 127) << 23;
    return (n & 1) ? v.f * 1.41421356f : v.f;
}

/* Envelope and noise-floor scalefactors to linear values (§4.6.18.7.1):
 * E = 64 * 2^(E_q / 2) at 1.5 dB steps or 64 * 2^E_q at 3 dB, and
 * Q = 2^(6 - Q_q). A coupled pair sends a level and a balance; the level
 * splits as 2E / (1 + 2^(pan - balance)) and its complement. */
static void sbr_dequant(const SBRElement *el, SBRChannel *c0, SBRChannel *c1,
                        float E0[SBR_MAX_ENV][SBR_MAX_BANDS], float Q0[2][SBR_MAX_NQ],
                        float E1[SBR_MAX_ENV][SBR_MAX_BANDS], float Q1[2][SBR_MAX_NQ])
{
    if (!el->coupling || !c1) {
        SBRChannel *chs[2] = { c0, c1 };
        float (*Es[2])[SBR_MAX_BANDS] = { E0, E1 };
        float (*Qs[2])[SBR_MAX_NQ] = { Q0, Q1 };
        for (int c = 0; c < 2 && chs[c]; c++) {
            SBRChannel *ch = chs[c];
            int step = ch->amp_res ? 2 : 1; /* half-steps per unit */
            for (int l = 0; l < ch->L_E; l++) {
                int nb = ch->freq_res[l] ? el->n_high : el->n_low;
                for (int k = 0; k < nb; k++) Es[c][l][k] = 64.0f * sbr_pow2_half(step * ch->E[l][k]);
            }
            for (int l = 0; l < ch->L_Q; l++)
                for (int k = 0; k < el->n_q; k++) Qs[c][l][k] = sbr_pow2_half(2 * (SBR_NOISE_FLOOR_OFFSET - ch->Q[l][k]));
        }
        return;
    }
    int step = c0->amp_res ? 2 : 1;
    int pan = c0->amp_res ? 12 : 24;
    for (int l = 0; l < c0->L_E; l++) {
        int nb = c0->freq_res[l] ? el->n_high : el->n_low;
        for (int k = 0; k < nb; k++) {
            float e = 64.0f * sbr_pow2_half(step * c0->E[l][k]);
            float b = sbr_pow2_half(step * (pan - c1->E[l][k]));
            E0[l][k] = 2.0f * e / (1.0f + b);
            E1[l][k] = 2.0f * e / (1.0f + 1.0f / b);
        }
    }
    for (int l = 0; l < c0->L_Q; l++) {
        for (int k = 0; k < el->n_q; k++) {
            float q = sbr_pow2_half(2 * (SBR_NOISE_FLOOR_OFFSET - c0->Q[l][k]));
            float b = sbr_pow2_half(2 * (12 - c1->Q[l][k]));
            Q0[l][k] = 2.0f * q / (1.0f + b);
            Q1[l][k] = 2.0f * q / (1.0f + 1.0f / b);
        }
    }
}

static void sbr_hf_adjust(const SBRElement *el, SBRChannel *ch, SBRScratch *sc,
                          float E[SBR_MAX_ENV][SBR_MAX_BANDS], float Q[2][SBR_MAX_NQ])
{
    int kx = el->kx, M = el->M;
    float e_orig[SBR_MAX_BANDS], q_map[SBR_MAX_BANDS], e_curr[SBR_MAX_BANDS];
    float gain[SBR_MAX_BANDS], q_m[SBR_MAX_BANDS], s_m[SBR_MAX_BANDS];
    uint8_t s_index[SBR_MAX_BANDS], s_mapped[SBR_MAX_BANDS];
    /* Smoothing history is primed with the first envelope after a reset. */
    bool prime_hist = !ch->primed;
    ch->primed = true;

    for (int l = 0; l < ch->L_E; l++) {
        bool hi = ch->freq_res[l];
        const uint8_t *tab = hi ? el->f_high : el->f_low;
        int nb = hi ? el->n_high : el->n_low;
        int q_env = (ch->L_Q > 1 && ch->t_E[l] >= ch->t_Q[1]) ? 1 : 0;
        int slot0 = 2 * ch->t_E[l] + SBR_T_HFADJ, slot1 = 2 * ch->t_E[l + 1] + SBR_T_HFADJ;
        bool transient = (l == ch->l_A) || (l == 0 && ch->l_A_prev == (int)ch->L_E_prev && ch->L_E_prev > 0);

        /* mapping (§4.6.18.7.2) */
        for (int i = 0; i < nb; i++)
            for (int k = tab[i]; k < tab[i + 1]; k++) e_orig[k - kx] = E[l][i];
        for (int i = 0; i < el->n_q; i++)
            for (int k = el->f_noise[i]; k < el->f_noise[i + 1]; k++) q_map[k - kx] = Q[q_env][i];
        memset(s_index, 0, (size_t)M);
        memset(s_mapped, 0, (size_t)M);
        if (ch->add_harmonic_flag) {
            for (int i = 0; i < el->n_high; i++) {
                if (!ch->add_harmonic[i]) continue;
                int mid = (el->f_high[i] + el->f_high[i + 1]) >> 1;
                if (l >= ch->l_A || ch->s_index_prev[mid - kx]) s_index[mid - kx] = 1;
            }
        }
        for (int i = 0; i < nb; i++) {
            int any = 0;
            for (int k = tab[i]; k < tab[i + 1]; k++) any |= s_index[k - kx];
            for (int k = tab[i]; k < tab[i + 1]; k++) s_mapped[k - kx] = (uint8_t)any;
        }

        /* E_curr estimation (§4.6.18.7.3) */
        float inv_slots = 1.0f / (float)(slot1 - slot0);
        if (el->interpol_freq) {
            for (int m = 0; m < M; m++) {
                float (*xh)[2] = sc->y[kx + m];
                float acc = 0.0f;
                for (int n = slot0; n < slot1; n++) acc += xh[n][0] * xh[n][0] + xh[n][1] * xh[n][1];
                e_curr[m] = acc * inv_slots;
            }
        } else {
            for (int i = 0; i < nb; i++) {
                float acc = 0.0f;
                for (int k = tab[i]; k < tab[i + 1]; k++) {
                    float (*xh)[2] = sc->y[k];
                    for (int n = slot0; n < slot1; n++) acc += xh[n][0] * xh[n][0] + xh[n][1] * xh[n][1];
                }
                acc *= inv_slots / (float)(tab[i + 1] - tab[i]);
                for (int k = tab[i]; k < tab[i + 1]; k++) e_curr[k - kx] = acc;
            }
        }

        /* gains (§4.6.18.7.5) */
        for (int m = 0; m < M; m++) {
            float eo = e_orig[m], q = q_map[m], ec = e_curr[m];
            q_m[m] = sqrtf(eo * q / (1.0f + q));
            s_m[m] = s_index[m] ? sqrtf(eo / (1.0f + q)) : 0.0f;
            if (!s_mapped[m])
                gain[m] = sqrtf(eo / ((1.0f + ec) * (1.0f + (transient ? 0.0f : q))));
            else
                gain[m] = sqrtf(eo * q / ((1.0f + ec) * (1.0f + q)));
        }

        /* limiter and boost per limiter band */
        for (int i = 0; i < el->n_lim; i++) {
            int m0 = el->f_lim[i] - kx, m1 = el->f_lim[i + 1] - kx;
            if (m0 < 0) m0 = 0;
            if (m1 > M) m1 = M;
            if (m1 <= m0) continue;
            float so = 1e-12f, sc_ = 1e-12f;
            for (int m = m0; m < m1; m++) { so += e_orig[m]; sc_ += e_curr[m]; }
            float g_max = sqrtf(so / sc_) * sbr_lim_gain[el->limiter_gains];
            if (g_max > 1e5f) g_max = 1e5f;
            for (int m = m0; m < m1; m++) {
                if (gain[m] > g_max) {
                    q_m[m] *= g_max / gain[m];
                    gain[m] = g_max;
                }
            }
            float den = 1e-12f;
            for (int m = m0; m < m1; m++) {
                den += e_curr[m] * gain[m] * gain[m] + s_m[m] * s_m[m];
                if (!transient && s_m[m] == 0.0f) den += q_m[m] * q_m[m];
            }
            float boost = sqrtf(so / den);
            if (boost > 1.584893192f) boost = 1.584893192f;
            for (int m = m0; m < m1; m++) {
                gain[m] *= boost;
                q_m[m] *= boost;
                s_m[m] *= boost;
            }
        }

        /* smoothing over the four previous envelopes (§4.6.18.7.6) */
        /* the history is a ring of four; hist_pos is the oldest entry */
        if (prime_hist) {
            for (int h = 0; h < 4; h++) {
                memcpy(ch->g_hist[h], gain, sizeof(float) * (size_t)M);
                memcpy(ch->q_hist[h], q_m, sizeof(float) * (size_t)M);
            }
            prime_hist = false;
        }
        float g_filt[SBR_MAX_BANDS], q_filt[SBR_MAX_BANDS];
        bool smooth = (el->smoothing_mode == 0) && !transient;
        if (smooth) {
            for (int m = 0; m < M; m++) {
                g_filt[m] = gain[m] * sbr_h_smooth[0];
                q_filt[m] = q_m[m] * sbr_h_smooth[0];
            }
            for (int h = 0; h < 4; h++) {
                /* h + 1 envelopes back */
                const float *gh = ch->g_hist[(ch->hist_pos + 3 - h) & 3];
                const float *qh = ch->q_hist[(ch->hist_pos + 3 - h) & 3];
                for (int m = 0; m < M; m++) {
                    g_filt[m] += gh[m] * sbr_h_smooth[h + 1];
                    q_filt[m] += qh[m] * sbr_h_smooth[h + 1];
                }
            }
        } else {
            memcpy(g_filt, gain, sizeof(float) * (size_t)M);
            if (transient) memset(q_filt, 0, sizeof(float) * (size_t)M);
            else memcpy(q_filt, q_m, sizeof(float) * (size_t)M);
        }
        memcpy(ch->g_hist[ch->hist_pos], gain, sizeof(float) * (size_t)M);
        memcpy(ch->q_hist[ch->hist_pos], q_m, sizeof(float) * (size_t)M);
        ch->hist_pos = (uint8_t)((ch->hist_pos + 1) & 3);

        /* Y = G X + Q noise, plus the sinusoids. The noise index runs on by
         * one per slot and by M across the bands of a slot, and a band with
         * a sinusoid takes no noise: its Q is zeroed so one uniform pass
         * covers every band, then the few sinusoid bands get their tone. */
        for (int m = 0; m < M; m++) if (s_m[m] != 0.0f) q_filt[m] = 0.0f;
        int noise0 = (ch->index_noise + 1) & 511;
        int phase0 = ch->index_sine;
        int nslots = slot1 - slot0;
        for (int m = 0; m < M; m++) {
            float (*yb)[2] = sc->y[kx + m] + slot0;
            float g = g_filt[m], q = q_filt[m];
            int idx = (noise0 + m) & 511;
            for (int n = 0; n < nslots; n++) {
                yb[n][0] = yb[n][0] * g + q * sbr_noise_table[idx][0];
                yb[n][1] = yb[n][1] * g + q * sbr_noise_table[idx][1];
                idx = (idx + M) & 511;
            }
            if (s_m[m] != 0.0f) {
                /* exp(j pi/2 idx), the imaginary part alternating in sign
                 * from band to band */
                static const float ph_c[4] = { 1.0f, 0.0f, -1.0f, 0.0f };
                static const float ph_s[4] = { 0.0f, 1.0f, 0.0f, -1.0f };
                float sr = s_m[m], si = ((kx + m) & 1) ? -s_m[m] : s_m[m];
                for (int n = 0; n < nslots; n++) {
                    int ph = (phase0 + n) & 3;
                    yb[n][0] += sr * ph_c[ph];
                    yb[n][1] += si * ph_s[ph];
                }
            }
        }
        ch->index_noise = (uint16_t)((ch->index_noise + nslots * M) & 511);
        ch->index_sine = (uint8_t)((ch->index_sine + nslots) & 3);
        if (l == ch->L_E - 1) memcpy(ch->s_index_prev, s_index, (size_t)M);
    }
}

/* Frame processing */

/* Run the analysis bank on one channel's core PCM, filling the scratch
 * X_low buffer (previous tail + 32 new slots). */
static void sbr_analyse(SBRChannel *ch, SBRScratch *sc, const float *pcm)
{
    for (int k = 0; k < 32; k++)
        memcpy(sc->x_low[k], ch->x_low_tail[k], sizeof(ch->x_low_tail[k]));
    float slot[2][32][2];
    for (int t = 0; t < SBR_SLOTS; t += 2) {
        qmf_analysis_pair(ch, pcm + t * 32, slot[0], slot[1]);
        for (int k = 0; k < 32; k++) {
            sc->x_low[k][SBR_T_HFGEN + t][0] = slot[0][k][0];
            sc->x_low[k][SBR_T_HFGEN + t][1] = slot[0][k][1];
            sc->x_low[k][SBR_T_HFGEN + t + 1][0] = slot[1][k][0];
            sc->x_low[k][SBR_T_HFGEN + t + 1][1] = slot[1][k][1];
        }
    }
    for (int k = 0; k < 32; k++)
        memcpy(ch->x_low_tail[k], &sc->x_low[k][SBR_SLOTS], sizeof(ch->x_low_tail[k]));
}

/* Assemble X for the 32 output slots from the low band and the adjusted HF,
 * honouring the previous frame's band split where its last envelope reaches
 * into this frame (§4.6.18.7.6 / 4.6.18.8.1). */
static void sbr_assemble(const SBRElement *el, SBRChannel *ch, SBRScratch *sc, bool have_hf, int nslots)
{
    int i_temp = have_hf ? (int)ch->t_E_end_prev - SBR_SLOTS : 0;
    if (i_temp < 0) i_temp = 0;
    for (int i = 0; i < nslots; i++) {
        int n = i + SBR_T_HFADJ;
        int kx = have_hf ? ((i < i_temp) ? ch->kx_prev : el->kx) : 32;
        int kend = have_hf ? ((i < i_temp) ? ch->kx_prev + ch->M_prev : el->kx + el->M) : 32;
        if (i >= SBR_SLOTS) kend = kx; /* look-ahead slots: low band only */
        for (int k = 0; k < kx && k < 32; k++) {
            sc->x[i][k][0] = sc->x_low[k][n][0];
            sc->x[i][k][1] = sc->x_low[k][n][1];
        }
        for (int k = kx; k < kend; k++) {
            sc->x[i][k][0] = sc->y[k][n][0];
            sc->x[i][k][1] = sc->y[k][n][1];
        }
        for (int k = kend; k < 64; k++) sc->x[i][k][0] = sc->x[i][k][1] = 0.0f;
    }
}

static void sbr_process_channel(const SBRElement *el, SBRChannel *ch, SBRScratch *sc, const float *pcm,
                                float E[SBR_MAX_ENV][SBR_MAX_BANDS], float Q[2][SBR_MAX_NQ], bool have_hf, int nslots)
{
    sbr_analyse(ch, sc, pcm);

    /* Y carries the previous frame's tail; the HF generator then writes
     * the region this frame adjusts, the adjuster rewrites it in place
     * (each envelope's energy is read before its slots are scaled), and
     * the rest stays zero. */
    memset(sc->y, 0, sizeof(sc->y));
    for (int k = 0; k < SBR_MAX_BANDS; k++)
        memcpy(sc->y[k], ch->y_tail[k], sizeof(ch->y_tail[k]));

    if (have_hf) {
        sbr_chirp(el, ch);
        sbr_hf_generate(el, ch, sc);
        sbr_hf_adjust(el, ch, sc, E, Q);
    }

    sbr_assemble(el, ch, sc, have_hf, nslots);

    for (int k = 0; k < SBR_MAX_BANDS; k++)
        memcpy(ch->y_tail[k], &sc->y[k][SBR_SLOTS], sizeof(ch->y_tail[k]));

    if (have_hf) {
        /* remember what the next frame's leading slots and deltas refer to */
        int last = ch->L_E - 1;
        int nb = ch->freq_res[last] ? el->n_high : el->n_low;
        memcpy(ch->E_prev, ch->E[last], sizeof(int16_t) * (size_t)nb);
        ch->freq_res_prev = ch->freq_res[last];
        memcpy(ch->Q_prev, ch->Q[ch->L_Q - 1], sizeof(int16_t) * el->n_q);
        ch->l_A_prev = ch->l_A;
        ch->L_E_prev = ch->L_E;
        ch->t_E_end_prev = (uint8_t)(2 * ch->t_E[ch->L_E]);
        ch->kx_prev = el->kx;
        ch->M_prev = el->M;
    }
}

#else /* FAAD_DISABLE_SBR */

faad_status sbr_decode_extension(struct faad_decoder *dec, BitReader *bs, uint32_t ch0, uint32_t syntax_id, bool crc)
{
    (void)dec; (void)bs; (void)ch0; (void)syntax_id; (void)crc;
    return FAAD_OK;
}
#endif /* FAAD_DISABLE_SBR */

/* Entry point */

#if defined(FAAD_STATS) && !defined(FAAD_DISABLE_SBR)
/* One 'F' record for one SBR channel of one frame: dequantized envelope/
 * noise levels are captured here (post sbr_dequant, pre sbr_hf_adjust), so
 * they are E_orig/Q before any decoder-side limiter or smoothing gain. */
static void sbr_dump_frame(FILE *df, unsigned int frame_idx, uint32_t ch, const SBRElement *el,
                           const SBRChannel *sch, float E[SBR_MAX_ENV][SBR_MAX_BANDS],
                           float Q[2][SBR_MAX_NQ])
{
    char freq_res_s[2 * SBR_MAX_ENV + 1] = {0};
    for (int l = 0; l < sch->L_E; l++) {
        char b[3];
        snprintf(b, sizeof(b), "%s%d", l ? "," : "", sch->freq_res[l]);
        strncat(freq_res_s, b, sizeof(freq_res_s) - strlen(freq_res_s) - 1);
    }
    char invf_s[4 * SBR_MAX_NQ + 1] = {0};
    for (int k = 0; k < el->n_q; k++) {
        char b[5];
        snprintf(b, sizeof(b), "%s%d", k ? "," : "", sch->invf_mode[k]);
        strncat(invf_s, b, sizeof(invf_s) - strlen(invf_s) - 1);
    }
    int nharm = 0;
    if (sch->add_harmonic_flag)
        for (int i = 0; i < el->n_high; i++) if (sch->add_harmonic[i]) nharm++;

    double e_sum = 0.0; int e_n = 0;
    for (int l = 0; l < sch->L_E; l++) {
        int nb = sch->freq_res[l] ? el->n_high : el->n_low;
        for (int k = 0; k < nb; k++) {
            float e = E[l][k];
            e_sum += 10.0 * log10((double)(e > 1e-9f ? e : 1e-9f));
            e_n++;
        }
    }
    double q_sum = 0.0; int q_n = 0;
    for (int l = 0; l < sch->L_Q; l++) {
        for (int k = 0; k < el->n_q; k++) {
            float q = Q[l][k];
            q_sum += 10.0 * log10((double)(q > 1e-9f ? q : 1e-9f));
            q_n++;
        }
    }
    fprintf(df, "F %u %u %u %u %u %s %d %s %d %d %d %d %.2f %.2f\n",
            frame_idx, ch, sch->frame_class, sch->L_E, sch->L_Q, freq_res_s, (int)sch->amp_res,
            invf_s, (int)sch->add_harmonic_flag, nharm, (int)el->coupling, -1,
            e_n > 0 ? e_sum / e_n : 0.0, q_n > 0 ? q_sum / q_n : 0.0);
    fprintf(df, "G %u %u %u %u %u", frame_idx, ch, sch->frame_class, sch->L_E, sch->bs_pointer);
    for (int l = 0; l <= sch->L_E; l++) fprintf(df, " %u", sch->t_E[l]);
    fprintf(df, "\n");
}
#endif

/* In place: channel c's core frame is read from, and its SBR output
 * written to, pcm + c * SBR_OUT_LEN (the PS path writes channel 1 there too). */
#define SBR_OUT_LEN 2048
void sbr_apply(struct faad_decoder *dec, uint32_t num_ch, float *pcm)
{
    float *pcm_in = pcm, *pcm_out = pcm;
#ifndef FAAD_DISABLE_SBR
    SBRScratch *sc = &dec->scratch.sbr;
    float E0[SBR_MAX_ENV][SBR_MAX_BANDS], E1[SBR_MAX_ENV][SBR_MAX_BANDS];
    float Q0[2][SBR_MAX_NQ], Q1[2][SBR_MAX_NQ];

    for (uint32_t ch = 0; ch < num_ch; ) {
        SBRElement *el = &dec->sbr_el[ch];
        bool pair = el->header_present && el->nch == 2 && (ch + 1 < num_ch);
        int nch = pair ? 2 : 1;
        bool have_hf = el->header_present && dec->sbr[ch].have_frame && (!pair || dec->sbr[ch + 1].have_frame);

        if (have_hf) sbr_dequant(el, &dec->sbr[ch], pair ? &dec->sbr[ch + 1] : NULL, E0, Q0, E1, Q1);

#ifdef FAAD_STATS
        if (have_hf) {
            FILE *df = faad_dump_file();
            if (df) {
                sbr_dump_frame(df, g_faadStats.totalFrames, ch, el, &dec->sbr[ch], E0, Q0);
                if (pair) sbr_dump_frame(df, g_faadStats.totalFrames, ch + 1, el, &dec->sbr[ch + 1], E1, Q1);
            }
        }
#endif

#ifndef FAAD_DISABLE_PS
        if (dec->ps_seen && num_ch == 1) {
            sbr_process_channel(el, &dec->sbr[0], sc, pcm_in, E0, Q0, have_hf, PS_IN_SLOTS);
            dec->num_channels = 2;
            if (dec->ps.start) ps_frame_begin(dec, sc->x, have_hf ? el->kx + el->M : 32);
            for (int t = 0; t < SBR_SLOTS; t++) {
                float L[64][2], R[64][2];
                float (*l)[2] = sc->x[t], (*r)[2] = sc->x[t]; /* no PS data yet: dual mono */
                if (dec->ps.start) {
                    ps_slot(dec, t, sc->x, L, R);
                    l = L; r = R;
                }
                qmf_synthesis_slot(&dec->sbr[0], l, pcm_out + t * 64);
                qmf_synthesis_slot(&dec->sbr[1], r, pcm_out + 2048 + t * 64);
            }
            return;
        }
#endif
        for (int c = 0; c < nch; c++) {
            SBRChannel *sch = &dec->sbr[ch + c];
            sbr_process_channel(el, sch, sc, pcm_in + (ch + c) * SBR_OUT_LEN, c ? E1 : E0, c ? Q1 : Q0, have_hf, SBR_SLOTS);
            for (int t = 0; t < SBR_SLOTS; t++)
                qmf_synthesis_slot(sch, sc->x[t], pcm_out + (ch + c) * 2048 + t * 64);
        }
        ch += (uint32_t)nch;
    }
#else
    (void)dec;
    (void)pcm_in; (void)pcm_out;
    /* no SBR decoder: linear 2x upsampling, backwards so it can run in place */
    for (uint32_t ch = 0; ch < num_ch; ch++) {
        float *p = pcm + ch * SBR_OUT_LEN;
        for (int i = FRAME_LEN_LONG - 1; i >= 0; i--) {
            float sample = p[i], prev = i ? p[i - 1] : sample;
            p[i * 2 + 1] = sample;
            p[i * 2]     = 0.5f * (prev + sample);
        }
    }
#endif
}
