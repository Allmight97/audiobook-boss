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

#ifndef SBR_ANALYSIS_H
#define SBR_ANALYSIS_H

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

#ifndef SBR_QMF_BANDS_64
#define SBR_QMF_BANDS_64 64
#endif

#ifndef SBR_MAX_ENVELOPES
#define SBR_MAX_ENVELOPES 4
#endif

struct SBRInfo;

typedef struct SignalAnalysisChannel {
    /* Each channel owns its envelope energy over the shared grid. */
    float bandE[SBR_MAX_ENVELOPES][SBR_QMF_BANDS_64];
} SignalAnalysisChannel;

typedef struct SignalAnalysis {
    /* Frame envelope grid configuration. Synchronized across all channels. */
    SbrFrameClass frameClass;
    int numEnvelopes;
    int tEnv[SBR_MAX_ENVELOPES + 1];
    int envSampled[SBR_MAX_ENVELOPES];

#if FAAC_ENCODER_PS
    float psE[2][4][64], psCross[4][64], psCrossIm[4][64];
    float psLow[2][3][44][2];
#endif
    SignalAnalysisChannel ch[];
} SignalAnalysis;

void SbrAnalyze(SignalAnalysis *sa, float *fullPtrs[], int nch, const bool *isLfe, int numSamples, struct SBRInfo *sbr);

#ifdef __cplusplus
}
#endif

#endif
