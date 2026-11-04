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

#ifndef FAAC_PS_H
#define FAAC_PS_H

#include <stdbool.h>
#include <stdint.h>

#define PS_BANDS 20
#define PS_PHASE_BANDS 11
#define PS_LOW_BANDS 10
#define PS_LOW_PHASE_BANDS 5
#define PS_LOW_RATE_MAX_BITRATE 16000 /* total bits/s, explicit bitrate only */

struct SBRInfo;
struct SBRContext;
struct SignalAnalysis;
struct SbrFrameData;
struct BitStream;

/* Preserve the preceding envelope for temporal deltas during rate-control retries. */
typedef struct PsRef {
    int8_t iid[PS_BANDS], icc[PS_BANDS], ipd[PS_PHASE_BANDS], opd[PS_PHASE_BANDS];
    uint8_t valid, band_mode;
} PsRef;
typedef struct PsHybrid {
    float history[2][3][6][2], coef[8][7][2];
    int initialized;
} PsHybrid;

void PsHybridAnalyze(PsHybrid *, float[2][3][44][2], float[2][4][64], float[4][64],
                     float[4][64], int);

typedef struct PsCarrier {
    float history[2][256], overlap[256], window[512], invWindowPower[256];
    float work[1024], leftSpectrum[1024];
    int initialized;
    double smoothL, smoothR, smoothCross;
} PsCarrier;

void PsSpectralDownmix(PsCarrier *, float *, float *, float *, int);
void PsDownmix(float *, const float *, int);

/* Pass the preceding envelope for temporal deltas, or NULL when band modes differ. */
int PsWrite(const struct SbrFrameData *, struct BitStream *, const PsRef *);

/* SBR integration hooks. Disabled builds inline these away and omit PS storage.
 * Carrier and extension hooks require the caller to check PsIsActive first. */
#if FAAC_ENCODER_PS
void PsConfigure(struct SBRInfo *, bool, unsigned long, int);
void PsPrepareCarrier(struct SBRContext *, float *[]);
void PsCommitCarrier(struct SBRContext *, float *);
void PsAnalyze(struct SBRInfo *, struct SignalAnalysis *, struct SbrFrameData *);
void PsWriteExtension(struct SBRInfo *, const struct SbrFrameData *, struct BitStream *);
#else
static inline void PsConfigure(struct SBRInfo *sbr, bool active, unsigned long bitrate, int rateLimited)
{
    (void)sbr; (void)active; (void)bitrate; (void)rateLimited;
}
static inline void PsPrepareCarrier(struct SBRContext *ctx, float *input[])
{
    (void)ctx; (void)input;
}
static inline void PsCommitCarrier(struct SBRContext *ctx, float *output)
{
    (void)ctx; (void)output;
}
static inline void PsAnalyze(struct SBRInfo *sbr, struct SignalAnalysis *sa, struct SbrFrameData *fd)
{
    (void)sbr; (void)sa; (void)fd;
}
static inline void PsWriteExtension(struct SBRInfo *sbr, const struct SbrFrameData *fd, struct BitStream *bs)
{
    (void)sbr; (void)fd; (void)bs;
}
#endif

#if FAAC_ENCODER_PS && defined(FAAC_STATS)
void PsCountFrame(const struct SBRInfo *, const struct SbrFrameData *, int);
void PsStatsPrint(void);
#else
static inline void PsCountFrame(const struct SBRInfo *sbr, const struct SbrFrameData *fd, int id_aac)
{
    (void)sbr; (void)fd; (void)id_aac;
}
static inline void PsStatsPrint(void) {}
#endif

#endif /* FAAC_PS_H */
