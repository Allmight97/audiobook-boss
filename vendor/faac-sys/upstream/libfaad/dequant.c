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

void apply_pns(ICSInfo *ics, float *spec, uint32_t *pns_seed)
{
    int window_offset = 0;
    uint32_t seed = *pns_seed;

    for (int g = 0; g < ics->num_window_groups && g < 8; g++) {
        int max_sfb = ics->max_sfb < ics->num_sfbs ? ics->max_sfb : ics->num_sfbs;
        if (max_sfb > MAX_SFB) max_sfb = MAX_SFB;
        for (int sfb = 0; sfb < max_sfb; sfb++) {
            if (ics->sfb_cb[g][sfb] == 13) {
                /* §4.6.13.3: the band's summed energy is 2^(noise_nrg/2), with
                 * no SF_OFFSET -- noise_nrg is not a scalefactor. */
                int nrg = ics->scalefactors[g][sfb];
                if (nrg < -120) nrg = -120;
                if (nrg > 120) nrg = 120;
                float scale = powf(2.0f, 0.25f * (float)nrg);

                int start_k = ics->sfb_offsets[sfb];
                int end_k = ics->sfb_offsets[sfb + 1];
                if (start_k >= FRAME_LEN_LONG) continue;
                if (end_k > FRAME_LEN_LONG) end_k = FRAME_LEN_LONG;
                int len = end_k - start_k;

                for (int w = 0; w < ics->window_group_length[g]; w++) {
                    int win_offset_k = (window_offset + w) * 128 + start_k;
                    if (win_offset_k < 0 || win_offset_k + len > FRAME_LEN_LONG) continue;
                    float * restrict ptr = spec + win_offset_k;
                    float energy = 0.0f;

                    for (int k = 0; k < len; k++) {
                        seed = (seed * 1664525U) + 1013904223U;
                        float noise = ((float)(int32_t)seed) * (1.0f / 2147483648.0f);
                        ptr[k] = noise;
                        energy += noise * noise;
                    }

                    if (energy > 0.0f) {
                        float norm = scale / sqrtf(energy);
                        for (int k = 0; k < len; k++) {
                            ptr[k] *= norm;
                        }
                    }
                }
            }
        }
        window_offset += ics->window_group_length[g];
    }
    *pns_seed = seed;
}
