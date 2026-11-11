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

/*
 * The public faac_* encoder API (see include/faac.h).
 *
 * This is a thin facade over the private encoder core in frame.c (declared in
 * faac_internal.h): it supplies the params-at-open entry point, uniform
 * faac_status error reporting, resolved-property queries, and library-owned
 * ASC. The opaque faac_encoder* is the core's faacEncStruct*.
 */

#include <assert.h>
#include <stdlib.h>
#include <string.h>

#include "faac_internal.h"

#if FAAC_ENCODER_DRC
#include <math.h>
#endif

#include <faac.h>
#include "frame.h"
#include "bitstream.h"
#include "sbr.h"
#include "resample.h"
#include "util.h"

/* The public enums are width-pinned to 32 bits by their FAAC_*_MAX sentinels;
 * verify the compiler honored that so the ABI matches the documented layout. */
_Static_assert(sizeof(enum faac_status)        == 4, "faac_status must be 32-bit");
_Static_assert(sizeof(enum faac_object_type)   == 4, "faac_object_type must be 32-bit");
_Static_assert(sizeof(enum faac_rate_control)  == 4, "faac_rate_control must be 32-bit");
_Static_assert(sizeof(enum faac_stereo_mode)   == 4, "faac_stereo_mode must be 32-bit");
_Static_assert(sizeof(enum faac_shortctl_mode) == 4, "faac_shortctl_mode must be 32-bit");
_Static_assert(sizeof(enum faac_stream_format) == 4, "faac_stream_format must be 32-bit");
_Static_assert(sizeof(enum faac_input_format)  == 4, "faac_input_format must be 32-bit");
_Static_assert(sizeof(enum faac_drc_profile)   == 4, "faac_drc_profile must be 32-bit");

/* The modern enums mirror the legacy numeric values one-for-one, so parameter
 * translation is a plain field copy. Guard that assumption. */
_Static_assert((int)FAAC_OBJ_AUTO == AUTO && (int)FAAC_OBJ_LC == LOW
               && (int)FAAC_OBJ_HE_AAC_V1 == HE_V1
               && (int)FAAC_OBJ_HE_AAC_V2 == HE_V2, "object type drift");
_Static_assert((int)FAAC_STEREO_LR == JOINT_NONE && (int)FAAC_STEREO_AUTO == JOINT_MIXED, "joint mode drift");
_Static_assert((int)FAAC_SHORTCTL_NORMAL == SHORTCTL_NORMAL && (int)FAAC_SHORTCTL_NOLONG == SHORTCTL_NOLONG, "shortctl drift");
_Static_assert((int)FAAC_STREAM_RAW == RAW_STREAM && (int)FAAC_STREAM_ADTS == ADTS_STREAM
            && (int)FAAC_STREAM_ADTS_MPEG2 != RAW_STREAM && (int)FAAC_STREAM_ADTS_MPEG2 != ADTS_STREAM, "stream format drift");
_Static_assert((int)FAAC_INPUT_NULL  == INPUT_NULL  && (int)FAAC_INPUT_16BIT == INPUT_16BIT
            && (int)FAAC_INPUT_24BIT == INPUT_24BIT && (int)FAAC_INPUT_24IN32 == INPUT_32BIT
            && (int)FAAC_INPUT_FLOAT == INPUT_FLOAT, "input format drift");
_Static_assert((int)FAAC_DRC_OFF == DRC_OFF, "drc profile drift");

/* Baselines end at the last field so appended fields remain optional. */
#define PARAMS_BASELINE_SIZE ((uint32_t)FAAC_PARAMS_BASELINE)
#define LIBRARY_INFO_BASELINE_SIZE ((uint32_t)FAAC_LIBRARY_INFO_BASELINE)
#define ENCODER_INFO_BASELINE_SIZE ((uint32_t)FAAC_ENCODER_INFO_BASELINE)

/* The public rate modes are numbered ABR first so a zeroed field selects it;
 * the core keeps its own numbering. */
static unsigned int rate_mode_to_core(enum faac_rate_control mode)
{
    return mode == FAAC_RC_VBR ? RATE_VBR : mode == FAAC_RC_CBR ? RATE_CBR : RATE_ABR;
}

/* ABR and CBR default to 64 kbit/s for every channel; VBR carries no rate. */
static uint64_t resolved_bit_rate(const faac_params *p)
{
    return p->rate_control == FAAC_RC_VBR ? 0 :
        (p->bit_rate ? p->bit_rate : (uint64_t)64000u * p->channels);
}

/* faac_encoder* and faacEncHandle are the same underlying object. */
static inline faacEncStruct *unwrap(faac_encoder *encoder) { return (faacEncStruct *)encoder; }

FAACAPI faac_status faac_get_library_info(faac_library_info *out_info)
{
    faac_library_info info;
    char *vid = NULL, *vcopy = NULL;
    uint32_t caller_size, n;

    if (!out_info)
        return FAAC_ERR_INVALID_ARGUMENT;
    caller_size = out_info->struct_size;
    if (caller_size < LIBRARY_INFO_BASELINE_SIZE)
        return FAAC_ERR_INVALID_ARGUMENT;

    faacEncGetVersion(&vid, &vcopy);

    memset(&info, 0, sizeof(info));
    info.version        = vid;
    info.copyright      = vcopy;
    info.max_channels   = (uint32_t)MAX_CHANNELS;
    info.sbr_decimation = (uint32_t)FAAC_SBR_DECIMATION;
    info.flags          = (FAAC_ENCODER_SBR ? FAAC_LIB_SBR : 0) | (FAAC_ENCODER_PS ? FAAC_LIB_PS : 0) | (FAAC_ENCODER_DRC ? FAAC_LIB_DRC : 0);

    /* Write at most the caller's struct_size so a newer library cannot overrun
     * an older, smaller faac_library_info; report the byte count actually set. */
    n = caller_size < (uint32_t)sizeof(info) ? caller_size : (uint32_t)sizeof(info);
    info.struct_size = n;
    memcpy(out_info, &info, n);
    return FAAC_OK;
}

FAACAPI faac_status faac_params_init(faac_params *params, uint32_t caller_size)
{
    faac_params tmp;
    uint32_t n;

    if (!params)
        return FAAC_ERR_INVALID_ARGUMENT;
    if (caller_size < PARAMS_BASELINE_SIZE)
        return FAAC_ERR_INVALID_ARGUMENT;

    memset(&tmp, 0, sizeof(tmp));
    tmp.object_type   = FAAC_OBJ_LC;
    tmp.stereo_mode   = FAAC_STEREO_AUTO;
    tmp.flags         = FAAC_FLAG_TNS | FAAC_FLAG_PNS;
    tmp.bit_rate      = 0;              /* ABR default: 64 kbit/s per channel */
    tmp.bandwidth     = 0;              /* derive from bit_rate */
    tmp.quality       = 0;              /* derive from bit_rate */
    tmp.peak_bit_rate = 0;              /* no per-frame peak cap */
    tmp.rate_control  = FAAC_RC_ABR;
    tmp.stream_format = FAAC_STREAM_ADTS;
    tmp.input_format  = FAAC_INPUT_16BIT;
    tmp.short_control = FAAC_SHORTCTL_NORMAL;
    tmp.drc_ref_level = -23.0f;

    /* Write at most the caller's struct_size so a newer library cannot overrun
     * an older, smaller faac_params; report the byte count actually set. */
    n = caller_size < (uint32_t)sizeof(faac_params) ? caller_size : (uint32_t)sizeof(faac_params);
    tmp.struct_size = n;
    memcpy(params, &tmp, n);
    return FAAC_OK;
}

#define FAAC_MIN_SAMPLE_RATE 7350u
#define FAAC_MAX_SAMPLE_RATE 96000u

/* Validate the enumerated/range fields the caller supplied. Returns FAAC_OK,
 * FAAC_ERR_INVALID_ARGUMENT for out-of-range values, or FAAC_ERR_UNSUPPORTED
 * for values that are valid but not implemented in this build. */
static faac_status validate_params(const faac_params *p, uint64_t *bit_rate_per_channel)
{
    if (p->flags & ~(uint32_t)(FAAC_FLAG_TNS | FAAC_FLAG_PNS))
        return FAAC_ERR_INVALID_ARGUMENT;
    switch (p->object_type) {
        case FAAC_OBJ_HE_AAC_V1:
            if (!FAAC_ENCODER_SBR) return FAAC_ERR_UNSUPPORTED;
            break;
        case FAAC_OBJ_AUTO: case FAAC_OBJ_LC:
            break;
        case FAAC_OBJ_HE_AAC_V2:
            if (!IsPS(p->object_type)) return FAAC_ERR_UNSUPPORTED;
            if (p->channels != 2) return FAAC_ERR_INVALID_ARGUMENT;
            break;
        default:
            return FAAC_ERR_INVALID_ARGUMENT;
    }
    switch (p->stereo_mode) {
        case FAAC_STEREO_LR: case FAAC_STEREO_MS:
        case FAAC_STEREO_IS: case FAAC_STEREO_AUTO: break;
        default: return FAAC_ERR_INVALID_ARGUMENT;
    }
    switch (p->short_control) {
        case FAAC_SHORTCTL_NORMAL: case FAAC_SHORTCTL_NOSHORT:
        case FAAC_SHORTCTL_NOLONG: break;
        default: return FAAC_ERR_INVALID_ARGUMENT;
    }
#if FAAC_ENCODER_DRC
    switch (p->drc_profile) {
        case FAAC_DRC_OFF: case FAAC_DRC_FILM: case FAAC_DRC_FILM_LIGHT:
        case FAAC_DRC_MUSIC: case FAAC_DRC_MUSIC_LIGHT: case FAAC_DRC_SPEECH: break;
        default: return FAAC_ERR_INVALID_ARGUMENT;
    }
    if (p->drc_ref_level < -31.75f || p->drc_ref_level > 0.0f || isnan(p->drc_ref_level))
        return FAAC_ERR_INVALID_ARGUMENT;
#else
    if (p->drc_profile != FAAC_DRC_OFF)
        return FAAC_ERR_UNSUPPORTED;
#endif
    switch (p->stream_format) {
        case FAAC_STREAM_RAW: case FAAC_STREAM_ADTS: case FAAC_STREAM_ADTS_MPEG2: break;
        default: return FAAC_ERR_INVALID_ARGUMENT;
    }
    switch (p->input_format) {
        case FAAC_INPUT_16BIT: case FAAC_INPUT_24BIT: case FAAC_INPUT_24IN32: case FAAC_INPUT_FLOAT:
            break;
        default:
            return FAAC_ERR_INVALID_ARGUMENT;
    }
    /* The ADTS/ASC rate table spans 7350..96000 Hz. Outside it the rate index
     * would lie, and the rate-derived tables and bit budgets are only sized for
     * it. */
    if (p->sample_rate < FAAC_MIN_SAMPLE_RATE || p->sample_rate > FAAC_MAX_SAMPLE_RATE)
        return FAAC_ERR_INVALID_ARGUMENT;
    if (GetChannelConfig((int)p->channels) == 0)
        return FAAC_ERR_INVALID_ARGUMENT;
    if (p->channels > MAX_CHANNELS)
        return FAAC_ERR_UNSUPPORTED;
    if (p->channel_layout != FAAC_LAYOUT_AAC && p->channel_layout != FAAC_LAYOUT_WAV)
        return FAAC_ERR_INVALID_ARGUMENT;
    /* The core runs on a per-channel rate. The bound is only a ceiling: it is
     * the ISO/IEC 14496-3 per-frame limit and therefore real, whereas the
     * format sets no minimum and the encoder's own floor already ends the
     * descent. */
    uint64_t bit_rate = resolved_bit_rate(p);
    *bit_rate_per_channel = bit_rate / p->channels;
    switch (p->rate_control) {
    case FAAC_RC_VBR:
        if (p->bit_rate || !p->quality || p->quality > 5000)
            return FAAC_ERR_INVALID_ARGUMENT;
        break;
    case FAAC_RC_ABR:
    case FAAC_RC_CBR:
        if (p->quality || !*bit_rate_per_channel)
            return FAAC_ERR_INVALID_ARGUMENT;
        if (*bit_rate_per_channel > MaxBitrate(p->sample_rate))
            return FAAC_ERR_INVALID_ARGUMENT;
        break;
    default:
        return FAAC_ERR_INVALID_ARGUMENT;
    }
    if (p->peak_bit_rate) {
        /* Bounded because it is converted into a per-frame bit budget; an
         * absurd value is a caller error, not a request for an unlimited
         * frame. */
        if (p->peak_bit_rate > FAAC_MAX_PEAK_BIT_RATE)
            return FAAC_ERR_INVALID_ARGUMENT;
        /* A peak below the average is unsatisfiable by definition, and the two
         * controllers actively fight: rate control raises quality to reach
         * bit_rate while the limiter cuts it to stay under the cap, so frames
         * end up larger the tighter the cap gets. Reject it rather than emit a
         * stream that quietly busts the ceiling it was asked to respect. */
        if (p->peak_bit_rate < bit_rate)
            return FAAC_ERR_INVALID_ARGUMENT;
    }
    return FAAC_OK;
}

FAACAPI faac_status faac_encoder_open(const faac_params *caller, faac_encoder **out)
{
    faac_params local;
    const faac_params *config = &local;
    faacEncStruct *h;
    faacEncConfiguration *cfg;
    unsigned long inSamples = 0, maxOut = 0;
    faac_status st;
    uint64_t bit_rate_per_channel;
    int applied;

    if (!out)
        return FAAC_ERR_INVALID_ARGUMENT;
    *out = NULL;
    if (!caller)
        return FAAC_ERR_INVALID_ARGUMENT;

    /* Not sizeof(faac_params): that grows, which would reject older callers. */
    if (caller->struct_size < PARAMS_BASELINE_SIZE)
        return FAAC_ERR_INVALID_ARGUMENT;

    /* Read only what the caller's struct_size covers; appended fields stay zero. */
    memset(&local, 0, sizeof(local));
    memcpy(&local, caller, caller->struct_size < sizeof(local) ? caller->struct_size : sizeof(local));

    st = validate_params(config, &bit_rate_per_channel);
    if (st != FAAC_OK)
        return st;

    h = (faacEncStruct *)faacEncOpen(config->sample_rate, config->channels,
                                     &inSamples, &maxOut);
    if (!h)
        return FAAC_ERR_NO_MEMORY;

    /* Install validated parameters before the core's one-time initialization. */
    cfg = &h->config;
    cfg->mpegVersion   = config->stream_format == FAAC_STREAM_ADTS_MPEG2 ? MPEG2 : MPEG4;
    cfg->aacObjectType = (unsigned int)config->object_type;  /* LOW / HE_V1 / AUTO */
    cfg->jointmode     = (unsigned int)config->stereo_mode;
#if MAX_CHANNELS >= 6
    cfg->useLfe        = (config->channels == 6 || config->channels == 8) ? 1 : 0;
#else
    cfg->useLfe        = 0;
#endif
    cfg->useTns        = (config->flags & FAAC_FLAG_TNS) ? 1 : 0;
    cfg->bitRate       = (unsigned long)bit_rate_per_channel;
    cfg->bandWidth     = config->bandwidth;
    cfg->quantqual     = config->quality;
    cfg->outputFormat  = config->stream_format == FAAC_STREAM_RAW ? RAW_STREAM : ADTS_STREAM;
    cfg->inputFormat   = (unsigned int)config->input_format;
    cfg->shortctl      = (int)config->short_control;
    cfg->usePns        = (config->flags & FAAC_FLAG_PNS) ? 1 : 0;
    cfg->maxBitRate    = config->peak_bit_rate;
    cfg->rateControl   = rate_mode_to_core(config->rate_control);
#if FAAC_ENCODER_DRC
    cfg->drcProfile    = (unsigned int)config->drc_profile;
    cfg->drcRefLevel   = config->drc_ref_level;
#endif
    /* WAVE order must become AAC element order; each entry names the input
     * channel that feeds that output slot. AAC input keeps the identity map
     * faacEncOpen installed. */
#if MAX_CHANNELS >= 3
    if (config->channel_layout == FAAC_LAYOUT_WAV && config->channels >= 3) {
        static const uint8_t wav_to_aac[][8] = {
            {2, 0, 1},                   /* 3 channels */
#if MAX_CHANNELS >= 4
            {2, 0, 1, 3},                /* 4 */
#endif
#if MAX_CHANNELS >= 5
            {2, 0, 1, 3, 4},             /* 5 */
#endif
#if MAX_CHANNELS >= 6
            {2, 0, 1, 4, 5, 3},          /* 6 */
#endif
#if MAX_CHANNELS >= 7
            {0},                         /* 7: unsupported */
#endif
#if MAX_CHANNELS >= 8
            {2, 0, 1, 6, 7, 4, 5, 3},    /* 8 */
#endif
        };
        uint32_t i;
        for (i = 0; i < config->channels; i++)
            cfg->channel_map[i] = wav_to_aac[config->channels - 3][i];
    }
#endif

    applied = faacEncApplyConfig(h);
    if (applied != 1) {
        faacEncClose((faacEncHandle)h);
        return (applied < 0) ? FAAC_ERR_NO_MEMORY : FAAC_ERR_INVALID_ARGUMENT;
    }

    /* ASC depends only on the resolved configuration. MPEG-2 has none. */
    if (cfg->mpegVersion != MPEG2)
        h->ascLen = (uint32_t)faacEncGetDecoderSpecificInfo((faacEncHandle)h, h->asc);

    *out = (faac_encoder *)h;
    return FAAC_OK;
}

FAACAPI faac_status faac_encoder_close(faac_encoder **encoder)
{
    if (!encoder)
        return FAAC_ERR_INVALID_ARGUMENT;
    if (*encoder) {
        faacEncClose((faacEncHandle)unwrap(*encoder));
        *encoder = NULL;
    }
    return FAAC_OK;
}

/* LC: one frame of 50% MDCT overlap. HE-AAC: that same core delay at full
 * rate, plus the half-band downsampler's FIR group delay, plus the one sample
 * the input FIFO is primed with to make the count even. The SBR decoder's QMF
 * delay is excluded from priming; decoders add it themselves when trimming.
 * PS adds one 256-sample hop of spectral-carrier overlap. */
static uint32_t faacEncoderDelay(const faacEncStruct *h)
{
    if (IsPS(h->config.aacObjectType))
        return 2 * FRAME_LEN + RESAMPLE_FILTER_LEN / 2 + 1 + 256;
    switch (h->config.aacObjectType) {
        case LOW:   return FRAME_LEN;
        case HE_V1: return 2 * FRAME_LEN + RESAMPLE_FILTER_LEN / 2 + 1;
    }
    assert(0 && "faacEncoderDelay: unhandled aacObjectType");
    return FRAME_LEN;
}

FAACAPI faac_status faac_encoder_get_info(const faac_encoder *encoder, faac_encoder_info *out_info)
{
    faac_encoder_info info;
    const faacEncStruct *h;
    uint32_t caller_size, n;

    if (!encoder || !out_info)
        return FAAC_ERR_INVALID_ARGUMENT;
    caller_size = out_info->struct_size;
    if (caller_size < ENCODER_INFO_BASELINE_SIZE)
        return FAAC_ERR_INVALID_ARGUMENT;
    h = (const faacEncStruct *)encoder;

    memset(&info, 0, sizeof(info));
    info.frame_samples    = faacFrameSamples(h);
    info.max_output_bytes = (uint32_t)ADTS_FRAMESIZE;
    info.object_type      = (enum faac_object_type)h->config.aacObjectType;
    info.bit_rate         = (uint32_t)((uint64_t)h->config.bitRate * h->numChannels);
    info.bandwidth        = (uint32_t)h->config.bandWidth;
    info.quality        = (uint32_t)h->config.quantqual;
    info.stream_format    = h->config.outputFormat == RAW_STREAM ? FAAC_STREAM_RAW :
        h->config.mpegVersion == MPEG2 ? FAAC_STREAM_ADTS_MPEG2 : FAAC_STREAM_ADTS;
    info.encoder_delay    = faacEncoderDelay(h);
#if FAAC_ENCODER_DRC
    info.drc_profile      = (enum faac_drc_profile)h->config.drcProfile;
    info.drc_ref_level    = h->config.drcRefLevel;
#endif
    info.asc_size         = h->ascLen;
    memcpy(info.asc, h->asc, h->ascLen);

    /* Write at most the caller's struct_size so a newer library cannot overrun
     * an older, smaller faac_encoder_info; report the byte count actually set. */
    n = caller_size < (uint32_t)sizeof(info) ? caller_size : (uint32_t)sizeof(info);
    info.struct_size = n;
    memcpy(out_info, &info, n);
    return FAAC_OK;
}

FAACAPI faac_status faac_encoder_asc(const faac_encoder *encoder,
                                     const uint8_t **buf, uint32_t *len)
{
    const faacEncStruct *h;

    if (!encoder || !buf || !len)
        return FAAC_ERR_INVALID_ARGUMENT;
    h = (const faacEncStruct *)encoder;

    if (h->config.mpegVersion == MPEG2) {
        *buf = NULL;
        *len = 0;
        return FAAC_ERR_UNSUPPORTED;
    }

    *buf = h->asc;
    *len = h->ascLen;
    return FAAC_OK;
}

FAACAPI faac_status faac_encoder_encode(faac_encoder *encoder,
                                        const void *in, uint32_t in_samples,
                                        uint8_t *out, uint32_t out_cap,
                                        uint32_t *bytes_written)
{
    faacEncStruct *h;
    int rc;

    if (!encoder || !out || !bytes_written)
        return FAAC_ERR_INVALID_ARGUMENT;
    *bytes_written = 0;
    if (in_samples && !in)
        return FAAC_ERR_INVALID_ARGUMENT;
    h = unwrap(encoder);

    if (out_cap < (uint32_t)ADTS_FRAMESIZE)
        return FAAC_ERR_OUTPUT_TOO_SMALL;
    if (in_samples > faacFrameSamples(h) * h->numChannels)
        return FAAC_ERR_INPUT_OVERFLOW;

    /* faacEncEncode reinterprets the input bytes per the configured
     * input_format; the int32_t* parameter is historical and does not imply an
     * int32 layout. */
    rc = faacEncEncode((faacEncHandle)h, (int32_t *)(uintptr_t)in,
                       in_samples, out, out_cap);
    /* A too-small output buffer was already rejected above, so a negative
     * return here is an unexpected core fault, not a buffer-size problem --
     * don't disguise it as one or the caller will grow the buffer and retry. */
    if (rc < 0)
        return FAAC_ERR_INTERNAL;

    *bytes_written = (uint32_t)rc;
    return FAAC_OK;
}

FAACAPI const char *faac_strerror(faac_status status)
{
    switch (status) {
        case FAAC_OK:                   return "success";
        case FAAC_ERR_INVALID_ARGUMENT: return "invalid argument";
        case FAAC_ERR_UNSUPPORTED:      return "unsupported configuration";
        case FAAC_ERR_NO_MEMORY:        return "out of memory";
        case FAAC_ERR_OUTPUT_TOO_SMALL: return "output buffer too small";
        case FAAC_ERR_INPUT_OVERFLOW:   return "input sample count too large";
        case FAAC_ERR_INTERNAL:         return "internal encoder error";
        case FAAC_STATUS_MAX:           break;
    }
    return "unknown error";
}
