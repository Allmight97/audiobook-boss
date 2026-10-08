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
#include <stdbool.h>
#include <string.h>

#include "coder.h"
#include "filtbank.h"
#include "frame.h"
#include "fft.h"
#include "util.h"

/* Sine windows, ISO/IEC 13818-7 4.6.4, and the MDCT pre/post-twiddles
 * cos/sin(freq*(i+1/8)) for both block sizes, short slice first. Built once
 * per process in double, rounded to float when stored, and shared read-only
 * by every handle. A twiddle table breaks the serial cos/sin recurrence that
 * kept the MDCT twiddle loops from vectorizing, and is more accurate. The two
 * long slopes sit in one array indexed by ics_info window_shape, so picking a
 * window is address arithmetic, not a pointer select the compiler would unswitch
 * the windowing loops on. */
static float window_long[2][BLOCK_LEN_LONG];
static float sin_window_short[BLOCK_LEN_SHORT];
static fftfloat mdct_cos[FFT_TBL_LEN];
static fftfloat mdct_sin[FFT_TBL_LEN];

/* Kaiser-Bessel-derived window, alpha 4, rising half, ISO/IEC 14496-3
 * 4.6.11.3.2: the cumulative Kaiser kernel, normalised, under a square root.
 * The kernel is evaluated once into w[] and accumulated in place; its last
 * point (v = 1) is I0(0) = 1. With v = 2i/n - 1, 1 - v^2 = 4i(n - i)/n^2. */
static void kbd_window(float *w, int n)
{
    const double c = 16.0 * M_PI_DOUBLE * M_PI_DOUBLE / ((double)n * n);
    double sum = 1.0, run = 0.0;
    int i;

    for (i = 0; i < n; i++) {
        double q = c * (i * (n - i));
        double term = 1.0, k = 1.0;
        int m;

        for (m = 1; term > k * 1e-17; m++) {
            term *= q / ((double)m * m);
            k += term;
        }
        w[i] = (float)k;
        sum += k;
    }
    for (i = 0; i < n; i++) {
        run += w[i];
        /* fabs: the quotient is never negative, which tells the compiler sqrt
         * needs no errno path (a libm call inside the loop, with its spills). */
        w[i] = (float)sqrt(fabs(run / sum));
    }
}

void FilterBankTablesInit(void)
{
    static const unsigned char logms[2] = { FFT_LOGM_SHORT, FFT_LOGM_LONG };
    int t;

    /* One loop body for both sizes: two constant-argument calls would be
     * cloned and unrolled separately under LTO. */
    kbd_window(window_long[KBD_WINDOW], BLOCK_LEN_LONG);

    for (t = 0; t < 2; t++)
    {
        int logm = logms[t];
        int size = 1 << logm;
        int halfLen = 2 * size;
        float *win = (logm == FFT_LOGM_SHORT) ? sin_window_short : window_long[SINE_WINDOW];
        int off = FFT_TBL_OFFSET(logm);
        double freq = 2.0 * M_PI_DOUBLE / (double)(4 * size);
        int i;

        for (i = 0; i < halfLen; i++)
            win[i] = (float)sin((M_PI_DOUBLE / (2 * halfLen)) * (i + 0.5));

        for (i = 0; i < size; i++)
        {
            double theta = freq * ((double)i + 0.125);
            mdct_cos[off + i] = (fftfloat)cos(theta);
            mdct_sin[off + i] = (fftfloat)sin(theta);
        }
    }
}

int FilterBankInit(faacEncStruct* hEncoder)
{
    unsigned int channel;

    for (channel = 0; channel < hEncoder->numChannels; channel++) {
        hEncoder->freqBuff[channel] = (float*)AllocMemory(2*FRAME_LEN*sizeof(float));
        if (!hEncoder->freqBuff[channel]) return 0;
    }

    hEncoder->gpsyInfo.sharedWorkBuffLong = (float*)AllocMemory(2*BLOCK_LEN_LONG*sizeof(float));
    return hEncoder->gpsyInfo.sharedWorkBuffLong != NULL;
}

void FilterBankEnd(faacEncStruct* hEncoder)
{
    unsigned int channel;

    for (channel = 0; channel < hEncoder->numChannels; channel++) {
        if (hEncoder->freqBuff[channel]) FreeMemory(hEncoder->freqBuff[channel]);
    }

    if (hEncoder->gpsyInfo.sharedWorkBuffLong) FreeMemory(hEncoder->gpsyInfo.sharedWorkBuffLong);
}

/* Four ICS window sequences, ISO/IEC 13818-7 4.3.2.4.
 * Applying sine windowing directly in vectorizable loops without indirect struct dispatch. */

static inline void ApplyWindowDirect(float * restrict dst,
                                     const float * restrict src,
                                     const float * restrict win,
                                     int len)
{
    int i;
    for (i = 0; i < len; i++) {
        dst[i] = src[i] * win[i];
    }
}

static inline void ApplyWindowReverse(float * restrict dst,
                                      const float * restrict src,
                                      const float * restrict win,
                                      int len)
{
    int i;
    for (i = 0; i < len; i++) {
        dst[i] = src[i] * win[len - 1 - i];
    }
}

static inline void CopyFlat(float * restrict dst, const float * restrict src, int len)
{
    memcpy(dst, src, len * sizeof(float));
}

static inline void ZeroFlat(float * restrict dst, int len)
{
    memset(dst, 0, len * sizeof(float));
}

/* Left long half is windowed with the previous frame's shape and the right
 * with this frame's, as the decoder does; short slopes are always sine. */
static void WindowAndMdct(faacEncStruct* hEncoder, int block_type, int lshape, int rshape,
                          const float * restrict p_prev_data,
                          const float * restrict p_in_data,
                          float * restrict p_out_mdct)
{
    float * restrict overlapBuf = hEncoder->gpsyInfo.sharedWorkBuffLong;
    const float *winL = window_long[lshape];
    const float *winR = window_long[rshape];
    int k;

    /* Assemble the 2048-sample overlap window from the previous and
       current frame's time-domain samples. */
    memcpy(overlapBuf, p_prev_data, BLOCK_LEN_LONG*sizeof(float));
    memcpy(overlapBuf+BLOCK_LEN_LONG, p_in_data, BLOCK_LEN_LONG*sizeof(float));

    switch (block_type) {
    case ONLY_LONG_WINDOW: {
        ApplyWindowDirect(p_out_mdct, overlapBuf, winL, BLOCK_LEN_LONG);
        ApplyWindowReverse(p_out_mdct+BLOCK_LEN_LONG, overlapBuf+BLOCK_LEN_LONG, winR, BLOCK_LEN_LONG);
        MDCT(p_out_mdct, 2*BLOCK_LEN_LONG, hEncoder->gpsyInfo.sharedWorkBuffLong);
        break;
    }

    case LONG_SHORT_WINDOW: {
        ApplyWindowDirect(p_out_mdct, overlapBuf, winL, BLOCK_LEN_LONG);
        CopyFlat(p_out_mdct+BLOCK_LEN_LONG, overlapBuf+BLOCK_LEN_LONG, NFLAT_LS);
        ApplyWindowReverse(p_out_mdct+BLOCK_LEN_LONG+NFLAT_LS, overlapBuf+BLOCK_LEN_LONG+NFLAT_LS, sin_window_short, BLOCK_LEN_SHORT);
        ZeroFlat(p_out_mdct+BLOCK_LEN_LONG+NFLAT_LS+BLOCK_LEN_SHORT, NFLAT_LS);
        MDCT(p_out_mdct, 2*BLOCK_LEN_LONG, hEncoder->gpsyInfo.sharedWorkBuffLong);
        break;
    }

    case SHORT_LONG_WINDOW: {
        ZeroFlat(p_out_mdct, NFLAT_LS);
        ApplyWindowDirect(p_out_mdct+NFLAT_LS, overlapBuf+NFLAT_LS, sin_window_short, BLOCK_LEN_SHORT);
        CopyFlat(p_out_mdct+NFLAT_LS+BLOCK_LEN_SHORT, overlapBuf+NFLAT_LS+BLOCK_LEN_SHORT, NFLAT_LS);
        ApplyWindowReverse(p_out_mdct+BLOCK_LEN_LONG, overlapBuf+BLOCK_LEN_LONG, winR, BLOCK_LEN_LONG);
        MDCT(p_out_mdct, 2*BLOCK_LEN_LONG, hEncoder->gpsyInfo.sharedWorkBuffLong);
        break;
    }

    case ONLY_SHORT_WINDOW: {
        const float * restrict win = sin_window_short;
        float * restrict src = overlapBuf + NFLAT_LS;
        float * restrict dst = p_out_mdct;

        for (k = 0; k < MAX_SHORT_WINDOWS; k++) {
            ApplyWindowDirect(dst, src, win, BLOCK_LEN_SHORT);
            ApplyWindowReverse(dst+BLOCK_LEN_SHORT, src+BLOCK_LEN_SHORT, win, BLOCK_LEN_SHORT);
            MDCT(dst, 2*BLOCK_LEN_SHORT, hEncoder->gpsyInfo.sharedWorkBuffLong);

            dst += BLOCK_LEN_SHORT;
            src += BLOCK_LEN_SHORT;
        }
        break;
    }
    }
}

/* The LC core's long windows are Kaiser-Bessel-derived. A sine window's
 * sidelobes spread a loud, stationary low-frequency tone across the whole
 * spectrum as a smooth skirt. A clean source's own floor is far below it, so
 * the skirt is the loudest thing above the bass, and it can only be coded or
 * discarded: discarded, it comes back as broadband haze (and a smooth skirt is
 * what TNS fits at absurd gain). A KBD window falls off far faster, so the skirt
 * is not there. Elsewhere the two shapes trade resolution against leakage about
 * evenly, so no content test is made. The HE core keeps sine long windows: KBD
 * there costs more above 10 kHz than it saves below.
 *
 * Every channel's spectrum ends up in freqBuff. Both channels of an element
 * share one shape (common_window carries a single ics_info). Shapes follow the
 * decoder's rule: the left long half uses the previous frame's shape, the right
 * half this frame's. A frame that ends in a short slope stays sine, since short
 * windows are always sine here. */
void FilterBankElement(faacEncStruct* hEncoder, CoderInfo *coderInfo, const AACElement *el)
{
    int nch = (el->type == ID_CPE) ? 2 : 1;
    int bt = coderInfo[el->channels[0]].block_type;
    int prev = coderInfo[el->channels[0]].window_shape;
    int shape = (el->type != ID_LFE) && hEncoder->config.aacObjectType != HE_V1 &&
                (bt == ONLY_LONG_WINDOW || bt == SHORT_LONG_WINDOW) ? KBD_WINDOW : SINE_WINDOW;
    int c;

    for (c = 0; c < nch; c++) {
        int ch = el->channels[c];

        WindowAndMdct(hEncoder, coderInfo[ch].block_type, prev, shape,
                      hEncoder->audioFIFO[ch][FIFO_PAST],
                      hEncoder->audioFIFO[ch][FIFO_CURR], hEncoder->freqBuff[ch]);
        coderInfo[ch].window_shape = shape;
    }
}

void MDCT( float * restrict data, int N, float * restrict work )
{
    const int N2 = N >> 1;
    const int N4 = N >> 2;
    const int N8 = N >> 3;
    const int logm = (N == 2 * BLOCK_LEN_LONG) ? FFT_LOGM_LONG : FFT_LOGM_SHORT;

    const fftfloat * restrict cosT = mdct_cos + FFT_TBL_OFFSET(logm);
    const fftfloat * restrict sinT = mdct_sin + FFT_TBL_OFFSET(logm);

    /* work holds N floats: the fold's complex input in the first half, the
       FFT's natural-order output in the second. */
    float * restrict xr = work;
    float * restrict xi = work + N4;
    const float * restrict yr = work + N2;
    const float * restrict yi = work + N2 + N4;

    int i;

    /* Sign pattern flips at N/8 - the real input's symmetry folds
       differently on either side of that midpoint. */
    for (i = 0; i < N8; i++) {
        int n1 = N2 - 1 - 2*i;
        int n2 = 2*i;
        float foldedRe = data[N4 + n1] + data[N + N4 - 1 - n1];
        float foldedIm = data[N4 + n2] - data[N4 - 1 - n2];

        xr[i] = foldedRe * cosT[i] + foldedIm * sinT[i];
        xi[i] = foldedIm * cosT[i] - foldedRe * sinT[i];
    }
    for (; i < N4; i++) {
        int n1 = N2 - 1 - 2*i;
        int n2 = 2*i;
        float foldedRe = data[N4 + n1] - data[N4 - 1 - n1];
        float foldedIm = data[N4 + n2] + data[N + N4 - 1 - n2];

        xr[i] = foldedRe * cosT[i] + foldedIm * sinT[i];
        xi[i] = foldedIm * cosT[i] - foldedRe * sinT[i];
    }

    fft(work, work + N2, logm);

    /* Unfold N/4 complex FFT outputs into N real coefficients, one write
       per output quarter. */
    for (i = 0; i < N4; i++) {
        int n2 = 2*i;
        float unfoldRe = 2.0f * (yr[i] * cosT[i] + yi[i] * sinT[i]);
        float unfoldIm = 2.0f * (yi[i] * cosT[i] - yr[i] * sinT[i]);

        data[n2]             = -unfoldRe;
        data[N2 - 1 - n2]    =  unfoldIm;
        data[N2 + n2]        = -unfoldIm;
        data[N - 1 - n2]     =  unfoldRe;
    }
}
