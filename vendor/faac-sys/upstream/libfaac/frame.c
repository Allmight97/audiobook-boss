/*
 * FAAC - Freeware Advanced Audio Coder
 * Copyright (C) 2001 Menno Bakker
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

#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#include <string.h>
#include <math.h>

#include "frame.h"
#include "ps.h"
#include "coder.h"
#include "channels.h"
#include "bitstream.h"
#include "filtbank.h"
#include "quantize.h"
#include "util.h"
#include "tns.h"
#include "stereo.h"
#include "sbr.h"
#include "ratecontrol.h"
#include "atomic.h"
#include "endian.h"

/* HE-AAC auto-mode thresholds; measured with ViSQOL and Zimtohrli. */
#define HE_MIN_SAMPLE_RATE    32000  /* Fs/2 < 16 kHz below this → core too narrow for SBR */
/* HE only wins harder as the rate falls: the further below Nyquist the LC core
 * lands, the more spectrum SBR is rescuing. 8000 is HE-AAC's design floor and
 * the lowest rate measured. */
#define HE_MIN_BITRATE_PER_CH 8000
/* Stereo measurements put the HE/LC crossover at 32 kb/s/ch for 32 kHz and
 * above 48 kb/s/ch for 48 kHz. Use one bit/s per channel per Hz between those
 * rates, capped at 48 kb/s/ch above 48 kHz. Multichannel uses the same ceiling. */
#define HE_CEILING_FULL_FS 48000
/* Frozen, not derived: quantqual doesn't map onto a bitrate ceiling cleanly
 * (the two are off by 2-4.5x across the range), so this is set by measurement.
 * At matched bitrate HE leads LC well past this, further than under ABR,
 * because LC's VBR bandwidth doesn't narrow with -q. Re-measure it with a -q
 * sweep whenever the ABR crossover or the LC VBR bandwidth moves. */
#define HE_VBR_QUANTQUAL_MAX  75
/* bps per channel; below it the HE core codes every frame short. */
#define HE_SHORT_ONLY_BITRATE 14000
/* HE-v1's gate for -q, which has no bitrate: long windows lose at -q 30
 * (the 24 kbps stereo rung) and win at -q 50 (32 kbps); this is the midpoint. */
#define HE_SHORT_ONLY_QUANTQUAL 40

/* Top of the bandwidth curve: widening past it loses at every reachable rate,
 * the band above holds ~0.006% of program energy and sits at the edge of
 * hearing. VBR, having no rate, codes at the top. */
#define BANDWIDTH_CEILING     18750

/* In the encoder's 16-bit sample units, so 256 times unity full scale; anything
 * beyond it is not audio. */
#define FLOAT_INPUT_LIMIT     8388608.0f

#if (defined WIN32 || defined _WIN32 || defined WIN64 || defined _WIN64) && !defined(PACKAGE_VERSION)
#include "win32_ver.h"
#endif

/* Bounds on the peak limiter's quality scale factor: the ceiling guarantees
 * each retry makes progress, the floor keeps one outsized frame from
 * collapsing quality to MINQUAL in a single step. */
#define PEAK_BACKOFF_CEILING   0.85f
#define PEAK_BACKOFF_FLOOR     0.10f
#define PEAK_MAX_RETRIES       12

static char *libfaacName = PACKAGE_VERSION;
static char *libCopyright =
  "FAAC - Freeware Advanced Audio Coder (https://freewareadvancedaudio.github.io)\n"
  " Copyright (C) 1999-2001, Menno Bakker\n"
  " Copyright (C) 2002-2017, Krzysztof Nikiel\n"
  " Copyright (C) 2004, Dan Villiom P. Christiansen\n"
  " Copyright (C) 2005-2026, Fabian Greffrath\n"
  " Copyright (C) 2026, Nils Schimmelmann\n";

static unsigned int CalcBandwidth(unsigned long bitRate)
{
    /* Anchors land on long-block band edges, so CalcBW()'s snap is a no-op and
     * these constants are the cutoff you actually get. Divisors are powers of
     * two. LC in effect: HE reaches here too, but faacEncApplyConfig then
     * overwrites bandWidth with the SBR crossover, which the core must meet. */
    if (bitRate <= 12000) {
        /* Unmeasured below here; ramp rather than extend the plateau down.
         * bitRate is unsigned, so guard the subtraction. */
        return (bitRate > 8000) ? 9250 + ((bitRate - 8000) * 5 / 4) : 9250;
    }
    if (bitRate <= 32000) {
        /* Flat by measurement: the optimum does not move across this range. */
        return 14250;
    }
    if (bitRate <= 48000)
        return 14250 + ((bitRate - 32000) * 9 / 32);
    return BANDWIDTH_CEILING;
}

/* Element-to-channel mapping is fixed for the session once InitElements has
 * run, so cache which channels are LFE here instead of rescanning
 * hEncoder->elements[] for every channel on every frame. */
static void RefreshLfeMap(faacEncStruct *hEncoder)
{
    memset(hEncoder->isLfeChannel, 0, sizeof(hEncoder->isLfeChannel));
#if MAX_CHANNELS >= 6
    for (int e = 0; e < hEncoder->numElements; e++) {
        if (hEncoder->elements[e].type == ID_LFE)
            hEncoder->isLfeChannel[hEncoder->elements[e].channels[0]] = true;
    }
#else
    (void)hEncoder;
#endif
}

int faacEncGetVersion( char **faac_id_string,
			      				char **faac_copyright_string)
{
  if (faac_id_string)
    *faac_id_string = libfaacName;

  if (faac_copyright_string)
    *faac_copyright_string = libCopyright;

  return FAAC_CFG_VERSION;
}


/* The configuration is fixed at open, so LC and HE share one non-allocating
 * writer. The longest ASC is mono HE: 16 core + 21 SBR + 12 PS bits. */
_Static_assert(sizeof(((faacEncStruct *)0)->asc) * 8 >= 16 + 21 + 12,
               "ASC buffer must hold the mono HE extensions");
#if MAX_CHANNELS >= 8
/* An eight-channel ASC carries the PCE before the SBR extension. */
_Static_assert(sizeof(((faacEncStruct *)0)->asc) >= PCE_HE_ASC_BYTES,
               "ASC buffer must hold the PCE and SBR extensions");
_Static_assert(sizeof(((faacEncStruct *)0)->pce) >= PCE_ADTS_BYTES,
               "PCE buffer must hold the in-band element");
#endif

int faacEncGetDecoderSpecificInfo(faacEncHandle hpEncoder, unsigned char *buf)
{
    faacEncStruct *hEncoder = (faacEncStruct *)hpEncoder;
    BitStream bs;

    if (hEncoder == NULL || buf == NULL)
        return -1;
    if (hEncoder->config.mpegVersion == MPEG2)
        return -2; /* not supported */

    int he = IsSBR(hEncoder->config.aacObjectType) && hEncoder->sbrContext;
    InitBitStream(&bs, buf, sizeof(hEncoder->asc));
    PutBit(&bs, he ? LOW : hEncoder->config.aacObjectType, 5);
    PutBit(&bs, hEncoder->sampleRateIdx, 4);
    PutBit(&bs, SignaledChannelConfig(CoreChannels(hEncoder->config.aacObjectType, (int)hEncoder->numChannels)), 4);
    PutBit(&bs, 0, 3); /* frameLengthFlag, dependsOnCoreCoder, extensionFlag */
#if MAX_CHANNELS >= 8
    if (HasPCE((int)hEncoder->numChannels)) {
        /* Same PCE as the in-band image, minus the element id; copying it bit
         * by bit keeps a single copy of the field writer. */
        for (uint32_t i = LEN_SE_ID; i < LEN_SE_ID + PCE_BODY_BITS; i++)
            PutBit(&bs, (hEncoder->pce[i >> 3] >> (7 - (i & 7))) & 1, 1);
        PutBit(&bs, 0, (int)((8 - (bs.currentBit & 7)) & 7));
        PutBit(&bs, 0, 8);
    }
#endif
    if (he) {
        /* Backward-compatible sync extension: AAC-LC at Fs/2, SBR at full Fs. */
        PutBit(&bs, 0x2b7, 11);
        PutBit(&bs, HE_V1, 5);
        PutBit(&bs, 1, 1);
        PutBit(&bs, SbrContextGetFullRateIdx(hEncoder->sbrContext), 4);
        /* Explicit PS absence prevents decoders expanding mono to stereo. */
        if (hEncoder->numChannels == 1 || IsPS(hEncoder->config.aacObjectType)) {
            PutBit(&bs, 0x548, 11);
            PutBit(&bs, IsPS(hEncoder->config.aacObjectType), 1);
        }
    }
    return (int)((bs.currentBit + 7) / 8);
}


/* Called once on a fresh handle with parameters validated by the facade.
 * Resolve the profile and initialize only its final configuration. Returns 1
 * on success, 0 for an unsupported core rate, -1 on allocation failure. */
int faacEncApplyConfig(faacEncStruct* hEncoder)
{
    faacEncConfigurationPtr config = &hEncoder->config;
    int maxqual = config->outputFormat ? MAXQUALADTS : MAXQUAL;

    /* Resolve AUTO to LC or HE-AAC. HE-AAC wins for low rates, but only
     * at Fs >= 32 kHz so the Fs/2 core stays >= 16 kHz; below that the
     * narrow-band core + SBR reconstruction collapses. MPEG-2 resolves to
     * LC, since SBR is MPEG-4 only and AUTO must not overrule the version
     * the caller named. */
    if (hEncoder->config.aacObjectType == AUTO) {
        unsigned long rate_per_ch = config->bitRate;
        int rate_ok;
        if (rate_per_ch > 0) {
            /* The measured ceiling is one bit/s per channel per Hz through
             * 48 kHz, then remains fixed. */
            unsigned int max_he_rate = hEncoder->sampleRate < HE_CEILING_FULL_FS
                ? hEncoder->sampleRate : HE_CEILING_FULL_FS;
            rate_ok = (rate_per_ch >= HE_MIN_BITRATE_PER_CH && rate_per_ch <= max_he_rate);
        } else {
            rate_ok = (config->quantqual <= HE_VBR_QUANTQUAL_MAX);
        }
        hEncoder->config.aacObjectType =
            (FAAC_ENCODER_SBR && rate_ok && hEncoder->sampleRate >= HE_MIN_SAMPLE_RATE &&
             hEncoder->config.mpegVersion != MPEG2) ? HE_V1 : LOW;
#if FAAC_ENCODER_PS
        /* Explicit-bitrate stereo uses PS up to the ceiling, below HE v1's
         * floor too; just above it stays on HE v1 rather than falling to LC.
         * The ceiling is where v1 starts to win, which rises with the rate. */
        unsigned long ps_ceiling = hEncoder->sampleRate == 48000 ? 8000 :
            hEncoder->sampleRate == 44100 ? 7000 :
            hEncoder->sampleRate == 32000 ? 6000 : 0;
        if (ps_ceiling && hEncoder->numChannels == 2 &&
            hEncoder->config.mpegVersion != MPEG2 && rate_per_ch > 0) {
            if (rate_per_ch <= ps_ceiling)
                hEncoder->config.aacObjectType = HE_V2;
            else if (rate_per_ch < HE_MIN_BITRATE_PER_CH)
                hEncoder->config.aacObjectType = HE_V1;
        }
#endif
    }

    if (IsSBR(hEncoder->config.aacObjectType)
        && hEncoder->sampleRate < HE_MIN_SAMPLE_RATE)
        return 0;

    /* HE-AAC: encode the core as AAC-LC; SBR rebuilds the top octave. The core
     * runs dual-rate at Fs/2; the original rate is kept for SBR and the ASC.
     * (Single-rate SBR is not supported: decoders unconditionally reconstruct
     * the SBR band table from 2*core_rate, so a full-Fs core is undecodeable.) */
    if (IsSBR(hEncoder->config.aacObjectType)) {
        /* SBR is MPEG-4 only; an explicit HE-AAC request outranks the
         * requested version. */
        hEncoder->config.mpegVersion = MPEG4;
        hEncoder->sbrContext = SbrContextInit(hEncoder->numChannels);

        if (!hEncoder->sbrContext)
            return -1;

        SbrContextResolveRate(hEncoder->sbrContext, &hEncoder->sampleRate, &hEncoder->sampleRateIdx, &hEncoder->srInfo);
    }

    /* MaxBitrate() is already per channel, and its frame is FRAME_LEN samples
     * at the core rate -- so the clamp has to follow the HE-AAC resolution
     * above, which halves that rate. */
    unsigned int maxBitrate = MaxBitrate(hEncoder->sampleRate);
    if (config->bitRate > maxBitrate)
        config->bitRate = maxBitrate;

    /* TNS tables depend on the resolved core rate. */
    TnsInit(hEncoder);

    if (DrcInit(hEncoder) != 0)
        return -1;

    if (config->bitRate && !config->bandWidth)
    {
        config->bandWidth = CalcBandwidth(config->bitRate);

        if (!config->quantqual)
        {
            /* Scale initial quality seed by sample-rate frame duration factor (44100 / sampleRate)
             * so low sampling rates (e.g. 16 kHz) start at appropriate quality scale factors for
             * fast rate-control convergence on short audio clips. */
            float rateFactor = 44100.0f / (float)hEncoder->sampleRate;
            /* Precise target-bitrate quality seeding curve: maps bitRate to optimal initial quantqual
             * for rapid rate-control convergence without early overshoot or undershoot. */
            float bps = (float)config->bitRate;
            float q_seed;
            if (bps <= 16000.0f) {
                q_seed = 10.0f + 22.0f * (bps / 16000.0f);
            } else if (bps <= 64000.0f) {
                q_seed = 32.0f + 68.0f * ((bps - 16000.0f) / 48000.0f);
            } else {
                q_seed = bps / 640.0f;
            }
            /* Boost initial seed for mono speech streams */
            if (hEncoder->numChannels == 1 && bps >= 32000.0f) q_seed *= 2.5f;
            config->quantqual = q_seed * (float)hEncoder->numChannels * rateFactor;
            if (config->quantqual > DEFQUAL)
                config->quantqual = (config->quantqual - DEFQUAL) * 3.0f + DEFQUAL;
        }
    }

    if (!config->quantqual)
        config->quantqual = DEFQUAL;

    /* Only VBR reaches here without a bandwidth: ABR derived its own above.
     * Hi-res VBR at the default quality or higher keeps the full band so that
     * nothing above the ceiling is discarded; below it the ceiling saves bits.
     * ABR keeps the ceiling because the extra band costs bits at a fixed rate. */
    if (!config->bandWidth)
        config->bandWidth = (hEncoder->sampleRate > 48000 && config->quantqual >= DEFQUAL)
            ? (unsigned int)(hEncoder->sampleRate / 2) : BANDWIDTH_CEILING;

    // check bandwidth
    if (hEncoder->config.bandWidth < 100)
		hEncoder->config.bandWidth = 100;
    if (hEncoder->config.bandWidth > (hEncoder->sampleRate / 2))
		hEncoder->config.bandWidth = hEncoder->sampleRate / 2;

    if (config->quantqual > (unsigned long)maxqual)
        config->quantqual = maxqual;
    if (config->quantqual < MINQUAL)
        config->quantqual = MINQUAL;

    if (config->mpegVersion == MPEG2)
        config->usePns = 0;

    {
        /* sampleRate is the core rate, halved above for HE-AAC. Narrowband mono
         * has few bands above the substitution threshold, and coding them
         * outright beats the noise the default level puts there. */
        unsigned long outputRate = hEncoder->sampleRate;
        if (IsSBR(hEncoder->config.aacObjectType))
            outputRate *= 2;
        hEncoder->aacquantCfg.pnslevel = !config->usePns ? 0 :
            (outputRate <= 16000 && hEncoder->numChannels == 1)
                ? PNSLEVEL_NARROWBAND_MONO : PNSLEVEL_DEFAULT;
    }
    /* set quantization quality */
    hEncoder->aacquantCfg.quality = config->quantqual;
    if (IsSBR(hEncoder->config.aacObjectType)) {
        SBRContext *sCtx = hEncoder->sbrContext;
        unsigned long sbr_bitrate = hEncoder->config.bitRate ? (hEncoder->config.bitRate * hEncoder->numChannels) : ((unsigned long)hEncoder->config.quantqual * 1280);
        if (!SbrContextUpdateConfig(sCtx, hEncoder->numChannels, sbr_bitrate,
                                    hEncoder->config.aacObjectType, hEncoder->config.bitRate != 0))
            return -1;
        /* kx * Fs / (2*64): each QMF band is Fs/(2*SBR_QMF_BANDS_64) Hz wide.
         * Matching core bandwidth to the SBR crossover avoids a gap or overlap. */
        hEncoder->config.bandWidth = SbrContextGetXOverBandwidth(sCtx);
    }

    /* Input FIFO: holds one frame plus up to one full incoming chunk of leftover.
     * HE-AAC frames are 2*FRAME_LEN (the dual-rate core runs at Fs/2), LC is
     * FRAME_LEN. Allocate for the resolved profile once. */
    {
        unsigned int cap = 2 * faacFrameSamples(hEncoder);
        unsigned int channel;
        for (channel = 0; channel < hEncoder->numChannels; channel++)
        {
            hEncoder->inputFifo[channel] =
                (float *)AllocMemory(cap * sizeof(float));
            if (!hEncoder->inputFifo[channel]) return -1;
        }
        hEncoder->inputFifoCap  = cap;
        /* HE-AAC's pipeline delay is 3*FRAME_LEN - 31 full-rate samples, an odd
         * count the core-rate container timescale cannot express. One zero
         * sample ahead of the stream makes it even, so gapless trimming is
         * exact; faacEncoderDelay reports the padded figure. */
        hEncoder->inputFifoFill = 0;
        if (IsSBR(hEncoder->config.aacObjectType)) {
            for (channel = 0; channel < hEncoder->numChannels; channel++)
                hEncoder->inputFifo[channel][0] = 0.0f;
            hEncoder->inputFifoFill = 1;
        }
    }

    /* Peak-limiter retry scratch: allocated for all encoders to enforce ISO 6144 bits/ch frame ceiling. */
    {
        unsigned int ch;
        for (ch = 0; ch < CoreChannels(hEncoder->config.aacObjectType, hEncoder->numChannels); ch++) {
            hEncoder->peakSnap[ch] = (unsigned char *)AllocMemory(MAX_SCFAC_BANDS * (sizeof(uint8_t) + sizeof(int)));
            if (!hEncoder->peakSnap[ch])
                return -1;
        }
    }

    CalcBW(&hEncoder->config.bandWidth,
              hEncoder->sampleRate,
              hEncoder->srInfo,
              &hEncoder->aacquantCfg,
              hEncoder->sfbOffsetShort,
              hEncoder->sfbOffsetLong);

    {
        const int *sfbOffset[2] = { hEncoder->sfbOffsetLong, hEncoder->sfbOffsetShort };
        const int  sfbn[2]      = { hEncoder->aacquantCfg.max_cbl, hEncoder->aacquantCfg.max_cbs };
        QuantizeConfigureMask(&hEncoder->aacquantCfg, hEncoder->sampleRate,
                              hEncoder->config.bitRate, hEncoder->numChannels,
                              IsSBR(hEncoder->config.aacObjectType));
        StereoConfigure(&hEncoder->stereoCfg, (JointMode)hEncoder->config.jointmode, hEncoder->sampleRate,
                        hEncoder->config.bitRate,
                        CoreChannels(hEncoder->config.aacObjectType, hEncoder->numChannels),
                        IsSBR(hEncoder->config.aacObjectType), sfbOffset, sfbn);
    }

    /* Initialize psychoacoustic state for the resolved core rate. */
    if (!PsyInit(&hEncoder->gpsyInfo, hEncoder->psyInfo,
                 CoreChannels(hEncoder->config.aacObjectType, hEncoder->numChannels),
                 hEncoder->sampleRate, IsSBR(hEncoder->config.aacObjectType)))
        return -1;

    InitElements(hEncoder->elements, &hEncoder->numElements,
                 (int)CoreChannels(hEncoder->config.aacObjectType, hEncoder->numChannels),
                 hEncoder->config.useLfe);
    RefreshLfeMap(hEncoder);

    RateControlReset(&hEncoder->rc,
                     CoreChannels(hEncoder->config.aacObjectType, hEncoder->numChannels),
                     CoreBitrate(hEncoder->config.aacObjectType, hEncoder->config.bitRate),
                     hEncoder->sampleRate, hEncoder->config.rateControl == RATE_CBR);

#if MAX_CHANNELS >= 8
    /* Both the ASC and the in-band repeats copy this image, so it must exist
     * whether or not the caller asks for the ASC. */
    if (HasPCE((int)hEncoder->numChannels))
        BuildPCE(hEncoder->pce, hEncoder->sampleRateIdx);
#endif

    return 1;
}

#ifdef FAAC_STATS
faacEncStats g_faacStats;
#endif

/* Lookup tables every handle reads but none owns. Independent handles may be
 * opened from several threads at once, so the build is a one-shot. */
static faac_once_t shared_tables_once = FAAC_ONCE_INIT;

static void InitSharedTables(void)
{
    fft_init();
    FilterBankTablesInit();
    QuantizeInit();
}

faacEncHandle faacEncOpen(unsigned long sampleRate,
                                  unsigned int numChannels,
                                  unsigned long *inputSamples,
                                  unsigned long *maxOutputBytes)
{
    faac_once_run(&shared_tables_once, InitSharedTables);

#ifdef FAAC_STATS
    memset(&g_faacStats, 0, sizeof(faacEncStats));
    RateControlStatsInit();
#endif
    unsigned int channel;
    faacEncStruct* hEncoder;

    *inputSamples = FRAME_LEN*numChannels;
    *maxOutputBytes = ADTS_FRAMESIZE;

    size_t handleSize = sizeof(faacEncStruct) + numChannels * sizeof(CoderInfo);
    hEncoder = (faacEncStruct*)AllocMemory(handleSize);
    if (!hEncoder) return NULL;
    memset(hEncoder, 0, handleSize);

    hEncoder->numChannels = numChannels;
    hEncoder->sampleRate = sampleRate;
    hEncoder->sampleRateIdx = GetSRIndex(sampleRate);

    /* Identity map; faac_encoder_open() sets every other config field. */
	for( channel = 0; channel < MAX_CHANNELS; channel++ )
		hEncoder->config.channel_map[channel] = channel;

    /* find correct sampling rate depending parameters */
    hEncoder->srInfo = &srInfo[hEncoder->sampleRateIdx];

    for (channel = 0; channel < numChannels; channel++)
	{
        int buf;
        hEncoder->coderInfo[channel].block_type = ONLY_LONG_WINDOW;
        hEncoder->coderInfo[channel].groups.n = 1;
        hEncoder->coderInfo[channel].groups.len[0] = 1;

        for (buf = 0; buf < 4; buf++) {
            hEncoder->audioFIFO[channel][buf] = (float*)AllocMemory(FRAME_LEN*sizeof(float));
            if (!hEncoder->audioFIFO[channel][buf])
            {
                faacEncClose(hEncoder);
                return NULL;
            }
            memset(hEncoder->audioFIFO[channel][buf], 0, FRAME_LEN*sizeof(float));
        }
    }

    /* Rate-dependent state is initialized by ApplyConfig after resolution.
     * Close accepts the zeroed psychoacoustic state if allocation fails here. */
    if (!FilterBankInit(hEncoder)) {
        faacEncClose(hEncoder);
        return NULL;
    }

    /* Return handle */
    return hEncoder;
}


/* Append the caller's (interleaved) input to the per-channel input FIFO,
 * de-interleaving and converting to float once here so the rest of the
 * encoder is agnostic to the input format. samplesInput may be any count that
 * fits the FIFO; returns -1 on overflow or an invalid format. */
static int appendInputFifo(faacEncStruct *hEncoder, int32_t *inputBuffer,
                           unsigned int samplesInput)
{
    unsigned int numChannels = hEncoder->numChannels;
    unsigned int spch = samplesInput / numChannels;
    unsigned int channel, i;

    if (spch == 0) return 0;
    if (hEncoder->inputFifoFill + spch > hEncoder->inputFifoCap) return -1;

    for (channel = 0; channel < numChannels; channel++) {
        float *dst = hEncoder->inputFifo[channel] + hEncoder->inputFifoFill;
        switch (hEncoder->config.inputFormat) {
            case INPUT_16BIT: {
                short *src = (short *)inputBuffer + hEncoder->config.channel_map[channel];
                for (i = 0; i < spch; i++) { dst[i] = (float)*src; src += numChannels; }
                break;
            }
            case INPUT_24BIT: {
                const uint8_t *src_base = (const uint8_t *)inputBuffer;
                for (i = 0; i < spch; i++) {
                    const uint8_t *src = src_base + (i * numChannels + hEncoder->config.channel_map[channel]) * 3;
                    int32_t s = read_pcm24(src, FAAC_BIG_ENDIAN);
                    dst[i] = (1.0f / 256.0f) * (float)s;
                }
                break;
            }
            case INPUT_32BIT: {
                int32_t *src = (int32_t *)inputBuffer + hEncoder->config.channel_map[channel];
                for (i = 0; i < spch; i++) { dst[i] = (1.0f/256) * (float)*src; src += numChannels; }
                break;
            }
            case INPUT_FLOAT: {
                float *src = (float *)inputBuffer + hEncoder->config.channel_map[channel];
                for (i = 0; i < spch; i++) { dst[i] = 32768.0f * *src; src += numChannels; }
                /* Not audio: silence NaN, Inf and anything past the limit. A
                 * separate pass because the conditional store keeps the
                 * compiler from unrolling it. */
                for (i = 0; i < spch; i++)
                    if (!(fabsf(dst[i]) < FLOAT_INPUT_LIMIT))
                        dst[i] = 0.0f;
                break;
            }
            default: return -1;
        }
    }
    hEncoder->inputFifoFill += spch;
    return 0;
}

/* Drop n samples/channel from the front of the FIFO, shifting the leftover down. */
static void consumeInputFifo(faacEncStruct *hEncoder, unsigned int n)
{
    unsigned int numChannels = hEncoder->numChannels;
    unsigned int channel, rem;

    if (n > hEncoder->inputFifoFill) n = hEncoder->inputFifoFill;
    rem = hEncoder->inputFifoFill - n;
    if (rem)
        for (channel = 0; channel < numChannels; channel++)
            memmove(hEncoder->inputFifo[channel], hEncoder->inputFifo[channel] + n, rem * sizeof(float));
    hEncoder->inputFifoFill = rem;
}

int faacEncClose(faacEncHandle hpEncoder)
{
    faacEncStruct* hEncoder = (faacEncStruct*)hpEncoder;
    unsigned int channel;

    if (!hEncoder) return 0;

#ifdef FAAC_STATS
    if (g_faacStats.totalFrames > 0)
    {
        double qavg = g_faacStats.totalQuality / g_faacStats.totalFrames;
        double tr = 100.0 * g_faacStats.transientFrames / g_faacStats.totalFrames;
        double tns = g_faacStats.longBlocks > 0 ? 100.0 * g_faacStats.longBlocksTNS / g_faacStats.longBlocks : 0.0;
        double ms = g_faacStats.totalBands > 0 ? 100.0 * g_faacStats.msBands / g_faacStats.totalBands : 0.0;
        double is = g_faacStats.totalBands > 0 ? 100.0 * g_faacStats.isBands / g_faacStats.totalBands : 0.0;
        double pns = g_faacStats.totalBands > 0 ? 100.0 * g_faacStats.pnsBands / g_faacStats.totalBands : 0.0;
        double att_avg = g_faacStats.attackCount > 0 ? g_faacStats.totalAttack / g_faacStats.attackCount : 0.0;
        float att_max = g_faacStats.maxAttack;

        fprintf(stderr, "\n--- Encoder Diagnostics ---\n");
        fprintf(stderr, " Quality             : Qavg    = %6.2f\n", qavg);

        if (g_faacStats.sbrFrames > 0)
        {
            double sbr_tr = 100.0 * g_faacStats.sbrTransientFrames / g_faacStats.sbrFrames;
            fprintf(stderr, " Transients & Grid   : Core Tr = %5.1f%% (%u/%u) | SBR Grid = %5.1f%% Var | Attack = %.1fx avg, %.1fx max\n",
                    tr, g_faacStats.transientFrames, g_faacStats.totalFrames, sbr_tr, att_avg, att_max);
        }
        else
        {
            fprintf(stderr, " Transients & Grid   : Core Tr = %5.1f%% (%u/%u) | Attack = %.1fx avg, %.1fx max\n",
                    tr, g_faacStats.transientFrames, g_faacStats.totalFrames, att_avg, att_max);
        }

        if (g_faacStats.shortChannels > 0)
        {
            double grp_avg = (double)g_faacStats.shortGroupSum / g_faacStats.shortChannels;
            double split = 100.0 * g_faacStats.shortSplitChannels / g_faacStats.shortChannels;
            fprintf(stderr, " Short Grouping      : Groups  = %5.2f avg/ch | Split = %5.1f%% of %u short ch\n",
                    grp_avg, split, g_faacStats.shortChannels);
        }


        if (g_faacStats.sbrFrames > 0)
        {
            double sbr_invf = g_faacStats.sbrInvfCount > 0 ? (double)g_faacStats.sbrInvfSum / g_faacStats.sbrInvfCount : 0.0;
            fprintf(stderr, " Tool Allocation     : M/S     = %5.1f%% | I/S = %5.1f%% | PNS = %5.1f%% | TNS = %5.1f%% | INVF = %.2f\n",
                    ms, is, pns, tns, sbr_invf);
        }
        else
        {
            fprintf(stderr, " Tool Allocation     : M/S     = %5.1f%% | I/S = %5.1f%% | PNS = %5.1f%% | TNS = %5.1f%%\n",
                    ms, is, pns, tns);
        }
        PsStatsPrint();
        RateControlStatsPrint(stderr);
        fprintf(stderr, "---------------------------\n");
    }
#endif

    PsyEnd(hEncoder->psyInfo, hEncoder->numChannels);
    FilterBankEnd(hEncoder);

    for (channel = 0; channel < hEncoder->numChannels; channel++)
	{
        int buf;
        for (buf = 0; buf < 4; buf++) {
            if (hEncoder->audioFIFO[channel][buf])
                FreeMemory(hEncoder->audioFIFO[channel][buf]);
        }
		if (hEncoder->inputFifo[channel])
			FreeMemory (hEncoder->inputFifo[channel]);
        if (hEncoder->peakSnap[channel])
            FreeMemory(hEncoder->peakSnap[channel]);
    }

    if (hEncoder->sbrContext) {
        SbrContextEnd(hEncoder->sbrContext);
        hEncoder->sbrContext = NULL;
    }

    DrcClose(hEncoder);

    FreeMemory(hEncoder);

    return 0;
}

/* HE-AAC per-frame front end: take one assembled full-rate frame from the FIFO
 * front (realPerCh real samples/ch, the rest silence-padded), run SBR analysis
 * on it, then 2:1 downsample to produce the AAC-LC core signal. The FIFO is not
 * consumed here; the caller drops the frame after the core has read heHalfRate.
 */
static void doHEAACFrame(faacEncStruct *hEncoder, unsigned int realPerCh,
                         float *heHalfRate[MAX_CHANNELS])
{
    SbrContextProcessFrame(hEncoder->sbrContext, hEncoder->numChannels, hEncoder->isLfeChannel, (int)realPerCh,
                           (int)hEncoder->flushFrame, hEncoder->inputFifo, heHalfRate);
}

/* Admission gate: TNS shapes noise along the temporal envelope, so a window
 * with no envelope discontinuity has nothing for it to do, but the LPC gate
 * only discovers that after normalization, autocorrelation and Levinson-Durbin
 * have run. Screening on the envelope first skips that work for frames headed
 * for rejection anyway.
 *
 * Scaled to PsyGetAttack's statistic (largest relative energy jump between
 * adjacent sub-blocks). Not portable to a different sub-block count/size --
 * the same transient reads as a smaller jump with fewer, longer sub-blocks. */
#define TNS_ATTACK_MIN 0.5f

int faacEncEncode(faacEncHandle hpEncoder,
                          int32_t *inputBuffer,
                          unsigned int samplesInput,
                          unsigned char *outputBuffer,
                          unsigned int bufferSize
                          )
{
    faacEncStruct* hEncoder = (faacEncStruct*)hpEncoder;
    unsigned int channel;
    int frameBytes;
    BitStream bitStream;

    CoderInfo *coderInfo = hEncoder->coderInfo;
    unsigned int numChannels = CoreChannels(hEncoder->config.aacObjectType, hEncoder->numChannels);
    unsigned int useTns = hEncoder->config.useTns;
    unsigned int shortctl = hEncoder->config.shortctl;
    /* A starved HE core can't afford a long window's scalefactors and
     * sections; the frequency resolution they buy loses to the bits.
     * PS concentrates the core bits in mono; forcing short windows in VBR
     * loses quality even at the bottom of its reachable bitrate range. */
    bool shortOnly = IsSBR(hEncoder->config.aacObjectType)
        && (hEncoder->config.bitRate
            ? CoreBitrate(hEncoder->config.aacObjectType, hEncoder->config.bitRate) < HE_SHORT_ONLY_BITRATE
            : !IsPS(hEncoder->config.aacObjectType)
                && hEncoder->config.quantqual < HE_SHORT_ONLY_QUANTQUAL);
    hEncoder->gpsyInfo.needBass = shortOnly && !IsPS(hEncoder->config.aacObjectType);
    if (IsPS(hEncoder->config.aacObjectType) && shortOnly)
        shortctl = SHORTCTL_NOLONG;
    int maxqual = hEncoder->config.outputFormat ? MAXQUALADTS : MAXQUAL;

    /* The input FIFO decouples the caller's chunk size from the encoder frame
     * size: append whatever we were handed, then emit at most one frame. A frame
     * is mult*FRAME_LEN samples/channel (mult==2 for HE-AAC, whose dual-rate core
     * runs at Fs/2; 1 for LC). While fewer than a full frame is
     * buffered we just return 0 without touching any per-frame state, so the
     * encoder behaves identically regardless of the caller's chunk size. */
    unsigned int frameSamplesPerCh = faacFrameSamples(hEncoder);
    int flushing = (samplesInput == 0 || inputBuffer == NULL);

    /* SBR's coded-payload ring (frameFIFO) trails the core FIFO by one
     * extra tick, so HE-AAC needs one more flush tick than LC to drain. */
    unsigned int flushBudget = (IsSBR(hEncoder->config.aacObjectType)) ?
        SBR_FRAME_FIFO : (LOOKAHEAD_DEPTH + 1);

    if (samplesInput > 0 && inputBuffer != NULL)
    {
        if (appendInputFifo(hEncoder, inputBuffer, samplesInput) < 0)
            return -1;
    }

    /* A 0-byte return is ambiguous during end-of-stream flushing. While flushing,
     * absorb no-output pipeline priming ticks internally until an encoded frame
     * is produced or core lookahead delay is fully drained. */
    do {
        int realPerCh;          /* real (non-padded) input samples/ch in this frame */
        if (hEncoder->inputFifoFill >= frameSamplesPerCh)
            realPerCh = (int)frameSamplesPerCh;           /* full frame ready */
        else if (flushing && hEncoder->inputFifoFill > 0)
            realPerCh = (int)hEncoder->inputFifoFill;     /* final partial frame */
        else if (flushing)
            realPerCh = 0;                                /* drain core lookahead */
        else
            return 0;                                     /* accumulating */

        /* Increase frame number */
        hEncoder->frameNum++;

        /* A pure (FIFO-empty) flush frame pushes silence to drain the core's
         * algorithmic delay; a final partial frame still carries real samples and is
         * counted like a data frame, matching the pre-FIFO behavior. */
        if (realPerCh == 0)
            hEncoder->flushFrame++;

        if (hEncoder->flushFrame > flushBudget)
            return 0;

        /* Automatic DRC loudness measurement on incoming full-rate PCM frame in inputFifo */
        DrcProcessFrame(hEncoder, realPerCh > 0 ? hEncoder->inputFifo : NULL,
                        (uint32_t)(realPerCh > 0 ? realPerCh : 0));

        /* HE-AAC: run SBR + downsample first; the core then encodes heHalfRate.
         * Flush frames (realPerCh == 0) included -- the SBR payload runs
         * SBR_FRAME_FIFO-1 frames behind, so the pipeline has to keep ticking
         * through the drain or the tail access units re-emit stale envelopes. */
        float *heHalfRate[MAX_CHANNELS] = {0};
        if (IsSBR(hEncoder->config.aacObjectType) && SbrContextIsPresent(hEncoder->sbrContext))
            doHEAACFrame(hEncoder, (unsigned int)realPerCh, heHalfRate);

        /* Update current sample buffers */
        for (channel = 0; channel < numChannels; channel++)
        {
            float *tmp = hEncoder->audioFIFO[channel][FIFO_PAST];
            hEncoder->audioFIFO[channel][FIFO_PAST]   = hEncoder->audioFIFO[channel][FIFO_CURR];
            hEncoder->audioFIFO[channel][FIFO_CURR]   = hEncoder->audioFIFO[channel][FIFO_AHEAD1];
            hEncoder->audioFIFO[channel][FIFO_AHEAD1]  = hEncoder->audioFIFO[channel][FIFO_AHEAD2];
            hEncoder->audioFIFO[channel][FIFO_AHEAD2] = tmp;

            if (IsSBR(hEncoder->config.aacObjectType) && heHalfRate[channel])
            {
                /* ahead of the flush case: this carries the resampler's tail */
                memcpy(hEncoder->audioFIFO[channel][FIFO_AHEAD2], heHalfRate[channel], FRAME_LEN * sizeof(float));
            }
            else if (realPerCh == 0)
            {
                /* start flushing*/
                memset(hEncoder->audioFIFO[channel][FIFO_AHEAD2], 0, FRAME_LEN * sizeof(float));
            }
            else
            {
                /* LC: take one frame from the FIFO front (already float),
                 * silence-padding a short final frame. */
                unsigned int spc = ((unsigned int)realPerCh < FRAME_LEN) ? (unsigned int)realPerCh : FRAME_LEN;
                memcpy(hEncoder->audioFIFO[channel][FIFO_AHEAD2], hEncoder->inputFifo[channel], spc * sizeof(float));
                if (spc < FRAME_LEN)
                    memset(hEncoder->audioFIFO[channel][FIFO_AHEAD2] + spc, 0, (FRAME_LEN - spc) * sizeof(float));
            }

            /* LFE's block_type is always forced to ONLY_LONG_WINDOW in PsyCalculate,
             * and forced short windows neither switch nor run TNS, so the
             * transient analysis below would be discarded -- skip it. */
#if MAX_CHANNELS >= 6
            if (!hEncoder->isLfeChannel[channel] && shortctl != SHORTCTL_NOLONG)
#else
            if (shortctl != SHORTCTL_NOLONG)
#endif
            {
                PsyBufferUpdate(&hEncoder->gpsyInfo, &hEncoder->psyInfo[channel],
                    hEncoder->audioFIFO[channel][FIFO_AHEAD1],
                    hEncoder->audioFIFO[channel][FIFO_AHEAD2]);
            }
        }

        /* Drop the consumed frame from the FIFO front (both the LC copy and the
         * HE doHEAACFrame read the leading frameSamplesPerCh samples). */
        if (realPerCh > 0)
            consumeInputFifo(hEncoder, frameSamplesPerCh);

        if (hEncoder->frameNum > LOOKAHEAD_DEPTH)
            break;
    } while (flushing);

    if (!flushing && hEncoder->frameNum <= LOOKAHEAD_DEPTH) /* Still filling up the buffers */
        return 0;

    /* Psychoacoustics */
    PsyCalculate(hEncoder->psyInfo, hEncoder->isLfeChannel, numChannels);

    BlockSwitch(coderInfo, hEncoder->psyInfo, numChannels);

#ifdef FAAC_STATS
    g_faacStats.totalFrames++;
    if (coderInfo[0].block_type == ONLY_SHORT_WINDOW || coderInfo[0].block_type == LONG_SHORT_WINDOW)
    {
        g_faacStats.transientFrames++;
    }
#endif

    /* force block type; LFE must stay ONLY_LONG_SEQUENCE (ISO/IEC 14496-3) */
    for (channel = 0; channel < numChannels; channel++)
    {
#if MAX_CHANNELS >= 6
        if (hEncoder->isLfeChannel[channel] || shortctl == SHORTCTL_NOSHORT)
#else
        if (shortctl == SHORTCTL_NOSHORT)
#endif
            coderInfo[channel].block_type = ONLY_LONG_WINDOW;
        else if ((hEncoder->frameNum <= (LOOKAHEAD_DEPTH + 1)) || (shortctl == SHORTCTL_NOLONG))
            coderInfo[channel].block_type = ONLY_SHORT_WINDOW;
    }

    for (channel = 0; channel < numChannels; channel++) {
        if (coderInfo[channel].block_type == ONLY_SHORT_WINDOW) {
            coderInfo[channel].sfbn = hEncoder->aacquantCfg.max_cbs;
            coderInfo[channel].sfb_offset = hEncoder->sfbOffsetShort;
        } else {
            coderInfo[channel].sfbn = hEncoder->aacquantCfg.max_cbl;
            coderInfo[channel].sfb_offset = hEncoder->sfbOffsetLong;

            coderInfo[channel].groups.n = 1;
            coderInfo[channel].groups.len[0] = 1;
        }
    }

    /* Funnelled through one call site so BlocGroup stays a single inlined copy. */
    for (int e = 0; e < hEncoder->numElements; e++)
    {
        AACElement *el = &hEncoder->elements[e];
        int l = el->channels[0];
        int r = (el->type == ID_CPE) ? el->channels[1] : -1;
        CoderInfo *a = NULL, *b = NULL;
        float *xa = NULL, *xb = NULL;

        /* AAC Filterbank, MDCT with overlap and add */
        FilterBankElement(hEncoder, coderInfo, el);

        if (coderInfo[l].block_type == ONLY_SHORT_WINDOW)
        {
            a = &coderInfo[l];
            xa = hEncoder->freqBuff[l];
            if (r >= 0 && coderInfo[r].block_type == ONLY_SHORT_WINDOW)
            {
                b = &coderInfo[r];
                xb = hEncoder->freqBuff[r];
            }
        }
        else if (r >= 0 && coderInfo[r].block_type == ONLY_SHORT_WINDOW)
        {
            a = &coderInfo[r];
            xa = hEncoder->freqBuff[r];
        }

        if (a)
        {
            BlocGroup(a, xa, b, xb, &hEncoder->aacquantCfg);
#ifdef FAAC_STATS
            /* Everything downstream scales with groups.n * sfbn, so this one
             * number covers both the throughput and the bitrate axis. */
            {
                unsigned int nch = b ? 2 : 1;
                g_faacStats.shortChannels += nch;
                g_faacStats.shortGroupSum += (unsigned long)a->groups.n * nch;
                if (a->groups.n > 1)
                    g_faacStats.shortSplitChannels += nch;
            }
#endif
        }
    }

    /* Perform TNS analysis and filtering */
    for (channel = 0; channel < numChannels; channel++) {
#if MAX_CHANNELS >= 6
        if (!hEncoder->isLfeChannel[channel] && useTns && coderInfo[channel].block_type != ONLY_SHORT_WINDOW) {
#else
        if (useTns && coderInfo[channel].block_type != ONLY_SHORT_WINDOW) {
#endif
            float attack = PsyGetAttack(&hEncoder->psyInfo[channel]);

#ifdef FAAC_STATS
            if (attack > 0.0f && isfinite(attack)) {
                g_faacStats.totalAttack += attack;
                if (attack > g_faacStats.maxAttack) {
                    g_faacStats.maxAttack = attack;
                }
                g_faacStats.attackCount++;
            }
            g_faacStats.longBlocks++;
#endif

            /* No envelope yet (the first frames) means no basis to reject
               on, so admit and let the LPC gates decide. */
            if (attack > 0.0f && attack < TNS_ATTACK_MIN) {
                coderInfo[channel].tnsInfo.tnsDataPresent = 0;
                continue;
            }

            TnsEncode(&coderInfo[channel], hEncoder->freqBuff[channel]);
        } else {
            coderInfo[channel].tnsInfo.tnsDataPresent = 0;      /* TNS not used for LFE or short blocks */
        }
    }

#if MAX_CHANNELS >= 6
    for (channel = 0; channel < numChannels; channel++) {
        if (hEncoder->isLfeChannel[channel]) {
            /* CalcBW only initializes offsets through max_cbl. */
            int max_cbl = hEncoder->aacquantCfg.max_cbl;
            coderInfo[channel].sfbn = max_cbl < 3 ? max_cbl : 3;
        }
    }
#endif

    /* Clear each channel's section state before AACstereo pre-loads intensity
     * bands and BlocQuant resolves the rest. */
    for (channel = 0; channel < numChannels; channel++)
        ResetCoderSections(&coderInfo[channel]);

    AACstereo(coderInfo, hEncoder->elements, hEncoder->numElements, hEncoder->freqBuff,
              (float)hEncoder->aacquantCfg.quality/DEFQUAL, &hEncoder->stereoCfg);

    /* AACstereo has already consumed freqBuff in place and BlocQuant
     * accumulates into sf[] while reading book[], so a retry can re-run
     * neither. Snapshot what they produce -- book, sf, and the sfbn the CPE fix
     * below rewrites -- so a retry restarts from identical state. */
    unsigned long long peakBits = 0;
    float baseQuality = hEncoder->aacquantCfg.quality;
    int sfbnSnap[MAX_CHANNELS];
    int attempt;
    /* The raw_data_block includes periodic PCE bytes, so caps and reservoir
     * accounting charge them along with audio; only the ADTS header is excluded. */
    int hdrBytes = (hEncoder->config.outputFormat == 1) ? ADTS_HEADER_SIZE : 0;
    int payloadBits = 0;
#ifdef FAAC_STATS
    int resBound = 0;
#endif

    /* ISO/IEC 14496-3 standard frame limit: 6144 bits per channel */
    peakBits = (unsigned long long)numChannels * AAC_MAX_BITS_PER_CH;

    /* If output format is ADTS (outputFormat == 1), respect the 13-bit ADTS
     * container frame length limit (ADTS_MAX_FRAME_SIZE = 8191 bytes), less the header. */
    if (hEncoder->config.outputFormat == 1)
    {
        unsigned long long adtsPeakBits = (unsigned long long)(ADTS_MAX_FRAME_SIZE - ADTS_HEADER_SIZE) * 8;
        if (adtsPeakBits < peakBits)
            peakBits = adtsPeakBits;
    }

    if (hEncoder->config.maxBitRate)
    {
        /* maxBitRate is whole-stream, so no channel factor here. For HE-AAC
         * sampleRate is the halved core rate, which is what makes FRAME_LEN
         * cover the right span of output samples. */
        unsigned long long userPeakBits = (unsigned long long)hEncoder->config.maxBitRate
            * FRAME_LEN / hEncoder->sampleRate;
        if (userPeakBits < peakBits)
            peakBits = userPeakBits;
    }

    /* CBR bit reservoir: one more term in the min() of caps, enforced by the
       same retry loop, so a frame that fits costs nothing. */
    {
        int resAvail = RateControlFrameCap(&hEncoder->rc);
        if (resAvail && (unsigned long long)resAvail < peakBits)
        {
            peakBits = resAvail;
#ifdef FAAC_STATS
            resBound = 1;
#endif
        }
    }

    for (channel = 0; channel < numChannels; channel++) {
        memcpy(hEncoder->peakSnap[channel], coderInfo[channel].book,
               sizeof(coderInfo[channel].book));
        memcpy(hEncoder->peakSnap[channel] + MAX_SCFAC_BANDS, coderInfo[channel].sf,
               MAX_SCFAC_BANDS * sizeof(int));
        sfbnSnap[channel] = coderInfo[channel].sfbn;
    }

    /* Retry while the frame busts peakBits. The search is bounded, not
     * exact-fit: an exact fit can fail to terminate on pathological input. */
    for (attempt = 0; attempt <= PEAK_MAX_RETRIES; attempt++)
    {
        for (channel = 0; channel < numChannels; channel++) {
            BlocQuant(&coderInfo[channel], hEncoder->freqBuff[channel],
                      &(hEncoder->aacquantCfg));
        }

        /* A common window shares one max_sfb between the two channels. */
        for (channel = 0; channel < numChannels; channel++)
        {
            CoderInfo *cil = &coderInfo[channel], *cir = cil->partner;
            if (cir)
                cil->sfbn = cir->sfbn = max(cil->sfbn, cir->sfbn);
        }

        /* Write the AAC bitstream; the write doubles as the size probe. */
        InitBitStream(&bitStream, outputBuffer, bufferSize);

        if (WriteBitstream(hEncoder, coderInfo, hEncoder->elements, hEncoder->numElements, &bitStream) < 0) {
            return -1;
        }

        /* Round the written bits up to complete bytes. */
        frameBytes = (int)((bitStream.currentBit + 7) >> 3);
        payloadBits = (frameBytes - hdrBytes) * 8;

        if (!peakBits || (unsigned long long)payloadBits <= peakBits
            || hEncoder->aacquantCfg.quality <= MINQUAL)
            break;

        /* Aim at the budget rather than stepping down by a fixed factor: rate
         * control can park quality anywhere up to MAXQUAL (5000), and a fixed
         * halving needs far more passes to cross that to MINQUAL than any
         * sane retry budget. Frame bits grow sub-linearly with quality, so
         * scaling by the bit ratio undershoots the budget and converges in a
         * pass or two. */
        float scale = (float)peakBits / (float)payloadBits;
        if (scale > PEAK_BACKOFF_CEILING) scale = PEAK_BACKOFF_CEILING;
        if (scale < PEAK_BACKOFF_FLOOR)   scale = PEAK_BACKOFF_FLOOR;
        hEncoder->aacquantCfg.quality *= scale;
        if (hEncoder->aacquantCfg.quality < MINQUAL)
            hEncoder->aacquantCfg.quality = MINQUAL;

        for (channel = 0; channel < numChannels; channel++) {
            memcpy(coderInfo[channel].book, hEncoder->peakSnap[channel],
                   sizeof(coderInfo[channel].book));
            memcpy(coderInfo[channel].sf, hEncoder->peakSnap[channel] + MAX_SCFAC_BANDS,
                   MAX_SCFAC_BANDS * sizeof(int));
            coderInfo[channel].sfbn = sfbnSnap[channel];
        }
    }

    /* The caps are per frame, so the backoff must not outlive the frame: left
     * sticky, one hard frame drags the stream down (2% BD-rate), and with
     * bitRate == 0 nothing below ever claws the quality back. */
    hEncoder->aacquantCfg.quality = baseQuality;

#ifdef FAAC_STATS
    if (attempt > 0)
    {
        g_faacStats.peakRetryFrames++;
        if (resBound) g_faacStats.resBoundRetryFrames++;
    }
    g_faacStats.totalQuality += hEncoder->aacquantCfg.quality;
#endif

    /* Adjust quality to get correct average bitrate */
    if (hEncoder->config.bitRate)
        hEncoder->aacquantCfg.quality = RateControlUpdate(&hEncoder->rc, payloadBits,
                                                          hEncoder->aacquantCfg.quality, maxqual);

    return frameBytes;
}


/* Scalefactorband data table for 1024 transform length */
SR_INFO srInfo[12+1] =
{
    { 96000, 41, 12,
        {
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            8, 8, 8, 8, 8, 12, 12, 12, 12, 12, 16, 16, 24, 28,
            36, 44, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64
        },{
            4, 4, 4, 4, 4, 4, 8, 8, 8, 16, 28, 36
        }
    }, { 88200, 41, 12,
        {
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            8, 8, 8, 8, 8, 12, 12, 12, 12, 12, 16, 16, 24, 28,
            36, 44, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64, 64
        },{
            4, 4, 4, 4, 4, 4, 8, 8, 8, 16, 28, 36
        }
    }, { 64000, 47, 12,
        {
            4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
            8, 8, 8, 8, 12, 12, 12, 16, 16, 16, 20, 24, 24, 28,
            36, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40, 40,
            40, 40, 40, 40, 40
        },{
            4, 4, 4, 4, 4, 4, 8, 8, 8, 16, 28, 36
        }
    }, { 48000, 49, 14,
        {
            4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  8,  8,  8,  8,  8,  8,  8,
            12, 12, 12, 12, 16, 16, 20, 20, 24, 24, 28, 28, 32, 32, 32, 32, 32, 32,
            32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 96
        }, {
            4,  4,  4,  4,  4,  8,  8,  8, 12, 12, 12, 16, 16, 16
        }
    }, { 44100, 49, 14,
        {
            4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  8,  8,  8,  8,  8,  8,  8,
            12, 12, 12, 12, 16, 16, 20, 20, 24, 24, 28, 28, 32, 32, 32, 32, 32, 32,
            32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 96
        }, {
            4,  4,  4,  4,  4,  8,  8,  8, 12, 12, 12, 16, 16, 16
        }
    }, { 32000, 51, 14,
        {
            4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  8,  8,  8,  8,
            8,  8,  8,  12, 12, 12, 12, 16, 16, 20, 20, 24, 24, 28,
            28, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32, 32,
            32, 32, 32, 32, 32, 32, 32, 32, 32
        },{
            4,  4,  4,  4,  4,  8,  8,  8,  12, 12, 12, 16, 16, 16
        }
    }, { 24000, 47, 15,
        {
            4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  8,  8,  8,  8,  8,  8,  8,
            8,  8,  8,  12, 12, 12, 12, 16, 16, 16, 20, 20, 24, 24, 28, 28, 32,
            36, 36, 40, 44, 48, 52, 52, 64, 64, 64, 64, 64
        }, {
            4,  4,  4,  4,  4,  4,  4,  8,  8,  8, 12, 12, 16, 16, 20
        }
    }, { 22050, 47, 15,
        {
            4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  4,  8,  8,  8,  8,  8,  8,  8,
            8,  8,  8,  12, 12, 12, 12, 16, 16, 16, 20, 20, 24, 24, 28, 28, 32,
            36, 36, 40, 44, 48, 52, 52, 64, 64, 64, 64, 64
        }, {
            4,  4,  4,  4,  4,  4,  4,  8,  8,  8, 12, 12, 16, 16, 20
        }
    }, { 16000, 43, 15,
        {
            8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 12, 12, 12,
            12, 12, 12, 12, 12, 12, 16, 16, 16, 16, 20, 20, 20, 24,
            24, 28, 28, 32, 36, 40, 40, 44, 48, 52, 56, 60, 64, 64, 64
        }, {
            4, 4, 4, 4, 4, 4, 4, 4, 8, 8, 12, 12, 16, 20, 20
        }
    }, { 12000, 43, 15,
        {
            8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 12, 12, 12,
            12, 12, 12, 12, 12, 12, 16, 16, 16, 16, 20, 20, 20, 24,
            24, 28, 28, 32, 36, 40, 40, 44, 48, 52, 56, 60, 64, 64, 64
        }, {
            4, 4, 4, 4, 4, 4, 4, 4, 8, 8, 12, 12, 16, 20, 20
        }
    }, { 11025, 43, 15,
        {
            8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8, 12, 12, 12,
            12, 12, 12, 12, 12, 12, 16, 16, 16, 16, 20, 20, 20, 24,
            24, 28, 28, 32, 36, 40, 40, 44, 48, 52, 56, 60, 64, 64, 64
        }, {
            4, 4, 4, 4, 4, 4, 4, 4, 8, 8, 12, 12, 16, 20, 20
        }
    }, { 8000, 40, 15,
        {
            12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 12, 16,
            16, 16, 16, 16, 16, 16, 20, 20, 20, 20, 24, 24, 24, 28,
            28, 32, 36, 36, 40, 44, 48, 52, 56, 60, 64, 80
        }, {
            4, 4, 4, 4, 4, 4, 4, 8, 8, 8, 8, 12, 16, 20, 20
        }
    },
{ -1, 0, 0, {0}, {0} }
};
