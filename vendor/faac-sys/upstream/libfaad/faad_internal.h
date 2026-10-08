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

#ifndef FAAD_INTERNAL_H
#define FAAD_INTERNAL_H

#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include <stdint.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <float.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

/* Memory management macros (overridable, e.g. for embedded PSRAM) */
#ifndef AllocMemory
#define AllocMemory(size) malloc(size)
#endif
#ifndef FreeMemory
#define FreeMemory(block) free(block)
#endif

#include "faad.h"
_Static_assert(sizeof(bool) == 1, "public ABI requires one-byte flags");
_Static_assert(sizeof(float) == 4 && FLT_RADIX == 2 && FLT_MANT_DIG == 24,
               "PCM requires IEEE binary32 floats");
#include "faad_stats.h"
#include "sbr_tables.h"
typedef struct {
    uint32_t code : 24;
    uint32_t len : 8;
} SBRHuffEntry;

extern const float sbr_noise_table[512][2];

/* Parametric stereo codebooks (ISO/IEC 14496-3 §8.6). */
#define PS_HUFF_IID_DF_FINE_OFFSET 30
#define PS_HUFF_IID_DF_FINE_NSYMS  61
extern const SBRHuffEntry ps_huff_iid_df_fine[PS_HUFF_IID_DF_FINE_NSYMS];
#define PS_HUFF_IID_DT_FINE_OFFSET 30
#define PS_HUFF_IID_DT_FINE_NSYMS  61
extern const SBRHuffEntry ps_huff_iid_dt_fine[PS_HUFF_IID_DT_FINE_NSYMS];
#define PS_HUFF_IID_DF_OFFSET 14
#define PS_HUFF_IID_DF_NSYMS  29
extern const SBRHuffEntry ps_huff_iid_df[PS_HUFF_IID_DF_NSYMS];
#define PS_HUFF_IID_DT_OFFSET 14
#define PS_HUFF_IID_DT_NSYMS  29
extern const SBRHuffEntry ps_huff_iid_dt[PS_HUFF_IID_DT_NSYMS];
#define PS_HUFF_ICC_DF_OFFSET 7
#define PS_HUFF_ICC_DF_NSYMS  15
extern const SBRHuffEntry ps_huff_icc_df[PS_HUFF_ICC_DF_NSYMS];
#define PS_HUFF_ICC_DT_OFFSET 7
#define PS_HUFF_ICC_DT_NSYMS  15
extern const SBRHuffEntry ps_huff_icc_dt[PS_HUFF_ICC_DT_NSYMS];
#define PS_HUFF_IPD_DF_OFFSET 0
#define PS_HUFF_IPD_DF_NSYMS  8
extern const SBRHuffEntry ps_huff_ipd_df[PS_HUFF_IPD_DF_NSYMS];
#define PS_HUFF_IPD_DT_OFFSET 0
#define PS_HUFF_IPD_DT_NSYMS  8
extern const SBRHuffEntry ps_huff_ipd_dt[PS_HUFF_IPD_DT_NSYMS];
#define PS_HUFF_OPD_DF_OFFSET 0
#define PS_HUFF_OPD_DF_NSYMS  8
extern const SBRHuffEntry ps_huff_opd_df[PS_HUFF_OPD_DF_NSYMS];
#define PS_HUFF_OPD_DT_OFFSET 0
#define PS_HUFF_OPD_DT_NSYMS  8
extern const SBRHuffEntry ps_huff_opd_dt[PS_HUFF_OPD_DT_NSYMS];

/* Channel capacity: the build's -Dmax-channels (config.h) when present. Every
 * per-channel buffer, including the SBR and PS state, scales with it. */
#ifndef MAX_CHANNELS
#define MAX_CHANNELS 8
#endif
#if (MAX_CHANNELS < 2 || defined(FAAD_DISABLE_SBR)) && !defined(FAAD_DISABLE_PS)
#define FAAD_DISABLE_PS /* parametric stereo needs SBR's QMF domain and two output channels */
#endif
#define FRAME_LEN_LONG 1024
#ifdef FAAD_DISABLE_SBR
#define FRAME_SAMPLES_MAX 1024 /* no SBR doubling of the output rate */
#else
#define FRAME_SAMPLES_MAX 2048
#endif
#define FRAME_LEN_SHORT 128
#define NUM_WINDOWS 8

/* Index into the SFB tables for a sample rate (asc.c). */
int get_sr_index(uint32_t sample_rate);

#define ID_SCE 0x0
#define ID_CPE 0x1
#define ID_CCE 0x2
#define ID_LFE 0x3
#define ID_DSE 0x4
#define ID_PCE 0x5
#define ID_FIL 0x6
#define ID_END 0x7

#define ONLY_LONG_SEQUENCE 0
#define LONG_START_SEQUENCE 1
#define EIGHT_SHORT_SEQUENCE 2
#define LONG_STOP_SEQUENCE 3

#define SINE_WINDOW 0
#define KBD_WINDOW  1

#define SBR_EXTENSION_DATA 13
#define SBR_EXTENSION_DATA_CRC 14
#define PS_EXTENSION_DATA 2

#define SBR_PS_BANDS 20
#define SBR_PS_IID_LEVELS 15
#define SBR_PS_ICC_LEVELS 8

typedef struct {
    const uint8_t *buffer;
    uint32_t len;
    uint32_t byte_pos;
    uint32_t bit_pos;    /* 0..7 within the byte, MSB first */
} BitReader;

void bits_init(BitReader *bs, const uint8_t *buffer, uint32_t len);

uint32_t bits_get(BitReader *bs, uint32_t nbits);
uint32_t bits_show(BitReader *bs, uint32_t nbits);

static inline uint32_t bits_get_1(BitReader *bs)
{
    if (bs->byte_pos < bs->len) {
        uint32_t bit = (bs->buffer[bs->byte_pos] >> (7 - bs->bit_pos)) & 1U;
        uint32_t next_bit = bs->bit_pos + 1;
        bs->byte_pos += next_bit >> 3;
        bs->bit_pos = next_bit & 7;
        return bit;
    }
    return 0;
}

static inline uint32_t bits_get_fast(BitReader *bs, uint32_t nbits)
{
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
    return bits_get(bs, nbits);
}

static inline uint32_t bits_show_fast(BitReader *bs, uint32_t nbits)
{
    if (nbits > 0 && nbits <= 24 && bs->byte_pos + 4 <= bs->len) {
        const uint8_t *ptr = bs->buffer + bs->byte_pos;
        uint32_t word = ((uint32_t)ptr[0] << 24) | ((uint32_t)ptr[1] << 16) |
                        ((uint32_t)ptr[2] << 8)  | (uint32_t)ptr[3];
        return (word << bs->bit_pos) >> (32 - nbits);
    }
    return bits_show(bs, nbits);
}
void bits_skip(BitReader *bs, uint32_t nbits);
void bits_byte_align(BitReader *bs);
uint32_t bits_get_consumed(BitReader *bs);

typedef struct {
    enum faad_object_type object_type;
    uint32_t sample_rate;
    uint32_t num_channels;
    bool is_sbr;
    bool is_ps;
    uint32_t sbr_sample_rate;
} AudioSpecificConfig;

faad_status asc_decode(BitReader *bs, AudioSpecificConfig *asc);
faad_status adts_decode_header(BitReader *bs, AudioSpecificConfig *asc, uint32_t *frame_length);

#define MAX_SFB       52 /* long blocks reach 51 bands, short ones 8 x 15 */
#define TNS_MAX_ORDER 12 /* AAC-LC: 12 for long blocks, 7 for short */

typedef struct {
    uint8_t window_sequence;
    uint8_t window_shape;
    uint8_t max_sfb;
    uint8_t num_window_groups;
    uint8_t window_group_length[NUM_WINDOWS];
    uint8_t num_windows;
    uint16_t sfb_offsets[68];
    uint8_t num_sfbs;
    int8_t sample_rate_index; /* core rate index, for TNS_MAX_BANDS */
    /* per window group and scalefactor band: the codebook (0 zero, 13 noise,
     * 14/15 intensity) and the scalefactor, noise energy or intensity position */
    uint8_t sfb_cb[8][MAX_SFB];
    int16_t scalefactors[8][MAX_SFB];
    uint8_t global_gain;

    /* Pulse data: up to four spectral lines of a long window whose
     * quantised magnitude is raised by pulse_amp */
    bool pulse_data_present;
    uint8_t pulse_count;
    uint8_t pulse_amp[4];
    uint16_t pulse_pos[4];

    bool tns_data_present;
    uint8_t tns_n_filt[8];
    uint8_t tns_length[8][4];
    uint8_t tns_order[8][4];
    uint8_t tns_direction[8][4];
    uint8_t tns_coef_res[8];
    int8_t  tns_coef[8][4][TNS_MAX_ORDER];

    bool gain_control_present;
} ICSInfo;

typedef struct {
    bool common_window;
    uint8_t ms_mask_present; /* 0 none, 1 per band, 2 all bands */
    uint8_t ms_used[8][MAX_SFB];
    ICSInfo ics[2];
} CPEInfo;


/* Parametric stereo (ISO/IEC 14496-3 §8.6) */
#define PS_MAX_ENV      5   /* four coded envelopes plus the implicit trailing one */
#define PS_NR_PAR       34
#define PS_NR_BANDS     91  /* hybrid sub-bands in the 34-parameter layout */
#define PS_QMF_SLOTS    32
#define PS_IN_SLOTS     38  /* QMF slots handed to the hybrid bank: 32 plus 6 of look-ahead */
#define PS_MAX_DELAY    14
#define PS_NR_ALLPASS   50  /* all-pass bands in the 34-parameter layout (30 in the 20) */
#define PS_AP_STATE     12  /* link delays 3 + 4 + 5 */

typedef struct {
    bool    start;          /* a header has been seen */
    bool    enable_iid, enable_icc, enable_ext, enable_ipdopd;
    bool    iid_quant;      /* fine (31-step) IID quantisation */
    uint8_t icc_mode;
    uint8_t nr_iid_par, nr_icc_par, nr_ipdopd_par;
    uint8_t frame_class, num_env, num_env_old;
    int8_t  border[PS_MAX_ENV + 1];
    int8_t  iid_par[PS_MAX_ENV][PS_NR_PAR];
    int8_t  icc_par[PS_MAX_ENV][PS_NR_PAR];
    int8_t  ipd_par[PS_MAX_ENV][PS_NR_PAR];
    int8_t  opd_par[PS_MAX_ENV][PS_NR_PAR];
    bool    is34, is34_old;

    float   in_buf[5][PS_IN_SLOTS + 6][2];                  /* hybrid analysis history */
    /* Decorrelator history, only what a frame leaves for the next: the
     * two-slot input delay and the 3+4+5 link states of each all-pass
     * band, the 14-slot line of each delay band (indexed from the first
     * delay band). */
    float   dc_in[PS_NR_ALLPASS][2][2];
    float   dc_ap[PS_NR_ALLPASS][PS_AP_STATE][2];
    float   dc_delay[PS_NR_BANDS - 30][PS_MAX_DELAY][2];
    uint8_t ap_pos[3];        /* ring positions of the three link states */
    uint8_t dl_pos;           /* ring position of the delay lines */
    float   peak_decay_nrg[PS_NR_PAR], power_smooth[PS_NR_PAR], peak_decay_diff_smooth[PS_NR_PAR];
    float   H[4][2][PS_MAX_ENV + 1][PS_NR_PAR];             /* mixing matrix per envelope border */
    int8_t  ipd_hist[17], opd_hist[17];                     /* two previous indices, packed */
} PSState;

/* Canonical SBR books need only symbol order and length boundaries; PS
 * books retain explicit codewords because some are not canonical. */
enum {
    HB_T_ENV_15, HB_F_ENV_15, HB_T_ENV_BAL_15, HB_F_ENV_BAL_15,
    HB_T_ENV_30, HB_F_ENV_30, HB_T_ENV_BAL_30, HB_F_ENV_BAL_30,
    HB_T_NOISE_30, HB_T_NOISE_BAL_30,
#ifndef FAAD_DISABLE_PS
    HB_PS_IID_DF_FINE, HB_PS_IID_DT_FINE, HB_PS_IID_DF, HB_PS_IID_DT,
    HB_PS_ICC_DF, HB_PS_ICC_DT, HB_PS_IPD_DF, HB_PS_IPD_DT, HB_PS_OPD_DF, HB_PS_OPD_DT,
#endif
    HB_COUNT
};
#define HB_MAX_LEN 20
typedef struct {
    const SBRHuffEntry *tab; /* NULL for canonical codebooks */
    const uint8_t *order;
    int8_t offset;
    uint8_t first[HB_MAX_LEN + 2];
} SBRHuffBook;
extern const SBRHuffBook sbr_books[HB_COUNT];
int  sbr_huff_decode(BitReader *bs, const SBRHuffBook *book);

/* SBR (ISO/IEC 14496-3 §4.6.18) */
#define SBR_SLOTS        32  /* QMF time slots per frame: numTimeSlots (16) * RATE (2) */
#define SBR_T_HFGEN      8   /* slots of the previous frame kept for the covariance and X_low */
#define SBR_T_HFADJ      2   /* offset of the envelope-adjusted region within the buffer */
#define SBR_BUF_SLOTS    (SBR_SLOTS + SBR_T_HFGEN)
#define SBR_MAX_BANDS    64
#define SBR_MAX_ENV      5
#define SBR_MAX_NQ       5
#define SBR_MAX_PATCHES  6
#define SBR_MAX_LIM      (SBR_MAX_BANDS + SBR_MAX_PATCHES + 2)

typedef struct {
    uint8_t frame_class, L_E, L_Q, bs_pointer;
    int8_t  l_A;
    uint8_t t_E[SBR_MAX_ENV + 1], t_Q[3], freq_res[SBR_MAX_ENV];
    uint8_t df_env[SBR_MAX_ENV], df_noise[2];
    uint8_t invf_mode[SBR_MAX_NQ], invf_mode_prev[SBR_MAX_NQ];
    bool    add_harmonic_flag;
    uint8_t add_harmonic[SBR_MAX_BANDS];
    int16_t E[SBR_MAX_ENV][SBR_MAX_BANDS];
    int16_t Q[2][SBR_MAX_NQ];
    bool    amp_res; /* this frame's resolution (a single FIXFIX envelope forces 1.5 dB) */

    int16_t E_prev[SBR_MAX_BANDS];
    int16_t Q_prev[SBR_MAX_NQ];
    uint8_t freq_res_prev;
    float   bw_array[SBR_MAX_NQ];
    float   g_hist[4][SBR_MAX_BANDS]; /* gains of the four previous envelopes, a ring */
    float   q_hist[4][SBR_MAX_BANDS];
    uint8_t hist_pos;                 /* oldest entry of the ring */
    uint8_t s_index_prev[SBR_MAX_BANDS];
    int8_t  l_A_prev;
    uint8_t L_E_prev;
    uint8_t t_E_end_prev; /* RATE * t_E(L_E) of the previous frame */
    uint8_t kx_prev, M_prev;
    uint16_t index_noise;
    uint8_t  index_sine;
    bool    have_frame;   /* a payload has been decoded since the last reset */
    bool    primed;       /* smoothing history holds real gains */
    float   x_low_tail[32][SBR_T_HFGEN][2];
    float   y_tail[SBR_MAX_BANDS][SBR_T_HFGEN][2];
    float   qmf_x[640];  /* analysis delay line, newest sample first, mirrored ring */
    uint16_t qmf_x_pos;  /* start of the newest block in qmf_x */
    float   qmf_v[1280]; /* synthesis delay line, ring of 128-sample blocks */
    uint16_t qmf_v_pos;  /* start of the newest block in qmf_v */
} SBRChannel;

/* Header and frequency tables, shared by the channels of one element and
 * stored at the element's first channel. */
typedef struct {
    bool header_present;
    bool coupling;
    uint8_t nch;
    bool amp_res;
    uint8_t start_freq, stop_freq, xover_band, freq_scale, alter_scale, noise_bands;
    uint8_t limiter_bands, limiter_gains, interpol_freq, smoothing_mode;
    uint8_t k0, k2, kx, M;
    uint8_t n_master, n_high, n_low, n_q, n_lim, num_patches;
    uint8_t f_master[SBR_MAX_BANDS + 1], f_high[SBR_MAX_BANDS + 1], f_low[SBR_MAX_BANDS + 1];
    uint8_t f_noise[SBR_MAX_NQ + 1], f_lim[SBR_MAX_LIM + 1];
    uint8_t patch_start[SBR_MAX_PATCHES], patch_num[SBR_MAX_PATCHES];
} SBRElement;

/* Per-frame working buffers, one channel at a time. */
typedef struct {
    float x_low[32][SBR_BUF_SLOTS][2];
    float y[SBR_MAX_BANDS][SBR_BUF_SLOTS][2]; /* generated HF, adjusted in place */
    float x[PS_IN_SLOTS][64][2]; /* assembled output per slot (38 for the PS look-ahead) */
} SBRScratch;

/* Working memory of one frame's decode phases, which never overlap: the
 * element being parsed (and a coupling element's discarded payload), the IMDCT
 * temporaries, then the SBR/PS buffers. */
typedef union {
    CPEInfo cpe;
    struct { ICSInfo ics; float spec[FRAME_LEN_LONG]; } cce;
    float work[2 * FRAME_LEN_LONG];
#ifndef FAAD_DISABLE_SBR
    SBRScratch sbr;
#endif
} FrameScratch;

struct faad_decoder {
    faad_config config;
    AudioSpecificConfig asc;
    bool asc_parsed;
    bool is_heap_allocated;
    void *heap_storage; /* original allocator pointer before alignment */
    bool format_known;
    bool pcm_emitted;
    /* Parsing and synthesis may advance on rejected packets; only emitted PCM
     * may replace the output format that callers use. */
    struct {
        uint32_t sample_rate, channels, frame_samples, channel_mask, decoder_delay;
    } emitted_format;
    bool ps_seen;
    uint32_t core_channels;
    uint32_t max_output_bytes;

    uint32_t frame_samples; /* 1024 or 2048 */
    uint32_t num_channels;
    uint32_t sample_rate;      /* nominal (post-SBR) rate, for reporting */
    uint32_t core_sample_rate; /* window/sfb layout rate: half of sample_rate with SBR */

    float spec[MAX_CHANNELS][FRAME_LEN_LONG];
    float overlap[MAX_CHANNELS][FRAME_LEN_LONG];
    uint8_t prev_window_shape[MAX_CHANNELS]; /* the left window half follows the previous block's shape */
    uint8_t prev_window_seq[MAX_CHANNELS];   /* what a concealed frame continues from */

#ifndef FAAD_DISABLE_SBR
    SBRChannel sbr[MAX_CHANNELS];
    SBRElement sbr_el[MAX_CHANNELS];
#endif
    bool sbr_present;
    bool sbr_seen; /* an SBR payload has appeared: frames without one still run at the SBR rate */

#ifndef FAAD_DISABLE_PS
    PSState ps;
#endif
    bool ps_present;

    uint32_t pns_seed;
    uint32_t consecutive_errors;
    float prev_spec[MAX_CHANNELS][FRAME_LEN_LONG];

    FrameScratch scratch;
    uint8_t win_seq[MAX_CHANNELS];   /* this frame's window of each decoded channel, for the IMDCT */
    uint8_t win_shape[MAX_CHANNELS];

    float pcm[MAX_CHANNELS * FRAME_SAMPLES_MAX]; /* core output, then SBR output in place */
};

/* Core-rate QMF delay; public metadata converts it to output samples. */
#define FAAD_SBR_CORE_DELAY 481u
_Static_assert(_Alignof(struct faad_decoder) <= FAAD_STATE_ALIGNMENT,
               "decoder exceeds public placement alignment");
_Static_assert(sizeof(struct faad_decoder) <= UINT32_MAX - (FAAD_STATE_ALIGNMENT - 1),
               "decoder storage size must fit 32-bit allocation arithmetic");

void setup_sfb_offsets(ICSInfo *ics, uint32_t sample_rate);
faad_status decode_scale_factor_data(BitReader *bs, ICSInfo *ics, uint32_t sample_rate);
faad_status decode_spectral_data(BitReader *bs, ICSInfo *ics, float *spec);
void apply_pns(ICSInfo *ics, float *spec, uint32_t *pns_seed);
void apply_ms_stereo(CPEInfo *cpe, float *spec_l, float *spec_r);
void apply_is_stereo(CPEInfo *cpe, float *spec_l, float *spec_r);
void apply_tns(ICSInfo *ics, float *spec);
void imdct_and_window(struct faad_decoder *dec, uint32_t ch, uint8_t window_sequence, uint8_t window_shape, float *spec, float *out_pcm);

faad_status decode_pce(BitReader *bs, struct faad_decoder *dec);
faad_status decode_cce(BitReader *bs, struct faad_decoder *dec);
faad_status decode_dse(BitReader *bs);
faad_status decode_ics(BitReader *bs, struct faad_decoder *dec, ICSInfo *ics, float *spec, bool common_window);
faad_status decode_cpe(BitReader *bs, struct faad_decoder *dec, CPEInfo *cpe, uint32_t ch);
faad_status decode_sce(BitReader *bs, struct faad_decoder *dec, ICSInfo *ics, uint32_t ch);

void faad_init_global_tables(void);
void sbr_init_tables(void);
faad_status sbr_decode_extension(struct faad_decoder *dec, BitReader *bs, uint32_t ch0, uint32_t syntax_id, bool crc);
void ps_read_data(struct faad_decoder *dec, BitReader *bs, uint32_t bits_left);
void ps_frame_begin(struct faad_decoder *dec, float X[PS_IN_SLOTS][64][2], int top);
void ps_slot(struct faad_decoder *dec, int n, float X[PS_IN_SLOTS][64][2], float L[64][2], float R[64][2]);
void init_ps_tables(void);
void sbr_apply(struct faad_decoder *dec, uint32_t num_ch, float *pcm);
#ifdef FAAD_STATS
FILE *faad_dump_file(void);
#endif

#endif /* FAAD_INTERNAL_H */
