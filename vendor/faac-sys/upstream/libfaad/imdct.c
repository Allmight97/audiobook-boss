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
#include "fft.h"

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

static float kbd_window_2048[1024];
static float sine_window_2048[1024];
static float kbd_window_256[128];
static float sine_window_256[128];


/* DCT-IV twiddles exp(-j*pi*(n + 1/8)/M) for M = 1024 and 128; the same
 * table serves the pre- and post-rotation. */
static float dct4_cos_1024[512];
static float dct4_sin_1024[512];
static float dct4_cos_128[64];
static float dct4_sin_128[64];


static double bessel_i0(double x)
{
    double sum = 1.0, term = 1.0, q = x * x / 4.0;
    for (int k = 1; k < 60; k++) {
        term *= q / ((double)k * k);
        sum += term;
        if (term < sum * 1e-17) break;
    }
    return sum;
}

/* Kaiser-Bessel-derived window, first half (ISO/IEC 14496-3 §4.6.11.3.2):
 * the cumulative Kaiser kernel, normalised, under a square root. */
static void kbd_window(float *w, int n, double alpha)
{
    double sum = 0.0, run = 0.0;
    for (int i = 0; i <= n; i++) {
        double v = (2.0 * i / n) - 1.0;
        sum += bessel_i0(M_PI * alpha * sqrt(1.0 - v * v));
    }
    for (int i = 0; i < n; i++) {
        double v = (2.0 * i / n) - 1.0;
        run += bessel_i0(M_PI * alpha * sqrt(1.0 - v * v));
        w[i] = (float)sqrt(run / sum);
    }
}

void init_windows(void)
{
    fft_init();

    for (int i = 0; i < 1024; i++) {
        sine_window_2048[i] = sinf((float)M_PI * (i + 0.5f) / 2048.0f);
    }
    for (int i = 0; i < 128; i++) {
        sine_window_256[i] = sinf((float)M_PI * (i + 0.5f) / 256.0f);
    }
    kbd_window(kbd_window_2048, 1024, 4.0);
    kbd_window(kbd_window_256, 128, 6.0);

    for (int k = 0; k < 512; k++) {
        double ang = -M_PI * (k + 0.125) / 1024.0;
        dct4_cos_1024[k] = (float)cos(ang);
        dct4_sin_1024[k] = (float)sin(ang);
    }
    for (int k = 0; k < 64; k++) {
        double ang = -M_PI * (k + 0.125) / 128.0;
        dct4_cos_128[k] = (float)cos(ang);
        dct4_sin_128[k] = (float)sin(ang);
    }

}

/* DCT-IV of length M through an M/2-point complex FFT: pack even-index
 * inputs against reversed odd-index inputs, rotate, transform, rotate again
 * and unzip. buf holds 2*M floats: z, which the result u then overwrites
 * (the FFT is done with it by then), and w. The result is buf[0..M). */
static void dct4(const float *in, float *buf, int M)
{
    int K = M / 2;
    int logm = (M == 1024) ? 9 : 6;
    const float *cs = (M == 1024) ? dct4_cos_1024 : dct4_cos_128;
    const float *sn = (M == 1024) ? dct4_sin_1024 : dct4_sin_128;
    float *z = buf, *w = buf + M, *u = buf;
    float *zr = z, *zi = z + K;

    for (int n = 0; n < K; n++) {
        float a = in[2 * n], b = in[M - 1 - 2 * n];
        zr[n] = a * cs[n] - b * sn[n];
        zi[n] = a * sn[n] + b * cs[n];
    }
    fft(z, w, logm);
    const float *wr = w, *wi = w + K;
    for (int k = 0; k < K; k++) {
        u[2 * k]         =  wr[k] * cs[k] - wi[k] * sn[k];
        u[M - 1 - 2 * k] = -(wr[k] * sn[k] + wi[k] * cs[k]);
    }
}

/* IMDCT (ISO/IEC 14496-3 §4.6.11.3.1): n0 = N/4 + 1/2 makes the transform a
 * DCT-IV of the coefficients, folded out with its odd/even symmetries. */
static void fast_imdct(const float *in, float *out, int n, float *tmp)
{
    int M = n / 2, H = M / 2;
    float *u = tmp; /* 2*M floats of work */
    float scale = 2.0f / (float)n;

    dct4(in, tmp, M);
    for (int i = 0; i < H; i++) {
        out[i]             =  u[H + i] * scale;
        out[M + H + i]     = -u[i] * scale;
    }
    for (int i = H; i < M + H; i++)
        out[i] = -u[M + H - 1 - i] * scale;
}

/* The 2048-sample IMDCT output, folded out of the 1024-point DCT-IV u (see
 * fast_imdct), sample i, scaled. */
static inline float imdct_sample(const float *u, int i, float scale)
{
    if (i < 512) return u[512 + i] * scale;
    if (i < 1536) return -u[1535 - i] * scale;
    return -u[i - 1536] * scale;
}

/* Left half: window, add the previous frame's overlap, emit. Right half:
 * window into the overlap for the next frame. */
static inline void imdct_emit(float * restrict out_pcm, float * restrict overlap, const float *u, float scale,
                              int i0, int i1, const float * restrict wl, int wl_dir, float wflat)
{
    /* window sample i is wl[i - i0] (wl_dir > 0), wl[i1 - 1 - i] (< 0) or wflat (wl == NULL) */
    for (int i = i0; i < i1; i++) {
        float w = wl ? (wl_dir > 0 ? wl[i - i0] : wl[i1 - 1 - i]) : wflat;
        float x = imdct_sample(u, i, scale) * w;
        if (i < FRAME_LEN_LONG) out_pcm[i] = x + overlap[i];
        else overlap[i - FRAME_LEN_LONG] = x;
    }
}

/* imdct_emit() for the common long-long block, cut where the fold and the
 * emit/overlap split change so each piece is a straight, vectorisable loop. */
static void imdct_emit_long(float * restrict out_pcm, float * restrict overlap, const float * restrict u, float scale,
                            const float * restrict wl, const float * restrict wr)
{
    for (int i = 0; i < 512; i++)      out_pcm[i] = u[512 + i] * scale * wl[i] + overlap[i];
    for (int i = 512; i < 1024; i++)   out_pcm[i] = -u[1535 - i] * scale * wl[i] + overlap[i];
    for (int i = 1024; i < 1536; i++)  overlap[i - 1024] = -u[1535 - i] * scale * wr[2047 - i];
    for (int i = 1536; i < 2048; i++)  overlap[i - 1024] = -u[i - 1536] * scale * wr[2047 - i];
}

void imdct_and_window(struct faad_decoder *dec, uint32_t ch, uint8_t window_sequence, uint8_t window_shape, float * restrict spec, float * restrict out_pcm)
{
    float * restrict work = dec->scratch.work;
    /* ISO/IEC 14496-3 §4.6.11.3.2: the left half of the window uses the
     * previous block's shape, the right half this block's. */
    uint8_t prev_shape = dec->prev_window_shape[ch];
    const float * restrict win_long_l = (prev_shape == KBD_WINDOW) ? kbd_window_2048 : sine_window_2048;
    const float * restrict win_short_l = (prev_shape == KBD_WINDOW) ? kbd_window_256 : sine_window_256;
    const float * restrict win_long = (window_shape == KBD_WINDOW) ? kbd_window_2048 : sine_window_2048;
    const float * restrict win_short = (window_shape == KBD_WINDOW) ? kbd_window_256 : sine_window_256;
    float * restrict overlap = dec->overlap[ch];
    dec->prev_window_shape[ch] = window_shape;
    dec->prev_window_seq[ch] = window_sequence;

    if (window_sequence == EIGHT_SHORT_SEQUENCE) {
        /* eight 256-sample blocks hopping by 128 cover samples 448..1599 */
        float * restrict acc = work;
        float * restrict block = work + 1152;
        float * restrict tmp = work + 1152 + 256;
        memset(acc, 0, sizeof(float) * 1152);
        for (int w = 0; w < 8; w++) {
            fast_imdct(spec + w * 128, block, 256, tmp);
            const float * restrict wl = (w == 0) ? win_short_l : win_short;
            float *dst = acc + w * 128;
            for (int i = 0; i < 128; i++) {
                dst[i]       += block[i] * wl[i];
                dst[255 - i] += block[255 - i] * win_short[i];
            }
        }
        for (int i = 0; i < 448; i++) out_pcm[i] = overlap[i];
        for (int i = 448; i < FRAME_LEN_LONG; i++) out_pcm[i] = acc[i - 448] + overlap[i];
        for (int i = 0; i < 576; i++) overlap[i] = acc[i + 576];
        memset(overlap + 576, 0, sizeof(float) * 448);
        return;
    }

    float * restrict u = work;
    const float scale = 2.0f / 2048.0f;
    dct4(spec, work, 1024);

    if (window_sequence == ONLY_LONG_SEQUENCE) {
        imdct_emit_long(out_pcm, overlap, u, scale, win_long_l, win_long);
        return;
    }
    if (window_sequence == LONG_STOP_SEQUENCE) {
        for (int i = 0; i < 448; i++) out_pcm[i] = overlap[i];
        imdct_emit(out_pcm, overlap, u, scale, 448, 576, win_short_l, 1, 0.0f);
        imdct_emit(out_pcm, overlap, u, scale, 576, 1024, NULL, 0, 1.0f);
    } else {
        imdct_emit(out_pcm, overlap, u, scale, 0, 1024, win_long_l, 1, 0.0f);
    }
    if (window_sequence == LONG_START_SEQUENCE) {
        imdct_emit(out_pcm, overlap, u, scale, 1024, 1472, NULL, 0, 1.0f);
        imdct_emit(out_pcm, overlap, u, scale, 1472, 1600, win_short, -1, 0.0f);
        memset(overlap + 576, 0, sizeof(float) * 448);
    } else {
        imdct_emit(out_pcm, overlap, u, scale, 1024, 2048, win_long, -1, 0.0f);
    }
}

