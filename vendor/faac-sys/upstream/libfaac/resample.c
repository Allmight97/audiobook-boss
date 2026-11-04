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

#include <string.h>

#include "resample.h"
#include "coder.h"
#include "util.h"

/* Equiripple half-band FIR for 2:1 decimation.
 * Leverages the zero-valued odd-indexed taps and symmetric even-indexed taps
 * to reduce the computational load by ~75% compared to a general FIR.
 * The passband is flat within 0.05 dB up to the SBR crossover region,
 * ensuring the core signal remains transparent before SBR reconstruction. */
#define HB_CENTER 0.5015570876767614f
static const resfloat hb_even[RESAMPLE_FILTER_LEN / 2 + 1] = {
    -2.39042884e-03f,  2.03978735e-03f, -2.88625768e-03f,  3.94878764e-03f,
    -5.26747336e-03f,  6.89408424e-03f, -8.89782634e-03f,  1.13774798e-02f,
    -1.44815390e-02f,  1.84491642e-02f, -2.36937924e-02f,  3.10005784e-02f,
    -4.20596122e-02f,  6.12815300e-02f, -1.04870415e-01f,  3.18777389e-01f,
     3.18777389e-01f, -1.04870415e-01f,  6.12815300e-02f, -4.20596122e-02f,
     3.10005784e-02f, -2.36937924e-02f,  1.84491642e-02f, -1.44815390e-02f,
     1.13774798e-02f, -8.89782634e-03f,  6.89408424e-03f, -5.26747336e-03f,
     3.94878764e-03f, -2.88625768e-03f,  2.03978735e-03f, -2.39042884e-03f,
};

Resampler *ResampleInit(int channels)
{
    size_t size = sizeof(Resampler) + channels * sizeof(float[RESAMPLE_FILTER_LEN]);
    Resampler *r = (Resampler *)AllocMemory(size);
    if (!r) return NULL;
    memset(r, 0, size);
    r->channels = channels;
    return r;
}

void ResampleEnd(Resampler *r)
{
    FreeMemory(r);
}

/* Contiguous phases let SIMD lanes retain each output's summation order. */
int Resample(Resampler *r, int input_len)
{
    int output_len = input_len / 2;
    const int H = RESAMPLE_FILTER_LEN - 1;            /* 62 */
    const int HALF = RESAMPLE_FILTER_LEN / 2;         /* 31 */
    int ch, i, j;

    for (ch = 0; ch < r->channels; ch++) {
        float *in = r->fullRate[ch];
        float *out = r->halfRate[ch];
        float * __restrict hist = r->buf[ch];

        /* Fixed-size buffers to avoid VLA (MSVC portability): history + one
         * full-rate HE frame (2 * FRAME_LEN input samples). */
        float even[FRAME_LEN + RESAMPLE_FILTER_LEN / 2];
        float odd[FRAME_LEN + RESAMPLE_FILTER_LEN / 2];
        float next_hist[RESAMPLE_FILTER_LEN - 1];
        int save_hist = input_len - H < output_len;

        /* De-interleave before any aliased output write, so adjacent outputs
         * use unit-stride loads without a full-frame stack copy. */
        for (i = 0; i < HALF; i++) {
            even[i] = hist[2 * i];
            odd[i] = hist[2 * i + 1];
        }
        for (i = 0; i < output_len; i++) {
            even[HALF + i] = in[2 * i];
            odd[HALF + i] = in[2 * i + 1];
        }

        /* Full HE frames leave the history tail beyond the output. Short
         * inputs can overlap it or retain old history, so save that tail now. */
        if (save_hist) {
            if (input_len < H) {
                memcpy(next_hist, hist + input_len, (H - input_len) * sizeof(float));
                memcpy(next_hist + H - input_len, in, input_len * sizeof(float));
            } else {
                memcpy(next_hist, in + input_len - H, H * sizeof(float));
            }
        }

        /* Fold symmetric taps, preserving the four scalar accumulation chains
         * and final grouping for bit-exact results in each SIMD lane. */
        for (i = 0; i < output_len; i++) {
            const float * __restrict c = even + i;
            float a0 = 0, a1 = 0, a2 = 0, a3 = 0;
            for (j = 0; j < 16; j += 4) {
                a0 += hb_even[j + 0] * (c[j + 0] + c[31 - j - 0]);
                a1 += hb_even[j + 1] * (c[j + 1] + c[31 - j - 1]);
                a2 += hb_even[j + 2] * (c[j + 2] + c[31 - j - 2]);
                a3 += hb_even[j + 3] * (c[j + 3] + c[31 - j - 3]);
            }
            *out++ = (a0 + a1) + (a2 + a3) + HB_CENTER * odd[i + HALF / 2];
        }

        memcpy(hist, save_hist ? next_hist : in + input_len - H, H * sizeof(float));
    }

    return output_len;
}
