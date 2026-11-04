/*
 * FAAM - Freeware Advanced Audio Muxer
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

#ifndef FAAC_COMMON_COVER_ART_H
#define FAAC_COMMON_COVER_ART_H

#include <string.h>
#include "faam.h"

/* Return the ilst data type, or 0 for an unknown image format.
 * Detection identifies a format; it does not decode or validate pixels. */
static inline uint32_t cover_le32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8)
        | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static inline uint8_t faam_detect_cover_type(const uint8_t *data, uint32_t size) {
    if (!data) return 0;
    if (size >= 8 && !memcmp(data, "\x89PNG\r\n\x1a\n", 8)) return FAAM_METADATA_PNG;
    if (size >= 6 && (!memcmp(data, "GIF87a", 6) || !memcmp(data, "GIF89a", 6)))
        return FAAM_METADATA_GIF;
    if (size >= 3 && !memcmp(data, "\xff\xd8\xff", 3)) return FAAM_METADATA_JPEG;
    if (size >= 26 && !memcmp(data, "BM", 2) && !data[6] && !data[7] && !data[8] && !data[9]) {
        uint32_t file_size = cover_le32(data + 2);
        uint32_t pixels = cover_le32(data + 10);
        uint32_t dib = cover_le32(data + 14);
        bool core = dib == 12;
        bool info = dib == 40 || dib == 52 || dib == 56 || dib == 108 || dib == 124;
        if ((core || info) && dib <= size - 14 && pixels >= 14 + dib
            && pixels < file_size && file_size <= size) {
            uint32_t width = core ? (uint32_t)data[18] | ((uint32_t)data[19] << 8) : cover_le32(data + 18);
            uint32_t height = core ? (uint32_t)data[20] | ((uint32_t)data[21] << 8) : cover_le32(data + 22);
            uint32_t planes_at = core ? 22 : 26;
            uint32_t bits = (uint32_t)data[planes_at + 2] | ((uint32_t)data[planes_at + 3] << 8);
            if (width && (core || width <= INT32_MAX) && height
                && (core || height != 0x80000000u)
                && data[planes_at] == 1 && data[planes_at + 1] == 0
                && (bits == 1 || bits == 4 || bits == 8 || bits == 24
                    || (!core && (bits == 16 || bits == 32))))
                return FAAM_METADATA_BMP;
        }
    }
    return 0;
}

#endif /* FAAC_COMMON_COVER_ART_H */
