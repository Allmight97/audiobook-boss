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

#ifndef FAAD_DISABLE_DAB
static float kbd_window_1920[960];
static float sine_window_1920[960];
static float kbd_window_240[120];
static float sine_window_240[120];
#endif

/* DCT-IV twiddles exp(-j*pi*(n + 1/8)/M) */
static float dct4_cos_1024[512];
static float dct4_sin_1024[512];
static float dct4_cos_128[64];
static float dct4_sin_128[64];

#ifndef FAAD_DISABLE_DAB
static float dct4_cos_960[480];
static float dct4_sin_960[480];
static float dct4_cos_120[60];
static float dct4_sin_120[60];
#endif


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
 * the cumulative Kaiser kernel, normalized, under a square root. */
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
#ifndef FAAD_DISABLE_DAB
    for (int i = 0; i < 960; i++) {
        sine_window_1920[i] = sinf((float)M_PI * (i + 0.5f) / 1920.0f);
    }
    for (int i = 0; i < 120; i++) {
        sine_window_240[i] = sinf((float)M_PI * (i + 0.5f) / 240.0f);
    }
#endif

    kbd_window(kbd_window_2048, 1024, 4.0);
    kbd_window(kbd_window_256, 128, 6.0);
#ifndef FAAD_DISABLE_DAB
    kbd_window(kbd_window_1920, 960, 4.0);
    kbd_window(kbd_window_240, 120, 6.0);
#endif

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
#ifndef FAAD_DISABLE_DAB
    for (int k = 0; k < 480; k++) {
        double ang = -M_PI * (k + 0.125) / 960.0;
        dct4_cos_960[k] = (float)cos(ang);
        dct4_sin_960[k] = (float)sin(ang);
    }
    for (int k = 0; k < 60; k++) {
        double ang = -M_PI * (k + 0.125) / 120.0;
        dct4_cos_120[k] = (float)cos(ang);
        dct4_sin_120[k] = (float)sin(ang);
    }
#endif
}

/* DCT-IV of length M through an M/2-point complex FFT: pack even-index
 * inputs against reversed odd-index inputs, rotate, transform, rotate again
 * and unzip. buf holds 2*M floats: z, which the result u then overwrites
 * (the FFT is done with it by then), and w. The result is buf[0..M). */
static void dct4(const float * restrict in, float * restrict buf, int M)
{
    int K = M / 2;
#ifndef FAAD_DISABLE_DAB
    const float *cs = (M == 1024) ? dct4_cos_1024 : ((M == 960) ? dct4_cos_960 : ((M == 128) ? dct4_cos_128 : dct4_cos_120));
    const float *sn = (M == 1024) ? dct4_sin_1024 : ((M == 960) ? dct4_sin_960 : ((M == 128) ? dct4_sin_128 : dct4_sin_120));
#else
    const float *cs = (M == 1024) ? dct4_cos_1024 : dct4_cos_128;
    const float *sn = (M == 1024) ? dct4_sin_1024 : dct4_sin_128;
#endif
    float *z = buf, *w = buf + M, *u = buf;
    float *zr = z, *zi = z + K;

    for (int n = 0; n < K; n++) {
        float a = in[2 * n], b = in[M - 1 - 2 * n];
        zr[n] = a * cs[n] - b * sn[n];
        zi[n] = a * sn[n] + b * cs[n];
    }
#ifndef FAAD_DISABLE_DAB
    if (M == 1024) fft(z, w, 9);
    else if (M == 960) fft480(z, w);
    else if (M == 128) fft(z, w, 6);
    else fft60(z, w);
#else
    fft(z, w, (M == 1024) ? 9 : 6);
#endif

    const float *wr = w, *wi = w + K;
    for (int k = 0; k < K; k++) {
        u[2 * k]         =  wr[k] * cs[k] - wi[k] * sn[k];
        u[M - 1 - 2 * k] = -(wr[k] * sn[k] + wi[k] * cs[k]);
    }
}

/* IMDCT (ISO/IEC 14496-3 §4.6.11.3.1): n0 = N/4 + 1/2 makes the transform a
 * DCT-IV of the coefficients, folded out with its odd/even symmetries. */
static void fast_imdct(const float * restrict in, float * restrict out, int n, float * restrict tmp)
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
static inline float imdct_sample(const float *u, int i, float scale, int frame_len_long)
{
    if (frame_len_long == 960) {
        if (i < 480) return u[480 + i] * scale;
        if (i < 1440) return -u[1439 - i] * scale;
        return -u[i - 1440] * scale;
    }
    if (i < 512) return u[512 + i] * scale;
    if (i < 1536) return -u[1535 - i] * scale;
    return -u[i - 1536] * scale;
}

/* Left half: window, add the previous frame's overlap, emit. Right half:
 * window into the overlap for the next frame. */
static inline void imdct_emit(float * restrict out_pcm, float * restrict overlap, const float *u, float scale,
                              int i0, int i1, const float * restrict wl, int wl_dir, float wflat, int frame_len_long)
{
    /* window sample i is wl[i - i0] (wl_dir > 0), wl[i1 - 1 - i] (< 0) or wflat (wl == NULL) */
    for (int i = i0; i < i1; i++) {
        float w = wl ? (wl_dir > 0 ? wl[i - i0] : wl[i1 - 1 - i]) : wflat;
        float x = imdct_sample(u, i, scale, frame_len_long) * w;
        if (i < frame_len_long) out_pcm[i] = x + overlap[i];
        else overlap[i - frame_len_long] = x;
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

void imdct_and_window(struct faad_decoder *dec, uint32_t ch, uint8_t sequence, uint8_t shape,
                      float * restrict spec, float * restrict pcm)
{
    imdct_window(dec, sequence, shape, dec->prev_window_shape[ch], dec->overlap[ch], spec, pcm);
    dec->prev_window_shape[ch] = shape;
    dec->prev_window_seq[ch] = sequence;
}

#define IMDCT_FRAME_LEN 1024
#define IMDCT_WINDOW_FN imdct_window_1024
#include "imdct_window.inc"
#undef IMDCT_FRAME_LEN
#undef IMDCT_WINDOW_FN

#ifndef FAAD_DISABLE_DAB
#define IMDCT_FRAME_LEN 960
#define IMDCT_WINDOW_FN imdct_window_960
#include "imdct_window.inc"
#undef IMDCT_FRAME_LEN
#undef IMDCT_WINDOW_FN
#endif

void imdct_window(struct faad_decoder *dec, uint8_t window_sequence, uint8_t window_shape,
                  uint8_t prev_shape, float * restrict overlap,
                  float * restrict spec, float * restrict out_pcm)
{
#ifndef FAAD_DISABLE_DAB
    if (FRAME_IS_960(dec)) {
        imdct_window_960(dec, window_sequence, window_shape, prev_shape, overlap, spec, out_pcm);
        return;
    }
#endif
    imdct_window_1024(dec, window_sequence, window_shape, prev_shape, overlap, spec, out_pcm);
}
