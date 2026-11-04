/*
 * FAAC - Freeware Advanced Audio Coder
 * Copyright (C) 2002 Krzysztof Nikiel
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

#include "common_symbols.h"

#include <math.h>

#include "fft.h"

/* Double-precision pi, for one-time twiddle-table generation where the extra
 * precision is free -- this runs once, never in the per-sample hot path. */
#ifndef M_PI_DOUBLE
#define M_PI_DOUBLE 3.14159265358979323846
#endif

/* Radix-4 twiddles laid out in the order the butterflies consume them: stage
 * by stage, six floats per butterfly (W^j, W^2j, W^3j as cos, -sin). One
 * unit-stride stream instead of three gathers keeps the kernel's inner loop
 * inside the register file. Built once per process and read-only afterwards,
 * so every handle shares them. */

/* Butterflies per transform: n/4 + n/16 + ... over the radix-4 stages. */
#ifndef FAAD_DISABLE_DAB
#define TW_5     (8 + 2)
#else
#define TW_5     0
#endif
#define TW_SHORT (16 + 4 + 1)
#define TW_LONG  (128 + 32 + 8 + 2)

#ifndef FAAD_DISABLE_DAB
#define TW_OFFSET(logm) (6 * ((logm) == FFT_LOGM_5 ? 0 : ((logm) == FFT_LOGM_SHORT ? TW_5 : (TW_5 + TW_SHORT))))
#else
#define TW_OFFSET(logm) (6 * ((logm) == FFT_LOGM_SHORT ? 0 : TW_SHORT))
#endif

static fftfloat twiddles[6 * (TW_5 + TW_SHORT + TW_LONG)];

void fft_init(void)
{
#ifndef FAAD_DISABLE_DAB
    static const unsigned char logms[3] = { FFT_LOGM_5, FFT_LOGM_SHORT, FFT_LOGM_LONG };
    int num_logms = 3;
#else
    static const unsigned char logms[2] = { FFT_LOGM_SHORT, FFT_LOGM_LONG };
    int num_logms = 2;
#endif
    int t;

    for (t = 0; t < num_logms; t++)
    {
        int logm = logms[t];
        int size = 1 << logm;
        fftfloat *tw = twiddles + TW_OFFSET(logm);
        int n2, i;

        for (n2 = size >> 2; n2 >= 1; n2 >>= 2)
        {
            /* stage twiddle W_N^(j*size/(4*n2)) = exp(-2*pi*i*j/(4*n2)) */
            for (i = 0; i < n2; i++)
            {
                int m;
                for (m = 1; m <= 3; m++)
                {
                    double theta = 2.0 * M_PI_DOUBLE * (double)(m * i) / (double)(4 * n2);
                    *tw++ = (fftfloat)cos(theta);
                    *tw++ = (fftfloat)-sin(theta);
                }
            }
        }
    }
}


/* Radix-4 DIF, self-sorting (Stockham). Stage k holds B = 4^k sub-transforms
 * interleaved, element j of sub-transform c at j*B + c, so a butterfly reads
 * the four quarters of src and writes four interleaved runs of dst. The output
 * lands in natural order without a permutation pass. logm=9 (512) isn't a
 * power of 4, so it ends with one radix-2 stage.
 */

#define BUTTERFLY_SUMS \
    float t1 = r1 + r3, t2 = i1 + i3; \
    float t3 = r2 + r4, t4 = i2 + i4; \
    float t5 = r1 - r3, t6 = i1 - i3; \
    float t7 = r2 - r4, t8 = i2 - i4

/* Stage 0: a single sub-transform, so the twiddle changes every butterfly
 * and the four outputs of butterfly j are adjacent at 4j. j=0's identity
 * twiddle isn't special-cased: one butterfly in n/4 isn't worth a second
 * copy of the loop body. */
static void radix4_first_stage(
    const float * restrict sr,
    const float * restrict si,
    float * restrict dr,
    float * restrict di,
    int n4,
    const fftfloat * restrict tw)
{
    int j;

    for (j = 0; j < n4; j++)
    {
        const float c1 = tw[6 * j],     s1 = tw[6 * j + 1];
        const float c2 = tw[6 * j + 2], s2 = tw[6 * j + 3];
        const float c3 = tw[6 * j + 4], s3 = tw[6 * j + 5];
        float * restrict o = dr + 4 * j;
        float * restrict p = di + 4 * j;

        float r1 = sr[j],          i1 = si[j];
        float r2 = sr[j + n4],     i2 = si[j + n4];
        float r3 = sr[j + 2 * n4], i3 = si[j + 2 * n4];
        float r4 = sr[j + 3 * n4], i4 = si[j + 3 * n4];
        BUTTERFLY_SUMS;

        o[0] = t1 + t3;
        p[0] = t2 + t4;

        r1 = t1 - t3; i1 = t2 - t4;
        r2 = t5 + t8; i2 = t6 - t7;
        r3 = t5 - t8; i3 = t6 + t7;

        o[1] = r2 * c1 - i2 * s1;
        p[1] = r2 * s1 + i2 * c1;
        o[2] = r1 * c2 - i1 * s2;
        p[2] = r1 * s2 + i1 * c2;
        o[3] = r3 * c3 - i3 * s3;
        p[3] = r3 * s3 + i3 * c3;
    }
}

/* B butterflies sharing one twiddle: element c of each of the four input
 * quarters to element c of each of the four output runs. A restrict pointer
 * per output run tells the vectorizer the runs can't overlap. */
static inline void radix4_butterflies(
    const float * restrict sr, const float * restrict si, int n4,
    float * restrict o1r, float * restrict o1i,
    float * restrict o2r, float * restrict o2i,
    float * restrict o3r, float * restrict o3i,
    float * restrict o4r, float * restrict o4i,
    int B,
    const fftfloat * restrict tw)
{
    const float c1 = tw[0], s1 = tw[1];
    const float c2 = tw[2], s2 = tw[3];
    const float c3 = tw[4], s3 = tw[5];
    int c;

    for (c = 0; c < B; c++)
    {
        float r1 = sr[c],          i1 = si[c];
        float r2 = sr[c + n4],     i2 = si[c + n4];
        float r3 = sr[c + 2 * n4], i3 = si[c + 2 * n4];
        float r4 = sr[c + 3 * n4], i4 = si[c + 3 * n4];
        BUTTERFLY_SUMS;

        o1r[c] = t1 + t3;
        o1i[c] = t2 + t4;

        r1 = t1 - t3; i1 = t2 - t4;
        r2 = t5 + t8; i2 = t6 - t7;
        r3 = t5 - t8; i3 = t6 + t7;

        o2r[c] = r2 * c1 - i2 * s1;
        o2i[c] = r2 * s1 + i2 * c1;
        o3r[c] = r1 * c2 - i1 * s2;
        o3i[c] = r1 * s2 + i1 * c2;
        o4r[c] = r3 * c3 - i3 * s3;
        o4i[c] = r3 * s3 + i3 * c3;
    }
}

/* Later stages: B = 4*B4 sub-transforms share each twiddle, so the inner
 * loop is unit-stride over c. Passing B4 rather than B lets the vectorizer
 * see the trip count is a multiple of four and skip the epilogue. */
static void radix4_stage(
    const float * restrict sr,
    const float * restrict si,
    float * restrict dr,
    float * restrict di,
    int n4,
    int B4,
    const fftfloat * restrict tw)
{
    const int B = 4 * B4;
    int n2 = n4 / B;
    int j;

    for (j = 0; j < n2; j++, tw += 6, sr += B, si += B, dr += 4 * B, di += 4 * B)
        radix4_butterflies(sr, si, n4, dr, di, dr + B, di + B,
                           dr + 2 * B, di + 2 * B, dr + 3 * B, di + 3 * B,
                           B, tw);
}

/* odd logm: 4^k can't fill it, one radix-2 stage mops up the remainder */
static void radix2_stage(
    const float * restrict r1p, const float * restrict i1p,
    const float * restrict r2p, const float * restrict i2p,
    float * restrict o1r, float * restrict o1i,
    float * restrict o2r, float * restrict o2i,
    int n2)
{
    int c;

    for (c = 0; c < n2; c++)
    {
        float r1 = r1p[c], i1 = i1p[c];
        float r2 = r2p[c], i2 = i2p[c];
        o1r[c] = r1 + r2; o1i[c] = i1 + i2;
        o2r[c] = r1 - r2; o2i[c] = i1 - i2;
    }
}

void fft(float * restrict x, float * restrict y, int logm)
{
    const fftfloat *tw = twiddles + TW_OFFSET(logm);
    int n = 1 << logm;
    int n4 = n >> 2;
    float *src = y, *dst = x;
    int B4;

    /* Stages ping-pong x -> y -> x ...; both sizes take an odd number of
     * them, so the result lands in y. */
    radix4_first_stage(x, x + n, y, y + n, n4, tw);
    tw += 6 * n4;
    for (B4 = 1; 4 * B4 <= n4; B4 *= 4)
    {
        float *t;
        radix4_stage(src, src + n, dst, dst + n, n4, B4, tw);
        tw += 6 * (n4 / (4 * B4));
        t = src; src = dst; dst = t;
    }
    /* Odd logm stops at 4*B4 = n/2 sub-transforms of length 2, whose two
     * elements sit n/2 apart: one radix-2 stage pairs them. */
    if (4 * B4 < n)
        radix2_stage(x, x + n, x + n / 2, x + n + n / 2,
                     y, y + n, y + n / 2, y + n + n / 2, 4 * B4);
}

static inline void dft3(float r0, float i0, float r1, float i1, float r2, float i2,
                        float *or, float *oi)
{
    float a_r = r1 + r2, a_i = i1 + i2;
    float b_r = (i1 - i2) * 0.8660254037844386f, b_i = (r2 - r1) * 0.8660254037844386f;
    or[0] = r0 + a_r;
    oi[0] = i0 + a_i;
    float m_r = r0 - 0.5f * a_r, m_i = i0 - 0.5f * a_i;
    or[1] = m_r + b_r;
    oi[1] = m_i + b_i;
    or[2] = m_r - b_r;
    oi[2] = m_i - b_i;
}

static inline void dft5(const float *ir, const float *ii, float *or, float *oi)
{
    static const float c1 = 0.3090169943749474f,  c2 = -0.8090169943749475f;
    static const float s1 = 0.9510565162951535f,  s2 =  0.5877852522924731f;

    float a1_r = ir[1] + ir[4], a1_i = ii[1] + ii[4];
    float a2_r = ir[2] + ir[3], a2_i = ii[2] + ii[3];
    float b1_r = ii[4] - ii[1], b1_i = ir[1] - ir[4];
    float b2_r = ii[3] - ii[2], b2_i = ir[2] - ir[3];

    float r0 = ir[0], i0 = ii[0];
    or[0] = r0 + a1_r + a2_r;
    oi[0] = i0 + a1_i + a2_i;

    or[1] = r0 + c1 * a1_r + c2 * a2_r - s1 * b1_r - s2 * b2_r;
    oi[1] = i0 + c1 * a1_i + c2 * a2_i - s1 * b1_i - s2 * b2_i;

    or[2] = r0 + c2 * a1_r + c1 * a2_r - s2 * b1_r + s1 * b2_r;
    oi[2] = i0 + c2 * a1_i + c1 * a2_i - s2 * b1_i + s1 * b2_i;

    or[3] = r0 + c2 * a1_r + c1 * a2_r + s2 * b1_r - s1 * b2_r;
    oi[3] = i0 + c2 * a1_i + c1 * a2_i + s2 * b1_i - s1 * b2_i;

    or[4] = r0 + c1 * a1_r + c2 * a2_r + s1 * b1_r + s2 * b2_r;
    oi[4] = i0 + c1 * a1_i + c2 * a2_i + s1 * b1_i + s2 * b2_i;
}

static inline void dft15(const float *ir, const float *ii, float *or, float *oi)
{
    float t1_r[3][5], t1_i[3][5];
    static const unsigned char pfa15_in[3][5] = {
        { 0,  3,  6,  9, 12},
        { 5,  8, 11, 14,  2},
        {10, 13,  1,  4,  7}
    };
    for (int j = 0; j < 5; j++) {
        int idx0 = pfa15_in[0][j], idx1 = pfa15_in[1][j], idx2 = pfa15_in[2][j];
        float col_r[3], col_i[3];
        dft3(ir[idx0], ii[idx0], ir[idx1], ii[idx1], ir[idx2], ii[idx2], col_r, col_i);
        t1_r[0][j] = col_r[0]; t1_i[0][j] = col_i[0];
        t1_r[1][j] = col_r[1]; t1_i[1][j] = col_i[1];
        t1_r[2][j] = col_r[2]; t1_i[2][j] = col_i[2];
    }
    float t2_r[3][5], t2_i[3][5];
    for (int i = 0; i < 3; i++) {
        dft5(t1_r[i], t1_i[i], t2_r[i], t2_i[i]);
    }
    static const unsigned char pfa15_out[3][5] = {
        { 0,  6, 12,  3,  9},
        {10,  1,  7, 13,  4},
        { 5, 11,  2,  8, 14}
    };
    for (int ka = 0; ka < 3; ka++) {
        for (int kb = 0; kb < 5; kb++) {
            int out_idx = pfa15_out[ka][kb];
            or[out_idx] = t2_r[ka][kb];
            oi[out_idx] = t2_i[ka][kb];
        }
    }
}

static inline void dft4(const float *ir, const float *ii, float *or, float *oi)
{
    float r0 = ir[0], i0 = ii[0];
    float r1 = ir[1], i1 = ii[1];
    float r2 = ir[2], i2 = ii[2];
    float r3 = ir[3], i3 = ii[3];

    float a_r = r0 + r2, a_i = i0 + i2;
    float b_r = r1 + r3, b_i = i1 + i3;
    float c_r = r0 - r2, c_i = i0 - i2;
    float d_r = i1 - i3, d_i = r3 - r1;

    or[0] = a_r + b_r; oi[0] = a_i + b_i;
    or[2] = a_r - b_r; oi[2] = a_i - b_i;
    or[1] = c_r + d_r; oi[1] = c_i + d_i;
    or[3] = c_r - d_r; oi[3] = c_i - d_i;
}

#ifndef FAAD_DISABLE_DAB
void fft60(float * restrict x, float * restrict y)
{
    const float *xr = x, *xi = x + 60;
    float t1_r[15][4], t1_i[15][4];

    for (int n1 = 0; n1 < 15; n1++) {
        float in_r[4], in_i[4], out_r[4], out_i[4];
        for (int n2 = 0; n2 < 4; n2++) {
            int idx = (4 * n1 + 15 * n2) % 60;
            in_r[n2] = xr[idx];
            in_i[n2] = xi[idx];
        }
        dft4(in_r, in_i, out_r, out_i);
        for (int n2 = 0; n2 < 4; n2++) {
            t1_r[n1][n2] = out_r[n2];
            t1_i[n1][n2] = out_i[n2];
        }
    }

    /* Each column is consumed before its results replace it. */
    for (int n2 = 0; n2 < 4; n2++) {
        float in_r[15], in_i[15], out_r[15], out_i[15];
        for (int n1 = 0; n1 < 15; n1++) {
            in_r[n1] = t1_r[n1][n2];
            in_i[n1] = t1_i[n1][n2];
        }
        dft15(in_r, in_i, out_r, out_i);
        for (int n1 = 0; n1 < 15; n1++) {
            t1_r[n1][n2] = out_r[n1];
            t1_i[n1][n2] = out_i[n1];
        }
    }

    float *yr = y, *yi = y + 60;
    for (int k1 = 0; k1 < 15; k1++) {
        for (int k2 = 0; k2 < 4; k2++) {
            int out_idx = (16 * k1 + 45 * k2) % 60;
            yr[out_idx] = t1_r[k1][k2];
            yi[out_idx] = t1_i[k1][k2];
        }
    }
}

void fft480(float * restrict x, float * restrict y)
{
    const float *xr = x, *xi = x + 480;
    float t1_r[15][32], t1_i[15][32];

    for (int n1 = 0; n1 < 15; n1++) {
        float col_buf[64], col_work[64];
        for (int n2 = 0; n2 < 32; n2++) {
            int idx = (32 * n1 + 15 * n2) % 480;
            col_buf[n2] = xr[idx];
            col_buf[32 + n2] = xi[idx];
        }
        fft(col_buf, col_work, 5);
        for (int n2 = 0; n2 < 32; n2++) {
            t1_r[n1][n2] = col_work[n2];
            t1_i[n1][n2] = col_work[32 + n2];
        }
    }

    /* Each column is consumed before its results replace it. */
    for (int n2 = 0; n2 < 32; n2++) {
        float in_r[15], in_i[15], out_r[15], out_i[15];
        for (int n1 = 0; n1 < 15; n1++) {
            in_r[n1] = t1_r[n1][n2];
            in_i[n1] = t1_i[n1][n2];
        }
        dft15(in_r, in_i, out_r, out_i);
        for (int n1 = 0; n1 < 15; n1++) {
            t1_r[n1][n2] = out_r[n1];
            t1_i[n1][n2] = out_i[n1];
        }
    }

    float *yr = y, *yi = y + 480;
    for (int k1 = 0; k1 < 15; k1++) {
        for (int k2 = 0; k2 < 32; k2++) {
            int out_idx = (256 * k1 + 225 * k2) % 480;
            yr[out_idx] = t1_r[k1][k2];
            yi[out_idx] = t1_i[k1][k2];
        }
    }
}
#endif
