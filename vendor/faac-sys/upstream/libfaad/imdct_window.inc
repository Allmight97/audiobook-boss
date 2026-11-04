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

/* Body of imdct_window() for one frame length, included once per length with
 * IMDCT_FRAME_LEN a literal so the loops and tables fold without relying on
 * the compiler to clone a large function. */
static void IMDCT_WINDOW_FN(struct faad_decoder *dec, uint8_t window_sequence, uint8_t window_shape,
                  uint8_t prev_shape, float * restrict overlap,
                  float * restrict spec, float * restrict out_pcm)
{
    float * restrict work = dec->scratch->work;
    const int frame_len_long = IMDCT_FRAME_LEN;
    int short_len = (frame_len_long == 960) ? 120 : 128;
    int short_win_len = 2 * short_len;
    int long_win_len = 2 * frame_len_long;

#ifndef FAAD_DISABLE_DAB
    const float * restrict win_long_l = (prev_shape == KBD_WINDOW) ? ((frame_len_long == 960) ? kbd_window_1920 : kbd_window_2048) : ((frame_len_long == 960) ? sine_window_1920 : sine_window_2048);
    const float * restrict win_short_l = (prev_shape == KBD_WINDOW) ? ((frame_len_long == 960) ? kbd_window_240 : kbd_window_256) : ((frame_len_long == 960) ? sine_window_240 : sine_window_256);
    const float * restrict win_long = (window_shape == KBD_WINDOW) ? ((frame_len_long == 960) ? kbd_window_1920 : kbd_window_2048) : ((frame_len_long == 960) ? sine_window_1920 : sine_window_2048);
    const float * restrict win_short = (window_shape == KBD_WINDOW) ? ((frame_len_long == 960) ? kbd_window_240 : kbd_window_256) : ((frame_len_long == 960) ? sine_window_240 : sine_window_256);
#else
    const float * restrict win_long_l = (prev_shape == KBD_WINDOW) ? kbd_window_2048 : sine_window_2048;
    const float * restrict win_short_l = (prev_shape == KBD_WINDOW) ? kbd_window_256 : sine_window_256;
    const float * restrict win_long = (window_shape == KBD_WINDOW) ? kbd_window_2048 : sine_window_2048;
    const float * restrict win_short = (window_shape == KBD_WINDOW) ? kbd_window_256 : sine_window_256;
#endif


    if (window_sequence == EIGHT_SHORT_SEQUENCE) {
        int n_flat = (frame_len_long - short_len) / 2; /* 448 for 1024, 420 for 960 */
        int acc_len = 8 * short_len + short_len;       /* 1152 for 1024, 1080 for 960 */
        float * restrict acc = work;
        float * restrict block = work + acc_len;
        float * restrict tmp = work + acc_len + short_win_len;
        memset(acc, 0, sizeof(float) * acc_len);

        for (int w = 0; w < 8; w++) {
            fast_imdct(spec + w * short_len, block, short_win_len, tmp);
            const float * restrict wl = (w == 0) ? win_short_l : win_short;
            float *dst = acc + w * short_len;
            for (int i = 0; i < short_len; i++) {
                dst[i]                       += block[i] * wl[i];
                dst[short_win_len - 1 - i] += block[short_win_len - 1 - i] * win_short[i];
            }
        }

        for (int i = 0; i < n_flat; i++) out_pcm[i] = overlap[i];
        for (int i = n_flat; i < frame_len_long; i++) out_pcm[i] = acc[i - n_flat] + overlap[i];

        int remaining = acc_len - (frame_len_long - n_flat); /* 576 for 1024, 540 for 960 */
        for (int i = 0; i < remaining; i++) overlap[i] = acc[i + (frame_len_long - n_flat)];
        memset(overlap + remaining, 0, sizeof(float) * n_flat);
        return;
    }

    float * restrict u = work;
    const float scale = 2.0f / (float)long_win_len;
    dct4(spec, work, frame_len_long);

    if (window_sequence == ONLY_LONG_SEQUENCE) {
        if (frame_len_long == 1024) {
            imdct_emit_long(out_pcm, overlap, u, scale, win_long_l, win_long);
        } else {
            for (int i = 0; i < 480; i++)     out_pcm[i] = u[480 + i] * scale * win_long_l[i] + overlap[i];
            for (int i = 480; i < 960; i++)   out_pcm[i] = -u[1439 - i] * scale * win_long_l[i] + overlap[i];
            for (int i = 960; i < 1440; i++)  overlap[i - 960] = -u[1439 - i] * scale * win_long[1919 - i];
            for (int i = 1440; i < 1920; i++) overlap[i - 960] = -u[i - 1440] * scale * win_long[1919 - i];
        }
        return;
    }

    int n_flat = (frame_len_long - short_len) / 2; /* 448 or 420 */

    if (window_sequence == LONG_STOP_SEQUENCE) {
        for (int i = 0; i < n_flat; i++) out_pcm[i] = overlap[i];
        imdct_emit(out_pcm, overlap, u, scale, n_flat, n_flat + short_len, win_short_l, 1, 0.0f, frame_len_long);
        imdct_emit(out_pcm, overlap, u, scale, n_flat + short_len, frame_len_long, NULL, 0, 1.0f, frame_len_long);
    } else {
        imdct_emit(out_pcm, overlap, u, scale, 0, frame_len_long, win_long_l, 1, 0.0f, frame_len_long);
    }

    if (window_sequence == LONG_START_SEQUENCE) {
        imdct_emit(out_pcm, overlap, u, scale, frame_len_long, frame_len_long + n_flat, NULL, 0, 1.0f, frame_len_long);
        imdct_emit(out_pcm, overlap, u, scale, frame_len_long + n_flat, frame_len_long + n_flat + short_len, win_short, -1, 0.0f, frame_len_long);
        memset(overlap + (long_win_len - frame_len_long - n_flat), 0, sizeof(float) * n_flat);
    } else {
        imdct_emit(out_pcm, overlap, u, scale, frame_len_long, long_win_len, win_long, -1, 0.0f, frame_len_long);
    }
}
