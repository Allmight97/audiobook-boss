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

#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "faac_internal.h"
#include "drc_analyzer.h"
#include "util.h"

#ifndef M_PI_DOUBLE
# define M_PI_DOUBLE 3.14159265358979323846
#endif

/* Ballistics follow the common line-mode profiles: fast and slow attack, a
 * fast release after a large drop and a profile-specific slow release. The
 * short level tracker lets a loud onset reach the curve within a frame or two
 * without chattering on isolated samples. */
#define DRC_TAU_FAST_ATTACK           0.050f
#define DRC_TAU_SLOW_ATTACK           0.100f
#define DRC_TAU_FAST_LEVEL            0.010f
#define DRC_LEVEL_STEP_THRESHOLD      15.0f   /* dB of level change selecting fast ballistics */
#define DRC_MAX_CUT_STEP_DB           6.00f   /* Maximum cut increase per frame (slew rate limit) */

/*
 * Static curves, relative to the program reference level, as gain over
 * loudness x = L - ref: profile-slope boost below boost_start up to boost_max,
 * a null band, then 2:1 cut above cut_start that steepens to cut_slope past
 * cut_knee. The edges, slopes and release times follow the common line-mode
 * profiles on the 0.25 dB metadata grid; speech boosts more steeply.
 */
typedef struct DrcProfile {
    float boost_start;  /* LU below which boost begins (negative) */
    float boost_max;    /* dB */
    float boost_slope;  /* dB per LU below boost_start */
    float cut_start;    /* LU above which cut begins */
    float cut_knee;     /* LU where the cut steepens; beyond the table range if never */
    float cut_slope;    /* dB per LU past the knee */
    float tau_release;  /* seconds, slow release for boosts */
    float tau_fast_release; /* seconds, release after a large level drop */
} DrcProfile;

static const DrcProfile drc_profiles[] = {
    /* FAAC_DRC_FILM         */ { -3.5f,   6.0f, 0.5f,  1.5f,  12.0f, 0.944f, 3.0f,  1.0f },
    /* FAAC_DRC_FILM_LIGHT   */ { -13.5f,  6.0f, 0.5f,  6.5f,  17.5f, 0.94f,  3.0f,  1.0f },
    /* FAAC_DRC_MUSIC        */ { -3.5f,  12.0f, 0.5f,  1.5f,  12.0f, 0.944f, 10.0f, 1.0f },
    /* FAAC_DRC_MUSIC_LIGHT  */ { -13.5f, 12.0f, 0.5f,  6.5f, 1000.0f, 0.5f,  3.0f,  1.0f },
    /* FAAC_DRC_SPEECH       */ { -3.5f,  15.0f, 0.8f,  1.5f,  12.0f, 0.944f, 1.0f,  0.2f },
};

/* Gain in dB for loudness x LU relative to the program reference level. */
static float ProfileCurveDb(const DrcProfile *pr, float x)
{
    if (x < pr->boost_start) {
        float g = pr->boost_slope * (pr->boost_start - x);
        return g < pr->boost_max ? g : pr->boost_max;
    }
    if (x <= pr->cut_start)
        return 0.0f;
    if (x <= pr->cut_knee)
        return -0.5f * (x - pr->cut_start);
    return -0.5f * (pr->cut_knee - pr->cut_start) - pr->cut_slope * (x - pr->cut_knee);
}

typedef struct BiquadState {
    float v1_1, v2_1; /* Pre-filter delay elements */
    float v1_2, v2_2; /* RLB filter delay elements */
    float weight;     /* BS.1770-4 channel weight (1.0 for main channels, 1.41 for surround, 0.0 for LFE) */
} BiquadState;

struct DrcAnalyzer {
    uint32_t num_channels;
    float ref_level_lufs;
    int8_t ref_code;
    const DrcProfile *profile;

    /* BS.1770-4 K-weighting pre-filter biquad coefficients */
    float b0_1, b1_1, b2_1;
    float a1_1, a2_1;

    /* BS.1770-4 RLB high-pass filter biquad coefficients */
    float b0_2, b1_2, b2_2;
    float a1_2, a2_2;

    BiquadState ch_state[MAX_CHANNELS];

    /* Ballistics and 1-frame lookahead state */
    bool primed;
    float prev_smooth_gain_db;
    float smooth_level_db;
    float fast_level_db;
    float alpha_fast_attack;
    float alpha_slow_attack;
    float alpha_fast_release;
    float alpha_release;
    float alpha_fast_level;

    int8_t lookahead_gain_code;

    /* Delay line from analyzed frame to emitted gain: hist[0] is the newest. */
    int8_t hist[2];
};

/*
 * Compute ITU-R BS.1770-4 K-weighting pre-filter biquad coefficients
 * for arbitrary sample rates via bilinear transform.
 */
static void ComputePreFilterCoeffs(double fs, float *b0, float *b1, float *b2, float *a1, float *a2)
{
    double f0 = 1681.974450955533;
    double Q  = 0.7071752369554196;

    double K = tan(M_PI_DOUBLE * f0 / fs);
    /* pow(10, G/20) and its square root for the fixed shelf gain. Keeping
     * them as literals avoids carrying the double-precision pow/sqrt path
     * in every build; the values are the exact round-to-nearest results. */
    double V0 = 1.5848647011308556;
    double sv = 1.2589140960092773;

    double d = 1.0 + (1.0 / Q) * K + K * K;
    double b0_d = (V0 + sv * (1.0 / Q) * K + K * K) / d;
    double b1_d = (2.0 * (K * K - V0)) / d;
    double b2_d = (V0 - sv * (1.0 / Q) * K + K * K) / d;
    double a1_d = (2.0 * (K * K - 1.0)) / d;
    double a2_d = (1.0 - (1.0 / Q) * K + K * K) / d;

    *b0 = (float)b0_d; *b1 = (float)b1_d; *b2 = (float)b2_d;
    *a1 = (float)a1_d; *a2 = (float)a2_d;
}

/*
 * Compute ITU-R BS.1770-4 RLB high-pass filter biquad coefficients
 * for arbitrary sample rates via bilinear transform.
 */
static void ComputeRLBCoeffs(double fs, float *b0, float *b1, float *b2, float *a1, float *a2)
{
    double f0 = 38.13547087602444;
    double Q  = 0.5003270373238773;

    double K = tan(M_PI_DOUBLE * f0 / fs);
    double d = 1.0 + (1.0 / Q) * K + K * K;

    double b0_d = 1.0 / d;
    double b1_d = -2.0 / d;
    double b2_d = 1.0 / d;
    double a1_d = (2.0 * (K * K - 1.0)) / d;
    double a2_d = (1.0 - (1.0 / Q) * K + K * K) / d;

    *b0 = (float)b0_d; *b1 = (float)b1_d; *b2 = (float)b2_d;
    *a1 = (float)a1_d; *a2 = (float)a2_d;
}

DrcAnalyzer *DrcAnalyzerInit(uint32_t sample_rate, uint32_t num_channels,
                             const bool *is_lfe_channel, float ref_level_lufs,
                             uint32_t profile, uint32_t samples_per_ch)
{
    if (sample_rate == 0 || num_channels == 0 || num_channels > MAX_CHANNELS
        || samples_per_ch == 0
        || profile < 1 || profile > sizeof(drc_profiles) / sizeof(drc_profiles[0]))
        return NULL;

    DrcAnalyzer *da = (DrcAnalyzer *)AllocMemory(sizeof(DrcAnalyzer));
    if (!da)
        return NULL;
    memset(da, 0, sizeof(DrcAnalyzer));

    da->num_channels = num_channels;
    da->ref_level_lufs = ref_level_lufs;
    int ref_code = (int)(fabsf(ref_level_lufs) / 0.25f + 0.5f);
    if (ref_code > 127)
        ref_code = 127;
    da->ref_code = (int8_t)ref_code;
    da->profile = &drc_profiles[profile - 1];

    float frame_dur = (float)samples_per_ch / (float)sample_rate;
    da->alpha_fast_attack = 1.0f - expf(-frame_dur / DRC_TAU_FAST_ATTACK);
    da->alpha_slow_attack = 1.0f - expf(-frame_dur / DRC_TAU_SLOW_ATTACK);
    da->alpha_fast_release = 1.0f - expf(-frame_dur / da->profile->tau_fast_release);
    da->alpha_release = 1.0f - expf(-frame_dur / da->profile->tau_release);
    da->alpha_fast_level = 1.0f - expf(-frame_dur / DRC_TAU_FAST_LEVEL);

    ComputePreFilterCoeffs((double)sample_rate, &da->b0_1, &da->b1_1, &da->b2_1, &da->a1_1, &da->a2_1);
    ComputeRLBCoeffs((double)sample_rate, &da->b0_2, &da->b1_2, &da->b2_2, &da->a1_2, &da->a2_2);

    /* Assign BS.1770-4 channel weights */
    for (uint32_t ch = 0; ch < num_channels; ch++) {
        if (is_lfe_channel && is_lfe_channel[ch]) {
            da->ch_state[ch].weight = 0.0f; /* LFE is excluded from ITU-R BS.1770 loudness measurement */
        } else if (num_channels >= 5 && ch >= 3 && (ch <= 4 || (num_channels >= 7 && ch <= 6))) {
            /* The FIFO is in AAC order (C L R Ls Rs [Lb Rb] [LFE]), so the
             * surrounds sit at 3-4 and, with seven or more channels, 5-6.
             * They get the BS.1770 ~1.5 dB (+1.41 linear) boost. */
            da->ch_state[ch].weight = 1.41f;
        } else {
            da->ch_state[ch].weight = 1.0f;  /* Main front/side/center channels */
        }
    }

    return da;
}

static void AnalyzeFrame(DrcAnalyzer *da, float * const *input_pcm,
                         uint32_t samples_per_ch, int8_t *out_gain_code)
{
    const bool first = !da->primed;

    float max_peak_lin = 0.0f;
    double frame_sum_sq = 0.0;
    const float pcm_scale = 1.0f / 32768.0f;

    const float b0_1 = da->b0_1, b1_1 = da->b1_1, b2_1 = da->b2_1, a1_1 = da->a1_1, a2_1 = da->a2_1;
    const float b0_2 = da->b0_2, b1_2 = da->b1_2, b2_2 = da->b2_2, a1_2 = da->a1_2, a2_2 = da->a2_2;

    /* Process channel-by-channel for cache locality and SIMD vectorization */
    for (uint32_t ch = 0; ch < da->num_channels; ch++) {
        const float *pcm = input_pcm[ch];
        const float weight = da->ch_state[ch].weight;

        if (weight <= 0.0f) {
            /* LFE samples still count toward peak protection. */
            for (uint32_t i = 0; i < samples_per_ch; i++) {
                float abs_x = fabsf(pcm[i] * pcm_scale);
                if (abs_x > max_peak_lin)
                    max_peak_lin = abs_x;
            }
            continue;
        }

        float v1_1 = da->ch_state[ch].v1_1, v2_1 = da->ch_state[ch].v2_1;
        float v1_2 = da->ch_state[ch].v1_2, v2_2 = da->ch_state[ch].v2_2;
        double ch_sum_sq = 0.0;

        for (uint32_t i = 0; i < samples_per_ch; i++) {
            float scaled = pcm[i] * pcm_scale;
            float abs_x = fabsf(scaled);
            if (abs_x > max_peak_lin)
                max_peak_lin = abs_x;

            /* Stage 1: High-shelf pre-filter (Direct Form II Transposed) */
            float y1 = b0_1 * scaled + v1_1;
            v1_1 = b1_1 * scaled - a1_1 * y1 + v2_1;
            v2_1 = b2_1 * scaled - a2_1 * y1;

            /* Stage 2: RLB high-pass filter */
            float y2 = b0_2 * y1 + v1_2;
            v1_2 = b1_2 * y1 - a1_2 * y2 + v2_2;
            v2_2 = b2_2 * y1 - a2_2 * y2;

            ch_sum_sq += (double)y2 * (double)y2;
        }

        da->ch_state[ch].v1_1 = v1_1; da->ch_state[ch].v2_1 = v2_1;
        da->ch_state[ch].v1_2 = v1_2; da->ch_state[ch].v2_2 = v2_2;

        frame_sum_sq += (double)weight * ch_sum_sq;
    }

    /* Track the per-frame level, not a long window: a short exponential level
     * reaches a loud onset in a few frames without chattering on isolated
     * samples. */
    float frame_mean_sq = (float)(frame_sum_sq / (double)samples_per_ch);
    float L_frame = (frame_mean_sq > 1e-12f) ? (-0.691f + 10.0f * log10f(frame_mean_sq)) : -100.0f;
    if (first) {
        da->fast_level_db = L_frame;
    } else {
        da->fast_level_db += da->alpha_fast_level * (L_frame - da->fast_level_db);
    }

    /* No absolute low-level gate: the line-mode profiles still boost quiet
     * program. */
    float gain_raw_db = ProfileCurveDb(da->profile, da->fast_level_db - da->ref_level_lufs);

    /* 50 ms attack when the level jumps, 100 ms for gradual cuts, a 1 s release
     * after a large drop and the profile's slow release otherwise. The gain
     * starts at 0 dB. */
    float gain_smooth_db = da->prev_smooth_gain_db;

    if (first) {
        gain_smooth_db = 0.0f;
        da->smooth_level_db = da->fast_level_db;
    } else {
        float level_delta = da->fast_level_db - da->smooth_level_db;
        float alpha;
        if (gain_raw_db < da->prev_smooth_gain_db) {
            alpha = level_delta > DRC_LEVEL_STEP_THRESHOLD ? da->alpha_fast_attack
                                                           : da->alpha_slow_attack;
        } else {
            alpha = level_delta < -DRC_LEVEL_STEP_THRESHOLD ? da->alpha_fast_release
                                                            : da->alpha_release;
        }
        da->smooth_level_db += alpha * level_delta;
        gain_smooth_db = da->prev_smooth_gain_db + alpha * (gain_raw_db - da->prev_smooth_gain_db);
        if (gain_smooth_db < da->prev_smooth_gain_db - DRC_MAX_CUT_STEP_DB)
            gain_smooth_db = da->prev_smooth_gain_db - DRC_MAX_CUT_STEP_DB;
    }

    /* Never boost past the headroom to 0 dBFS. */
    if (gain_smooth_db > 0.0f) {
        float peak_dbfs = 20.0f * log10f((max_peak_lin > 1e-9f) ? max_peak_lin : 1e-9f);
        float headroom_db = (peak_dbfs < 0.0f) ? -peak_dbfs : 0.0f;
        if (gain_smooth_db > headroom_db)
            gain_smooth_db = headroom_db;
    }

    /* Quantize to the existing 0.25 dB grid and keep the wire code. */
    float gain_quant = roundf(gain_smooth_db / 0.25f);
    if (gain_quant > 127.0f)
        gain_quant = 127.0f;
    if (gain_quant < -127.0f)
        gain_quant = -127.0f;

    int8_t curr_gain_code = (int8_t)(int)gain_quant;

    /* A cut needed at the next frame applies one frame early, so it lands
     * ahead of a loud onset. */
    if (first) {
        da->lookahead_gain_code = curr_gain_code;
        *out_gain_code = curr_gain_code;
    } else {
        /* If incoming frame M requires a cut deeper than lookahead frame, advance cut to lookahead frame */
        *out_gain_code = curr_gain_code < da->lookahead_gain_code
                       ? curr_gain_code : da->lookahead_gain_code;
        da->lookahead_gain_code = curr_gain_code;
    }

    /* Carry the unquantized gain: feeding back the 0.25 dB-rounded output
     * freezes the 1 s release, whose per-frame step is far below the grid. */
    da->prev_smooth_gain_db = gain_smooth_db;
    da->primed = true;
}

void DrcAnalyzerProcessFrame(DrcAnalyzer *da, float * const *input_pcm,
                             uint32_t samples_per_ch, DRCInfo *out_drc)
{
    if (!da || !out_drc)
        return;

    /* The analyzer sees the newest input frame, but the access unit being
     * written carries audio that entered the encoder one frame earlier. Delay
     * the gain so the FIL describes the audio it travels with. A call without
     * input (flush) keeps the pipeline ticking on the last gain. */
    const bool primed = da->primed;
    int8_t cur = da->hist[0];
    if (input_pcm && samples_per_ch > 0)
        AnalyzeFrame(da, input_pcm, samples_per_ch, &cur);
    else if (!primed)
        return;

    if (!primed) {
        da->hist[1] = cur;
        da->hist[0] = cur;
    } else {
        da->hist[1] = da->hist[0];
        da->hist[0] = cur;
    }
    out_drc->enabled = true;
    out_drc->ref_code = da->ref_code;
    out_drc->gain_code = da->hist[1];
}
