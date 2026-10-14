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

void bits_init(BitReader *bs, const uint8_t *buffer, uint32_t len)
{
    bs->buffer = buffer;
    bs->len = len;
    bs->byte_pos = 0;
    bs->bit_pos = 0;
}

uint32_t bits_get(BitReader *bs, uint32_t nbits)
{
    if (nbits == 0) return 0;

    if (nbits <= 24 && bs->byte_pos + 4 <= bs->len) {
        const uint8_t *ptr = bs->buffer + bs->byte_pos;
        uint32_t word = ((uint32_t)ptr[0] << 24) | ((uint32_t)ptr[1] << 16) |
                        ((uint32_t)ptr[2] << 8)  | (uint32_t)ptr[3];
        uint32_t val = (word >> (32 - bs->bit_pos - nbits)) & ((1U << nbits) - 1U);
        uint32_t total_bits = bs->bit_pos + nbits;
        bs->byte_pos += total_bits >> 3;
        bs->bit_pos = total_bits & 7;
        return val;
    }

    if (nbits > 32) nbits = 32;

    uint32_t val = 0;
    while (nbits > 0) {
        if (bs->byte_pos >= bs->len) {
            return nbits < 32 ? val << nbits : 0;
        }
        uint32_t bits_left_in_byte = 8 - bs->bit_pos;
        if (nbits <= bits_left_in_byte) {
            uint32_t mask = (1U << nbits) - 1U;
            val = (val << nbits) | ((bs->buffer[bs->byte_pos] >> (bits_left_in_byte - nbits)) & mask);
            bs->bit_pos += nbits;
            if (bs->bit_pos == 8) {
                bs->bit_pos = 0;
                bs->byte_pos++;
            }
            break;
        } else {
            val = (val << bits_left_in_byte) | (bs->buffer[bs->byte_pos] & ((1U << bits_left_in_byte) - 1U));
            nbits -= bits_left_in_byte;
            bs->bit_pos = 0;
            bs->byte_pos++;
        }
    }
    return val;
}

uint32_t bits_show(BitReader *bs, uint32_t nbits)
{
    BitReader tmp = *bs;
    return bits_get(&tmp, nbits);
}

void bits_skip(BitReader *bs, uint32_t nbits)
{
    uint64_t total_bits = (uint64_t)bs->byte_pos * 8 + bs->bit_pos + nbits;
    uint64_t byte_pos = total_bits / 8;
    bs->bit_pos = (uint32_t)(total_bits % 8);
    if (byte_pos >= (uint64_t)bs->len) {
        bs->byte_pos = bs->len;
        bs->bit_pos = 0;
    } else {
        bs->byte_pos = (uint32_t)byte_pos;
    }
}

void bits_byte_align(BitReader *bs)
{
    if (bs->bit_pos != 0) {
        bs->bit_pos = 0;
        bs->byte_pos++;
    }
}

uint32_t bits_get_consumed(BitReader *bs)
{
    return bs->byte_pos * 8 + bs->bit_pos;
}
