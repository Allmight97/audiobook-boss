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
#include "asc_codec.h"

/* SFB tables cover only the first 12 rates; any other rate (7350 Hz
 * included) has no layout to decode with, so it is -1 and never guessed. */
int get_sr_index(uint32_t sample_rate)
{
    for (int i = 0; i < 12; i++) {
        if (asc_codec_sample_rates[i] == sample_rate) {
            return i;
        }
    }
    return -1;
}

/* The shared ASC helper accepts zero-filled truncated syntax for container
 * probing; decoder initialization must not, so it is parsed here instead. */
static faad_status parse_asc(const uint8_t *buf, uint32_t len, AudioSpecificConfig *asc, bool *frame_length)
{
    if (!buf || len < 2 || len > UINT32_MAX / 8) return FAAD_ERR_INVALID_ARGUMENT;
    memset(asc, 0, sizeof(*asc));
    asc_bitreader br = { buf, len * 8, 0 };
    uint32_t aot = asc_br_get(&br, 5);
    if (aot == 31) aot = 32 + asc_br_get(&br, 6);
    uint32_t rate = asc_parse_sample_rate(&br);
    uint32_t channels = asc_br_get(&br, 4);
    uint32_t output_rate = rate * 2;
    bool explicit_sbr = aot == 5 || aot == 29;
    if (explicit_sbr) {
        output_rate = asc_parse_sample_rate(&br);
        asc->is_sbr = true;
        asc->is_ps = aot == 29;
        aot = asc_br_get(&br, 5);
        if (aot == 31) aot = 32 + asc_br_get(&br, 6);
    }
    if (br.pos > br.len_bits || !rate || !output_rate || channels > 7)
        return FAAD_ERR_INVALID_ARGUMENT;
    if (aot != 2) return FAAD_ERR_UNSUPPORTED;
    if (explicit_sbr && output_rate != 2 * rate) return FAAD_ERR_UNSUPPORTED;
    *frame_length = asc_br_get(&br, 1) != 0;
    if (asc_br_get(&br, 1)) asc_skip_core_coder_delay(&br);
    bool extension = asc_br_get(&br, 1) != 0;
    if (!channels) {
        /* Validate the ASC's PCE profile and sampling index; channel elements
         * are mapped from its tags during packet decoding. */
        if (!asc_parse_pce(&br, &asc->pce)) return FAAD_ERR_INVALID_ARGUMENT;
        if (asc->pce.profile != 1 || asc->pce.sr_index >= 12) return FAAD_ERR_UNSUPPORTED;
        /* Per-channel state is sized for MAX_CHANNELS. */
        if (asc->pce.channels > MAX_CHANNELS) return FAAD_ERR_UNSUPPORTED;
    }
    if (extension) asc_br_get(&br, 1);
    if (br.pos > br.len_bits) return FAAD_ERR_INVALID_ARGUMENT;
    if (asc_br_remaining(&br) >= 11 && asc_br_get(&br, 11) == ASC_SYNC_EXTENSION_SBR) {
        uint32_t ext_aot = asc_br_get(&br, 5);
        if (ext_aot == 5 && asc_br_get(&br, 1)) {
            uint32_t ext_rate = asc_parse_sample_rate(&br);
            if (!ext_rate || br.pos > br.len_bits) return FAAD_ERR_INVALID_ARGUMENT;
            if (ext_rate != rate * 2) return FAAD_ERR_UNSUPPORTED;
            asc->is_sbr = true;
            output_rate = ext_rate;
            if (asc_br_remaining(&br) >= 11 && asc_br_get(&br, 11) == ASC_SYNC_EXTENSION_PS)
                asc->is_ps = asc_br_get(&br, 1) != 0;
        }
    }
    if (br.pos > br.len_bits) return FAAD_ERR_INVALID_ARGUMENT;
    asc->object_type = FAAD_OBJ_LC;
    asc->sample_rate = rate;
    asc->num_channels = channels;
    asc->sbr_sample_rate = output_rate;
    return FAAD_OK;
}

faad_status asc_decode(BitReader *bs, AudioSpecificConfig *asc)
{
    /* asc_decode() is always called on a BitReader freshly bits_init()'d over
     * exactly the ASC bytes (see faad_decoder_open()). */
    bool frame_length;
    faad_status status = parse_asc(bs->buffer, bs->len, asc, &frame_length);
    if (status != FAAD_OK) return status;

    if (frame_length) {
#ifndef FAAD_DISABLE_DAB
        asc->frame_length_flag = true;
#else
        return FAAD_ERR_UNSUPPORTED;
#endif
    }

    return FAAD_OK;
}

faad_status adts_decode_header(BitReader *bs, AudioSpecificConfig *asc, uint32_t *frame_length)
{
    uint32_t sync = bits_get(bs, 12);
    if (sync != 0xFFF) {
        return FAAD_ERR_SYNC_LOST;
    }
    bits_skip(bs, 1);
    bits_skip(bs, 2);
    uint32_t protection_absent = bits_get(bs, 1);

    uint32_t profile = bits_get(bs, 2);
    uint32_t sr_idx = bits_get(bs, 4);
    bits_skip(bs, 1);
    uint32_t channel_config = bits_get(bs, 3);
    bits_skip(bs, 4);

    uint32_t flen = bits_get(bs, 13);
    bits_skip(bs, 11);
    uint32_t raw_blocks = bits_get(bs, 2);

    if (protection_absent == 0) {
        bits_skip(bs, 16);
    }

    uint32_t min_hdr = (protection_absent == 0) ? 9 : 7;
    /* A malformed header is reported before an unsupported profile: only a
     * well-formed header can be trusted to describe a frame worth skipping. */
    if (sr_idx >= 12 || asc_codec_sample_rates[sr_idx] == 0 || flen < min_hdr) {
        return FAAD_ERR_DECODE_FAILED;
    }

    if (profile != 1 || raw_blocks != 0) {
        return FAAD_ERR_UNSUPPORTED;
    }

    if (asc) {
        memset(asc, 0, sizeof(*asc));
        asc->object_type = (enum faad_object_type)(profile + 1);
        asc->sample_rate = asc_codec_sample_rates[sr_idx];
        asc->num_channels = channel_config;
    }

    if (frame_length) {
        *frame_length = flen;
    }

    return FAAD_OK;
}
