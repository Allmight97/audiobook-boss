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

#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <assert.h>

#include "sbr.h"
#include "sbr_tables.h"
#include "sbr_huff_tables.h"
#include "util.h"
#include "sbr_analysis.h"
#include "resample.h"
#include "bitstream.h"
#include "sbr_internal.h"
#include "faac_internal.h"
#include "channels.h"
#include "stats.h"

/* SBR master frequency band table (ISO/IEC 14496-3:2005 §4.6.18.3.2). kx/k2 are
 * spec-mandatory: the decoder reconstructs them from the sample rate alone, so
 * these must match its table exactly or the envelope band count desyncs. The
 * rate here is the full output rate (= 2*core), which is what the decoder uses. */

/* SBR start frequency (kx). Crossover alignment prevents aliasing/gaps. */
static int compute_kx(int sampleRate, int bs_start_freq)
{
    int temp = (sampleRate < 32000) ? 3000 : (sampleRate < 64000) ? 4000 : 5000;
    int start_min = ((temp << 7) + (sampleRate >> 1)) / sampleRate;
    int row = (sampleRate <= 16000) ? 0 : (sampleRate <= 22050) ? 1 : (sampleRate <= 24000) ? 2 : (sampleRate <= 32000) ? 3 : (sampleRate <= 64000) ? 4 : 5;
    return clamp_int(start_min + sbr_offset[row][bs_start_freq & 15], 1, 63);
}

static int cmp_int16(const void *a, const void *b) { return (int)(*(const short *)a) - (int)(*(const short *)b); }
static int cmp_int(const void *a, const void *b) { return *(const int *)a - *(const int *)b; }

/* Fewest QMF bands between kx and k2 that still yield an envelope band at
 * every freq_scale; above 64 kHz the 20 kHz target can fall below kx. */
#define SBR_MIN_SPAN 4

/* Widest k2 - kx decoders accept (§4.6.18.3.2.1). */
static int max_sbr_span(int sampleRate)
{
    return (sampleRate <= 32000) ? 48 : (sampleRate <= 44100) ? 35 : 32;
}

/* Smallest stop-frequency index reaching targetHz, or the widest one decoders
 * accept; *k2_out receives its QMF band. Searched rather than tabulated: the
 * index-to-frequency mapping shifts with sample rate, so a fixed table would
 * overshoot at some rates. Decoders derive k2 from bs_stop_freq alone (ISO
 * 14496-3 §4.6.18.3.2.1), so the table is rebuilt as the spec defines it:
 * stop_min plus the sf narrowest of 13 geometric steps. */
static int pick_stop_freq(int sampleRate, int kx, int targetHz, int *k2_out)
{
    int temp = (sampleRate < 32000) ? 3000 : (sampleRate < 64000) ? 4000 : 5000;
    int stop_min = ((temp << 8) + (sampleRate >> 1)) / sampleRate;
    short stop_dk[13];
    float prod = (float)stop_min;
    int prev = stop_min;
    float base = powf(64.0f / (float)stop_min, (float)(1.0f / 13.0f));
    for (int i = 0; i < 12; i++) {
        prod *= base;
        int present = (int)lrintf(prod);
        stop_dk[i] = (short)(present - prev);
        prev = present;
    }
    stop_dk[12] = (short)(64 - prev);
    qsort(stop_dk, 13, sizeof(short), cmp_int16);

    int two_region = sampleRate >= SBR_TWO_REGION_MIN_RATE && sampleRate <= SBR_TWO_REGION_MAX_RATE;
    int best = SBR_STOP_FREQ_MIN, best_k2 = stop_min, k2 = stop_min;
    for (int sf = SBR_STOP_FREQ_MIN; sf <= SBR_STOP_FREQ_MAX; sf++) {
        if (sf > 0) k2 += stop_dk[sf - 1];
        if ((!two_region && k2 * SBR_ONE_REGION_DEN > SBR_ONE_REGION_NUM * kx) || k2 - kx > max_sbr_span(sampleRate)) break;
        best = sf;
        best_k2 = k2;
        if (k2 - kx >= SBR_MIN_SPAN && (long)k2 * sampleRate / (2 * SBR_QMF_BANDS_64) >= targetHz) break;
    }
    *k2_out = best_k2;
    return best;
}

static int sbr_round_f(float v) { return (int)floorf(v + 0.5f); }

static int build_log_band_widths(int k_start, int k_end, int bands_per_octave, float warp, int *vdk)
{
    int nb = 2 * sbr_round_f(bands_per_octave * log2f((float)k_end / (float)k_start) / (2.0f * warp));
    nb = clamp_int(nb, 1, SBR_MAX_BANDS);
    int prev = k_start;
    int i = 0;
    do {
        int cur = sbr_round_f(k_start * powf((float)k_end / (float)k_start, (float)(i + 1) / (float)nb));
        vdk[i] = cur - prev;
        prev = cur;
    } while (++i < nb);
    qsort(vdk, nb, sizeof(int), cmp_int);
    return nb;
}

/* Master table (ISO 14496-3 §4.6.18.3.2.1). bs_freq_scale 1/2/3:
 * log-spaced with 12/10/8 bands per octave, widths of a geometric series
 * rounded and sorted so the narrow bands sit at the bottom. When k2/kx
 * exceeds 2.2449, the table splits at k1 = 2*kx into lower and upper regions,
 * warped by bs_alter_scale and re-sorted. */
int build_freq_table(SBRInfo *sbr)
{
    int kx = sbr->kx, k2 = sbr->k2;
    int *edges = sbr->bandEdges;
    int bands_per_octave = 14 - 2 * sbr->bs_freq_scale; /* 12, 10, 8 for bs_freq_scale 1, 2, 3 */
    int two_region = (float)k2 / (float)kx > SBR_TWO_REGION_RATIO;
    int k1 = two_region ? 2 * kx : k2;
    int widths[2 * SBR_MAX_BANDS];
    int n_master = build_log_band_widths(kx, k1, bands_per_octave, 1.0f, widths);

    if (two_region) {
        int *upper = widths + n_master;
        float warp = sbr->bs_alter_scale ? SBR_UPPER_REGION_WARP : 1.0f;
        int nb1 = build_log_band_widths(k1, k2, bands_per_octave, warp, upper);
        if (upper[0] < widths[n_master - 1]) {
            int change = widths[n_master - 1] - upper[0];
            int half = (upper[nb1 - 1] - upper[0]) / 2;
            if (change > half) change = half;
            upper[0] += change;
            upper[nb1 - 1] -= change;
            qsort(upper, nb1, sizeof(int), cmp_int);
        }
        n_master += nb1;
    }

    edges[0] = kx;
    for (int i = 0; i < n_master; i++) edges[i + 1] = edges[i] + widths[i];

    sbr->numBands = n_master;

    return n_master;
}

SBRInfo *SbrInit(int channels, int sampleRate, unsigned long bitRate)
{
    size_t size = sizeof(SBRInfo) + channels * sizeof(SBRChannel);
    SBRInfo *sbr = (SBRInfo *)AllocMemory(size);
    if (!sbr) return NULL;
    memset(sbr, 0, size);
    sbr->numChannels = channels;
    sbr->sampleRate = sampleRate;

    /* Pre-calculate twiddle factors for the FFT-based QMF analysis.
     * These coefficients rotate the subband indices into the odd-frequency
     * DFT space required by the SBR modulation kernel. */
    for (int m = 0; m < SBR_QMF_BANDS_64; m++) {
        sbr->twidCos[m] = (float)cos(M_PI_DOUBLE * m / 64.0);
        sbr->twidSin[m] = (float)sin(M_PI_DOUBLE * m / 64.0);
        sbr->oddCos[m] = (float)cos(M_PI_DOUBLE * (2 * m + 1) / 128.0);
        sbr->oddSin[m] = (float)sin(M_PI_DOUBLE * (2 * m + 1) / 128.0);
    }
    SbrUpdate(sbr, bitRate);
    return sbr;
}

/* Re-resolve SBR operational parameters (crossover, resolution) when the
 * bitrate or sample rate changes, avoiding handle reallocation. */
void SbrUpdate(SBRInfo *sbr, unsigned long bitRate)
{
    int sampleRate = sbr->sampleRate;
    unsigned long rate_per_ch = bitRate / sbr->numChannels;
    sbr->numEnvFixFix = (rate_per_ch >= SBR_FOUR_ENV_BITRATE_BPS) ? 4 : 1;
    sbr->rich = rate_per_ch >= SBR_RICH_BPS;
    /* A crossover near the core ceiling (~11.6 kHz) maximizes MOS while the
     * core is bit-starved: parametric reconstruction below 10 kHz is audible
     * and generally inferior to that core. A rich core can afford SBR from
     * about 8 kHz (SBR_RICH_BPS). */
    sbr->bs_start_freq = sbr->rich ? SBR_START_FREQ_RICH : SBR_START_FREQ_DEFAULT;
    /* Log-spaced envelope bands, fewer per octave while bits are scarce:
     * what they save, rate control hands to the core. */
    sbr->bs_freq_scale = (rate_per_ch >= SBR_FREQ_SCALE_COARSE_BPS) ? 3 : 2;
    sbr->bs_alter_scale = 0; /* only warps a two-region table; see build_freq_table */
    sbr->kx = compute_kx(sampleRate, sbr->bs_start_freq);

    /* Where the reconstruction stops. Aim at hearing rather than k2's ceiling:
     * bands above the target cost the same envelope bits as the ones below, so
     * there's no reason to stop short of what's audible. */
    sbr->bs_stop_freq = pick_stop_freq(sampleRate, sbr->kx,
                                       (rate_per_ch < SBR_NARROW_STOP_BPS && sampleRate > SBR_NARROW_STOP_MIN_RATE) ? SBR_STOP_FREQ_TARGET_NARROW_HZ : SBR_STOP_FREQ_TARGET_HZ,
                                       &sbr->k2);

    build_freq_table(sbr);
}

void SbrEnd(SBRInfo *sbr)
{
    if (!sbr) return;
    FreeMemory(sbr);
}

/* What analyzing a silent frame yields. Needed because zeroed memory is not a
 * legal payload: numEnvelopes == 0 encodes no grid at all. */
static void sbr_frame_silence(SbrFrameData *fd, int channels)
{
    memset(fd, 0, sizeof(*fd) + channels * sizeof(fd->ch[0]));
    fd->numEnvelopes = 1;
    fd->eff_amp_res  = 0;
    fd->frameClass   = SBR_FRAME_CLASS_FIXFIX;
    fd->tEnv[0]      = 0;
    fd->tEnv[1]      = SBR_NUM_TIME_SLOTS;
}

SBRContext *SbrContextInit(int channels)
{
    size_t frameSize = sizeof(SbrFrameData) + channels * sizeof(((SbrFrameData *)0)->ch[0]);
    SBRContext *sbrCtx = (SBRContext *)AllocMemory(sizeof(SBRContext) + SBR_FRAME_FIFO * frameSize);
    if (sbrCtx) {
        memset(sbrCtx, 0, sizeof(SBRContext));
        sbrCtx->resampler = ResampleInit(channels);
        size_t size = sizeof(SignalAnalysis) + channels * sizeof(SignalAnalysisChannel);
        sbrCtx->signalAnalysis = AllocMemory(size);
        if (!sbrCtx->resampler || !sbrCtx->signalAnalysis) {
            SbrContextEnd(sbrCtx);
            return NULL;
        }
        memset(sbrCtx->signalAnalysis, 0, size);
        /* The first access units carry the core's silent lead-in, so the ring
         * has to start full of payloads that describe silence. */
        for (int i = 0; i < SBR_FRAME_FIFO; i++) {
            sbrCtx->frameFIFO[i] = (SbrFrameData *)(sbrCtx->frameData + i * frameSize);
            sbr_frame_silence(sbrCtx->frameFIFO[i], channels);
        }
    }
    return sbrCtx;
}

void SbrContextEnd(SBRContext *sbrCtx)
{
    if (!sbrCtx) return;
    if (sbrCtx->sbrInfo) {
        SbrEnd(sbrCtx->sbrInfo);
    }
    if (sbrCtx->resampler) {
        ResampleEnd(sbrCtx->resampler);
    }
    FreeMemory(sbrCtx->signalAnalysis);
    FreeMemory(sbrCtx);
}

unsigned int SbrContextGetFullRateIdx(SBRContext *sCtx)
{
    return sCtx->fullSampleRateIdx;
}

#if FAAC_ENCODER_DRC
unsigned long SbrContextGetFullRate(SBRContext *sCtx, unsigned long defaultRate)
{
    return (sCtx && sCtx->fullSampleRate) ? sCtx->fullSampleRate : defaultRate;
}
#endif

unsigned int SbrContextGetXOverBandwidth(SBRContext *sbrCtx)
{
    if (!sbrCtx || !sbrCtx->sbrInfo) return 0;
    /* kx * Fs / (2*64): each QMF band is Fs/(2*SBR_QMF_BANDS_64) Hz wide.
     * Matching core bandwidth to the SBR crossover avoids a gap or overlap. */
    return (unsigned int)((sbrCtx->sbrInfo->kx * sbrCtx->fullSampleRate) /
                           (2 * SBR_QMF_BANDS_64));
}

int SbrContextUpdateConfig(SBRContext *sCtx, int channels, unsigned long bitrate, int aacObjectType, int rateLimited)
{
    sCtx->sbrInfo = SbrInit(channels, sCtx->fullSampleRate, bitrate);
    if (!sCtx->sbrInfo) return 0;
    PsConfigure(sCtx->sbrInfo, IsPS(aacObjectType), bitrate, rateLimited);
    return 1;
}

void SbrContextProcessFrame(SBRContext *sCtx, int numChannels, const bool *isLfe, int realPerCh, int flushTick, float *inputFifo[MAX_CHANNELS], float *heHalfRate[MAX_CHANNELS])
{
    unsigned int channel;
    Resampler *rs = sCtx->resampler;
    float *fullPtrs[MAX_CHANNELS];

    /* SbrEncode quantizes into the new head; SbrWrite (via SbrContextGetBits)
     * emits the oldest slot, which is the payload for this frame's core audio. */
    sCtx->frameHead = (sCtx->frameHead + 1) % SBR_FRAME_FIFO;
    SbrFrameData *fd = sCtx->frameFIFO[sCtx->frameHead];
    sCtx->sbrInfo->headerDecided = 0;

    /* Tick 1 still has real signal in the QMF overlap and the decimation FIR;
     * by tick 2 both are zero, so the rest of the drain is known silence. */
    if (realPerCh == 0 && flushTick > 1) {
        for (channel = 0; channel < (unsigned int)numChannels; channel++) {
            rs->halfRate[channel] = inputFifo[channel];
            memset(rs->halfRate[channel], 0, FRAME_LEN * sizeof(float));
            heHalfRate[channel] = rs->halfRate[channel];
        }
        sbr_frame_silence(fd, numChannels);
        if (PsIsActive(sCtx->sbrInfo)) PsNoteSilence(sCtx->sbrInfo, fd);
    } else {
        for (channel = 0; channel < (unsigned int)numChannels; channel++) {
            /* SBR finishes before the FIR overwrites the consumed FIFO front. */
            float *fullRate = inputFifo[channel];
            rs->fullRate[channel] = rs->halfRate[channel] = fullRate;
            fullPtrs[channel] = fullRate;
            /* Final partial frame: silence-pad the unfilled full-rate tail to
             * prevent the resampler from consuming stale data. */
            if (realPerCh < 2 * FRAME_LEN)
                memset(fullRate + realPerCh, 0, (2 * FRAME_LEN - realPerCh) * sizeof(float));
            heHalfRate[channel] = rs->halfRate[channel];
        }

        /* Always the full padded frame, never [0, realPerCh): the grid unconditionally
         * claims SBR_NUM_TIME_SLOTS, so normalizing a short frame over fewer slots
         * would inflate its levels, and the QMF-overlap save below reads the last
         * SBR_QMF_OVL_LEN_64 samples -- behind the buffer for a short frame. */
        if (PsIsActive(sCtx->sbrInfo)) PsPrepareCarrier(sCtx, fullPtrs);
        SbrAnalyze(sCtx->signalAnalysis, fullPtrs, numChannels, isLfe, 2 * FRAME_LEN, sCtx->sbrInfo);
        SbrEncode(sCtx->sbrInfo, fullPtrs, numChannels, isLfe, 2 * FRAME_LEN, sCtx->signalAnalysis, fd);
        /* Dual-rate decimation: produces the halved-rate core signal. */
        if (PsIsActive(sCtx->sbrInfo)) PsCommitCarrier(sCtx, rs->fullRate[0]);
        Resample(rs, 2 * FRAME_LEN);
    }
}

/* Dual-rate SBR: the AAC core encodes at Fs/2 while SBR reconstructs the top
 * octave back to the full rate. Halve the core rate here; the full rate is kept
 * in the context for SBR and the ASC. */
void SbrContextResolveRate(SBRContext *sCtx, unsigned long *sampleRate, unsigned int *sampleRateIdx, SR_INFO **srInfoPtr)
{
    if (sCtx->fullSampleRate == 0) {
        sCtx->fullSampleRate     = *sampleRate;
        sCtx->fullSampleRateIdx  = *sampleRateIdx;
        *sampleRate         = *sampleRate / 2;
        *sampleRateIdx      = GetSRIndex(*sampleRate);
        *srInfoPtr          = &srInfo[*sampleRateIdx];
    }
}

int SbrContextIsPresent(SBRContext *sCtx)
{
    return (sCtx && sCtx->sbrInfo) ? 1 : 0;
}

/* Optimized log2 approximation for energy-to-decibel conversion.
 * Precision is sufficient for the 1.5/3.0 dB envelope quantizer. */
#define FAST_LOG2_A         1.3424f
#define FAST_LOG2_B         0.3427f
#define FAST_LOG2_MANT_NORM (1.0f / (1 << 23))  /* 23-bit mantissa → [0, 1) */
static inline float fast_log2(float x)
{
    union { float f; int32_t i; } vx;
    vx.f = (float)x;
    int32_t exp = (vx.i >> 23) & 0xFF;
    float m = (float)(vx.i & 0x7FFFFF) * FAST_LOG2_MANT_NORM;
    return (float)(exp - 127) + (float)(m * (FAST_LOG2_A - FAST_LOG2_B * m));
}

/* 64-band subband analysis using a 64-point complex FFT.
 * Leverages conjugate symmetry to extract two 64-point real-subsequence
 * DFTs from one complex transform, reducing FLOPs by ~50% compared to
 * a standard 128-point implementation. y receives the FFT spectrum. */
static inline void qmf_transform(const SBRInfo *sbr, const float *ovl_pos, float *y)
{
    float x[128];
    float * restrict xr = x, * restrict xi = x + 64;
    /* The filter is symmetric except at offsets 128/512 and 256/384.
     * Compute the contiguous polyphase outputs before the complex rotation,
     * using the FFT output buffer as workspace. */
    for (int n = 0; n < 128; n++) {
        y[n] = qmf_c[n] * ovl_pos[639 - n]
             + qmf_c[128 + n] * ovl_pos[511 - n]
             + qmf_c[256 + n] * ovl_pos[383 - n]
             + qmf_c[256 - n] * ovl_pos[255 - n]
             + qmf_c[128 - n] * ovl_pos[127 - n];
    }
    /* Tap zero uses the two sign exceptions to the mirror rule. */
    y[0] = qmf_c[0] * ovl_pos[639]
         + qmf_c[128] * ovl_pos[511]
         + qmf_c[256] * ovl_pos[383]
         + (-qmf_c[256]) * ovl_pos[255]
         + (-qmf_c[128]) * ovl_pos[127];
    for (int m = 0; m < 64; m++) {
        float a = y[2 * m], b = y[2 * m + 1];
        xr[m] = a * sbr->twidCos[m] - b * sbr->twidSin[m];
        xi[m] = -(a * sbr->twidSin[m] + b * sbr->twidCos[m]);
    }
    fft(x, y, FFT_LOGM_SHORT);
}

/* Band k of the complex QMF output. */
static inline void qmf_band(const SBRInfo *sbr, const float *y, int k, float *re, float *im)
{
    const float * restrict yr = y, * restrict yi = y + 64;
    int kr = 63 - k;
    /* Separate the two real-subsequence DFTs by conjugate symmetry. */
    float Ar = 0.5f * (yr[k] + yr[kr]);
    float Ai = 0.5f * (yi[kr] - yi[k]);
    float Br = -0.5f * (yi[k] + yi[kr]);
    float Bi = 0.5f * (yr[kr] - yr[k]);
    /* Sr = Ar + w_k_real * Br - w_k_imag * Bi
     * Si = Ai + w_k_real * Bi + w_k_imag * Br */
    float wr = sbr->oddCos[k];
    float wi = sbr->oddSin[k];
    float Sr = Ar + wr * Br - wi * Bi;
    float Si = Ai + wr * Bi + wi * Br;
    *re = Sr;
    *im = Si;
}

#if FAAC_ENCODER_PS
void SbrQmfAnalysisComplex(SBRInfo *sbr, const float *ovl_pos, float *real, float *imag, int kx, int k2)
{
    float y[128];
    qmf_transform(sbr, ovl_pos, y);
    for (int k = kx; k < k2; k++) qmf_band(sbr, y, k, &real[k], &imag[k]);
}
#endif

/* The SBR bitstream only transmits envelope magnitudes, so the energy path
 * discards phase. */
void SbrQmfAnalysis(SBRInfo *sbr, const float * restrict ovl_pos, float * restrict energy, int kx, int k2)
{
    float y[128];
    qmf_transform(sbr, ovl_pos, y);
    for (int k = kx; k < k2; k++) {
        float Sr, Si;
        qmf_band(sbr, y, k, &Sr, &Si);
        energy[k] += Sr * Sr + Si * Si;
    }
}

static void sbr_adopt_envelope_grid(const struct SignalAnalysis *sa, SbrFrameData *fd)
{
    fd->numEnvelopes = sa->numEnvelopes;
    fd->frameClass   = sa->frameClass;
    for (int i = 0; i <= sa->numEnvelopes; i++) fd->tEnv[i] = sa->tEnv[i];
    fd->eff_amp_res = (fd->numEnvelopes == 1) ? 0 : SBR_AMP_RES;
}

static void sbr_quantize_envelopes(const SBRInfo *sbr, int nch, const bool *isLfe,
                                   const struct SignalAnalysis *sa, SbrFrameData *fd)
{
#if MAX_CHANNELS < 6
    (void)isLfe;
#endif
    int n_env = fd->numEnvelopes;
    /* Must match write_sbr_envelope's table, or the decoder desyncs. */
    int nb = sbr->numBands;
    const int *edges = sbr->bandEdges;

    for (int ch = 0; ch < nch; ch++) {
#if MAX_CHANNELS >= 6
        if (isLfe[ch]) continue;
#endif
        /* Read-only alias; the quantizer never writes back through it. */
        const float (* restrict bandE)[SBR_QMF_BANDS_64] = sa->ch[ch].bandE;
        int dlav = fd->eff_amp_res ? SBR_ENV_DELTA_LIMIT_HIRES : SBR_ENV_DELTA_LIMIT_LORES;
        for (int e = 0; e < n_env; e++) {
            int prevLevel = -1;
            for (int b = 0; b < nb; b++) {
                int k_lo = edges[b], k_hi = edges[b+1];
                /* Weight energy by the number of QMF slots per envelope to
                 * maintain normalized power levels across variable borders. */
                int e_slots = sa->envSampled[e];
                if (e_slots < 1) e_slots = 1;
                float E = 0;
                for (int k = k_lo; k < k_hi; k++) E += bandE[e][k];
                E /= (float)(e_slots * (k_hi - k_lo));
                float factor = fd->eff_amp_res ? 1.0f : 2.0f;
                int level = lrintf(factor * (fast_log2(E + SBR_LOG_ENERGY_FLOOR) - SBR_ENV_LEVEL_LOG2_OFFSET));
                int raw_level = clamp_int(level, 0, 127);
                /* Clamped so the frequency-delta chain stays codable. */
                if (prevLevel < 0)
                    raw_level = clamp_int(raw_level, 0, fd->eff_amp_res ? 63 : 127);
                else
                    raw_level = clamp_int(raw_level, prevLevel - dlav, prevLevel + dlav);
                fd->ch[ch].envData[e][b] = prevLevel = raw_level;
            }
        }
    }
}

void SbrEncode(SBRInfo *sbr, float *timeDomain[MAX_CHANNELS], int numChannels, const bool *isLfe, int numSamples, struct SignalAnalysis *sa, SbrFrameData *fd)
{
#if MAX_CHANNELS < 6
    (void)isLfe;
#endif
    for (int ch = 0; ch < numChannels; ch++)
#if MAX_CHANNELS >= 6
        if (!isLfe[ch])
#endif
            memcpy(sbr->ch[ch].qmfOvl64, timeDomain[ch] + numSamples - SBR_QMF_HIST_LEN, SBR_QMF_HIST_LEN * sizeof(float));

    sbr_adopt_envelope_grid(sa, fd);
    int coreChannels = numChannels;
    if (PsIsActive(sbr)) {
        PsAnalyze(sbr, sa, fd);
        coreChannels = 1;
    }
    sbr_quantize_envelopes(sbr, coreChannels, isLfe, sa, fd);

#ifdef FAAC_STATS
    g_faacStats.sbrFrames++;
    if (fd->frameClass != SBR_FRAME_CLASS_FIXFIX) {
        g_faacStats.sbrTransientFrames++;
    }
    for (int ch = 0; ch < numChannels; ch++) {
#if MAX_CHANNELS >= 6
        if (isLfe[ch]) continue;
#endif
        g_faacStats.sbrInvfSum += sbr->rich ? SBR_INVF_MODE_RICH : SBR_INVF_MODE;
        g_faacStats.sbrInvfCount++;
    }
#endif
}

/* SBR bitstream writer. Emits the SBR fill element payload into the bitstream.
 * Replays the write sequence into a counting sink during rate control to
 * ensure accurate bit budget allocation. */
