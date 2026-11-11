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

#ifndef FAAC_BITS_H
#define FAAC_BITS_H

#include <stdint.h>

/* Number of set bits. A clear-lowest-bit loop measures smaller than the
 * compiler builtins and their portable fallbacks, and the masks are short. */
static inline uint32_t faac_popcount32(uint32_t v)
{
    uint32_t n = 0;
    for (; v; v &= v - 1) n++;
    return n;
}

#endif
