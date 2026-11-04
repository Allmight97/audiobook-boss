/*
 * FAAC - Freeware Advanced Audio Coder
 * Copyright (C) 2026 Nils Schimmelmann
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public License
 * as published by the Free Software Foundation; either version 2.1 of
 * the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 */

/* PS tables are encoder-only to avoid adding decoder data footprint. */
#ifndef FAAC_PS_HUFF_TABLES_H
#define FAAC_PS_HUFF_TABLES_H

#include <stdint.h>

#define PS_HUFF_IID_DF_FINE_OFFSET 30
#define PS_HUFF_IID_DF_FINE_NSYMS 61
#define PS_HUFF_IID_DT_FINE_NSYMS 61
#define PS_HUFF_ICC_DF_NSYMS 15
#define PS_HUFF_ICC_DT_NSYMS 15
#define PS_HUFF_IPD_DF_NSYMS 8
#define PS_HUFF_IPD_DT_NSYMS 8
#define PS_HUFF_OPD_DF_NSYMS 8
#define PS_HUFF_OPD_DT_NSYMS 8

#define ps_iid_db_fine faac_ps_iid_db_fine
#define ps_icc_invq faac_ps_icc_invq

typedef struct {
    uint8_t byte[3];
} PsHuffPackedEntry;

extern const int8_t ps_iid_db_fine[31];
extern const float ps_icc_invq[8];

extern const PsHuffPackedEntry faac_ps_huff_iid_df_fine_packed[PS_HUFF_IID_DF_FINE_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_iid_dt_fine_packed[PS_HUFF_IID_DT_FINE_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_icc_df_packed[PS_HUFF_ICC_DF_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_icc_dt_packed[PS_HUFF_ICC_DT_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_ipd_df_packed[PS_HUFF_IPD_DF_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_ipd_dt_packed[PS_HUFF_IPD_DT_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_opd_df_packed[PS_HUFF_OPD_DF_NSYMS];
extern const PsHuffPackedEntry faac_ps_huff_opd_dt_packed[PS_HUFF_OPD_DT_NSYMS];

#endif /* FAAC_PS_HUFF_TABLES_H */
