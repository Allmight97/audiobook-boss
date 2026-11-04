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

#ifndef SBR_INTERNAL_H
#define SBR_INTERNAL_H

#include "sbr.h"
#include "sbr_analysis.h"
#include "resample.h"
#include "ps.h"

/* A written frame's last envelope and the layout it was coded in: the
 * reference for time-delta coding the next frame's first one. nb 0 = none yet. */
typedef struct {
    int env[SBR_MAX_BANDS];
    int nb;
    int ampRes;
    int coupled;
} SbrEnvRef;

/* Per-channel SBR analysis state. Everything indexed [ch] in SBRInfo lives here. */
typedef struct SBRChannel {
    float qmfOvl64[SBR_QMF_HIST_LEN]; /* QMF overlap plus analysis delay (carries across frames) */
    /* By access-unit parity: a frame reads the previous frame's slot and writes
     * its own, so a frame the rate loop writes again codes against the same
     * reference as the first time. */
    SbrEnvRef ref[2];
} SBRChannel;

/* A channel's quantized envelopes, one level per band. */
typedef struct {
    int envData[SBR_MAX_ENVELOPES][SBR_MAX_BANDS];
} SbrEnvData;

/* One frame's coded SBR payload: every field SbrWrite reads that varies per
 * frame. What it reads that is constant for the stream (bs_* header fields,
 * numBands) stays in SBRInfo.
 *
 * Sole home for these values: SbrEncode quantizes into a SBRContext.frameFIFO
 * slot and SbrWrite reads an older one, so the delay costs a ring index. Caching
 * a copy anywhere else reintroduces the skew this ring exists to remove. */
typedef struct SbrFrameData {
    uint8_t numEnvelopes;
    uint8_t eff_amp_res;
    uint8_t frameClass;
    uint8_t tEnv[SBR_MAX_ENVELOPES + 1];
    /* The noise floor and inverse-filter mode are stream constants
     * (SBR_NOISE_LEVEL_DEFAULT, SBR_INVF_MODE), so only the envelope is carried. */
#if FAAC_ENCODER_PS
    int8_t iid[PS_BANDS];
    int8_t icc[PS_BANDS];
    uint8_t enable_icc;
    int8_t ipd[PS_PHASE_BANDS], opd[PS_PHASE_BANDS];
    uint8_t enable_phase;
    uint8_t ps_num_env;
    uint8_t ps_band_mode;
    /* Second envelope; the first lives in the arrays above. */
    struct {
        int8_t iid[PS_BANDS], icc[PS_BANDS];
        int8_t ipd[PS_PHASE_BANDS], opd[PS_PHASE_BANDS];
    } ps_env2;
#endif
    SbrEnvData ch[];
} SbrFrameData;

struct SBRInfo {
    /* Frame scratch lives with the encoder handle, not on the caller's thread
     * stack; PS needs a second slot for the right channel. */
    float workspace[FAAC_ENCODER_PS ? 2 : 1][SBR_QMF_HIST_LEN + 2 * FRAME_LEN];
#if FAAC_ENCODER_PS
    int is_he_v2;
    PsHybrid psHybrid;
    PsRef psRef[2];
    int psNumEnv;
#endif
    /* Coupled rendition of a conventional stereo SBR pair: level, then balance. */
    SbrEnvData cpl[2];
    int frameCount;        /* access units so far; the header repeats every SBR_HEADER_PERIOD */
    int numChannels;
    int sampleRate;        /* full output rate; the dual-rate core runs at sampleRate/2 */

    /* --- frequency band configuration (set at init, constant per stream) --- */
    int kx;
    int k2;
    int numBands;
    int bandEdges[SBR_MAX_BANDS + 1];

    /* --- bitstream header fields --- */
    int bs_start_freq;
    int bs_stop_freq;
    int bs_alter_scale;
    int bs_freq_scale;     /* 1..3: log-spaced master table, 12/10/8 bands per octave */
    int numEnvFixFix;      /* envelopes in a frame without a transient: 1 or 4 */
    int rich;              /* core has bits to spare: selects the SBR_*_RICH settings */

    /* --- per-frame state --- */
    /* The header decision is made once per access unit, on the first write
     * request after analysis: the writer runs once per element and again on
     * every CBR retry, and only access units that are actually written count
     * toward the header period. */
    int headerDecided;
    int sendHeaderThisFrame;

    /* QMF analysis twiddle factors. */
    float twidCos[SBR_QMF_BANDS_64];
    float twidSin[SBR_QMF_BANDS_64];
    float oddCos [SBR_QMF_BANDS_64];
    float oddSin [SBR_QMF_BANDS_64];

    /* QMF history follows the fixed channel count. */
    SBRChannel ch[];
};

struct SBRContext {
    unsigned long fullSampleRate;
    unsigned int  fullSampleRateIdx;
    SBRInfo      *sbrInfo;
    struct Resampler *resampler;

    /* Shared signal analysis */
    SignalAnalysis *signalAnalysis;
    /* Coded-payload delay ring; see SBR_FRAME_FIFO. frameHead is the newest
       entry, so its successor (frameHead + 1) % SBR_FRAME_FIFO is the oldest --
       the payload the current access unit emits. */
    SbrFrameData *frameFIFO[SBR_FRAME_FIFO];
    int          frameHead;
#if FAAC_ENCODER_PS
    PsCarrier psCarrier;
    float psSpectralCarrier[2 * FRAME_LEN];
#endif
    /* Channel-sized payload slots share the context allocation. */
    unsigned char frameData[];
};

SBRInfo *SbrInit(int channels, int sampleRate, unsigned long bitRate);
/* Recompute the bitrate-dependent band config without reallocating; lets
 * SetConfiguration adjust an existing handle. */
void SbrUpdate(SBRInfo *sbr, unsigned long bitRate);
int build_freq_table(SBRInfo *sbr);
void SbrEnd(SBRInfo *sbr);

/* A compile-time false result removes all PS branches in disabled builds. */
static inline bool PsIsActive(const SBRInfo *sbr)
{
#if FAAC_ENCODER_PS
    return sbr->is_he_v2 != 0;
#else
    (void)sbr;
    return false;
#endif
}

#if FAAC_ENCODER_PS
void SbrQmfAnalysisComplex(SBRInfo *, const float *, float *, float *, int, int);
#endif
void SbrQmfAnalysis(SBRInfo *sbr, const float * restrict ovl_pos, float * restrict energy, int kx, int k2);
/* Quantizes this frame's payload directly into *fd (a delay-line slot). */
void SbrEncode(SBRInfo *sbr, float *timeDomain[MAX_CHANNELS], int numChannels, const bool *isLfe, int numSamples, struct SignalAnalysis *sa, SbrFrameData *fd);

#endif
