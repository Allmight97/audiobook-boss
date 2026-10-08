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

#ifdef FAAD_STATS
#include <stdio.h>
#endif

#include "atomic.h"
#include "endian.h"

/* Frozen initial layouts: never replace these with a growing sizeof(). */
#define CONFIG_BASELINE_SIZE ((uint32_t)(offsetof(faad_config, downmix_mode) + sizeof(enum faad_downmix_mode)))
#define LIBRARY_INFO_BASELINE_SIZE ((uint32_t)(offsetof(faad_library_info, reserved) + sizeof(((faad_library_info *)0)->reserved)))
#define STREAM_INFO_BASELINE_SIZE ((uint32_t)(offsetof(faad_stream_info, reserved) + sizeof(((faad_stream_info *)0)->reserved)))

/* Append-only fields must not increase the alignment an older caller supplies. */
_Static_assert(_Alignof(faad_config) == _Alignof(uint32_t), "config ABI alignment");
_Static_assert(_Alignof(faad_stream_info) == _Alignof(uint32_t), "stream ABI alignment");
_Static_assert(_Alignof(faad_library_info) == _Alignof(const char *), "library ABI alignment");

static uint32_t bounded_size(uint32_t caller_size, size_t library_size)
{
    return caller_size < library_size ? caller_size : (uint32_t)library_size;
}

/* Channels a channel_configuration carries (7 is 7.1); 0, a PCE, is
 * learnt from the first decoded frame, so start from stereo. */
static uint32_t config_channels(uint32_t channel_config)
{
    return channel_config == 7 ? 8 : (channel_config >= 1 && channel_config <= 6) ? channel_config : (MAX_CHANNELS < 2 ? 1 : 2);
}

/* Native (element) index of each output channel, WAV / SMPTE order, for
 * channel configurations 3..7 (ISO/IEC 14496-3 Table 1.19); NULL keeps
 * the element order (mono, stereo, PCE-defined layouts). */
static const uint8_t *output_channel_map(uint32_t channel_config, uint32_t num_chs)
{
    static const uint8_t map3[] = { 1, 2, 0 };                   /* C L R -> L R C */
    static const uint8_t map4[] = { 1, 2, 0, 3 };                /* C L R Cs -> L R C Cs */
    static const uint8_t map5[] = { 1, 2, 0, 3, 4 };             /* C L R Ls Rs -> L R C Ls Rs */
    static const uint8_t map6[] = { 1, 2, 0, 5, 3, 4 };          /* C L R Ls Rs LFE -> L R C LFE Ls Rs */
    static const uint8_t map8[] = { 1, 2, 0, 7, 5, 6, 3, 4 };    /* C L R Ls Rs Lb Rb LFE -> L R C LFE Lb Rb Ls Rs */
    switch (channel_config) {
    case 3: return num_chs == 3 ? map3 : NULL;
    case 4: return num_chs == 4 ? map4 : NULL;
    case 5: return num_chs == 5 ? map5 : NULL;
    case 6: return num_chs == 6 ? map6 : NULL;
    case 7: return num_chs == 8 ? map8 : NULL;
    default: return NULL;
    }
}

/* Time-domain downmix of the WAV-ordered channels src[] into pcm (one
 * frame_samples run per output channel): surround to stereo as
 * Lo = L + 0.707 (C + Ls), Ro = R + 0.707 (C + Rs), LFE dropped, scaled
 * so a full-scale input cannot clip; mono as (Lo + Ro) / 2. Returns the
 * output channel count, num_chs itself when there is nothing to mix. */
static uint32_t downmix_pcm(enum faad_downmix_mode mode, const float *src[], uint32_t num_chs,
                            uint32_t frame_samples, float *pcm)
{
    if (mode == FAAD_DOWNMIX_NONE || num_chs < 2 || (mode == FAAD_DOWNMIX_STEREO && num_chs == 2))
        return num_chs;
    /* surround indices in the WAV order output_channel_map() produces */
    int c = -1, ls = -1, rs = -1;
    switch (num_chs) {
    case 3: c = 2; break;
    case 4: c = 2; ls = rs = 3; break;
    case 5: c = 2; ls = 3; rs = 4; break;
    case 6: c = 2; ls = 4; rs = 5; break;
    case 8: c = 2; ls = 4; rs = 5; break; /* the side pair is folded in below */
    default: break;
    }
    const float k = 0.70710678f;
    float gain = 1.0f / (1.0f + (c >= 0 ? k : 0.0f) + (ls >= 0 ? (ls == rs ? 0.5f * k : k) : 0.0f)
                               + (num_chs == 8 ? k : 0.0f));
    float lsw = (ls == rs) ? 0.5f * k : k; /* a single rear channel feeds both sides */
    for (uint32_t i = 0; i < frame_samples; i++) {
        float lo = src[0][i], ro = src[1][i];
        if (c >= 0) { lo += k * src[c][i]; ro += k * src[c][i]; }
        if (ls >= 0) { lo += lsw * src[ls][i]; ro += lsw * src[rs][i]; }
        if (num_chs == 8) { lo += k * src[6][i]; ro += k * src[7][i]; }
        if (num_chs > 2) { lo *= gain; ro *= gain; }
        if (mode == FAAD_DOWNMIX_MONO) {
            pcm[i] = 0.5f * (lo + ro);
        } else {
            pcm[i] = lo;
            pcm[frame_samples + i] = ro;
        }
    }
    return mode == FAAD_DOWNMIX_MONO ? 1 : 2;
}


FAADAPI faad_status faad_get_library_info(faad_library_info *out)
{
    if (!out || out->struct_size < LIBRARY_INFO_BASELINE_SIZE) {
        return FAAD_ERR_INVALID_ARGUMENT;
    }
    faad_library_info info;
    memset(&info, 0, sizeof(info));
    info.struct_size = bounded_size(out->struct_size, sizeof(info));
    info.version = FAAD_VERSION_STRING;
    info.copyright =
        "FAAD - Freeware Advanced Audio Decoder (https://freewareadvancedaudio.github.io)\n"
        " Copyright (C) 2026, Nils Schimmelmann\n";
    info.max_channels = MAX_CHANNELS;
#ifndef FAAD_DISABLE_SBR
    info.sbr_supported = true;
#else
    info.sbr_supported = false;
#endif
#ifndef FAAD_DISABLE_PS
    info.ps_supported = true;
#else
    info.ps_supported = false;
#endif

    memcpy(out, &info, info.struct_size);
    return FAAD_OK;
}

FAADAPI faad_status faad_config_init(faad_config *cfg, uint32_t caller_size)
{
    if (!cfg || caller_size < CONFIG_BASELINE_SIZE) {
        return FAAD_ERR_INVALID_ARGUMENT;
    }
    faad_config tmp;
    uint32_t n = caller_size < sizeof(tmp) ? caller_size : (uint32_t)sizeof(tmp);
    memset(&tmp, 0, sizeof(tmp));
    tmp.struct_size = n;
    tmp.stream_format = FAAD_STREAM_ADTS;
    tmp.output_format = FAAD_OUTPUT_16BIT;
    tmp.downmix_mode = FAAD_DOWNMIX_NONE;
    memcpy(cfg, &tmp, n);
    return FAAD_OK;
}

static uint32_t output_sample_bytes(enum faad_output_format f)
{
    return f == FAAD_OUTPUT_16BIT ? 2 : f == FAAD_OUTPUT_24BIT ? 3 : 4;
}

static uint32_t output_capacity(const faad_decoder *dec)
{
    uint32_t channels = MAX_CHANNELS;
    if (dec->config.stream_format == FAAD_STREAM_RAW && dec->asc_parsed && dec->asc.num_channels)
        channels = config_channels(dec->asc.num_channels);
#ifndef FAAD_DISABLE_PS
    if (channels < 2) channels = 2;
#endif
    if (dec->config.downmix_mode == FAAD_DOWNMIX_MONO) channels = 1;
    else if (dec->config.downmix_mode == FAAD_DOWNMIX_STEREO && channels > 2) channels = 2;
    uint32_t samples = 1024;
#ifndef FAAD_DISABLE_SBR
    samples = 2048;
#endif
    return samples * channels * output_sample_bytes(dec->config.output_format);
}

static uint32_t channel_mask(const faad_decoder *dec, uint32_t channels)
{
    static const uint32_t masks[] = { 0, 0x4, 0x3, 0x7, 0x107, 0x37, 0x3f, 0, 0x63f };
    return channels <= 8 && (channels <= 2 || (dec->asc.num_channels
        && config_channels(dec->asc.num_channels) == channels)) ? masks[channels] : 0;
}

static faad_status resolve_config(const faad_config *cfg, faad_config *resolved)
{
    faad_config_init(resolved, sizeof(*resolved));
    if (!cfg) return FAAD_OK;
    if (cfg->struct_size < CONFIG_BASELINE_SIZE) return FAAD_ERR_INVALID_ARGUMENT;
    /* Add future fields here: absent or partially supplied fields keep defaults.
     * Never memcpy a partially supplied scalar or read beyond the caller. */
#define COPY_CONFIG_FIELD(field) \
    if (cfg->struct_size >= offsetof(faad_config, field) + sizeof(cfg->field)) \
        resolved->field = cfg->field
    COPY_CONFIG_FIELD(stream_format);
    COPY_CONFIG_FIELD(output_format);
    COPY_CONFIG_FIELD(downmix_mode);
#undef COPY_CONFIG_FIELD
    return (resolved->stream_format == FAAD_STREAM_RAW || resolved->stream_format == FAAD_STREAM_ADTS)
        && (resolved->output_format == FAAD_OUTPUT_16BIT || resolved->output_format == FAAD_OUTPUT_24BIT
            || resolved->output_format == FAAD_OUTPUT_32BIT || resolved->output_format == FAAD_OUTPUT_FLOAT)
        && (resolved->downmix_mode == FAAD_DOWNMIX_NONE || resolved->downmix_mode == FAAD_DOWNMIX_STEREO
            || resolved->downmix_mode == FAAD_DOWNMIX_MONO) ? FAAD_OK : FAAD_ERR_INVALID_ARGUMENT;
}

FAADAPI faad_status faad_get_state_size(uint32_t *state_bytes_out)
{
    if (!state_bytes_out) return FAAD_ERR_INVALID_ARGUMENT;
    *state_bytes_out = (uint32_t)sizeof(faad_decoder);
    return FAAD_OK;
}

static void faad_init_global_tables_impl(void)
{
    extern void init_dequant_tables(void);
    extern void init_huffman_luts(void);
    extern void init_windows(void);
#ifndef FAAD_DISABLE_SBR
    extern void init_qmf_twiddles(void);
#endif
    extern void init_is_tables(void);
#ifndef FAAD_DISABLE_PS
    extern void init_ps_tables(void);
#endif

    init_dequant_tables();
    init_huffman_luts();
    init_windows();
#ifndef FAAD_DISABLE_SBR
    init_qmf_twiddles();
#endif
    init_is_tables();
#ifndef FAAD_DISABLE_PS
    init_ps_tables();
#endif
}

void faad_init_global_tables(void)
{
    static faac_once_t once = FAAC_ONCE_INIT;
    faac_once_run(&once, faad_init_global_tables_impl);
}

FAADAPI faad_status faad_decoder_init(void *mem_buf, uint32_t mem_size,
                                      const faad_config *cfg,
                                      const uint8_t *asc_buf, uint32_t asc_len,
                                      faad_decoder **out_dec)
{
    if (out_dec) *out_dec = NULL;
    faad_config resolved;
    if (resolve_config(cfg, &resolved) != FAAD_OK || !mem_buf || !out_dec
        || ((uintptr_t)mem_buf & (FAAD_STATE_ALIGNMENT - 1))
        || ((asc_buf == NULL) != (asc_len == 0))
        || (resolved.stream_format == FAAD_STREAM_RAW && !asc_len)) {
        return FAAD_ERR_INVALID_ARGUMENT;
    }
    uint32_t state_size;
    faad_get_state_size(&state_size);
    if (mem_size < state_size) return FAAD_ERR_INSUFFICIENT_MEM;

    faad_init_global_tables();

    faad_decoder *dec = (faad_decoder *)mem_buf;
    memset(dec, 0, sizeof(faad_decoder));

    dec->config = resolved;

    dec->pns_seed = 0x12345678;

    if (asc_buf && asc_len > 0) {
        BitReader bs;
        bits_init(&bs, asc_buf, asc_len);
        faad_status st = asc_decode(&bs, &dec->asc);
        if (st != FAAD_OK) return st;

        dec->num_channels = config_channels(dec->asc.num_channels);
        if (dec->num_channels > MAX_CHANNELS) return FAAD_ERR_UNSUPPORTED;
        dec->core_sample_rate = dec->asc.sample_rate ? dec->asc.sample_rate : 44100;
        if (get_sr_index(dec->core_sample_rate) < 0) return FAAD_ERR_UNSUPPORTED;
        dec->sample_rate = dec->asc.is_sbr ? dec->asc.sbr_sample_rate : dec->core_sample_rate;
        dec->frame_samples = dec->asc.is_sbr ? 2048 : 1024;
        dec->asc_parsed = true;
        dec->format_known = dec->asc.num_channels != 0;
        dec->sbr_seen = dec->asc.is_sbr;
        dec->ps_seen = dec->asc.is_ps;
    } else {
        dec->num_channels = MAX_CHANNELS < 2 ? 1 : 2;
        dec->sample_rate = 44100;
        dec->core_sample_rate = 44100;
        dec->frame_samples = 1024;
    }
    dec->core_channels = dec->num_channels;
    dec->max_output_bytes = output_capacity(dec);

#ifdef FAAD_STATS
    if (g_faadStats.dumpFile) fclose(g_faadStats.dumpFile);
    memset(&g_faadStats, 0, sizeof(g_faadStats));
#endif
    *out_dec = dec;
    return FAAD_OK;
}

FAADAPI faad_status faad_decoder_open(const faad_config *cfg,
                                        const uint8_t *asc_buf, uint32_t asc_len,
                                        faad_decoder **out_dec)
{
    if (!out_dec) return FAAD_ERR_INVALID_ARGUMENT;
    *out_dec = NULL;

    uint32_t state_size = 0;
    if (faad_get_state_size(&state_size) != FAAD_OK) return FAAD_ERR_INVALID_ARGUMENT;

    /* malloc may only align to 8 bytes on 32-bit targets. Keep its original
     * pointer for free, and align instance placement independently. */
    void *storage = AllocMemory((size_t)state_size + FAAD_STATE_ALIGNMENT - 1);
    if (!storage) return FAAD_ERR_INSUFFICIENT_MEM;
    uintptr_t address = (uintptr_t)storage;
    size_t offset = (FAAD_STATE_ALIGNMENT - (address & (FAAD_STATE_ALIGNMENT - 1)))
        & (FAAD_STATE_ALIGNMENT - 1);
    void *mem = (uint8_t *)storage + offset;

    faad_status st = faad_decoder_init(mem, state_size, cfg, asc_buf, asc_len, out_dec);
    if (st != FAAD_OK) {
        FreeMemory(storage);
        return st;
    }

    (*out_dec)->is_heap_allocated = true;
    (*out_dec)->heap_storage = storage;
    return FAAD_OK;
}

#ifdef FAAD_STATS
faadDecStats g_faadStats;

static void faad_print_stats(void)
{
    const faadDecStats *s = &g_faadStats;
    if (s->totalFrames == 0) return;

    double tns_pct = s->icsCount > 0 ? 100.0 * s->tnsActiveFrames / s->icsCount : 0.0;
    double sbr_pct = s->totalFrames > 0 ? 100.0 * s->sbrActiveFrames / s->totalFrames : 0.0;
    double short_pct = s->icsCount > 0 ? 100.0 * s->shortBlockIcsCount / s->icsCount : 0.0;
    double pad_avg = s->fillElementCount > 0 ? (double)s->fillElementPadBitsSum / s->fillElementCount : 0.0;
    /* Percentages of scalefactor bands, counted per channel slot. */
    double ms_pct = s->totalBands > 0 ? 100.0 * s->msBands / s->totalBands : 0.0;
    double is_pct = s->totalBands > 0 ? 100.0 * s->isBands / s->totalBands : 0.0;
    double pns_pct = s->totalBands > 0 ? 100.0 * s->pnsBands / s->totalBands : 0.0;

    fprintf(stderr, "\n--- Decoder Diagnostics ---\n");
    fprintf(stderr, " Frames              : %u (non-END termination: %u)\n",
            s->totalFrames, s->nonEndTermination);
    fprintf(stderr, " Elements            : SCE=%u CPE=%u CCE=%u LFE=%u DSE=%u PCE=%u FIL=%u END=%u\n",
            s->elementCounts[0], s->elementCounts[1], s->elementCounts[2], s->elementCounts[3],
            s->elementCounts[4], s->elementCounts[5], s->elementCounts[6], s->elementCounts[7]);
    if (s->minChannels == s->maxChannels) {
        fprintf(stderr, " Channels            : %u (constant, 0 changes)\n", s->minChannels);
    } else {
        fprintf(stderr, " Channels            : varied %u-%u (%u changes)\n",
                s->minChannels, s->maxChannels, s->channelCountChanges);
    }
    fprintf(stderr, " TNS                 : active in %u/%u ics (%.1f%%)\n",
            s->tnsActiveFrames, s->icsCount, tns_pct);
    fprintf(stderr, " SBR                 : active in %u/%u frames (%.1f%%)\n",
            s->sbrActiveFrames, s->totalFrames, sbr_pct);
    fprintf(stderr, " SBR payloads        : %u headers, %u envelopes\n",
            s->sbrHeaderCount, s->sbrEnvelopeSum);
    fprintf(stderr, " Parametric stereo   : active in %u/%u frames\n",
            s->psActiveFrames, s->totalFrames);
    fprintf(stderr, " Short blocks        : %u/%u ics (%.1f%%)\n",
            s->shortBlockIcsCount, s->icsCount, short_pct);
    fprintf(stderr, " M/S, IS, PNS bands  : %.1f%% / %.1f%% / %.1f%% of %lu total\n",
            ms_pct, is_pct, pns_pct, s->totalBands);

    fprintf(stderr, " ESCBOOK magnitude   : %u values needed the >=16 escape path\n",
            s->escbookMagnitudeEscapes);
    fprintf(stderr, " Fill-elt pad-align  : %u elements, avg %.1f bits, max %u bits\n",
            s->fillElementCount, pad_avg, s->fillElementMaxPad);
    fprintf(stderr, " Error concealment   : %u frames\n", s->errorConcealmentFrames);
    fprintf(stderr, "---------------------------\n");
}
#endif

FAADAPI faad_status faad_decoder_close(faad_decoder **handle)
{
    if (!handle) return FAAD_ERR_INVALID_ARGUMENT;
    faad_decoder *dec = *handle;
    if (!dec) return FAAD_OK;
    *handle = NULL;
#ifdef FAAD_STATS
    faad_print_stats();
    if (g_faadStats.dumpFile) fclose(g_faadStats.dumpFile);
    g_faadStats.dumpFile = NULL;
    g_faadStats.dumpOpenTried = false;
#endif
    if (dec->is_heap_allocated) {
        FreeMemory(dec->heap_storage);
    }
    return FAAD_OK;
}

FAADAPI faad_status faad_decoder_get_info(const faad_decoder *dec, faad_stream_info *out_info)
{
    if (!dec || !out_info || out_info->struct_size < STREAM_INFO_BASELINE_SIZE)
        return FAAD_ERR_INVALID_ARGUMENT;
    faad_stream_info info;
    memset(&info, 0, sizeof(info));
    info.struct_size = bounded_size(out_info->struct_size, sizeof(info));
    info.max_output_bytes = dec->max_output_bytes;
    info.format_known = dec->format_known;
    if (!info.format_known) {
        memcpy(out_info, &info, info.struct_size);
        return FAAD_OK;
    }
    bool sbr = dec->asc.is_sbr || dec->sbr_seen;
    info.sample_rate = dec->core_sample_rate;
    info.frame_samples = 1024;
#ifndef FAAD_DISABLE_SBR
    if (sbr) { info.sample_rate *= 2; info.frame_samples = 2048; }
#endif
    info.channels = dec->num_channels;
#ifndef FAAD_DISABLE_PS
    if (dec->ps_seen && info.channels == 1) info.channels = 2;
#endif
    if (dec->config.downmix_mode == FAAD_DOWNMIX_MONO) info.channels = 1;
    else if (dec->config.downmix_mode == FAAD_DOWNMIX_STEREO && info.channels > 2) info.channels = 2;
    info.object_type = (dec->asc.is_ps || dec->ps_seen) ? FAAD_OBJ_HE_AAC_V2
        : sbr ? FAAD_OBJ_HE_AAC_V1 : FAAD_OBJ_LC;
#ifndef FAAD_DISABLE_SBR
    info.decoder_delay = sbr ? 2 * FAAD_SBR_CORE_DELAY : 0;
#endif
    info.channel_mask = channel_mask(dec, info.channels);
    if (dec->pcm_emitted) {
        info.sample_rate = dec->emitted_format.sample_rate;
        info.channels = dec->emitted_format.channels;
        info.frame_samples = dec->emitted_format.frame_samples;
        info.channel_mask = dec->emitted_format.channel_mask;
        info.decoder_delay = dec->emitted_format.decoder_delay;
    }
    memcpy(out_info, &info, info.struct_size);

    return FAAD_OK;
}

FAADAPI faad_status faad_decoder_flush(faad_decoder *dec)
{
    if (!dec) return FAAD_ERR_INVALID_ARGUMENT;

    memset(dec->overlap, 0, sizeof(dec->overlap));
    memset(dec->spec, 0, sizeof(dec->spec));
    memset(dec->prev_spec, 0, sizeof(dec->prev_spec));
    memset(dec->prev_window_shape, 0, sizeof(dec->prev_window_shape));
    memset(dec->prev_window_seq, 0, sizeof(dec->prev_window_seq));
    memset(dec->win_shape, 0, sizeof(dec->win_shape));
    memset(dec->win_seq, 0, sizeof(dec->win_seq));
    memset(dec->pcm, 0, sizeof(dec->pcm));
    dec->consecutive_errors = 0;
    dec->pns_seed = 0x12345678;
#ifndef FAAD_DISABLE_SBR
    memset(dec->sbr, 0, sizeof(dec->sbr));
    memset(dec->sbr_el, 0, sizeof(dec->sbr_el));
#endif
#ifndef FAAD_DISABLE_PS
    memset(&dec->ps, 0, sizeof(dec->ps));
#endif
    dec->sbr_present = false;
    dec->ps_present = false;

    return FAAD_OK;
}


#ifdef FAAD_STATS
/* Per-frame record of what the encoder chose, appended to the file named by
 * FAAD_DUMP, so streams from different encoders can be diffed decision by
 * decision. One line per record, first field the type, second the frame:
 *   C frame ch bits win_seq max_sfb groups global_gain | cb:sf:nnz:ms ... / ...
 *       one per ICS; bits is the whole element (both channels of a CPE, and
 *       the second channel of a CPE carries ms_mask_present there instead)
 *   H frame nch amp_res start stop xover freq_scale alter_scale noise_bands
 *     limiter_bands limiter_gains interpol smoothing reset kx M n_low n_high n_q e1 e2
 *   F frame ch class L_E L_Q freq_res amp_res invf harm_flag n_harm coupling -1 E_dB Q_dB
 *   G frame ch class L_E pointer t_E...
 *   P frame iid icc num_env
 *   B frame total hdr sect sf spec aux sbr ps fill
 *       bits by syntax part; sect/sf/spec/aux (pulse, TNS) sum over the ics,
 *       sbr excludes ps, fill is pad after the SBR payload, and the
 *       remainder of total is element and ics headers */
FILE *faad_dump_file(void)
{
    faadDecStats *st = &g_faadStats;
    if (!st->dumpOpenTried) {
        st->dumpOpenTried = true;
        const char *path = getenv("FAAD_DUMP");
        if (path && *path) st->dumpFile = fopen(path, "a");
    }
    return st->dumpFile;
}

static void core_dump_ics(int ch, const ICSInfo *ics, const float *spec,
                          const uint8_t (*ms)[MAX_SFB], unsigned bits)
{
    FILE *df = faad_dump_file();
    if (!df) return;
    fprintf(df, "C %u %d %u %u %u %u %u |", g_faadStats.totalFrames, ch, bits, ics->window_sequence,
            ics->max_sfb, ics->num_window_groups, ics->global_gain);
    int wo = 0;
    for (int g = 0; g < ics->num_window_groups && g < 8; g++) {
        for (int sfb = 0; sfb < ics->max_sfb && sfb < MAX_SFB; sfb++) {
            int nnz = 0;
            for (int w = 0; w < ics->window_group_length[g]; w++)
                for (int k = ics->sfb_offsets[sfb]; k < ics->sfb_offsets[sfb + 1] && k < 1024; k++)
                    nnz += spec[(wo + w) * 128 + k] != 0.0f;
            fprintf(df, " %u:%d:%d:%d", ics->sfb_cb[g][sfb], ics->scalefactors[g][sfb], nnz,
                    ms ? ms[g][sfb] : 0);
        }
        fprintf(df, " /");
        wo += ics->window_group_length[g];
    }
    fprintf(df, "\n");
}
#endif

/* Clamp and round to nearest: truncating toward zero costs the output half
 * an LSB of error against any rounding decoder, on every sample. */
static inline int16_t pcm_to_s16(float v)
{
    /* Clamp, then let the FPU round: adding 1.5*2^23 leaves the rounded
     * integer in the low mantissa bits. Straight-line float ops and one
     * narrowing, so the loops vectorise where copysign + convert did not. */
    v = v < 32767.0f ? v : 32767.0f;
    v = v > -32768.0f ? v : -32768.0f;
    v += 12582912.0f;
    int32_t bits;
    memcpy(&bits, &v, sizeof(bits));
    return (int16_t)bits;
}

/* Both integer 24-bit containers share scaling, rounding and clipping. */
static inline int32_t pcm_to_s24(float v)
{
    v *= 256.0f;
    v = v < 8388607.0f ? v : 8388607.0f;
    v = v > -8388608.0f ? v : -8388608.0f;
    return (int32_t)lrintf(v);
}

FAADAPI faad_status faad_decode_frame(faad_decoder *dec,
                                      const uint8_t *in_buf, uint32_t in_bytes,
                                      uint32_t *bytes_consumed,
                                      void *out_pcm, uint32_t out_cap_bytes,
                                      uint32_t *bytes_written,
                                      uint32_t *frame_flags)
{
    if (bytes_consumed) *bytes_consumed = 0;
    if (bytes_written) *bytes_written = 0;
    if (frame_flags) *frame_flags = 0;
    if (!dec || !in_buf || !bytes_consumed || !out_pcm || !bytes_written)
        return FAAD_ERR_INVALID_ARGUMENT;
    /* RAW lengths enter bit-count arithmetic directly. ADTS lengths are
     * bounded by the 13-bit frame header before payload parsing. */
    if (dec->config.stream_format == FAAD_STREAM_RAW && in_bytes > UINT32_MAX / 8)
        return FAAD_ERR_INVALID_ARGUMENT;
    uint32_t sample_bytes = output_sample_bytes(dec->config.output_format);
    if (sample_bytes != 3 && ((uintptr_t)out_pcm & (sample_bytes - 1)))
        return FAAD_ERR_INVALID_ARGUMENT;
    if (out_cap_bytes < dec->max_output_bytes) return FAAD_ERR_OUTPUT_TOO_SMALL;
    if (!in_bytes) return FAAD_ERR_NEED_MORE_DATA;

    BitReader bs;
    bits_init(&bs, in_buf, in_bytes);
    uint32_t adts_frame_len = in_bytes;
    if (dec->config.stream_format == FAAD_STREAM_ADTS) {
        /* Validate framing without touching the instance. Preserve a split
         * syncword rather than silently eating its first byte. */
        if (in_buf[0] == 0xff && in_bytes == 1) return FAAD_ERR_NEED_MORE_DATA;
        if (in_bytes < 2 || in_buf[0] != 0xff || (in_buf[1] & 0xf6) != 0xf0) {
            uint32_t skip = 1;
            while (skip + 1 < in_bytes && !(in_buf[skip] == 0xff
                && (in_buf[skip + 1] & 0xf6) == 0xf0)) skip++;
            if (skip + 1 >= in_bytes) skip = in_bytes - (in_buf[in_bytes - 1] == 0xff);
            *bytes_consumed = skip;
            return FAAD_ERR_SYNC_LOST;
        }
        uint32_t header_bytes = (in_buf[1] & 1) ? 7 : 9;
        if (in_bytes < header_bytes) return FAAD_ERR_NEED_MORE_DATA;
        adts_frame_len = ((uint32_t)(in_buf[3] & 3) << 11)
            | ((uint32_t)in_buf[4] << 3) | (in_buf[5] >> 5);
        if (adts_frame_len < header_bytes) {
            *bytes_consumed = 1;
            return FAAD_ERR_DECODE_FAILED;
        }
        if (adts_frame_len > in_bytes) return FAAD_ERR_NEED_MORE_DATA;
        AudioSpecificConfig asc;
        faad_status st = adts_decode_header(&bs, &asc, &adts_frame_len);
        if (st != FAAD_OK || config_channels(asc.num_channels) > MAX_CHANNELS) {
            *bytes_consumed = adts_frame_len;
            return st != FAAD_OK ? st : FAAD_ERR_UNSUPPORTED;
        }
        dec->asc = asc;
        dec->core_channels = config_channels(asc.num_channels);
        dec->core_sample_rate = asc.sample_rate;
        bs.len = adts_frame_len;
    }
    *bytes_consumed = adts_frame_len;
    dec->num_channels = dec->core_channels;
    bool decode_success = true;
    bool degraded = false;
#ifdef FAAD_STATS
    g_faadStats.totalFrames++;
#endif
    uint32_t active_chs = dec->num_channels;
    if (active_chs > MAX_CHANNELS) active_chs = MAX_CHANNELS;
    for (uint32_t c = 0; c < active_chs; c++)
        memset(dec->spec[c], 0, sizeof(dec->spec[c]));
    dec->sbr_present = false;
    dec->ps_present = false;

#ifdef FAAD_STATS
    g_faadStats.frameSectBits = g_faadStats.frameSfBits = g_faadStats.frameSpecBits = 0;
    g_faadStats.frameAuxBits = g_faadStats.frameSbrBits = g_faadStats.framePsBits = 0;
    g_faadStats.frameFillBits = 0;
    unsigned frame_hdr_bits = bits_get_consumed(&bs);
#endif
    uint32_t ch_idx = 0;
    uint32_t last_elem_type = ID_SCE;

    bool saw_end = false;
    if (decode_success) {
        /* Fill elements (SBR) follow the last channel element, so the loop
         * runs to END even once every channel slot is taken. */
        while (bits_get_consumed(&bs) + 3 <= bs.len * 8) {
            uint32_t syntax_id = bits_get(&bs, 3);
#ifdef FAAD_STATS
            g_faadStats.elementCounts[syntax_id]++;
#endif
            if (syntax_id == ID_END) {
                saw_end = true;
                break;
            } else if (syntax_id == ID_SCE || syntax_id == ID_LFE) {
                if (ch_idx >= MAX_CHANNELS) { decode_success = false; break; }
                last_elem_type = syntax_id;
#ifdef FAAD_STATS
                unsigned b0 = bits_get_consumed(&bs);
#endif
                ICSInfo *ics = &dec->scratch.cpe.ics[0]; /* cleared by decode_sce */
                if (decode_sce(&bs, dec, ics, ch_idx) != FAAD_OK) {
                    decode_success = false; /* the rest of the payload is out of step */
                    break;
                }
#ifdef FAAD_STATS
                core_dump_ics(ch_idx, ics, dec->spec[ch_idx], NULL,
                              bits_get_consumed(&bs) - b0);
#endif
                dec->win_seq[ch_idx] = ics->window_sequence;
                dec->win_shape[ch_idx] = ics->window_shape;
                apply_pns(ics, dec->spec[ch_idx], &dec->pns_seed);
                apply_tns(ics, dec->spec[ch_idx]);
                ch_idx += 1;
            } else if (syntax_id == ID_CPE) {
                if (ch_idx + 1 >= MAX_CHANNELS) { decode_success = false; break; }
                last_elem_type = ID_CPE;
                CPEInfo *cpe = &dec->scratch.cpe;
                memset(cpe, 0, sizeof(*cpe));
#ifdef FAAD_STATS
                unsigned b0 = bits_get_consumed(&bs);
#endif
                if (decode_cpe(&bs, dec, cpe, ch_idx) != FAAD_OK) {
                    decode_success = false;
                    break;
                }
#ifdef FAAD_STATS
                {
                    unsigned nb = bits_get_consumed(&bs) - b0;
                    /* ms_mask_present 2 sends no per-band flags, so every band is M/S. */
                    if (cpe->ms_mask_present == 2)
                        memset(cpe->ms_used, 1, sizeof(cpe->ms_used));
                    const uint8_t (*ms)[MAX_SFB] = cpe->ms_mask_present ? (const uint8_t (*)[MAX_SFB])cpe->ms_used : NULL;
                    core_dump_ics(ch_idx, &cpe->ics[0], dec->spec[ch_idx], ms, nb);
                    core_dump_ics(ch_idx + 1, &cpe->ics[1], dec->spec[ch_idx + 1], ms, cpe->ms_mask_present);
                }
#endif
                for (uint32_t i = 0; i < 2; i++) {
                    dec->win_seq[ch_idx + i] = cpe->ics[i].window_sequence;
                    dec->win_shape[ch_idx + i] = cpe->ics[i].window_shape;
                }

                apply_pns(&cpe->ics[0], dec->spec[ch_idx], &dec->pns_seed);
                apply_pns(&cpe->ics[1], dec->spec[ch_idx + 1], &dec->pns_seed);
                apply_ms_stereo(cpe, dec->spec[ch_idx], dec->spec[ch_idx + 1]);
                apply_is_stereo(cpe, dec->spec[ch_idx], dec->spec[ch_idx + 1]);
                apply_tns(&cpe->ics[0], dec->spec[ch_idx]);
                apply_tns(&cpe->ics[1], dec->spec[ch_idx + 1]);

                ch_idx += 2;
            } else if (syntax_id == ID_CCE) {
                decode_cce(&bs, dec);
            } else if (syntax_id == ID_DSE) {
                decode_dse(&bs);
            } else if (syntax_id == ID_PCE) {
                decode_pce(&bs, dec);
            } else if (syntax_id == ID_FIL) {
                uint32_t count = bits_get(&bs, 4);
                if (count == 15) count += bits_get(&bs, 8) - 1;
                if (count > 0) {
                    uint32_t fill_end = bits_get_consumed(&bs) + count * 8;
                    uint32_t ext_type = bits_get(&bs, 4);
                    if (ext_type == SBR_EXTENSION_DATA || ext_type == SBR_EXTENSION_DATA_CRC) {
                        uint32_t ch0 = 0;
                        if (last_elem_type == ID_CPE) {
                            ch0 = (ch_idx >= 2) ? (ch_idx - 2) : 0;
                        } else {
                            ch0 = (ch_idx >= 1) ? (ch_idx - 1) : 0;
                        }
#ifdef FAAD_STATS
                        unsigned sbr_mark = bits_get_consumed(&bs);
                        unsigned ps_mark = g_faadStats.framePsBits;
#endif
                        faad_status sbr_st = sbr_decode_extension(dec, &bs, ch0, last_elem_type, ext_type == SBR_EXTENSION_DATA_CRC);
#ifndef FAAD_DISABLE_SBR
                        if (sbr_st != FAAD_OK) {
                            /* The core is intact: play it band-limited rather than
                             * conceal it, and keep the half-read payload out of
                             * the envelope history. */
                            uint32_t nch = (last_elem_type == ID_CPE) ? 2 : 1;
                            for (uint32_t c = ch0; c < ch0 + nch && c < MAX_CHANNELS; c++) dec->sbr[c].have_frame = false;
                            dec->sbr_present = true;
                            degraded = true;
                        }
#else
                        (void)sbr_st;
#endif
                        uint32_t consumed = bits_get_consumed(&bs);
#ifdef FAAD_STATS
                        g_faadStats.fillElementCount++;
                        g_faadStats.frameSbrBits += consumed - sbr_mark - (g_faadStats.framePsBits - ps_mark);
#endif
                        if (consumed < fill_end) {
                            uint32_t pad = fill_end - consumed;
                            bits_skip(&bs, pad);
#ifdef FAAD_STATS
                            g_faadStats.fillElementPadBitsSum += pad;
                            g_faadStats.frameFillBits += pad;
                            if (pad > g_faadStats.fillElementMaxPad) {
                                g_faadStats.fillElementMaxPad = pad;
                            }
#endif
                        }
                    } else {
                        bits_skip(&bs, (count - 1) * 8 + 4);
                    }
                }
            }
        }
#ifdef FAAD_STATS
        if (!saw_end) {
            g_faadStats.nonEndTermination++;
        }
        FILE *df = faad_dump_file();
        if (df) {
            const faadDecStats *s = &g_faadStats;
            fprintf(df, "B %u %u %u %u %u %u %u %u %u %u\n", s->totalFrames, bits_get_consumed(&bs),
                    frame_hdr_bits, s->frameSectBits, s->frameSfBits, s->frameSpecBits, s->frameAuxBits,
                    s->frameSbrBits, s->framePsBits, s->frameFillBits);
        }
#endif
    }

    if (!saw_end) decode_success = false;
    /* A fixed RAW layout also fixes the lifetime output capacity. */
    if (dec->config.stream_format == FAAD_STREAM_RAW && dec->asc.num_channels
        && ch_idx > config_channels(dec->asc.num_channels))
        return FAAD_ERR_DECODE_FAILED;

    if (decode_success && ch_idx > 0) {
        dec->consecutive_errors = 0;
        memcpy(dec->prev_spec, dec->spec, sizeof(dec->spec[0]) * ch_idx);
        dec->num_channels = ch_idx;
        dec->core_channels = ch_idx;
#ifdef FAAD_STATS
        if (!g_faadStats.haveLastChannels) {
            g_faadStats.haveLastChannels = true;
            g_faadStats.lastChannels = ch_idx;
            g_faadStats.minChannels = ch_idx;
            g_faadStats.maxChannels = ch_idx;
        } else {
            if (ch_idx != g_faadStats.lastChannels) {
                g_faadStats.channelCountChanges++;
                g_faadStats.lastChannels = ch_idx;
            }
            if (ch_idx < g_faadStats.minChannels) g_faadStats.minChannels = ch_idx;
            if (ch_idx > g_faadStats.maxChannels) g_faadStats.maxChannels = ch_idx;
        }
#endif
    } else {
#ifdef FAAD_STATS
        g_faadStats.errorConcealmentFrames++;
#endif
        dec->consecutive_errors++;
        float fade = 0.0f;
        if (dec->consecutive_errors <= 5) {
            fade = powf(0.8f, (float)dec->consecutive_errors);
        }
        for (uint32_t c = 0; c < dec->num_channels; c++) {
            for (int i = 0; i < FRAME_LEN_LONG; i++) {
                dec->spec[c][i] = dec->prev_spec[c][i] * fade;
            }
            /* Repeat the last block's windowing; a start block's spectrum
             * follows its own short-window overlap as a stop block. */
            static const uint8_t next_seq[4] = { ONLY_LONG_SEQUENCE, LONG_STOP_SEQUENCE,
                                                 EIGHT_SHORT_SEQUENCE, ONLY_LONG_SEQUENCE };
            dec->win_seq[c] = next_seq[dec->prev_window_seq[c] & 3];
            dec->win_shape[c] = dec->prev_window_shape[c];
        }
    }

    /* One buffer, one frame_samples-long run per channel: SBR analyses a
     * channel's whole core frame before it synthesises that channel, so it
     * works in place over the core output at the start of each run. */
    if (dec->sbr_present) dec->sbr_seen = true;
#ifndef FAAD_DISABLE_PS
    if (dec->ps_present || dec->asc.is_ps) dec->ps_seen = true;
#endif
    bool sbr_frame = dec->asc.is_sbr || dec->sbr_present || dec->sbr_seen;
#ifdef FAAD_DISABLE_SBR
    sbr_frame = false;
#endif
    dec->frame_samples = sbr_frame ? 2048 : 1024;
    float *pcm_final = dec->pcm;
    for (uint32_t c = 0; c < dec->num_channels; c++) {
        imdct_and_window(dec, c, dec->win_seq[c], dec->win_shape[c], dec->spec[c], pcm_final + c * dec->frame_samples);
    }
    if (sbr_frame) sbr_apply(dec, dec->num_channels, pcm_final);

    uint32_t frame_samples = dec->frame_samples;
    uint32_t num_chs = dec->num_channels;

    /* Output in the WAV / SMPTE order (FL FR FC LFE BL BR SL SR), taken
     * from the element order the channel configuration implies. */
    const float *src[MAX_CHANNELS < 2 ? 2 : MAX_CHANNELS];
    const uint8_t *map = output_channel_map(dec->asc.num_channels, num_chs);
    for (uint32_t c = 0; c < num_chs; c++)
        src[c] = pcm_final + (map ? map[c] : c) * frame_samples;
    num_chs = downmix_pcm(dec->config.downmix_mode, src, num_chs, frame_samples, pcm_final);
    if (num_chs <= 2)
        for (uint32_t c = 0; c < num_chs; c++) src[c] = pcm_final + c * frame_samples;

    uint32_t required_bytes = frame_samples * num_chs * output_sample_bytes(dec->config.output_format);
    /* The entry check guarantees capacity before any history advances. A
     * lifetime-bound violation is a rejected frame, never a retryable buffer
     * error after synthesis has already consumed the packet. */
    if (required_bytes > dec->max_output_bytes) return FAAD_ERR_DECODE_FAILED;

    if (dec->config.output_format == FAAD_OUTPUT_16BIT) {
        int16_t * restrict out_int16 = (int16_t *)out_pcm;
        if (num_chs == 2) {
            const float * restrict pcm_l = src[0];
            const float * restrict pcm_r = src[1];
            for (uint32_t i = 0; i < frame_samples; i++) {
                out_int16[2 * i]     = pcm_to_s16(pcm_l[i]);
                out_int16[2 * i + 1] = pcm_to_s16(pcm_r[i]);
            }
        } else if (num_chs == 1) {
            const float * restrict pcm_m = src[0];
            for (uint32_t i = 0; i < frame_samples; i++)
                out_int16[i] = pcm_to_s16(pcm_m[i]);
        } else {
            for (uint32_t i = 0; i < frame_samples; i++)
                for (uint32_t c = 0; c < num_chs; c++)
                    out_int16[i * num_chs + c] = pcm_to_s16(src[c][i]);
        }
    } else if (dec->config.output_format == FAAD_OUTPUT_24BIT) {
        /* Packed in host byte order like the wider formats; byte-wise stores assume no alignment of out_pcm. */
        uint8_t * restrict out_u8 = (uint8_t *)out_pcm;
        for (uint32_t i = 0; i < frame_samples; i++)
            for (uint32_t c = 0; c < num_chs; c++, out_u8 += 3)
                write_pcm24(out_u8, pcm_to_s24(src[c][i]), WORDS_BIGENDIAN);
    } else if (dec->config.output_format == FAAD_OUTPUT_32BIT) {
        int32_t * restrict out_int32 = (int32_t *)out_pcm;
        for (uint32_t i = 0; i < frame_samples; i++)
            for (uint32_t c = 0; c < num_chs; c++)
                out_int32[i * num_chs + c] = pcm_to_s24(src[c][i]);
    } else {
        /* The core reconstructs at 16-bit full scale; float output is unity full scale. */
        const float norm = 1.0f / 32768.0f;
        float * restrict out_f32 = (float *)out_pcm;
        for (uint32_t i = 0; i < frame_samples; i++)
            for (uint32_t c = 0; c < num_chs; c++)
                out_f32[i * num_chs + c] = src[c][i] * norm;
    }

    *bytes_consumed = adts_frame_len;
    *bytes_written = required_bytes;

    bool sbr_active = sbr_frame;
#ifdef FAAD_STATS
    if (sbr_active) {
        g_faadStats.sbrActiveFrames++;
    }
#endif

    dec->format_known = dec->format_known || dec->asc.num_channels != 0
        || (decode_success && ch_idx > 0);
    dec->sample_rate = sbr_active ? 2 * dec->core_sample_rate : dec->core_sample_rate;
    uint32_t mask = channel_mask(dec, num_chs);
    uint32_t delay = sbr_active ? 2 * FAAD_SBR_CORE_DELAY : 0;
    uint32_t flags = 0;
    if (!dec->pcm_emitted || dec->emitted_format.sample_rate != dec->sample_rate
        || dec->emitted_format.channels != num_chs
        || dec->emitted_format.frame_samples != frame_samples
        || dec->emitted_format.channel_mask != mask
        || dec->emitted_format.decoder_delay != delay)
        flags |= FAAD_FRAME_FORMAT_CHANGED;
    if (sbr_active) flags |= FAAD_FRAME_SBR;
#ifndef FAAD_DISABLE_PS
    if (sbr_active && dec->ps_seen && dec->core_channels == 1) flags |= FAAD_FRAME_PS;
#endif
    if (!decode_success || ch_idx == 0) flags |= FAAD_FRAME_CONCEALED;
    if (degraded) flags |= FAAD_FRAME_DEGRADED;
    if (required_bytes) {
        dec->emitted_format.sample_rate = dec->sample_rate;
        dec->emitted_format.channels = num_chs;
        dec->emitted_format.frame_samples = frame_samples;
        dec->emitted_format.channel_mask = mask;
        dec->emitted_format.decoder_delay = delay;
        dec->pcm_emitted = true;
        dec->format_known = true;
        if (frame_flags) *frame_flags = flags;
    }

    return FAAD_OK;
}

FAADAPI const char *faad_strerror(faad_status status)
{
    switch (status) {
        case FAAD_OK:                   return "Success";
        case FAAD_ERR_INVALID_ARGUMENT: return "Invalid argument";
        case FAAD_ERR_UNSUPPORTED:      return "Unsupported configuration";
        case FAAD_ERR_INSUFFICIENT_MEM: return "Insufficient memory allocated";
        case FAAD_ERR_OUTPUT_TOO_SMALL: return "Output buffer too small";
        case FAAD_ERR_NEED_MORE_DATA:   return "Need more input data";
        case FAAD_ERR_DECODE_FAILED:    return "Decoding failed";
        case FAAD_ERR_SYNC_LOST:        return "Lost syncword alignment";
        default:                        return "Unknown status";
    }
}
