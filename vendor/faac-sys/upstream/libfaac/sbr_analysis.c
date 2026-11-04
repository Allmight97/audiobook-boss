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

#include "sbr.h"
#include "sbr_analysis.h"
#include "sbr_internal.h"
#include "util.h"
#include <string.h>

/* Envelope border in QMF slots, for binning the per-slot energies. The first
 * envelope also takes slots before tEnv[0] and the last the ones after its
 * end, so no slot's energy is dropped. Computed on demand: an array of them
 * is vectorized into more code than the whole analysis it serves. */
static inline int env_border(const SignalAnalysis *sa, int e, int num_slots)
{
    if (e == 0) return 0;
    if (e == sa->numEnvelopes) return num_slots;
    return sa->tEnv[e] * num_slots / SBR_NUM_TIME_SLOTS;
}

/* Multi-pass signal analysis: transient detection, temporal grid selection,
 * and subband energy accumulation. */
void SbrAnalyze(SignalAnalysis *sa, float *fullPtrs[], int nch, const bool *isLfe, int numSamples, struct SBRInfo *sbr)
{
#if MAX_CHANNELS < 6
    (void)isLfe;
#endif
    int num_slots = numSamples / SBR_QMF_BANDS_64;
    float *workspace = sbr->workspace[0];

    float frameStrength = 0.0f;

    /* The strongest non-LFE transient selects the shared envelope grid. */
    for (int ch = 0; ch < nch; ch++) {
#if MAX_CHANNELS >= 6
        if (isLfe[ch]) continue;
#endif
        float smax = 0.0f, ssum = 0.0f;
        for (int slot = 0; slot < num_slots; slot++) {
            /* The analyzed frame starts SBR_ANALYSIS_DELAY back, in the saved history. */
            int pos = slot * SBR_QMF_BANDS_64 - SBR_ANALYSIS_DELAY;
            const float * restrict p_in = (pos < 0) ? sbr->ch[ch].qmfOvl64 + SBR_QMF_HIST_LEN + pos
                                                    : fullPtrs[ch] + pos;
            float stot = 0.0f;
            for (int n = 0; n < SBR_QMF_BANDS_64; n += 4) {
                float v0 = p_in[0], v1 = p_in[1], v2 = p_in[2], v3 = p_in[3];
                stot += v0 * v0 + v1 * v1 + v2 * v2 + v3 * v3;
                p_in += 4;
            }

            if (stot > smax) {
                smax = stot;
            }
            ssum += stot;
        }

        float strength = smax * (float)num_slots / (ssum + SBR_ENERGY_FLOOR);
        if (strength > frameStrength) {
            frameStrength = strength;
        }
    }

    if (frameStrength > (sbr->rich ? SBR_TRANSIENT_THRESH_RICH : SBR_TRANSIENT_THRESH_DEFAULT)) {
        const int innerSbr = SBR_NUM_TIME_SLOTS / 2;
        sa->numEnvelopes = 2;
        sa->frameClass = SBR_FRAME_CLASS_VARFIX;
        sa->tEnv[0] = 0;
        sa->tEnv[1] = innerSbr;
        sa->tEnv[2] = SBR_NUM_TIME_SLOTS;
    } else {
        int ne = sbr->numEnvFixFix;
        sa->numEnvelopes = ne;
        sa->frameClass = SBR_FRAME_CLASS_FIXFIX;
        for (int e = 0; e <= ne; e++)
            sa->tEnv[e] = e * SBR_NUM_TIME_SLOTS / ne;
    }

    /* Slots analyzed per envelope, for power normalization: the multiples of
     * the decimation factor below each border. */
    for (int e = 0; e < sa->numEnvelopes; e++) {
        int n = (env_border(sa, e + 1, num_slots) + FAAC_SBR_DECIMATION - 1) / FAAC_SBR_DECIMATION
              - (env_border(sa, e, num_slots) + FAAC_SBR_DECIMATION - 1) / FAAC_SBR_DECIMATION;
        sa->envSampled[e] = n < 1 ? 1 : n;
    }

    if (sbr) {
#if FAAC_ENCODER_PS
        if (sbr->is_he_v2) {
            float *right = sbr->workspace[1];
            memcpy(workspace, sbr->ch[0].qmfOvl64, SBR_QMF_HIST_LEN * sizeof(float));
            memcpy(workspace + SBR_QMF_HIST_LEN, fullPtrs[0], numSamples * sizeof(float));
            memcpy(right, sbr->ch[1].qmfOvl64, SBR_QMF_HIST_LEN * sizeof(float));
            memcpy(right + SBR_QMF_HIST_LEN, fullPtrs[1], numSamples * sizeof(float));
            memset(sa->ch[0].bandE, 0, sizeof(sa->ch[0].bandE));
            memset(sa->psE, 0, sizeof(sa->psE));
            memset(sa->psCross, 0, sizeof(sa->psCross));
            memset(sa->psCrossIm, 0, sizeof(sa->psCrossIm));
            float (*low)[3][44][2] = sa->psLow;
            for (int ch = 0; ch < 2; ch++)
                for (int k = 0; k < 3; k++)
                    memcpy(low[ch][k], sbr->psHybrid.history[ch][k], sizeof(sbr->psHybrid.history[ch][k]));
            for (int slot = 0; slot < num_slots + 6; slot++) {
                float lr[64], li[64], rr[64], ri[64];
                int sampled = slot < num_slots && slot % FAAC_SBR_DECIMATION == 0;
                SbrQmfAnalysisComplex(sbr, workspace + slot * 64, lr, li, 0, sampled ? 64 : 3);
                SbrQmfAnalysisComplex(sbr, right + slot * 64, rr, ri, 0, sampled ? 64 : 3);
                for (int k = 0; k < 3; k++) {
                    low[0][k][slot+6][0]=lr[k]; low[0][k][slot+6][1]=li[k];
                    low[1][k][slot+6][0]=rr[k]; low[1][k][slot+6][1]=ri[k];
                }
                if (!sampled) continue;
                int e = 0;
                while (e + 1 < sa->numEnvelopes &&
                       slot >= env_border(sa, e + 1, num_slots)) e++;
                int p = slot * 4 / num_slots;
                static const unsigned char edges[13] = {3,4,5,6,7,8,9,11,14,18,23,35,64};
                int band = 8;
                for (int k = 0; k < 64; k++) {
                    /* The mono carrier preserves mean stereo energy. */
                    sa->ch[0].bandE[e][k] +=
                        0.5f * (lr[k]*lr[k]+li[k]*li[k]+rr[k]*rr[k]+ri[k]*ri[k]);
                    if (k < 3) continue;
                    /* Band edges are monotonic in k. */
                    while (k >= edges[band-7]) band++;
                    sa->psE[0][p][band] += lr[k]*lr[k]+li[k]*li[k];
                    sa->psE[1][p][band] += rr[k]*rr[k]+ri[k]*ri[k];
                    sa->psCross[p][band] += lr[k]*rr[k]+li[k]*ri[k];
                    sa->psCrossIm[p][band] += li[k]*rr[k]-lr[k]*ri[k];
                }
            }
            PsHybridAnalyze(&sbr->psHybrid, low, sa->psE, sa->psCross, sa->psCrossIm, num_slots);
            for (int ch = 0; ch < 2; ch++)
                for (int k = 0; k < 3; k++)
                    memcpy(sbr->psHybrid.history[ch][k], low[ch][k] + num_slots,
                           sizeof(sbr->psHybrid.history[ch][k]));
            return;
        }
#endif
    /* Pass 2: subband analysis, accumulating QMF band energy per envelope.
     * Only [kx, k2) feeds the quantizer, so skip bands below kx. */
        int kx = sbr->kx;
        int kEnd = sbr->k2;
        for (int ch = 0; ch < nch; ch++) {
#if MAX_CHANNELS >= 6
            if (isLfe[ch]) continue;
#endif
            memset(sa->ch[ch].bandE, 0, (size_t)sa->numEnvelopes * sizeof(sa->ch[ch].bandE[0]));

            memcpy(workspace, sbr->ch[ch].qmfOvl64, SBR_QMF_HIST_LEN * sizeof(float));
            memcpy(workspace + SBR_QMF_HIST_LEN, fullPtrs[ch], numSamples * sizeof(float));

            for (int e = 0; e < sa->numEnvelopes; e++) {
                int end = env_border(sa, e + 1, num_slots);
                for (int slot = env_border(sa, e, num_slots); slot < end; slot++) {
#if FAAC_SBR_DECIMATION > 1
                    if (slot % FAAC_SBR_DECIMATION != 0) continue;
#endif
                    SbrQmfAnalysis(sbr, workspace + slot * SBR_QMF_BANDS_64, sa->ch[ch].bandE[e], kx, kEnd);
                }
            }
        }
    }
}
