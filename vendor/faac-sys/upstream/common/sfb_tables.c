/*
 * FAAC - Freeware Advanced Audio Coder
 * ISO/IEC 14496-3 Scale Factor Band (SFB) tables (non-copyrightable facts)
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

/* Scalefactor band offsets per sampling rate. */

#include "sfb_tables.h"

/* 96000 Hz/88200 Hz Long (41 SFBs, 42 entries) */
static const uint16_t sfb_1024_96000[] = {
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56,
    64, 72, 80, 88, 96, 108, 120, 132, 144, 156, 172, 188, 212, 240, 276,
    320, 384, 448, 512, 576, 640, 704, 768, 832, 896, 960, 1024
};

/* 64000 Hz Long (47 SFBs, 48 entries) */
static const uint16_t sfb_1024_64000[] = {
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56,
    64, 72, 80, 88, 100, 112, 124, 140, 156, 172, 192, 216, 240, 268, 304,
    344, 384, 424, 464, 504, 544, 584, 624, 664, 704, 744, 784, 824, 864, 904,
    944, 984, 1024
};

/* 48000 Hz/44100 Hz Long (49 SFBs, 50 entries) */
static const uint16_t sfb_1024_48000[] = {
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72,
    80, 88, 96, 108, 120, 132, 144, 160, 176, 196, 216, 240, 264, 292, 320,
    352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704, 736, 768, 800,
    832, 864, 896, 928, 1024
};

/* 32000 Hz Long (51 SFBs, 52 entries) */
static const uint16_t sfb_1024_32000[] = {
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 48, 56, 64, 72,
    80, 88, 96, 108, 120, 132, 144, 160, 176, 196, 216, 240, 264, 292, 320,
    352, 384, 416, 448, 480, 512, 544, 576, 608, 640, 672, 704, 736, 768, 800,
    832, 864, 896, 928, 960, 992, 1024
};

/* 24000 Hz/22050 Hz Long (47 SFBs, 48 entries) */
static const uint16_t sfb_1024_24000[] = {
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 52, 60, 68,
    76, 84, 92, 100, 108, 116, 124, 136, 148, 160, 172, 188, 204, 220, 240,
    260, 284, 308, 336, 364, 396, 432, 468, 508, 552, 600, 652, 704, 768, 832,
    896, 960, 1024
};

/* 16000 Hz/12000 Hz/11025 Hz Long (43 SFBs, 44 entries) */
static const uint16_t sfb_1024_16000[] = {
    0, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 100, 112, 124,
    136, 148, 160, 172, 184, 196, 212, 228, 244, 260, 280, 300, 320, 344, 368,
    396, 424, 456, 492, 532, 572, 616, 664, 716, 772, 832, 896, 960, 1024
};

/* 8000 Hz Long (40 SFBs, 41 entries) */
static const uint16_t sfb_1024_8000[] = {
    0, 12, 24, 36, 48, 60, 72, 84, 96, 108, 120, 132, 144, 156, 172,
    188, 204, 220, 236, 252, 268, 288, 308, 328, 348, 372, 396, 420, 448, 476,
    508, 544, 580, 620, 664, 712, 764, 820, 880, 944, 1024
};

const uint16_t * const sfb_offsets_1024[12] = {
    sfb_1024_96000, sfb_1024_96000, sfb_1024_64000, sfb_1024_48000,
    sfb_1024_48000, sfb_1024_32000, sfb_1024_24000, sfb_1024_24000,
    sfb_1024_16000, sfb_1024_16000, sfb_1024_16000, sfb_1024_8000
};

const uint8_t num_sfbs_1024[12] = {
    41, 41, 47, 49, 49, 51, 47, 47, 43, 43, 43, 40
};

/* 96000 Hz/88200 Hz/64000 Hz Short (12 SFBs) */
static const uint16_t sfb_128_96000[] = { 0, 4, 8, 12, 16, 20, 24, 32, 40, 48, 64, 92, 128 };
/* 48000 Hz/44100 Hz/32000 Hz Short (14 SFBs) */
static const uint16_t sfb_128_48000[] = { 0, 4, 8, 12, 16, 20, 28, 36, 44, 56, 68, 80, 96, 112, 128 };
/* 24000 Hz/22050 Hz Short (15 SFBs) */
static const uint16_t sfb_128_24000[] = { 0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 64, 76, 92, 108, 128 };
/* 16000 Hz/12000 Hz/11025 Hz Short (15 SFBs) */
static const uint16_t sfb_128_16000[] = { 0, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48, 60, 72, 88, 108, 128 };
/* 8000 Hz Short (15 SFBs) */
static const uint16_t sfb_128_8000[] = { 0, 4, 8, 12, 16, 20, 24, 28, 36, 44, 52, 60, 72, 88, 108, 128 };

const uint16_t * const sfb_offsets_128[12] = {
    sfb_128_96000, sfb_128_96000, sfb_128_96000, sfb_128_48000,
    sfb_128_48000, sfb_128_48000, sfb_128_24000, sfb_128_24000,
    sfb_128_16000, sfb_128_16000, sfb_128_16000, sfb_128_8000
};

const uint8_t num_sfbs_128[12] = {
    12, 12, 12, 14, 14, 14, 15, 15, 15, 15, 15, 15
};
