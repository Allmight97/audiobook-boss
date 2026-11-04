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

#include <limits.h>
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "quantize.h"
#include "huff2.h"
#include "huffdata.h"
#include "cpu_compute.h"
#include "stats.h"

typedef int (*QuantizeFunc)(const float * __restrict xr, int * __restrict xi, int n4, float sfacfix);

#if defined(HAVE_SSE2)
extern int quantize_sse2(const float * __restrict xr, int * __restrict xi, int n4, float sfacfix);
#endif

/* Written so the loop auto-vectorizes: fabsf() makes the sqrtf() argument
 * provably non-negative (no errno path), the sign is re-applied as a
 * two's-complement mask, and the width is a known multiple of four. */
static int quantize_scalar(const float * __restrict xr, int * __restrict xi, int n4, float sfacfix)
{
    int i, maxq = 0;
    for (i = 0; i < 4 * n4; i++)
    {
        float val = xr[i];
        float tmp = fabsf(val * sfacfix);
        int q, m;
        tmp = sqrtf(tmp * sqrtf(tmp));
        q = (int)(tmp + MAGIC_NUMBER);
        m = -(val < 0.0f);
        if (q > maxq) maxq = q;
        xi[i] = (q ^ m) - m;
    }
    return maxq;
}

static QuantizeFunc qfunc = quantize_scalar;
static float sfstep;
static float max_quant_limit;

#define GAIN_LUT_SIZE 512
#define GAIN_LUT_BIAS 256
/* ISO 14496-3 §8.3.4 scalefactors use quarter-dB steps (1 unit = 2^(1/4) in amplitude).
 * Precomputed 2^(sfac/4) LUT eliminates repeated transcendental powf calls during gain coupling. */
static float gain_lut[GAIN_LUT_SIZE];
static float log10_width_sf_lut[128];

#define SF_CHAIN_UNSET INT_MIN

void QuantizeInit(void)
{
    int i;
#if defined(HAVE_SSE2)
    CPUCaps caps = get_cpu_caps();
    if (caps & CPU_CAP_SSE2)
        qfunc = quantize_sse2;
    else
#endif
        qfunc = quantize_scalar;

    sfstep = SF_STEP_AMPL;
    /* Pre-calculate lookup table for 10^(sfac / sfstep) = 2^(sfac / 4) */
    for (i = -GAIN_LUT_BIAS; i < GAIN_LUT_SIZE - GAIN_LUT_BIAS; i++)
        gain_lut[i + GAIN_LUT_BIAS] = powf(10.0f, (float)i / sfstep);

    /* Pre-multiply width logarithm by SF_STEP_ENRG (= sfstep / 2) */
    for (i = 1; i < 128; i++)
        log10_width_sf_lut[i] = log10f((float)i) * SF_STEP_ENRG;

    /* One-time constant: computed in double so the stored float is
     * correctly rounded, at zero runtime cost. */
    max_quant_limit = (float)pow((double)MAX_HUFF_ESC_VAL + 1.0 - (double)MAGIC_NUMBER, 4.0/3.0);
}

static inline float sfac_to_gain(int sfac)
{
    unsigned int idx = (unsigned int)(sfac + GAIN_LUT_BIAS);
    if (idx < GAIN_LUT_SIZE)
        return gain_lut[idx];
    return powf(10.0f, (float)sfac / sfstep);
}

/* sfac and gain are coupled; clamping one forces a recompute of the other. */
static float gain_with_overflow_clamp(int *sfac, float band_peak)
{
    float gain = sfac_to_gain(*sfac);
    if (band_peak > 0.0f && gain * band_peak > max_quant_limit)
    {
        gain = max_quant_limit / band_peak;
        *sfac = (int)floorf(log10f(gain) * sfstep);
        gain = sfac_to_gain(*sfac);
    }
    return gain;
}

enum { MASKING_TILT_KNOTS = 8, MASKING_PROFILE_DEFAULT = 0, MASKING_PROFILE_TUNED = 1 };

typedef struct
{
    float frequency_khz;
    float gain;
} MaskingTiltKnot;

typedef struct
{
    float avg_weight;
    float loud_exponent;
    float slope_scale;
    float peak_floor;   /* fraction of the per-window reference, about -23 dB at 0.005 */
} MaskingProfile;

typedef struct
{
    float silence_rms;      /* 16-bit PCM units */
    float peak_weight;
    float avg_floor;
    float quiet_band;
    float sf_smooth;
    float rich_slope;
    unsigned tune_min_rate; /* bits/s per channel */
    unsigned nudge_min_rate;
    unsigned rich_min_rate;
    unsigned slope_rate_threshold;
    float slope_reference_rate;
    float tilt_nyquist_khz;
    MaskingProfile profiles[2];
    MaskingTiltKnot tilt[MASKING_TILT_KNOTS];
} MaskingModel;

/* Masking-model parameters, not AAC syntax constants or an absolute hearing
 * threshold. The default balance and fitted stereo profile share these
 * controls; changing them redistributes quantization error between bands. */
static const MaskingModel masking_model = {
    /* Time-domain RMS in 16-bit PCM units; silence_energy() converts it to
     * this MDCT's energy scale so the gate is independent of block length. */
    .silence_rms = 0.4f,
    .peak_weight = 0.45f,    /* tonal contribution after the average-energy share */
    /* Fractions of uniform-energy baselines: average uses the group reference,
     * peak uses the per-window reference (about -30 and -23 dB respectively).
     * Floors keep weak bands' power-law terms from collapsing to zero; the peak
     * floor is per profile. */
    .avg_floor = 0.0010f,
    /* Below about -35 dB relative to that reference, taper the target again
     * so the floors do not spend bits on bands masked by the rest of the frame. */
    .quiet_band = 0.0003f,
    /* Pull coded long-block scalefactors 0.6 of the way toward the mean of
     * their coded neighbors, reducing abrupt changes between bands. */
    .sf_smooth = 0.6f,
    /* Steeper treble de-emphasis leaves more bits for lower bands. Its rate
     * threshold avoids starving treble when the available budget is small. */
    .rich_slope = 3.0f,
    .tune_min_rate = 32000, /* fitted profile's lower bound, bits/s per channel */
    .rich_min_rate = 56000, /* steep-slope threshold, bits/s per channel */
    /* Scalefactor nudge, for any channel count at a set rate. Measured on
     * stereo and mono, LC and HE-AAC, and 5.1. At 24 kbps per channel, 16 kHz
     * mono speech loses up to 0.18 MOS on a few clips; from 28 kbps no clip is
     * past -0.10. */
    .nudge_min_rate = 28000, /* bits/s per channel */
    /* Empirical sample-rate policy, in Hz: above 48 kHz scale the per-line
     * slope relative to 44.1 kHz. These are not the tilt's frequency units. */
    .slope_rate_threshold = 48000,
    .slope_reference_rate = 44100.0f,
    .tilt_nyquist_khz = 24.0f, /* 48 kHz reference rate / 2, not stream Nyquist */
    .profiles = {
        /* Average-energy share, power-law loudness exponent, slope multiplier.
         * The exponent compresses band/reference energy ratios; both weights
         * balance noise-like average energy against tonal peak energy. */
        { .avg_weight = 0.2f, .loud_exponent = 0.4f, .slope_scale = 1.0f, .peak_floor = 0.0050f },
        /* Calibrated for rate-limited stereo LC. Keep other stream types on
         * the default balance; this fit does not establish their quality. */
        { .avg_weight = 0.15f, .loud_exponent = 0.30f, .slope_scale = 3.0f, .peak_floor = 0.0100f },
    },
    /* Empirical target offsets converted to linear multipliers at the knots.
     * Positions are kHz at a 48 kHz reference, so they follow the same
     * normalized bin position at other rates (physical kHz changes with rate).
     * Interpolate gains between knots rather than dB, so
     * intermediate targets differ from a dB-linear curve. Each stored gain is
     * 10^(offset_dB/20): -1.5 dB, +3 dB and -3 dB retain the fitted anchors. */
    .tilt = {
        { 0.5f, 1.0f }, { 1.5f, 0.84139514f }, { 3.0f, 0.84139514f }, { 5.0f, 1.0f },
        { 8.0f, 1.0f }, { 11.0f, 1.41253754f }, { 14.0f, 1.0f }, { 18.0f, 0.70794578f },
    },
};

typedef struct
{
    float sum;      /* energy summed across group windows */
    float peak_energy; /* mean of the per-window peak energies */
} BandEnergy;

static float silence_energy(const CoderInfo *ci)
{
    int len = (ci->block_type == ONLY_SHORT_WINDOW) ? BLOCK_LEN_SHORT : BLOCK_LEN_LONG;
    /* The unnormalized, doubled MDCT has 2*len times the input mean-square
     * energy for a sine-windowed stationary signal. Keep the silence floor
     * in sample units rather than making it depend on the transform length. */
    return (2.0f * len) * (masking_model.silence_rms * masking_model.silence_rms);
}

static float measure_band_energy(const CoderInfo * __restrict ci, const float * __restrict xr0,
                                  int gnum, int cutoff, BandEnergy * __restrict out)
{
    int gsize = ci->groups.len[gnum];
    float group_total = 0.0f;
    float silence = silence_energy(ci);
    int sfb;

    for (sfb = 0; sfb < ci->sfbn; sfb++)
    {
        int lo = ci->sfb_offset[sfb];
        int len = ci->sfb_offset[sfb + 1] - lo;
        float sum = 0.0f, peak = 0.0f;
        int w;

        /* Above the coded cutoff a short block is zero, so both the sum and the
         * peak there are exactly 0.0f. */
        if (lo >= cutoff)
        {
            out[sfb].sum = 0.0f;
            out[sfb].peak_energy = 0.0f;
            continue;
        }

        for (w = 0; w < gsize; w++)
        {
            const float * __restrict line = xr0 + w * BLOCK_LEN_SHORT + lo;
            float wpeak = 0.0f;
            int k;
            for (k = 0; k < len; k += 4)
            {
                float a = line[k], b = line[k + 1], c = line[k + 2], d = line[k + 3];
                float ea = a * a, eb = b * b, ec = c * c, ed = d * d;

                sum += ea; if (ea > wpeak) wpeak = ea;
                sum += eb; if (eb > wpeak) wpeak = eb;
                sum += ec; if (ec > wpeak) wpeak = ec;
                sum += ed; if (ed > wpeak) wpeak = ed;
            }

            /* Mean of the per-window peaks rather than the group maximum: a
             * maximum over more windows is systematically larger, which would
             * tie the tonal term to the grouping decision instead of the signal. */
            peak += wpeak;
        }
        peak /= (float)gsize;

        /* An M/S band is coded against half its weaker channel's L/R level;
         * its own level would code the side as finely as the mid. A half
         * that is silent on its own stays silent. */
        float ms = ci->msEl[gnum * ci->sfbn + sfb];
        if (ms > 0.0f && sum >= silence * (float)(gsize * len))
        {
            float ref = 0.5f * fminf(ms, ci->msPeer[gnum * ci->sfbn + sfb]);
            if (sum > 0.0f)
                peak *= ref / sum;
            sum = ref;
        }

        out[sfb].sum = sum;
        out[sfb].peak_energy = peak;
        group_total += sum;
    }

    return group_total;
}

static float loudness(float energy_ratio, float exponent)
{
    return powf(energy_ratio, exponent);
}

/* Frequency is normalized to a 48 kHz rate. Interpolate the empirical knot
 * gains directly; this is a lower-cost approximation to dB interpolation. */
static float tilt_gain(float khz)
{
    float gain = masking_model.tilt[MASKING_TILT_KNOTS - 1].gain;
    int k;
    if (khz <= masking_model.tilt[0].frequency_khz)
        gain = masking_model.tilt[0].gain;
    else if (khz < masking_model.tilt[MASKING_TILT_KNOTS - 1].frequency_khz)
    {
        for (k = 0; khz > masking_model.tilt[k + 1].frequency_khz; k++) ;
        gain = masking_model.tilt[k].gain
           + (masking_model.tilt[k + 1].gain - masking_model.tilt[k].gain)
           * (khz - masking_model.tilt[k].frequency_khz)
           / (masking_model.tilt[k + 1].frequency_khz - masking_model.tilt[k].frequency_khz);
    }
    return gain;
}

void QuantizeConfigureMask(AACQuantCfg *cfg, unsigned sample_rate,
                           unsigned long bitrate_per_ch, unsigned channels, int is_he)
{
    const int tuned = bitrate_per_ch >= masking_model.tune_min_rate
                   && channels == 2 && !is_he;
    const unsigned profile = tuned ? MASKING_PROFILE_TUNED : MASKING_PROFILE_DEFAULT;
    cfg->mask_tuned = tuned;
    cfg->treble_slope = (bitrate_per_ch >= masking_model.rich_min_rate)
        ? masking_model.rich_slope : 1.0f;
    if (sample_rate > masking_model.slope_rate_threshold)
        cfg->treble_slope *= ((float)sample_rate / masking_model.slope_reference_rate);

    cfg->avg_weight = masking_model.profiles[profile].avg_weight;
    cfg->loud_exponent = masking_model.profiles[profile].loud_exponent;
    cfg->peak_floor = masking_model.profiles[profile].peak_floor;
    if (tuned)
        cfg->treble_slope *= masking_model.profiles[profile].slope_scale;
    /* Any set rate (VBR has none) at or above the nudge's measured floor. */
    cfg->sf_nudge = bitrate_per_ch >= masking_model.nudge_min_rate;
    /* VBR has no rate to re-spend what the softer boost saves, so there it
     * only smears attacks. */
    cfg->onset_soften = is_he && bitrate_per_ch;
}

// masking sensitivity drops above ~4 kHz; de-emphasize bands toward Nyquist
static float treble_rolloff(int lo, int hi, float slope_per_line)
{
    return 10.0f / (1.0f + (float)(lo + hi) * slope_per_line);
}

static void derive_masking_targets(CoderInfo * __restrict ci, int gnum, int group_start, float quality,
                                    const AACQuantCfg * __restrict cfg, const BandEnergy * __restrict be, float group_total,
                                    float * __restrict target_out)
{
    const float treble_slope = cfg->treble_slope;
    const float avg_weight = cfg->avg_weight;
    const float loud_exp = cfg->loud_exponent;
    int gsize = ci->groups.len[gnum];
    int total_len = ci->sfb_offset[ci->sfbn];
    int sfb;
    float group_gain = 1.0f; /* strongest onset gain among the group's windows */

    if (ci->block_type == ONLY_SHORT_WINDOW)
    {
        int win;
        for (win = group_start; win < group_start + gsize; win++)
            if (ci->temporal_gain[win] > group_gain)
                group_gain = ci->temporal_gain[win];
        /* At the same rate the HE core gets less back from the full boost than
         * it costs; the power keeps most of the drop in pre-echo and caps the
         * gain at ~5.6. */
        if (cfg->onset_soften)
            group_gain = sqrtf(group_gain * sqrtf(group_gain));
    }

    // whole group below the silence gate: force every band to a zero target
    if (group_total < silence_energy(ci) * (float)(gsize * total_len))
    {
        for (sfb = 0; sfb < ci->sfbn; sfb++)
        {
            target_out[sfb] = 0.0f;
        }
        return;
    }

    int block_len = (ci->block_type == ONLY_SHORT_WINDOW) ? BLOCK_LEN_SHORT : BLOCK_LEN_LONG;
    float inv_block_len = 1.0f / (float)block_len;
    float slope_per_line = treble_slope * inv_block_len;

    for (sfb = 0; sfb < ci->sfbn; sfb++)
    {
        int lo = ci->sfb_offset[sfb], hi = ci->sfb_offset[sfb + 1];
        float avg = be[sfb].sum;
        float peak = be[sfb].peak_energy;
        float ref = (group_total * inv_block_len) * (hi - lo);
        /* avg and ref are both group totals, so their ratio is independent of
         * group size. peak is a single window's energy, so it needs a
         * single-window reference; otherwise the tonal term decays as 1/gsize. */
        float ref_win = ref / (float)gsize;
        float target;

        // floor before pow(): formula is monotonic, so this floors the output too
        if (avg < ref * masking_model.avg_floor) avg = ref * masking_model.avg_floor;
        if (peak < ref_win * cfg->peak_floor) peak = ref_win * cfg->peak_floor;

        target = avg_weight * loudness(avg / ref, loud_exp)
               + (1.0f - avg_weight) * masking_model.peak_weight * loudness(peak / ref_win, loud_exp);
        if (cfg->mask_tuned)
        {
            float center = (float)(lo + hi);
            float tilt = tilt_gain(0.5f * center * (masking_model.tilt_nyquist_khz * inv_block_len));
            target *= treble_rolloff(lo, hi, slope_per_line) * tilt;
        }
        else
            target *= treble_rolloff(lo, hi, slope_per_line);
        if (ci->block_type == ONLY_SHORT_WINDOW)
            target *= group_gain;
        /* A band this far under the frame is masked by it; the floors above
         * would otherwise pin its target at the floor. Falls at half the
         * loudness exponent. */
        if (be[sfb].sum < ref * masking_model.quiet_band)
            target *= sqrtf(loudness(be[sfb].sum / (ref * masking_model.quiet_band), loud_exp));

        target_out[sfb] = target * quality;
    }
}

// per-band codebook assignment: zero / PNS / regular+Huffman

/* Re-derives gain after each clamp stage since scalefactor and gain are
 * coupled. Reports the final relative (bitstream-delta) and absolute
 * scalefactors. */
static float resolve_band_gain(int sfac, int sf_bias, float band_peak, int last_abs,
                                    int * __restrict out_sf_rel, int * __restrict out_sf_abs)
{
    float gain = gain_with_overflow_clamp(&sfac, band_peak);
    int sf_rel = SF_OFFSET - sfac;
    int sf_abs = sf_bias + sf_rel;

    if (last_abs != SF_CHAIN_UNSET)
    {
        int wanted = sf_abs - last_abs;
        int allowed = clamp_sf_diff(wanted);
        if (allowed != wanted)
        {
            sf_abs = last_abs + allowed;
            sf_rel = sf_abs - sf_bias;
            sfac = SF_OFFSET - sf_rel;
            gain = gain_with_overflow_clamp(&sfac, band_peak);
            sf_rel = SF_OFFSET - sfac;
            sf_abs = sf_bias + sf_rel;
        }
    }

    if (sf_abs < 0 || sf_abs > SF_MAX_ABS)
    {
        sf_abs = (sf_abs < 0) ? 0 : SF_MAX_ABS;
        sf_rel = sf_abs - sf_bias;
        sfac = SF_OFFSET - sf_rel;
        gain = gain_with_overflow_clamp(&sfac, band_peak);
        sf_rel = SF_OFFSET - sfac;
        sf_abs = sf_bias + sf_rel;
    }

    *out_sf_rel = sf_rel;
    *out_sf_abs = sf_abs;
    return gain;
}

/* The scalefactor a coded band wants. Long blocks pull it most of the way
 * towards the mean of its coded neighbors: the masking target tracks local
 * band energy too steeply, which quantizes peaks too coarsely and the bands
 * beside them too finely. */
static int smoothed_sfac(int want, int prev, int next, int sf_bias, int smooth)
{
    int sfac = sf_bias - want;
    if (smooth)
    {
        int cnt = 0, sum = 0;
        if (prev != INT_MIN) { sum += prev; cnt++; }
        if (next != INT_MIN) { sum += next; cnt++; }
        if (cnt)
            sfac += lrintf(masking_model.sf_smooth * ((float)want - (float)sum / (float)cnt));
        /* smoothing must not turn a coded band into a zero band */
        if (SF_OFFSET - sfac < SF_MIN)
            sfac = SF_OFFSET - SF_MIN;
    }
    return sfac;
}

/* Each coded band may take its scalefactor one step coarser. Pick the moves
 * that make the scalefactor deltas cheapest to code (Viterbi over the coded
 * bands of the group, seeded from the previous group's last band). Costs use
 * integer quarter bits; ties prefer the unchanged state. */
#define SF_NUDGE_COST 1  /* quarter bits charged per band moved */
enum { SF_NUDGE_STATES = 2 };
static inline int sf_delta_cost(int diff)
{
    return book12[SF_DELTA + clamp_sf_diff(diff)].len;
}

static void nudge_scalefactors(const CoderInfo *ci, int *sel, int n, int base, int smooth,
                               int last_abs, int run_dp)
{
    /* state 0: one step coarser, state 1: as wanted */
    unsigned char bp[NSFB_LONG];
    /* The previous group's scalefactor seeds the chain without a nudge. */
    int cost[SF_NUDGE_STATES] = {INT_MAX / 2, 0};
    /* Retain the raw neighbor so smoothing is independent of in-place writes. */
    int prev_a0 = last_abs, prev_want = INT_MIN, sb, c;

    for (sb = 0; sb < n; sb++)
    {
        int next[SF_NUDGE_STATES], a0;
        int raw = sel[sb], prev = prev_want;
        prev_want = raw;
        if (raw == INT_MIN)
            continue;
        sel[sb] = smoothed_sfac(raw, prev, sb + 1 < n ? sel[sb + 1] : INT_MIN,
                                ci->sf[base + sb], smooth);
        if (!run_dp)
            continue;
        /* absolute scalefactor of state s is a0 + 1 - s */
        a0 = ci->sf[base + sb] + SF_OFFSET - sel[sb];
        {
            int edge[SF_NUDGE_STATES + 1] = {0};
            int state;
            if (prev_a0 != SF_CHAIN_UNSET)
            {
                int d0 = a0 - prev_a0;
                edge[0] = 4 * sf_delta_cost(d0 - 1);
                edge[1] = 4 * sf_delta_cost(d0);
                edge[2] = 4 * sf_delta_cost(d0 + 1);
            }
            bp[sb] = 0;
            for (state = 0; state < SF_NUDGE_STATES; state++)
            {
                int pred = SF_NUDGE_STATES - 1;
                int best = cost[pred] + edge[pred - state + 1];
                int best_pred = pred;
                for (pred = SF_NUDGE_STATES - 2; pred >= 0; pred--)
                {
                    int candidate = cost[pred] + edge[pred - state + 1];
                    if (candidate < best)
                    {
                        best = candidate;
                        best_pred = pred;
                    }
                }
                next[state] = best;
                bp[sb] |= (unsigned char)(best_pred << state);
            }
        }
        next[0] += SF_NUDGE_COST;
        cost[0] = next[0]; cost[1] = next[1];
        prev_a0 = a0;
    }
    if (!run_dp)
        return;
    c = cost[0] < cost[1] ? 0 : 1;  /* final state */
    for (sb = n - 1; sb >= 0; sb--)
    {
        if (sel[sb] == INT_MIN)
            continue;
        sel[sb] -= 1 - c;
        c = (bp[sb] >> c) & 1;
    }
}

static void assign_band_codebooks(CoderInfo * __restrict ci, const float * __restrict xr0,
                                   const float * __restrict target,
                                   const BandEnergy * __restrict be, int gnum, int pnslevel,
                                   int * __restrict p_last_abs, int * __restrict qs, int * __restrict p_qlen, int sf_nudge)
{
    int gsize = ci->groups.len[gnum];
    float silence = silence_energy(ci);
    float pns_threshold = 0.1f * (float)pnslevel;
    int smooth = ci->block_type != ONLY_SHORT_WINDOW;
    int base = ci->bandcnt;
    int n = ci->sfbn;
    /* per band: sf bias minus the wanted scalefactor, or INT_MIN if not coded */
    int band_sf[NSFB_LONG];
    int sb;

    if (n > MAX_SCFAC_BANDS - base)
        n = MAX_SCFAC_BANDS - base;

    /* Pass 1: settle zero and PNS bands, and the scalefactor each coded band wants. */
    for (sb = 0; sb < n; sb++)
    {
        int band = base + sb;

#ifdef FAAC_STATS
        g_faacStats.totalBands++;
#endif

        band_sf[sb] = INT_MIN;
        if (ci->book[band] != HCB_NONE)
            continue;

        int width = ci->sfb_offset[sb + 1] - ci->sfb_offset[sb];
        if (target[sb] == 0.0f)
        {
            ci->book[band] = HCB_ZERO;
            continue;
        }
        float avg_per_window = be[sb].sum / (float)gsize;
        if (avg_per_window < silence * width)
        {
            ci->book[band] = HCB_ZERO;
            continue;
        }

        /* Log-domain identity: log10(target/rms) = log10(target) - 0.5*log10(avg) + 0.5*log10(width).
         * Reuses sf_enrg_avg (log10(avg) * SF_STEP_ENRG) shared with PNS to avoid division and sqrtf. */
        float sf_enrg_avg = log10f(avg_per_window) * SF_STEP_ENRG;

        /* PNS is fine inside TNS-covered bands -- the decoder's inverse
         * TNS filter shapes the substituted noise too. Decoders skip M/S on a
         * noise band, so an M/S band that wants noise goes back to L/R noise
         * in both channels, decided on the mid. Its flag isn't restored on a
         * retry, so the fallback sticks. A side band left under M/S drops to
         * zero instead, leaving the band mono. */
        if (target[sb] < pns_threshold || (ci->msEl[band] > 0.0f && ci->msUsed && !ci->msUsed[band]))
        {
            if (ci->msEl[band] > 0.0f)
            {
                CoderInfo *r = ci->partner;
                if (!r)
                {
                    ci->book[band] = HCB_ZERO;
                    continue;
                }
                ci->msUsed[band] = 0;
                sf_enrg_avg = log10f(ci->msEl[band] / (float)gsize) * SF_STEP_ENRG;
                r->book[band] = HCB_PNS;
                r->sf[band] += lrintf(log10f(r->msEl[band] / (float)gsize) * SF_STEP_ENRG);
            }
            ci->book[band] = HCB_PNS;
#ifdef FAAC_STATS
            g_faacStats.pnsBands++;
#endif
            ci->sf[band] += lrintf(sf_enrg_avg);
            continue;
        }

        float log10_w_sf = (width < 128) ? log10_width_sf_lut[width] : log10f((float)width) * SF_STEP_ENRG;
        int sfac = lrintf(log10f(target[sb]) * sfstep - sf_enrg_avg + log10_w_sf);
        if (SF_OFFSET - sfac < SF_MIN)
        {
            ci->book[band] = HCB_ZERO;
            ci->sf[band] += SF_OFFSET - sfac;
            continue;
        }
        band_sf[sb] = ci->sf[band] - sfac;
    }

    /* All raw neighbors are known before this buffer becomes gain scalefactors. */
    nudge_scalefactors(ci, band_sf, n, base, smooth, *p_last_abs, sf_nudge);

    /* Pass 2: quantize the coded bands. Long blocks first pull each
     * scalefactor most of the way towards the mean of its coded neighbors:
     * the masking target tracks local band energy too steeply, which
     * quantizes peaks too coarsely and the bands beside them too finely. */
    for (sb = 0; sb < n; sb++)
    {
        int band = base + sb;

        if (band_sf[sb] == INT_MIN)
            continue;

        int sf_bias = ci->sf[band];
        int sfac = band_sf[sb];

        int lo = ci->sfb_offset[sb];
        int width = ci->sfb_offset[sb + 1] - lo;
        int sf_rel, sf_abs;
        float gain = resolve_band_gain(sfac, sf_bias, sqrtf(be[sb].peak_energy), *p_last_abs, &sf_rel, &sf_abs);
        int *xi = qs + *p_qlen;
        int win, maxq = 0;

        for (win = 0; win < gsize; win++)
        {
            int qm = qfunc(xr0 + win * BLOCK_LEN_SHORT + lo, xi + win * width, width >> 2, gain);
            if (qm > maxq) maxq = qm;
        }
        /* huffbook picks the final book; record the lowest that covers maxq */
        ci->book[band] = !maxq ? HCB_ZERO : maxq <= LAV_1 ? HCB_1 : maxq <= LAV_2 ? HCB_3
                       : maxq <= LAV_4 ? HCB_5 : maxq <= LAV_7 ? HCB_7 : maxq <= LAV_12 ? HCB_9 : HCB_ESC;
        if (maxq)
            *p_qlen += gsize * width;
        *p_last_abs = sf_abs;
        ci->sf[band] += sf_rel;
    }
    ci->bandcnt = base + n;
}

void ResetCoderSections(CoderInfo *coder)
{
    int i, n = coder->groups.n * coder->sfbn;
    coder->partner = NULL;
    coder->useRef = 0;
    coder->msUsed = NULL;
    coder->msPeer = NULL;
    /* msEl[] is only read below n. */
    memset(coder->msEl, 0, n * sizeof(coder->msEl[0]));
    for (i = 0; i < n; i++)
    {
        coder->book[i] = HCB_NONE;
        coder->sf[i] = 0;
    }
}

/* The band scans here and in stereo.c step four coefficients with no remainder,
 * which holds only because every band width in srInfo[] is a multiple of four.
 * That is a property of the tables, so verify the one actually selected. */
static void assert_band_widths_align(const CoderInfo * __restrict ci)
{
    int sfb;

    for (sfb = 0; sfb < ci->sfbn; sfb++)
        assert((ci->sfb_offset[sfb + 1] - ci->sfb_offset[sfb]) % 4 == 0);
}

/* Decoders disagree on whether an intensity band copies the left channel's
 * substituted noise or its still-empty lines, so an intensity band over a
 * noise band can decode silent. Code it as noise at the level the intensity
 * position implies (both are 1.5 dB steps). Runs after the left
 * channel's BlocQuant and before the right's. */
static void ResolveIntensityNoise(const CoderInfo *left, CoderInfo *right)
{
    for (int i = 0; i < left->bandcnt; i++) {
        int b = right->book[i];
        if (left->book[i] == HCB_PNS && (b == HCB_INTENSITY || b == HCB_INTENSITY2)) {
            right->book[i] = HCB_PNS;
            right->sf[i] = left->sf[i] - right->sf[i];
        }
    }
}

int BlocQuant(CoderInfo * __restrict coder, float * __restrict xr, AACQuantCfg *aacquantCfg)
{
    /* These arrays hold one group; MAX_SCFAC_BANDS spans all short groups. */
    float target[NSFB_LONG];
    BandEnergy be[NSFB_LONG];
    int qs[FRAME_LEN];
    int i, lastsf = SF_CHAIN_UNSET, qlen = 0;
    float *gxr = xr;
    int group_start = 0;
    int cutoff = (coder->block_type == ONLY_SHORT_WINDOW)
               ? aacquantCfg->max_l / 8 : coder->sfb_offset[coder->sfbn];

    assert_band_widths_align(coder);

    coder->bandcnt = coder->datacnt = 0;
    for (i = 0; i < coder->groups.n; i++)
    {
        float group_total = measure_band_energy(coder, gxr, i, cutoff, be);
        if (coder->useRef)
            group_total = coder->refTotal[i];

        derive_masking_targets(coder, i, group_start, (float)aacquantCfg->quality / DEFQUAL, aacquantCfg, be, group_total, target);
        assign_band_codebooks(coder, gxr, target, be, i, aacquantCfg->pnslevel, &lastsf, qs, &qlen, aacquantCfg->sf_nudge);
        gxr += coder->groups.len[i] * BLOCK_LEN_SHORT;
        group_start += coder->groups.len[i];
    }
    /* assign_band_codebooks fills qs through the qfunc pointer, which the
     * checker cannot follow; huffbook reads only the bands that wrote it. */
    // cppcheck-suppress uninitvar
    huffbook(coder, qs);

    // global_gain must come from a regular band: it's an 8-bit bitstream field,
    // and intensity/PNS bands store stereo-position/noise-energy on a different
    // (possibly negative) scale that would truncate and desync the decoder.
    coder->global_gain = 0;
    for (i = 0; i < coder->bandcnt; i++)
    {
        int b = coder->book[i];
        if (b && b != HCB_INTENSITY && b != HCB_INTENSITY2 && b != HCB_PNS)
        {
            coder->global_gain = coder->sf[i];
            break;
        }
    }

    int lastis = 0, lastpns = coder->global_gain - SF_PNS_OFFSET;
    for (i = 0; i < coder->bandcnt; i++)
    {
        int b = coder->book[i];
        if (b == HCB_INTENSITY || b == HCB_INTENSITY2)
        {
            int diff = clamp_sf_diff(coder->sf[i] - lastis);
            lastis += diff;
            coder->sf[i] = lastis;
        }
        else if (b == HCB_PNS)
        {
            int diff = clamp_sf_diff(coder->sf[i] - lastpns);
            lastpns += diff;
            coder->sf[i] = lastpns;
        }
    }
    if (coder->partner)
        ResolveIntensityNoise(coder, coder->partner);
    return 1;
}

/* sfbOffsetShort/Long are filled as a side effect of the same bandwidth walk
 * that picks max_cbs/max_cbl -- a prefix sum over the same table, to the same
 * bound, so there's nothing left for a caller to redo afterward. Only
 * [0, max_cbs] and [0, max_cbl] are written; callers never index sfb_offset
 * past their own sfbn, which is exactly max_cbs/max_cbl. */
void CalcBW(unsigned *bw, int rate, SR_INFO *sr, AACQuantCfg *aacquantCfg,
            int *sfbOffsetShort, int *sfbOffsetLong)
{
    int i, l = 0, max = *bw * (BLOCK_LEN_SHORT << 1) / rate;
    for (i = 0; i < sr->num_cb_short && l < max; i++) {
        sfbOffsetShort[i] = l;
        l += sr->cb_width_short[i];
    }
    sfbOffsetShort[i] = l;
    aacquantCfg->max_cbs = i;

    /* Snap on the long grid only. Both loops round up, so also snapping to the
     * 14-band short grid compounds two round-ups and leaves the cutoff far
     * coarser than the long grid can express. */

    l = 0, max = *bw * (BLOCK_LEN_LONG << 1) / rate;
    for (i = 0; i < sr->num_cb_long && l < max; i++) {
        sfbOffsetLong[i] = l;
        l += sr->cb_width_long[i];
    }
    sfbOffsetLong[i] = l;
    aacquantCfg->max_cbl = i;
    aacquantCfg->max_l = l;
    *bw = (float)l * rate / (BLOCK_LEN_LONG << 1);
}

// short-window grouping: keep spectrally-similar windows together so they
// share scalefactors; a transient onset starts a fresh group instead
#define GROUP_MIN_SFB     2    // bands below this are too coarse/DC-heavy to inform grouping
#define GROUP_ONSET_RATIO 3.0f  // running max/min energy ratio that counts as a transient

/* Four independent lanes: strict FP forbids reordering one float accumulator,
 * so a single-sum loop can never vectorize. n4 is the width in quads. */
static inline float band_energy_sum(const float * __restrict line, int n4)
{
    float s0 = 0.0f, s1 = 0.0f, s2 = 0.0f, s3 = 0.0f;
    int k;

    for (k = 0; k < 4 * n4; k += 4)
    {
        s0 += line[k] * line[k];
        s1 += line[k + 1] * line[k + 1];
        s2 += line[k + 2] * line[k + 2];
        s3 += line[k + 3] * line[k + 3];
    }
    return (s0 + s1) + (s2 + s3);
}

/* Accumulates, so a CPE can sum both channels into one energy vector. */
static void window_band_energy(const CoderInfo * __restrict ci, const float * __restrict w,
                                int from_sfb, int to_sfb, float * __restrict e_out)
{
    int sfb;
    for (sfb = from_sfb; sfb < to_sfb; sfb++)
    {
        int lo = ci->sfb_offset[sfb];
        e_out[sfb] += band_energy_sum(w + lo, (ci->sfb_offset[sfb + 1] - lo) >> 2);
    }
}

/* Splits the 8 short windows into groups at detected onsets. For a CPE
 * (ci_r != NULL) both channels' band energies are summed so the pair shares one
 * grouping, which the bitstream requires anyway. Long blocks never reach here. */
void BlocGroup(CoderInfo *coderInfo, float *xr, CoderInfo *ci_r, float *xr_r, AACQuantCfg *cfg)
{
    CoderInfo *ci[2];
    float *xrs[2];
    int nch = ci_r ? 2 : 1;

    int maxsfb = cfg->max_cbs;
    int cutoff = cfg->max_l / 8;
    int active_bands = maxsfb - GROUP_MIN_SFB;
    int onset_quorum = (active_bands * 3) >> 2;

    float band_e[NSFB_SHORT], run_min[NSFB_SHORT], run_max[NSFB_SHORT];
    int win, group_start = 0;

    ci[0] = coderInfo; ci[1] = ci_r;
    xrs[0] = xr;       xrs[1] = xr_r;

    coderInfo->groups.n = 0;

    for (win = 0; win < MAX_SHORT_WINDOWS; win++)
    {
        int k, sfb, c;

        for (sfb = GROUP_MIN_SFB; sfb < maxsfb; sfb++)
            band_e[sfb] = 0.0f;

        for (c = 0; c < nch; c++)
        {
            float *w = xrs[c] + win * BLOCK_LEN_SHORT;

            for (k = cutoff; k < ci[c]->sfb_offset[maxsfb]; k++)
                w[k] = 0.0f;

            window_band_energy(ci[c], w, GROUP_MIN_SFB, maxsfb, band_e);
        }

        if (win == group_start)
        {
            for (sfb = GROUP_MIN_SFB; sfb < maxsfb; sfb++)
                run_min[sfb] = run_max[sfb] = band_e[sfb];
            continue;
        }

        int onset_votes = 0;
        for (sfb = GROUP_MIN_SFB; sfb < maxsfb; sfb++)
        {
            if (band_e[sfb] < run_min[sfb]) run_min[sfb] = band_e[sfb];
            if (band_e[sfb] > run_max[sfb]) run_max[sfb] = band_e[sfb];
            if (run_max[sfb] > GROUP_ONSET_RATIO * run_min[sfb]) onset_votes++;
        }

        if (onset_votes > onset_quorum)
        {
            coderInfo->groups.len[coderInfo->groups.n++] = win - group_start;
            group_start = win;
            for (sfb = GROUP_MIN_SFB; sfb < maxsfb; sfb++)
                run_min[sfb] = run_max[sfb] = band_e[sfb];
        }
    }
    coderInfo->groups.len[coderInfo->groups.n++] = MAX_SHORT_WINDOWS - group_start;

    if (ci_r)
        ci_r->groups = coderInfo->groups;
}
