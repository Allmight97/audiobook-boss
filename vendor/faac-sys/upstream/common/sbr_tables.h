/*
 * FAAC - Freeware Advanced Audio Coder
 * SBR tables reproduced from ISO/IEC 14496-3 (non-copyrightable facts)
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

/* Shared SBR tables: QMF prototype filter and frequency-band offsets.
 * All values are normative data from ISO/IEC 14496-3:2005. */

#ifndef SBR_TABLES_H
#define SBR_TABLES_H

#include <stdint.h>


#ifdef __cplusplus
extern "C" {
#endif

typedef float sbrfloat;

extern const sbrfloat qmf_c[640];
extern const int8_t sbr_offset[6][16];

#ifdef __cplusplus
}
#endif

#endif /* SBR_TABLES_H */
