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

#ifndef COMMON_COMMON_SYMBOLS_H
#define COMMON_COMMON_SYMBOLS_H

#ifndef COMMON_PREFIX
#error "COMMON_PREFIX must be defined by the consumer"
#endif

#define COMMON_CAT_IMPL(prefix, name) prefix ## name
#define COMMON_CAT(prefix, name) COMMON_CAT_IMPL(prefix, name)

#define fft COMMON_CAT(COMMON_PREFIX, fft)
#define fft_init COMMON_CAT(COMMON_PREFIX, fft_init)
#define num_sfbs_1024 COMMON_CAT(COMMON_PREFIX, num_sfbs_1024)
#define num_sfbs_128 COMMON_CAT(COMMON_PREFIX, num_sfbs_128)
#ifndef FAAD_DISABLE_DAB
#define num_sfbs_960 COMMON_CAT(COMMON_PREFIX, num_sfbs_960)
#define num_sfbs_120 COMMON_CAT(COMMON_PREFIX, num_sfbs_120)
#endif
#define qmf_c COMMON_CAT(COMMON_PREFIX, qmf_c)
#define sbr_offset COMMON_CAT(COMMON_PREFIX, sbr_offset)
#define sfb_offsets_1024 COMMON_CAT(COMMON_PREFIX, sfb_offsets_1024)
#define sfb_offsets_128 COMMON_CAT(COMMON_PREFIX, sfb_offsets_128)
#ifndef FAAD_DISABLE_DAB
#define sfb_offsets_960 COMMON_CAT(COMMON_PREFIX, sfb_offsets_960)
#define sfb_offsets_120 COMMON_CAT(COMMON_PREFIX, sfb_offsets_120)
#endif

#endif
