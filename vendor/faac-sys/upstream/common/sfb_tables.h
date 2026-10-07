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

#ifndef SFB_TABLES_H
#define SFB_TABLES_H

#include <stdint.h>


extern const uint16_t * const sfb_offsets_1024[12];
extern const uint8_t num_sfbs_1024[12];

extern const uint16_t * const sfb_offsets_128[12];
extern const uint8_t num_sfbs_128[12];

#endif /* SFB_TABLES_H */
