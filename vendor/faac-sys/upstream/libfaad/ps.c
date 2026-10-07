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

#ifndef FAAD_DISABLE_PS

/* Band layout for the 20- and 34-parameter configurations. */
static const int ps_nr_par_bands[2]    = { 20, 34 };
static const int ps_nr_ipdopd_bands[2] = { 11, 17 };
static const int ps_nr_bands[2]        = { 71, 91 };
static const int ps_decay_cutoff[2]    = { 10, 32 };
static const int ps_nr_allpass[2]      = { 30, 50 };
static const int ps_short_delay_band[2] = { 42, 62 };
#define PS_DECAY_SLOPE 0.05f

static const uint8_t ps_nr_iidicc_par[6] = { 10, 20, 34, 10, 20, 34 };
static const uint8_t ps_nr_ipdopd_par[6] = { 5, 11, 17, 5, 11, 17 };
static const uint8_t ps_num_env_tab[2][4] = { { 0, 1, 2, 4 }, { 1, 2, 3, 4 } };

/* Table 8.48 / 8.49: parameter band b(k) of each hybrid sub-band k. */
static const int8_t ps_k_to_b_20[71] = {
     1,  0,  0,  1,  2,  3,  4,  5,  6,  7,  8,  9, 10, 11, 12, 13, 14, 14, 15,
    15, 15, 16, 16, 16, 16, 17, 17, 17, 17, 17, 18, 18, 18, 18, 18, 18, 18, 18,
    18, 18, 18, 18, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19,
    19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19, 19
};
static const int8_t ps_k_to_b_34[91] = {
     0,  1,  2,  3,  4,  5,  6,  6,  7,  2,  1,  0, 10, 10,  4,  5,  6,  7,  8,
     9, 10, 11, 12,  9, 14, 11, 12, 13, 14, 15, 16, 13, 16, 17, 18, 19, 20, 21,
    22, 22, 23, 23, 24, 24, 25, 25, 26, 26, 27, 27, 27, 28, 28, 28, 29, 29, 29,
    30, 30, 30, 31, 31, 31, 31, 32, 32, 32, 32, 33, 33, 33, 33, 33, 33, 33, 33,
    33, 33, 33, 33, 33, 33, 33, 33, 33, 33, 33, 33, 33, 33, 33
};

/* Hybrid filter prototypes (§8.6.4.3), first 7 of 13 symmetric taps. */
static const float ps_g0_q8[7]  = { 0.00746082949812f, 0.02270420949825f, 0.04546865930473f, 0.07266113929591f, 0.09885108575264f, 0.11793710567217f, 0.125f };
static const float ps_g0_q12[7] = { 0.04081179924692f, 0.03812810994926f, 0.05144908135699f, 0.06399831151592f, 0.07428313801106f, 0.08100347892914f, 0.08333333333333f };
static const float ps_g1_q8[7]  = { 0.01565675600122f, 0.03752716391991f, 0.05417891378782f, 0.08417044116767f, 0.10307344158036f, 0.12222452249753f, 0.125f };
static const float ps_g2_q4[7]  = { -0.05908211155639f, -0.04871498374946f, 0.0f, 0.07778723915851f, 0.16486303567403f, 0.23279856662996f, 0.25f };
static const float ps_g1_q2[7]  = { 0.0f, 0.01899487526049f, 0.0f, -0.07293139167538f, 0.0f, 0.30596630545168f, 0.5f };

/* IID quantisation levels in dB (Tables 8.24 / 8.25) and ICC values (Table 8.26). */
static const int8_t ps_iid_db_default[15] = { -25, -18, -14, -10, -7, -4, -2, 0, 2, 4, 7, 10, 14, 18, 25 };
static const int8_t ps_iid_db_fine[31] = { -50, -45, -40, -35, -30, -25, -22, -19, -16, -13, -10, -8, -6, -4, -2, 0,
                                           2, 4, 6, 8, 10, 13, 16, 19, 22, 25, 30, 35, 40, 45, 50 };
static const float ps_icc_invq[8] = { 1.0f, 0.937f, 0.84118f, 0.60092f, 0.36764f, 0.0f, -0.589f, -1.0f };

/* Sub-band centre frequencies in QMF-band units, hybrid bands only. */
static const int8_t ps_f_center_20[10] = { -3, -1, 1, 3, 5, 7, 10, 14, 18, 22 };            /* /8  */
static const int8_t ps_f_center_34[32] = { 2, 6, 10, 14, 18, 22, 26, 30, 34, -10, -6, -2, 51, 57, 15, 21,
                                           27, 33, 39, 45, 54, 66, 78, 42, 102, 66, 78, 90, 102, 114, 126, 90 }; /* /24 */

/* The complex hybrid filters G_q(n) = g(n) exp(-j 2pi/Q (q + 1/2)(n - 6))
 * for each prototype and split factor Q, taps 0..6 (the rest by symmetry).
 * The 20-band layout splits QMF band 0 eight ways (prototype g0); the
 * 34-band layout splits band 0 twelve ways (g0), band 1 eight ways (g1) and
 * bands 2..4 four ways (g2). */
static float ps_split8_g0[8][7][2], ps_split12_g0[12][7][2], ps_split8_g1[8][7][2], ps_split4_g2[4][7][2];
static float ps_phi_fract[2][50][2];
static float ps_q_fract[2][50][3][2];
static float ps_HA[46][8][4], ps_HB[46][8][4];
static void ps_init_band_maps(void);

static void ps_init_split_filters(void)
{
    static const struct { float (*coef)[7][2]; const float *proto; uint8_t Q; } split[4] = {
        { ps_split8_g0, ps_g0_q8, 8 }, { ps_split12_g0, ps_g0_q12, 12 },
        { ps_split8_g1, ps_g1_q8, 8 }, { ps_split4_g2, ps_g2_q4, 4 },
    };
    for (int i = 0; i < 4; i++) {
        for (int q = 0; q < split[i].Q; q++) {
            for (int n = 0; n < 7; n++) {
                double phase = -2.0 * M_PI * (q + 0.5) * (n - 6) / split[i].Q;
                split[i].coef[q][n][0] = (float)(split[i].proto[n] * cos(phase));
                split[i].coef[q][n][1] = (float)(split[i].proto[n] * sin(phase));
            }
        }
    }
}

void init_ps_tables(void)
{
    static const double links[3] = { 0.43, 0.75, 0.347 };
    const double gain = 0.39;

    ps_init_split_filters();

    for (int is34 = 0; is34 < 2; is34++) {
        for (int k = 0; k < ps_nr_allpass[is34]; k++) {
            double fc;
            if (!is34) fc = (k < 10) ? ps_f_center_20[k] / 8.0 : k - 6.5;
            else       fc = (k < 32) ? ps_f_center_34[k] / 24.0 : k - 26.5;
            for (int m = 0; m < 3; m++) {
                double th = -M_PI * links[m] * fc;
                ps_q_fract[is34][k][m][0] = (float)cos(th);
                ps_q_fract[is34][k][m][1] = (float)sin(th);
            }
            double th = -M_PI * gain * fc;
            ps_phi_fract[is34][k][0] = (float)cos(th);
            ps_phi_fract[is34][k][1] = (float)sin(th);
        }
    }

    /* Mixing matrices (§8.6.4.6.2 type A, §8.6.4.6.3 type B) for every IID/ICC index. */
    for (int i = 0; i < 46; i++) {
        double db = (i < 15) ? ps_iid_db_default[i] : ps_iid_db_fine[i - 15];
        double c = pow(10.0, db / 20.0);
        double c1 = sqrt(2.0) / sqrt(1.0 + c * c);
        double c2 = c * c1;
        for (int icc = 0; icc < 8; icc++) {
            double alpha = 0.5 * acos(ps_icc_invq[icc]);
            double beta = alpha * (c1 - c2) / sqrt(2.0);
            ps_HA[i][icc][0] = (float)(c2 * cos(beta + alpha));
            ps_HA[i][icc][1] = (float)(c1 * cos(beta - alpha));
            ps_HA[i][icc][2] = (float)(c2 * sin(beta + alpha));
            ps_HA[i][icc][3] = (float)(c1 * sin(beta - alpha));

            double rho = ps_icc_invq[icc] > 0.05 ? ps_icc_invq[icc] : 0.05;
            double a = 0.5 * atan2(2.0 * c * rho, c * c - 1.0);
            double mu = c + 1.0 / c;
            mu = sqrt(1.0 + (4.0 * rho * rho - 4.0) / (mu * mu));
            double gamma = atan(sqrt((1.0 - mu) / (1.0 + mu)));
            if (a < 0) a += M_PI / 2;
            ps_HB[i][icc][0] = (float)( sqrt(2.0) * cos(a) * cos(gamma));
            ps_HB[i][icc][1] = (float)( sqrt(2.0) * sin(a) * cos(gamma));
            ps_HB[i][icc][2] = (float)(-sqrt(2.0) * sin(a) * sin(gamma));
            ps_HB[i][icc][3] = (float)( sqrt(2.0) * cos(a) * sin(gamma));
        }
    }
    ps_init_band_maps();
}

/* Bitstream (§8.6.2) */

/* Deltas along frequency (df) or against the previous envelope (dt). */
static bool ps_read_par(BitReader *bs, PSState *ps, int8_t par[PS_MAX_ENV][PS_NR_PAR], int num, int e, bool dt,
                        const SBRHuffBook *book, int mask, int limit)
{
    if (dt) {
        int e_prev = e ? e - 1 : (int)ps->num_env_old - 1;
        if (e_prev < 0) e_prev = 0;
        for (int b = 0; b < num; b++) {
            int v = par[e_prev][b] + sbr_huff_decode(bs, book);
            if (mask) v &= mask;
            par[e][b] = (int8_t)v;
            if (limit && (v > limit || v < -limit)) return false;
        }
    } else {
        int v = 0;
        for (int b = 0; b < num; b++) {
            v += sbr_huff_decode(bs, book);
            if (mask) v &= mask;
            par[e][b] = (int8_t)v;
            if (limit && (v > limit || v < -limit)) return false;
        }
    }
    return true;
}

static void ps_copy_envelope(PSState *ps, int to, int from)
{
    memcpy(ps->iid_par[to], ps->iid_par[from], sizeof(ps->iid_par[0]));
    memcpy(ps->icc_par[to], ps->icc_par[from], sizeof(ps->icc_par[0]));
    memcpy(ps->ipd_par[to], ps->ipd_par[from], sizeof(ps->ipd_par[0]));
    memcpy(ps->opd_par[to], ps->opd_par[from], sizeof(ps->opd_par[0]));
}

static void ps_clear_params(PSState *ps)
{
    memset(ps->iid_par, 0, sizeof(ps->iid_par));
    memset(ps->icc_par, 0, sizeof(ps->icc_par));
    memset(ps->ipd_par, 0, sizeof(ps->ipd_par));
    memset(ps->opd_par, 0, sizeof(ps->opd_par));
}

/* ps_data(): bits_left is the extension payload still available. */
void ps_read_data(struct faad_decoder *dec, BitReader *bs, uint32_t bits_left)
{
    PSState *ps = &dec->ps;
    uint32_t start_pos = bits_get_consumed(bs);
    bool ok = true;

    if (bits_get(bs, 1)) { /* enable_ps_header */
        ps->enable_iid = bits_get(bs, 1);
        if (ps->enable_iid) {
            int mode = (int)bits_get(bs, 3);
            if (mode > 5) { ok = false; mode = 0; }
            ps->nr_iid_par = ps_nr_iidicc_par[mode];
            ps->iid_quant = mode > 2;
            ps->nr_ipdopd_par = ps_nr_ipdopd_par[mode];
        }
        ps->enable_icc = bits_get(bs, 1);
        if (ps->enable_icc) {
            int mode = (int)bits_get(bs, 3);
            if (mode > 5) { ok = false; mode = 0; }
            ps->icc_mode = (uint8_t)mode;
            ps->nr_icc_par = ps_nr_iidicc_par[mode];
        }
        ps->enable_ext = bits_get(bs, 1);
        ps->start = true;
    }

    ps->frame_class = (uint8_t)bits_get(bs, 1);
    ps->num_env_old = ps->num_env;
    int num_env = ps_num_env_tab[ps->frame_class & 1][bits_get(bs, 2) & 3];
    ps->num_env = (uint8_t)num_env;
    ps->border[0] = -1;
    if (ps->frame_class) {
        for (int e = 1; e <= num_env && e < PS_MAX_ENV; e++) {
            ps->border[e] = (int8_t)bits_get(bs, 5);
            if (ps->border[e] < ps->border[e - 1]) ok = false;
        }
    } else {
        for (int e = 1; e <= num_env && e < PS_MAX_ENV; e++) {
            int shift = (ps->num_env == 4) ? 2 : (ps->num_env == 2) ? 1 : 0;
            ps->border[e] = (int8_t)(((e * 32) >> shift) - 1);
        }
    }

    if (ps->enable_iid) {
        int limit = ps->iid_quant ? 15 : 7;
        for (int e = 0; e < ps->num_env && ok; e++) {
            bool dt = bits_get(bs, 1);
            int book = ps->iid_quant ? (dt ? HB_PS_IID_DT_FINE : HB_PS_IID_DF_FINE)
                                     : (dt ? HB_PS_IID_DT : HB_PS_IID_DF);
            ok = ps_read_par(bs, ps, ps->iid_par, ps->nr_iid_par, e, dt, &sbr_books[book], 0, limit);
        }
    } else {
        memset(ps->iid_par, 0, sizeof(ps->iid_par));
    }
    if (ps->enable_icc) {
        for (int e = 0; e < ps->num_env && ok; e++) {
            bool dt = bits_get(bs, 1);
            ok = ps_read_par(bs, ps, ps->icc_par, ps->nr_icc_par, e, dt,
                             &sbr_books[dt ? HB_PS_ICC_DT : HB_PS_ICC_DF], 0, 0);
            for (int b = 0; b < ps->nr_icc_par; b++) if ((unsigned)ps->icc_par[e][b] > 7) ok = false;
        }
    } else {
        memset(ps->icc_par, 0, sizeof(ps->icc_par));
    }

    if (ps->enable_ext && ok) {
        int cnt = (int)bits_get(bs, 4);
        if (cnt == 15) cnt += (int)bits_get(bs, 8);
        cnt *= 8;
        while (cnt > 7) {
            uint32_t before = bits_get_consumed(bs);
            int id = (int)bits_get(bs, 2);
            if (id == 0) {
                ps->enable_ipdopd = bits_get(bs, 1);
                if (ps->enable_ipdopd) {
                    for (int e = 0; e < ps->num_env; e++) {
                        bool dt = bits_get(bs, 1);
                        ps_read_par(bs, ps, ps->ipd_par, ps->nr_ipdopd_par, e, dt,
                                    &sbr_books[dt ? HB_PS_IPD_DT : HB_PS_IPD_DF], 7, 0);
                        dt = bits_get(bs, 1);
                        ps_read_par(bs, ps, ps->opd_par, ps->nr_ipdopd_par, e, dt,
                                    &sbr_books[dt ? HB_PS_OPD_DT : HB_PS_OPD_DF], 7, 0);
                    }
                }
                bits_skip(bs, 1);
            }
            uint32_t used = bits_get_consumed(bs) - before;
            if (used == 0) { ok = false; break; }
            cnt -= (int)used;
        }
        if (cnt < 0) ok = false;
        else bits_skip(bs, (uint32_t)cnt);
    }

    if (ok) {
        /* The parameters must reach the end of the frame (§8.6.4.4): a
         * frame without envelopes holds the previous frame's last
         * parameters for its whole length, one whose last border stops
         * short repeats its last envelope up to slot 31. */
        if (ps->num_env == 0) {
            if (ps->num_env_old > 1) ps_copy_envelope(ps, 0, ps->num_env_old - 1);
            ps->num_env = 1;
            ps->border[1] = 31;
        } else if (ps->border[ps->num_env] < 31) {
            ps_copy_envelope(ps, ps->num_env, ps->num_env - 1);
            ps->num_env++;
            ps->border[ps->num_env] = 31;
        }
        ps->is34_old = ps->is34;
        if (ps->enable_iid || ps->enable_icc)
            ps->is34 = (ps->enable_iid && ps->nr_iid_par == 34) || (ps->enable_icc && ps->nr_icc_par == 34);
        if (!ps->enable_ipdopd) {
            memset(ps->ipd_par, 0, sizeof(ps->ipd_par));
            memset(ps->opd_par, 0, sizeof(ps->opd_par));
        }
        dec->ps_present = true;
#ifdef FAAD_STATS
        g_faadStats.psActiveFrames++;
        {
            FILE *df = faad_dump_file();
            if (df) fprintf(df, "P %u %d %d %d\n", g_faadStats.totalFrames,
                            (int)ps->enable_iid, (int)ps->enable_icc, (int)ps->num_env);
        }
#endif
    } else {
        ps->start = false;
        ps_clear_params(ps);
    }

    uint32_t used = bits_get_consumed(bs) - start_pos;
    if (used < bits_left) bits_skip(bs, bits_left - used);
}

/* Hybrid filterbank (§8.6.4.3) */

/* One slot of a Q-way split of one QMF band. The 13-tap filters f_q share
 * the symmetric real prototype, so f_q[12-j] = conj(f_q[j]) and each tap
 * pair costs one complex multiply: f x_j + conj(f) x_{12-j}. */
static void ps_split_slot(float out[][2], float (*in)[2], float (*f)[7][2], int bands)
{
    for (int q = 0; q < bands; q++) {
        float sr = f[q][6][0] * in[6][0], si = f[q][6][0] * in[6][1];
        for (int j = 0; j < 6; j++) {
            float fr = f[q][j][0], fi = f[q][j][1];
            float ar = in[j][0], ai = in[j][1], br = in[12 - j][0], bi = in[12 - j][1];
            sr += fr * (ar + br) - fi * (ai - bi);
            si += fr * (ai + bi) + fi * (ar - br);
        }
        out[q][0] = sr;
        out[q][1] = si;
    }
}

/* Two-way split with the real prototype g1_Q2, whose even taps are zero:
 * the two sub-bands are the centre tap plus and minus the odd-tap sum. */
static void ps_split2_slot(float out[2][2], float (*in)[2], int reverse)
{
    float cr = ps_g1_q2[6] * in[6][0], ci = ps_g1_q2[6] * in[6][1];
    float sr = 0.0f, si = 0.0f;
    for (int j = 1; j < 6; j += 2) {
        sr += ps_g1_q2[j] * (in[j][0] + in[12 - j][0]);
        si += ps_g1_q2[j] * (in[j][1] + in[12 - j][1]);
    }
    out[reverse][0] = cr + sr;   out[reverse][1] = ci + si;
    out[!reverse][0] = cr - sr;  out[!reverse][1] = ci - si;
}

/* Slot n of the hybrid domain: the low QMF bands split, the rest passed
 * through. in_buf holds the QMF slots with six of look-back, so the filter
 * centred on slot n reads in_buf[n .. n+12]. */
static void ps_hybrid_analysis_slot(PSState *ps, int n, float out[PS_NR_BANDS][2], float X[PS_IN_SLOTS][64][2])
{
    if (ps->is34) {
        ps_split_slot(out,      ps->in_buf[0] + n, ps_split12_g0, 12);
        ps_split_slot(out + 12, ps->in_buf[1] + n, ps_split8_g1, 8);
        ps_split_slot(out + 20, ps->in_buf[2] + n, ps_split4_g2, 4);
        ps_split_slot(out + 24, ps->in_buf[3] + n, ps_split4_g2, 4);
        ps_split_slot(out + 28, ps->in_buf[4] + n, ps_split4_g2, 4);
        for (int k = 5; k < 64; k++) { out[k + 27][0] = X[n][k][0]; out[k + 27][1] = X[n][k][1]; }
    } else {
        /* QMF band 0 splits eight ways and the sub-bands merge to six in
         * frequency order; bands 1 and 2 split two ways. */
        float t[8][2];
        ps_split_slot(t, ps->in_buf[0] + n, ps_split8_g0, 8);
        out[0][0] = t[6][0];           out[0][1] = t[6][1];
        out[1][0] = t[7][0];           out[1][1] = t[7][1];
        out[2][0] = t[0][0];           out[2][1] = t[0][1];
        out[3][0] = t[1][0];           out[3][1] = t[1][1];
        out[4][0] = t[2][0] + t[5][0]; out[4][1] = t[2][1] + t[5][1];
        out[5][0] = t[3][0] + t[4][0]; out[5][1] = t[3][1] + t[4][1];
        ps_split2_slot(out + 6, ps->in_buf[1] + n, 1);
        ps_split2_slot(out + 8, ps->in_buf[2] + n, 0);
        for (int k = 3; k < 64; k++) { out[k + 7][0] = X[n][k][0]; out[k + 7][1] = X[n][k][1]; }
    }
}

/* Hybrid synthesis of one slot: the sub-bands of a split QMF band sum
 * back, the others pass through. */
static void ps_hybrid_synthesis_slot(float in[PS_NR_BANDS][2], float out[64][2], bool is34)
{
    if (is34) {
        static const uint8_t counts[5] = { 12, 8, 4, 4, 4 };
        int k = 0;
        for (int b = 0; b < 5; b++) {
            float sr = 0.0f, si = 0.0f;
            for (int q = 0; q < counts[b]; q++, k++) { sr += in[k][0]; si += in[k][1]; }
            out[b][0] = sr; out[b][1] = si;
        }
        for (int b = 5; b < 64; b++) { out[b][0] = in[b + 27][0]; out[b][1] = in[b + 27][1]; }
    } else {
        float sr = 0.0f, si = 0.0f;
        for (int q = 0; q < 6; q++) { sr += in[q][0]; si += in[q][1]; }
        out[0][0] = sr; out[0][1] = si;
        out[1][0] = in[6][0] + in[7][0]; out[1][1] = in[6][1] + in[7][1];
        out[2][0] = in[8][0] + in[9][0]; out[2][1] = in[8][1] + in[9][1];
        for (int b = 3; b < 64; b++) { out[b][0] = in[b + 7][0]; out[b][1] = in[b + 7][1]; }
    }
}

/* Decorrelation (§8.6.4.6) */

/* Transient detection for one slot: per parameter band, a peak-decay
 * envelope against smoothed power gives the gain G_tr(b). */
static void ps_transient_gain_slot(PSState *ps, float s[PS_NR_BANDS][2], float gain[PS_NR_PAR])
{
    const float peak_decay = 0.76592833836465f, impact = 1.5f, alpha = 0.25f;
    bool is34 = ps->is34;
    const int8_t *k_to_b = is34 ? ps_k_to_b_34 : ps_k_to_b_20;

    float power[PS_NR_PAR] = { 0 };
    for (int k = 0; k < ps_nr_bands[is34]; k++) power[k_to_b[k]] += s[k][0] * s[k][0] + s[k][1] * s[k][1];
    for (int b = 0; b < ps_nr_par_bands[is34]; b++) {
        float pw = power[b];
        float peak = ps->peak_decay_nrg[b] * peak_decay;
        if (peak < pw) peak = pw;
        ps->peak_decay_nrg[b] = peak;
        ps->power_smooth[b] += alpha * (pw - ps->power_smooth[b]);
        ps->peak_decay_diff_smooth[b] += alpha * (peak - pw - ps->peak_decay_diff_smooth[b]);
        float thr = impact * ps->peak_decay_diff_smooth[b];
        gain[b] = (thr > ps->power_smooth[b]) ? ps->power_smooth[b] / thr : 1.0f;
    }
}

/* One slot through
 *   H_k(z) = z^-2 phi_k prod_m (Q_m z^-d_m - a_m g_k) / (1 - a_m g_k Q_m z^-d_m)
 * for the all-pass bands (link delays 3, 4, 5, each in direct form II so its
 * state is d_m samples in a ring), and through a 14- or 1-slot delay for the
 * rest; then scaled by the transient gain of the band's parameter band.
 * The rings are indexed by slot counters shared by all bands. */
static void ps_decorrelate_slot(PSState *ps, float s[PS_NR_BANDS][2], float d[PS_NR_BANDS][2])
{
    static const float a[3] = { 0.65143905753106f, 0.56471812200776f, 0.48954165955695f };
    static const int link_delay[3] = { 3, 4, 5 };
    bool is34 = ps->is34;
    const int8_t *k_to_b = is34 ? ps_k_to_b_34 : ps_k_to_b_20;
    const int nr_allpass = ps_nr_allpass[is34];
    float gain[PS_NR_PAR];

    ps_transient_gain_slot(ps, s, gain);

    int k = 0;
    for (; k < nr_allpass; k++) {
        float slope = 1.0f - PS_DECAY_SLOPE * (float)(k - ps_decay_cutoff[is34]);
        if (slope < 0.0f) slope = 0.0f;
        if (slope > 1.0f) slope = 1.0f;

        /* two-slot input delay, then the fractional-delay phase */
        float (*in)[2] = ps->dc_in[k];
        float xr = in[0][0], xi = in[0][1];
        in[0][0] = in[1][0]; in[0][1] = in[1][1];
        in[1][0] = s[k][0];  in[1][1] = s[k][1];
        const float *phi = ps_phi_fract[is34][k];
        float ur = xr * phi[0] - xi * phi[1];
        float ui = xr * phi[1] + xi * phi[0];

        float (*st)[2] = ps->dc_ap[k];
        for (int m = 0; m < 3; m++) {
            const float ag = a[m] * slope;
            const float *q = ps_q_fract[is34][k][m];
            float (*v)[2] = st + ps->ap_pos[m]; /* v(n - d_m) */
            float tr = v[0][0] * q[0] - v[0][1] * q[1]; /* Q v(n-d) */
            float ti = v[0][0] * q[1] + v[0][1] * q[0];
            float vr = ur + ag * tr, vi = ui + ag * ti;   /* v(n) */
            ur = tr - ag * vr;                            /* link output */
            ui = ti - ag * vi;
            v[0][0] = vr; v[0][1] = vi;
            st += link_delay[m];
        }
        float g = gain[k_to_b[k]];
        d[k][0] = g * ur;
        d[k][1] = g * ui;
    }
    for (; k < ps_nr_bands[is34]; k++) {
        int dl = (k < ps_short_delay_band[is34]) ? PS_MAX_DELAY : 1;
        float (*line)[2] = ps->dc_delay[k - nr_allpass];
        int rd = (ps->dl_pos + PS_MAX_DELAY - dl) % PS_MAX_DELAY;
        float g = gain[k_to_b[k]];
        d[k][0] = g * line[rd][0];
        d[k][1] = g * line[rd][1];
        line[ps->dl_pos][0] = s[k][0];
        line[ps->dl_pos][1] = s[k][1];
    }
    for (int m = 0; m < 3; m++) ps->ap_pos[m] = (uint8_t)((ps->ap_pos[m] + 1) % link_delay[m]);
    ps->dl_pos = (uint8_t)((ps->dl_pos + 1) % PS_MAX_DELAY);
}

/* Parameter band layouts */

/* The 10, 20 and 34-band layouts meet when a frame switches layout or
 * codes IPD/OPD on fewer bands: parameters and the previous frame's mixing
 * matrix have to be read on another grid. The 10-band layout pairs the
 * 20-band one. Between 20 and 34 bands, a target band takes the mean of the
 * source bands whose centre frequency falls inside it, or the nearest one;
 * those maps are built from the band centres at start. */
typedef struct { uint8_t first, count; } PSBandMap;
static PSBandMap ps_map_34_20[20], ps_map_20_34[34];

static void ps_layout_centres(bool is34, float *centre)
{
    const int8_t *k_to_b = is34 ? ps_k_to_b_34 : ps_k_to_b_20;
    int count[PS_NR_PAR] = { 0 };
    for (int b = 0; b < ps_nr_par_bands[is34]; b++) centre[b] = 0.0f;
    for (int k = 0; k < ps_nr_bands[is34]; k++) {
        float f = !is34 ? ((k < 10) ? ps_f_center_20[k] / 8.0f : k - 6.5f)
                        : ((k < 32) ? ps_f_center_34[k] / 24.0f : k - 26.5f);
        centre[k_to_b[k]] += fabsf(f);
        count[k_to_b[k]]++;
    }
    for (int b = 0; b < ps_nr_par_bands[is34]; b++) centre[b] /= (float)count[b];
}

static void ps_build_band_map(PSBandMap *map, const float *src, int n_src, const float *dst, int n_dst)
{
    for (int j = 0; j < n_dst; j++) {
        float lo = j ? 0.5f * (dst[j - 1] + dst[j]) : 0.0f;
        float hi = (j + 1 < n_dst) ? 0.5f * (dst[j] + dst[j + 1]) : 1e9f;
        int first = -1, count = 0;
        for (int i = 0; i < n_src; i++) {
            if (src[i] < lo || src[i] >= hi) continue;
            if (first < 0) first = i;
            count++;
        }
        if (count == 0) {
            first = 0;
            for (int i = 1; i < n_src; i++)
                if (fabsf(src[i] - dst[j]) < fabsf(src[first] - dst[j])) first = i;
            count = 1;
        }
        map[j].first = (uint8_t)first;
        map[j].count = (uint8_t)count;
    }
}

static void ps_init_band_maps(void)
{
    float c20[PS_NR_PAR], c34[PS_NR_PAR];
    ps_layout_centres(false, c20);
    ps_layout_centres(true, c34);
    ps_build_band_map(ps_map_34_20, c34, 34, c20, 20);
    ps_build_band_map(ps_map_20_34, c20, 20, c34, 34);
}

/* Reads band j of the target layout from src, which has n_src bands of the
 * source layout (10, 20 or 34 bands, or the low 5, 11 or 17 of them). */
static float ps_band_read(const float *src, int n_src, int src_layout, int dst_layout, int j)
{
    if (src_layout == dst_layout) return j < n_src ? src[j] : 0.0f;
    if (src_layout == 10) {
        if (dst_layout == 20) return (j >> 1) < n_src ? src[j >> 1] : 0.0f;
        /* 10 -> 34 through the 20-band grid */
        const PSBandMap *m = &ps_map_20_34[j];
        float sum = 0.0f; int n = 0;
        for (int i = m->first; i < m->first + m->count; i++)
            if ((i >> 1) < n_src) { sum += src[i >> 1]; n++; }
        return n ? sum / (float)n : 0.0f;
    }
    const PSBandMap *m = (src_layout == 34) ? &ps_map_34_20[j] : &ps_map_20_34[j];
    float sum = 0.0f; int n = 0;
    for (int i = m->first; i < m->first + m->count && i < n_src; i++) { sum += src[i]; n++; }
    return n ? sum / (float)n : 0.0f;
}

static int ps_layout_of(int n_par) { return n_par >= 34 || n_par == 17 ? 34 : n_par >= 20 || n_par == 11 ? 20 : 10; }

/* Parameters of every envelope onto this frame's grid (full: all its bands,
 * else the low IPD/OPD subset). */
static void ps_remap(int8_t dst[PS_MAX_ENV][PS_NR_PAR], int8_t src[PS_MAX_ENV][PS_NR_PAR], int n_src, int num_env, bool full, bool is34)
{
    int dst_layout = is34 ? 34 : 20;
    int n_dst = full ? dst_layout : ps_nr_ipdopd_bands[is34];
    int src_layout = ps_layout_of(n_src);
    for (int e = 0; e < num_env; e++) {
        float v[PS_NR_PAR];
        for (int i = 0; i < n_src; i++) v[i] = (float)src[e][i];
        for (int j = 0; j < n_dst; j++) {
            float x = ps_band_read(v, n_src, src_layout, dst_layout, j);
            dst[e][j] = (int8_t)(x >= 0.0f ? (int)(x + 0.5f) : -(int)(0.5f - x));
        }
    }
}

/* The previous frame's mixing matrix row onto the other grid. */
static void ps_remap_values(float *p, bool to34)
{
    float src[PS_NR_PAR];
    memcpy(src, p, sizeof(src));
    int n = to34 ? 34 : 20;
    for (int j = 0; j < n; j++) p[j] = ps_band_read(src, to34 ? 20 : 34, to34 ? 20 : 34, to34 ? 34 : 20, j);
}

/* Stereo processing (§8.6.4.7) */

static void ps_pd_smooth(int pd0, int pd1, int pd2, float *re, float *im)
{
    static const float c[8] = { 1.0f, 0.70710678f, 0.0f, -0.70710678f, -1.0f, -0.70710678f, 0.0f, 0.70710678f };
    static const float s[8] = { 0.0f, 0.70710678f, 1.0f, 0.70710678f, 0.0f, -0.70710678f, -1.0f, -0.70710678f };
    float r = 0.25f * c[pd0] + 0.5f * c[pd1] + c[pd2];
    float i = 0.25f * s[pd0] + 0.5f * s[pd1] + s[pd2];
    float mag = 1.0f / hypotf(r, i);
    *re = r * mag;
    *im = i * mag;
}

/* The mixing matrix at every envelope border, from the IID/ICC (and IPD/OPD)
 * parameters; slots interpolate between borders. Entry [e][b] is the matrix
 * at the end of envelope e - 1, [0] carrying over from the previous frame. */
static void ps_mixing_matrices(PSState *ps)
{
    bool is34 = ps->is34;
    float (*H_LUT)[8][4] = (ps->icc_mode < 3) ? ps_HA : ps_HB;
    int8_t iid_m[PS_MAX_ENV][PS_NR_PAR], icc_m[PS_MAX_ENV][PS_NR_PAR], ipd_m[PS_MAX_ENV][PS_NR_PAR], opd_m[PS_MAX_ENV][PS_NR_PAR];
    float (*H)[2][PS_MAX_ENV + 1][PS_NR_PAR] = ps->H; /* [matrix entry][re/im][border][band] */

    if (ps->num_env_old) {
        for (int i = 0; i < 4; i++)
            for (int c = 0; c < 2; c++)
                memcpy(H[i][c][0], H[i][c][ps->num_env_old], sizeof(H[i][c][0]));
    }
    ps_remap(iid_m, ps->iid_par, ps->nr_iid_par, ps->num_env, true, is34);
    ps_remap(icc_m, ps->icc_par, ps->nr_icc_par, ps->num_env, true, is34);
    if (ps->enable_ipdopd) {
        ps_remap(ipd_m, ps->ipd_par, ps->nr_ipdopd_par, ps->num_env, false, is34);
        ps_remap(opd_m, ps->opd_par, ps->nr_ipdopd_par, ps->num_env, false, is34);
    }
    if (is34 != ps->is34_old) {
        for (int i = 0; i < 4; i++)
            for (int c = 0; c < 2; c++) {
                ps_remap_values(H[i][c][0], is34);
            }
        memset(ps->ipd_hist, 0, sizeof(ps->ipd_hist));
        memset(ps->opd_hist, 0, sizeof(ps->opd_hist));
    }

    for (int e = 0; e < ps->num_env; e++) {
        for (int b = 0; b < ps_nr_par_bands[is34]; b++) {
            int iid = iid_m[e][b] + 7 + 23 * (ps->iid_quant ? 1 : 0);
            int icc = icc_m[e][b] & 7;
            if (iid < 0) iid = 0;
            if (iid > 45) iid = 45;
            float h[4] = { H_LUT[iid][icc][0], H_LUT[iid][icc][1], H_LUT[iid][icc][2], H_LUT[iid][icc][3] };
            float hi[4] = { 0, 0, 0, 0 };
            if (ps->enable_ipdopd && b < ps_nr_ipdopd_bands[is34]) {
                int opd = opd_m[e][b] & 7, ipd = ipd_m[e][b] & 7;
                float opd_re, opd_im, ipd_re, ipd_im;
                ps_pd_smooth(ps->opd_hist[b] >> 3, ps->opd_hist[b] & 7, opd, &opd_re, &opd_im);
                ps_pd_smooth(ps->ipd_hist[b] >> 3, ps->ipd_hist[b] & 7, ipd, &ipd_re, &ipd_im);
                ps->opd_hist[b] = (int8_t)(((ps->opd_hist[b] & 7) << 3) | opd);
                ps->ipd_hist[b] = (int8_t)(((ps->ipd_hist[b] & 7) << 3) | ipd);
                float adj_re = opd_re * ipd_re + opd_im * ipd_im;
                float adj_im = opd_im * ipd_re - opd_re * ipd_im;
                hi[0] = h[0] * opd_im; h[0] *= opd_re;
                hi[1] = h[1] * adj_im; h[1] *= adj_re;
                hi[2] = h[2] * opd_im; h[2] *= opd_re;
                hi[3] = h[3] * adj_im; h[3] *= adj_re;
            }
            for (int i = 0; i < 4; i++) { H[i][0][e + 1][b] = h[i]; H[i][1][e + 1][b] = hi[i]; }
        }
    }
}

/* Mix one slot: slot n of envelope e (border[e] < n <= border[e+1]) uses
 * the matrix interpolated (n - border[e]) / (border[e+1] - border[e]) of
 * the way from border e to border e + 1. */
static void ps_mix_slot(PSState *ps, int n, float l[PS_NR_BANDS][2], float r[PS_NR_BANDS][2])
{
    bool is34 = ps->is34;
    const int8_t *k_to_b = is34 ? ps_k_to_b_34 : ps_k_to_b_20;
    float (*H)[2][PS_MAX_ENV + 1][PS_NR_PAR] = ps->H;
    int e = 0;
    while (e + 1 < ps->num_env && n > ps->border[e + 1]) e++;
    int start = ps->border[e], stop = ps->border[e + 1];
    if (stop <= start) return;
    float t = (float)(n - start) / (float)(stop - start);

    /* the matrix varies per parameter band; interpolate it once per band */
    float h[PS_NR_PAR][4], hI[PS_NR_PAR][4], hF[PS_NR_PAR][4];
    for (int b = 0; b < ps_nr_par_bands[is34]; b++)
        for (int i = 0; i < 4; i++) h[b][i] = H[i][0][e][b] + (H[i][0][e + 1][b] - H[i][0][e][b]) * t;
    if (ps->enable_ipdopd) {
        for (int b = 0; b < ps_nr_par_bands[is34]; b++)
            for (int i = 0; i < 4; i++) {
                hI[b][i] = H[i][1][e][b] + (H[i][1][e + 1][b] - H[i][1][e][b]) * t;
                hF[b][i] = -H[i][1][e][b] + (H[i][1][e + 1][b] + H[i][1][e][b]) * t;
            }
    }

    for (int k = 0; k < ps_nr_bands[is34]; k++) {
        const float *hb = h[k_to_b[k]];
        float lr = l[k][0], li = l[k][1], rr = r[k][0], ri = r[k][1];
        if (!ps->enable_ipdopd) {
            l[k][0] = hb[0] * lr + hb[2] * rr; l[k][1] = hb[0] * li + hb[2] * ri;
            r[k][0] = hb[1] * lr + hb[3] * rr; r[k][1] = hb[1] * li + hb[3] * ri;
        } else {
            /* the phase runs the other way in the bands whose sub-band
             * order the hybrid split reversed */
            bool flip = (is34 && k >= 9 && k <= 13) || (!is34 && k <= 1);
            const float *hi = flip ? hF[k_to_b[k]] : hI[k_to_b[k]];
            l[k][0] = hb[0] * lr + hb[2] * rr - hi[0] * li - hi[2] * ri;
            l[k][1] = hb[0] * li + hb[2] * ri + hi[0] * lr + hi[2] * rr;
            r[k][0] = hb[1] * lr + hb[3] * rr - hi[1] * li - hi[3] * ri;
            r[k][1] = hb[1] * li + hb[3] * ri + hi[1] * lr + hi[3] * rr;
        }
    }
}

/* Frame driver */

/* Prepares a frame: the QMF input line with its six slots of look-back, the
 * mixing matrices, and the resets a band-layout change needs. */
void ps_frame_begin(struct faad_decoder *dec, float X[PS_IN_SLOTS][64][2], int top)
{
    PSState *ps = &dec->ps;
    bool is34 = ps->is34;

    for (int i = 0; i < 5; i++)
        for (int j = 0; j < PS_IN_SLOTS; j++) {
            ps->in_buf[i][j + 6][0] = X[j][i][0];
            ps->in_buf[i][j + 6][1] = X[j][i][1];
        }
    if (is34 != ps->is34_old) {
        memset(ps->peak_decay_nrg, 0, sizeof(ps->peak_decay_nrg));
        memset(ps->power_smooth, 0, sizeof(ps->power_smooth));
        memset(ps->peak_decay_diff_smooth, 0, sizeof(ps->peak_decay_diff_smooth));
        memset(ps->dc_in, 0, sizeof(ps->dc_in));
        memset(ps->dc_ap, 0, sizeof(ps->dc_ap));
        memset(ps->dc_delay, 0, sizeof(ps->dc_delay));
    }
    /* bands above the SBR range carry nothing: keep their lines silent */
    {
        int na = ps_nr_allpass[is34], nb = ps_nr_bands[is34];
        int top_k = top + nb - 64;
        if (top_k < 0) top_k = 0;
        if (top_k < na) {
            memset(ps->dc_in + top_k, 0, sizeof(ps->dc_in[0]) * (size_t)(na - top_k));
            memset(ps->dc_ap + top_k, 0, sizeof(ps->dc_ap[0]) * (size_t)(na - top_k));
        }
        int td = top_k > na ? top_k - na : 0;
        if (td < nb - na) memset(ps->dc_delay + td, 0, sizeof(ps->dc_delay[0]) * (size_t)(nb - na - td));
    }
    ps_mixing_matrices(ps);
}

/* One slot: hybrid domain, decorrelated copy, mix, back to the QMF domain. */
void ps_slot(struct faad_decoder *dec, int n, float X[PS_IN_SLOTS][64][2], float L[64][2], float R[64][2])
{
    PSState *ps = &dec->ps;
    float l[PS_NR_BANDS][2], r[PS_NR_BANDS][2];

    ps_hybrid_analysis_slot(ps, n, l, X);
    ps_decorrelate_slot(ps, l, r);
    ps_mix_slot(ps, n, l, r);
    ps_hybrid_synthesis_slot(l, L, ps->is34);
    ps_hybrid_synthesis_slot(r, R, ps->is34);
    if (n == PS_QMF_SLOTS - 1) {
        for (int i = 0; i < 5; i++)
            memmove(ps->in_buf[i], ps->in_buf[i] + PS_QMF_SLOTS, 6 * sizeof(ps->in_buf[i][0]));
        ps->is34_old = ps->is34;
    }
}

#endif /* FAAD_DISABLE_PS */
