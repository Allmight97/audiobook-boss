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

static void decode_ics_info(BitReader *bs, ICSInfo *ics)
{
    bits_skip(bs, 1);
    ics->window_sequence = bits_get(bs, 2);
    ics->window_shape = bits_get(bs, 1);

    if (ics->window_sequence == EIGHT_SHORT_SEQUENCE) {
        ics->max_sfb = bits_get(bs, 4);
        if (ics->max_sfb > 64) ics->max_sfb = 64;
        uint32_t scale_factor_grouping = bits_get(bs, 7);

        ics->num_window_groups = 1;
        ics->window_group_length[0] = 1;
        for (int i = 0; i < 7; i++) {
            if ((scale_factor_grouping >> (6 - i)) & 1) {
                ics->window_group_length[ics->num_window_groups - 1]++;
            } else {
                if (ics->num_window_groups < 8) {
                    ics->num_window_groups++;
                    ics->window_group_length[ics->num_window_groups - 1] = 1;
                }
            }
        }
        ics->num_windows = 8;
    } else {
        ics->max_sfb = bits_get(bs, 6);
        if (ics->max_sfb > 64) ics->max_sfb = 64;
        ics->num_window_groups = 1;
        ics->window_group_length[0] = 1;
        ics->num_windows = 1;

        if (bits_get(bs, 1)) {
            bits_skip(bs, 1);
        }
    }
}

static faad_status decode_section_data(BitReader *bs, ICSInfo *ics)
{
    uint32_t sect_bits = (ics->window_sequence == EIGHT_SHORT_SEQUENCE) ? 3 : 5;
    for (int g = 0; g < ics->num_window_groups && g < 8; g++) {
        int k = 0;
        int i = 0;
        while (k < ics->max_sfb && i < MAX_SFB) {
            uint32_t cb = bits_get(bs, 4);
            if (cb == 12) {
                return FAAD_ERR_DECODE_FAILED; /* Codebook 12 is reserved in ISO/IEC 14496-3 */
            }
            uint32_t max_run = (1U << sect_bits) - 1;
            uint32_t run_field = bits_get(bs, sect_bits);
            uint32_t len = run_field;
            /* A run_field equal to max_run escapes: another one follows and adds on. */
            while (run_field == max_run) {
                run_field = bits_get(bs, sect_bits);
                len += run_field;
            }
            if (len == 0) len = 1;
            int end_sfb = k + len;
            if (end_sfb > ics->max_sfb) end_sfb = ics->max_sfb;
            if (end_sfb > MAX_SFB) end_sfb = MAX_SFB;
            for (; k < end_sfb; k++) ics->sfb_cb[g][k] = (uint8_t)cb;
            i++;
        }
    }
    return FAAD_OK;
}

faad_status decode_pce(BitReader *bs, struct faad_decoder *dec)
{
    (void)dec;
    bits_skip(bs, 4); /* element_instance_tag */
    bits_skip(bs, 2); /* object_type */
    bits_skip(bs, 4); /* sampling_frequency_index */
    uint32_t num_front = bits_get(bs, 4);
    uint32_t num_side = bits_get(bs, 4);
    uint32_t num_back = bits_get(bs, 4);
    uint32_t num_lfe = bits_get(bs, 2);
    uint32_t num_assoc_data = bits_get(bs, 3);
    uint32_t num_valid_cc = bits_get(bs, 4);

    if (bits_get(bs, 1)) bits_skip(bs, 4); /* mono_mixdown */
    if (bits_get(bs, 1)) bits_skip(bs, 4); /* stereo_mixdown */
    if (bits_get(bs, 1)) bits_skip(bs, 3); /* matrix_mixdown */

    for (uint32_t i = 0; i < num_front; i++) bits_skip(bs, 5);
    for (uint32_t i = 0; i < num_side; i++) bits_skip(bs, 5);
    for (uint32_t i = 0; i < num_back; i++) bits_skip(bs, 5);
    for (uint32_t i = 0; i < num_lfe; i++) bits_skip(bs, 4);
    for (uint32_t i = 0; i < num_assoc_data; i++) bits_skip(bs, 4);
    for (uint32_t i = 0; i < num_valid_cc; i++) bits_skip(bs, 5);

    bits_byte_align(bs);
    uint32_t comment_bytes = bits_get(bs, 8);
    for (uint32_t i = 0; i < comment_bytes; i++) bits_skip(bs, 8);

    return FAAD_OK;
}

faad_status decode_cce(BitReader *bs, struct faad_decoder *dec)
{
    bits_skip(bs, 4); /* element_instance_tag */
    bits_skip(bs, 1); /* coupling_point */
    uint32_t num_coupled_elements = bits_get(bs, 3);
    for (uint32_t c = 0; c <= num_coupled_elements; c++) {
        bool is_cpe = bits_get(bs, 1);
        bits_skip(bs, 4); /* tag_select */
        if (is_cpe) {
            bits_skip(bs, 1); /* cc_l */
            bits_skip(bs, 1); /* cc_r */
        }
    }
    bits_skip(bs, 1); /* cc_domain */
    bits_skip(bs, 1); /* gain_element_sign */
    bits_skip(bs, 2); /* gain_element_scale */

    /* The payload is parsed to stay in step, then discarded. */
    memset(&dec->scratch.cce.ics, 0, sizeof(dec->scratch.cce.ics));
    return decode_ics(bs, dec, &dec->scratch.cce.ics, dec->scratch.cce.spec, false);
}

faad_status decode_dse(BitReader *bs)
{
    bits_skip(bs, 4); /* element_instance_tag */
    bool byte_align = bits_get(bs, 1);
    uint32_t count = bits_get(bs, 8);
    if (count == 255) count += bits_get(bs, 8);
    if (byte_align) bits_byte_align(bs);
    for (uint32_t i = 0; i < count; i++) bits_skip(bs, 8);
    return FAAD_OK;
}

faad_status decode_ics(BitReader *bs, struct faad_decoder *dec, ICSInfo *ics, float *spec, bool common_window)
{
#ifdef FAAD_STATS
    g_faadStats.icsCount++;
#endif

    ics->global_gain = bits_get(bs, 8);

    if (!common_window) {
        decode_ics_info(bs, ics);
    }

    extern void setup_sfb_offsets(ICSInfo *ics, uint32_t sample_rate);
    /* Window/sfb layout is defined relative to the AAC core codec's own rate
     * (Fs/2 of the nominal rate when SBR is present), not the nominal
     * post-SBR rate reported to callers. */
    setup_sfb_offsets(ics, dec->core_sample_rate);
    if (ics->max_sfb > ics->num_sfbs) return FAAD_ERR_DECODE_FAILED; /* a corrupt or misparsed ics_info */

#ifdef FAAD_STATS
    if (ics->window_sequence == EIGHT_SHORT_SEQUENCE) {
        g_faadStats.shortBlockIcsCount++;
    }
#endif

#ifdef FAAD_STATS
    unsigned bits_mark = bits_get_consumed(bs);
#endif
    faad_status st = decode_section_data(bs, ics);
    if (st != FAAD_OK) return st;
#ifdef FAAD_STATS
    g_faadStats.frameSectBits += bits_get_consumed(bs) - bits_mark;
    bits_mark = bits_get_consumed(bs);
#endif

    decode_scale_factor_data(bs, ics, dec->core_sample_rate);
#ifdef FAAD_STATS
    g_faadStats.frameSfBits += bits_get_consumed(bs) - bits_mark;
    bits_mark = bits_get_consumed(bs);
#endif

    ics->pulse_data_present = bits_get(bs, 1);
    ics->pulse_count = 0;
    if (ics->pulse_data_present) {
        uint32_t number_pulse = bits_get(bs, 2);
        uint32_t pulse_start_sfb = bits_get(bs, 6);
        /* §4.6.4: pulses are for long windows only, and start inside the table */
        if (ics->window_sequence == EIGHT_SHORT_SEQUENCE || pulse_start_sfb >= ics->num_sfbs)
            return FAAD_ERR_DECODE_FAILED;
        uint32_t pos = ics->sfb_offsets[pulse_start_sfb];
        for (uint32_t i = 0; i <= number_pulse; i++) {
            pos += bits_get(bs, 5); /* pulse_offset */
            if (pos >= FRAME_LEN_LONG) return FAAD_ERR_DECODE_FAILED;
            ics->pulse_pos[i] = (uint16_t)pos;
            ics->pulse_amp[i] = (uint8_t)bits_get(bs, 4);
        }
        ics->pulse_count = (uint8_t)(number_pulse + 1);
    }

    ics->tns_data_present = bits_get(bs, 1);
#ifdef FAAD_STATS
    if (ics->tns_data_present) {
        g_faadStats.tnsActiveFrames++;
    }
#endif
    if (ics->tns_data_present) {
        uint32_t n_filt_bits = (ics->window_sequence == EIGHT_SHORT_SEQUENCE) ? 1 : 2;
        int num_windows = (ics->window_sequence == EIGHT_SHORT_SEQUENCE) ? ics->num_windows : 1;
        for (int w = 0; w < num_windows && w < 8; w++) {
            ics->tns_n_filt[w] = bits_get(bs, n_filt_bits);
            if (ics->tns_n_filt[w]) {
                uint32_t coef_res = bits_get(bs, 1);
                ics->tns_coef_res[w] = coef_res;
                for (int f = 0; f < ics->tns_n_filt[w] && f < 4; f++) {
                    ics->tns_length[w][f] = bits_get(bs, (ics->window_sequence == EIGHT_SHORT_SEQUENCE) ? 4 : 6);
                    ics->tns_order[w][f] = bits_get(bs, (ics->window_sequence == EIGHT_SHORT_SEQUENCE) ? 3 : 5);
                    if (ics->tns_order[w][f]) {
                        ics->tns_direction[w][f] = bits_get(bs, 1);
                        uint32_t coef_compress = bits_get(bs, 1);
                        int bits_per_coef = (coef_res ? 4 : 3) - (int)coef_compress;
                        /* every coefficient is in the stream; only the LC
                         * order is kept and filtered */
                        for (int c = 0; c < ics->tns_order[w][f]; c++) {
                            uint32_t val = bits_get(bs, bits_per_coef);
                            int32_t sval = (int32_t)val;
                            if (sval & (1 << (bits_per_coef - 1))) {
                                sval |= ~((1 << bits_per_coef) - 1);
                            }
                            if (c < TNS_MAX_ORDER) ics->tns_coef[w][f][c] = (int8_t)sval;
                        }
                        if (ics->tns_order[w][f] > TNS_MAX_ORDER) ics->tns_order[w][f] = TNS_MAX_ORDER;
                    }
                }
            }
        }
    }

    ics->gain_control_present = bits_get(bs, 1);
    if (ics->gain_control_present) {
    }

#ifdef FAAD_STATS
    g_faadStats.frameAuxBits += bits_get_consumed(bs) - bits_mark;
    bits_mark = bits_get_consumed(bs);
#endif
    st = decode_spectral_data(bs, ics, spec);
#ifdef FAAD_STATS
    g_faadStats.frameSpecBits += bits_get_consumed(bs) - bits_mark;
#endif
    return st;
}

faad_status decode_cpe(BitReader *bs, struct faad_decoder *dec, CPEInfo *cpe, uint32_t ch)
{
#if MAX_CHANNELS < 2
    (void)bs; (void)dec; (void)cpe; (void)ch;
    return FAAD_ERR_DECODE_FAILED;
#else
    if (ch + 1 >= MAX_CHANNELS) return FAAD_ERR_DECODE_FAILED;
    bits_skip(bs, 4);
    cpe->common_window = bits_get(bs, 1);

    if (cpe->common_window) {
        decode_ics_info(bs, &cpe->ics[0]);
        cpe->ics[1] = cpe->ics[0];

        cpe->ms_mask_present = bits_get(bs, 2);
        if (cpe->ms_mask_present == 1) {
            for (int g = 0; g < cpe->ics[0].num_window_groups && g < 8; g++) {
                for (int sfb = 0; sfb < cpe->ics[0].max_sfb && sfb < MAX_SFB; sfb++) {
                    cpe->ms_used[g][sfb] = bits_get(bs, 1);
                }
            }
        }
    }

    faad_status st = decode_ics(bs, dec, &cpe->ics[0], dec->spec[ch], cpe->common_window);
    if (st != FAAD_OK) return st;
    return decode_ics(bs, dec, &cpe->ics[1], dec->spec[ch + 1], cpe->common_window);
#endif
}

faad_status decode_sce(BitReader *bs, struct faad_decoder *dec, ICSInfo *ics, uint32_t ch)
{
    if (ch >= MAX_CHANNELS) return FAAD_ERR_DECODE_FAILED;
    memset(ics, 0, sizeof(*ics));
    bits_skip(bs, 4);
    return decode_ics(bs, dec, ics, dec->spec[ch], false);
}
