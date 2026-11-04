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

#ifndef SBR_H
#define SBR_H

/* SBR frame classes (ISO 14496-3:2009 §4.6.18.3, Table 4.80). */
typedef enum SbrFrameClass {
    SBR_FRAME_CLASS_FIXFIX = 0,
    SBR_FRAME_CLASS_FIXVAR = 1,
    SBR_FRAME_CLASS_VARFIX = 2,
    SBR_FRAME_CLASS_VARVAR = 3
} SbrFrameClass;

#include <stddef.h>
#include "coder.h"
#include "channels.h"
#include "fft.h"
#include "sbr_analysis.h"

#define LOOKAHEAD_DEPTH 2

/* Depth of the coded-SBR-payload delay ring. Envelopes must describe the audio
   the access unit actually carries, which is FIFO_PAST: the MDCT window spans
   (FIFO_PAST, FIFO_CURR) and, at 50% overlap, an access unit completes the first
   half of its own window. So the payload emitted on call N belongs to frame
   N-(LOOKAHEAD_DEPTH+1), while SbrEncode has just analyzed frame N -- that delay,
   plus a slot for the newest entry. */
#define SBR_FRAME_FIFO (LOOKAHEAD_DEPTH + 2)

#ifdef __cplusplus
extern "C" {
#endif

struct BitStream;

#define SBR_QMF_BANDS_64     64
#define SBR_QMF_OVL_LEN_64   576
/* A decoder applies a frame's envelopes to QMF slots six slots later than the
   input frame they are paired with here. Measuring over input delayed by two
   more slots keeps an attack's high-frequency energy out of the envelope
   before it; a further slot costs several times the MOS. Whole slots keep the
   analysis on the decoder's grid. */
#define SBR_ANALYSIS_DELAY   (8 * SBR_QMF_BANDS_64)
#define SBR_QMF_HIST_LEN     (SBR_QMF_OVL_LEN_64 + SBR_ANALYSIS_DELAY)
/* All encoded envelopes use the high-resolution master band table. */
#define SBR_FREQ_RES_HIGH     1
/* Every master band is coded; VARFIX grids have no transient envelope. */
#define SBR_XOVER_BAND        0
#define SBR_GRID_POINTER      0
#define SBR_MAX_BANDS        64
#define SBR_MAX_ENVELOPES     4
#define SBR_HEADER_PERIOD    30

/* Envelope time-slot resolution the decoder uses for an AAC-LC core frame
 * (ISO 14496-3 §4.6.18.3: numTimeSlots = 16). All bs_rel_bord/t_env values
 * written in a variable grid live in [0, SBR_NUM_TIME_SLOTS]. */
#define SBR_NUM_TIME_SLOTS   16

/* SBR extension types (ISO 14496-3 §4.6.18). */
#define SBR_EXT_TYPE_SBR     0xd
#define SBR_EXT_TYPE_SBR_CRC 0xe

/* Transient detection threshold (peak-to-mean power ratio). A flagged frame
 * is coded as two variable-border envelopes instead of the four-envelope
 * fixed grid. At rich rates the finer fixed grid is worth more than splitting
 * on a weak peak, so only strong attacks switch. */
#define SBR_TRANSIENT_THRESH_DEFAULT    (4.0f)
#define SBR_TRANSIENT_THRESH_RICH       (8.0f)
/* div-by-zero guard for the peak/mean ratio in silence frames (~-150 dBFS^2). */
#define SBR_ENERGY_FLOOR                (1e-15f)
/* log2(0) guard in envelope quantization: -200 dBFS^2, below all SBR quantizer ranges. */
#define SBR_LOG_ENERGY_FLOOR            (1e-20f)
/* Noise floor level, written for the single noise band of every noise
 * envelope (ISO 14496-3 §4.6.18.6.4). The decoder adds noise at
 * 2^(6 - level) relative to the patched signal; a higher level lowers
 * synthesized noise in the patched bands. */
#define SBR_NOISE_LEVEL_DEFAULT         18
/* Inverse filtering mode, written for every channel (ISO 14496-3 §4.6.18.6.4).
 * The heavier modes whiten the patched band; a rich core leaves tonal
 * structure in the copied-up low band worth keeping, so it gets the lightest
 * mode. */
#define SBR_INVF_MODE                   3
#define SBR_INVF_MODE_RICH              1
/* 6 = log2(64): normalizes 64-band QMF energy to per-band level. ISO 14496-3 §4.6.18.6.3. */
#define SBR_ENV_LEVEL_LOG2_OFFSET       (6.0f)
/* Envelope level step, 1 = 3 dB. The finer 1.5 dB grid costs more than it
 * buys at every rate; frames with one FIXFIX envelope use it regardless. */
#define SBR_AMP_RES                     1
/* Rate-dependent resolution thresholds. */
/* Frames without a transient get four envelopes from here up; below it one
 * envelope leaves more bits for the core. */
#define SBR_FOUR_ENV_BITRATE_BPS        17000u
/* Master table density, bands per octave 12/10/8 for bs_freq_scale 1/2/3:
 * the coarsest table wins from 12 kbps/ch up, but below that it costs
 * speech-like clips more than it saves. */
#define SBR_FREQ_SCALE_COARSE_BPS       12000u
/* Stop-frequency search bounds (bs_stop_freq). The search starts at 0 so a
 * low crossover still finds a stop within the span decoders accept. 13 is
 * the largest worth searching: it already pins k2 to its 64-band ceiling at
 * every supported rate, so higher indices would just signal more range for
 * the same band. */
#define SBR_STOP_FREQ_MIN               0
#define SBR_STOP_FREQ_MAX               13
/* From this per-channel rate the core spends bits better below the crossover
 * than above it, so SBR takes over from about 8 kHz instead of 11.6 kHz. Under
 * it the core needs the bandwidth: a patch that low is audibly worse than a
 * starved core. The _RICH inverse filtering and transient settings are gated
 * on the same rate. */
#define SBR_RICH_BPS                    20000u
#define SBR_START_FREQ_DEFAULT          15
#define SBR_START_FREQ_RICH             12
/* The stop frequency may pass k2/kx = 2.2449 (NUM/DEN), where ISO 14496-3
 * §4.6.18.3.2.1 builds the master table in two regions, at these sample rates:
 * a rich core then keeps SBR up to 20 kHz. Measured at 44.1 and 48 kHz only. At
 * 32 kHz the wider range cost bits and above 48 kHz nothing was measured, so
 * those rates keep the stop frequency under the ratio. */
#define SBR_TWO_REGION_MIN_RATE         44100
#define SBR_TWO_REGION_MAX_RATE         48000
#define SBR_ONE_REGION_NUM              22449
#define SBR_ONE_REGION_DEN              10000
#define SBR_TWO_REGION_RATIO            2.2449f
#define SBR_UPPER_REGION_WARP           1.3f
/* Below this per-channel rate the core is too starved to cover much, so the
 * envelope stops short of 20 kHz instead of spending bits on the quietest top
 * bands. Not at 32 kHz or below: Nyquist is 16 kHz there, so a 14 kHz stop
 * saves little and cuts bright program material. */
#define SBR_NARROW_STOP_BPS             10000u
#define SBR_NARROW_STOP_MIN_RATE        32000
#define SBR_STOP_FREQ_TARGET_NARROW_HZ  14000
/* Where widening aims. Past this the bands are inaudible to essentially every
 * listener while costing exactly as much as the ones below. */
#define SBR_STOP_FREQ_TARGET_HZ         20000
/* Max delta-coded step for envelope data, per bs_amp_res grid (ISO 14496-3
 * §4.6.18.3.6): the fine (amp_res=1) grid has half the step size of the coarse
 * grid, so its delta range must be roughly double to cover the same dB span. */
#define SBR_ENV_DELTA_LIMIT_HIRES       31
#define SBR_ENV_DELTA_LIMIT_LORES       60

typedef struct SBRInfo SBRInfo;
struct SignalAnalysis;

typedef struct SBRContext SBRContext;

#if FAAC_ENCODER_SBR
SBRContext *SbrContextInit(int channels);
void SbrContextEnd(SBRContext *sbrCtx);
unsigned int SbrContextGetFullRateIdx(SBRContext *sCtx);
unsigned long SbrContextGetFullRate(SBRContext *sCtx, unsigned long defaultRate);
unsigned int SbrContextGetXOverBandwidth(SBRContext *sbrCtx);
int SbrContextUpdateConfig(SBRContext *sCtx, int channels, unsigned long bitrate, int aacObjectType, int rateLimited);
void SbrContextProcessFrame(SBRContext *sCtx, int numChannels, const bool *isLfe, int realPerCh, int flushTick, float *inputFifo[MAX_CHANNELS], float *heHalfRate[MAX_CHANNELS]);
int SbrContextIsPresent(SBRContext *sCtx);
void SbrContextResolveRate(SBRContext *sCtx, unsigned long *sampleRate, unsigned int *sampleRateIdx, SR_INFO **srInfo);

/* The EXT_SBR_DATA fill element following one SCE/CPE; none after an LFE. */
int SbrContextGetBits(SBRContext *sCtx, struct BitStream *bs, const AACElement *elem, int aacObjectType);

#else
static inline SBRContext *SbrContextInit(int channels)
{
    (void)channels;
    return NULL;
}

static inline void SbrContextEnd(SBRContext *sbrCtx)
{
    (void)sbrCtx;
}

static inline unsigned int SbrContextGetFullRateIdx(SBRContext *sCtx)
{
    (void)sCtx;
    return 0;
}

static inline unsigned long SbrContextGetFullRate(SBRContext *sCtx, unsigned long defaultRate)
{
    (void)sCtx;
    return defaultRate;
}

static inline unsigned int SbrContextGetXOverBandwidth(SBRContext *sbrCtx)
{
    (void)sbrCtx;
    return 0;
}

static inline int SbrContextUpdateConfig(SBRContext *sCtx, int channels, unsigned long bitrate,
                                         int aacObjectType, int rateLimited)
{
    (void)sCtx;
    (void)channels;
    (void)bitrate;
    (void)aacObjectType;
    (void)rateLimited;
    return 0;
}

static inline void SbrContextProcessFrame(SBRContext *sCtx, int numChannels, const bool *isLfe, int realPerCh, int flushTick, float *inputFifo[MAX_CHANNELS], float *heHalfRate[MAX_CHANNELS])
{
    (void)sCtx;
    (void)numChannels;
    (void)isLfe;
    (void)realPerCh;
    (void)flushTick;
    (void)inputFifo;
    (void)heHalfRate;
}

static inline int SbrContextIsPresent(SBRContext *sCtx)
{
    (void)sCtx;
    return 0;
}

static inline void SbrContextResolveRate(SBRContext *sCtx, unsigned long *sampleRate, unsigned int *sampleRateIdx, SR_INFO **srInfo)
{
    (void)sCtx;
    (void)sampleRate;
    (void)sampleRateIdx;
    (void)srInfo;
}

static inline int SbrContextGetBits(SBRContext *sCtx, struct BitStream *bs, const AACElement *elem, int aacObjectType)
{
    (void)sCtx;
    (void)bs;
    (void)elem;
    (void)aacObjectType;
    return 0;
}
#endif

#ifdef __cplusplus
}
#endif

#endif
