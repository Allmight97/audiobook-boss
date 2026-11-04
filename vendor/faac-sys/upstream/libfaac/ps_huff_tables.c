/*
 * FAAC - Freeware Advanced Audio Coder
 * Copyright (C) 2026 Nils Schimmelmann
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public License
 * as published by the Free Software Foundation; either version 2.1 of
 * the License, or (at your option) any later version.
 */
/* Encoder-only PS tables avoid decoder data overhead. Three-byte entries use
 * explicit byte order for host independence.
 */
#include "ps_huff_tables.h"
#define PS_PACK(code, len)                                                                         \
    {{(uint8_t)((((uint32_t)(code) << 5) | (uint32_t)(len)) & 0xffu),                              \
      (uint8_t)((((uint32_t)(code) << 5) | (uint32_t)(len)) >> 8),                                 \
      (uint8_t)((((uint32_t)(code) << 5) | (uint32_t)(len)) >> 16)}}

/* Keep encoder analysis constants out of decoder builds to limit data size. */
const int8_t ps_iid_db_fine[31] = {-50, -45, -40, -35, -30, -25, -22, -19, -16, -13, -10,
                                   -8,  -6,  -4,  -2,  0,   2,   4,   6,   8,   10,  13,
                                   16,  19,  22,  25,  30,  35,  40,  45,  50};
const float ps_icc_invq[8] = {1.0f, 0.937f, 0.84118f, 0.60092f, 0.36764f, 0.0f, -0.589f, -1.0f};

const PsHuffPackedEntry faac_ps_huff_iid_df_fine_packed[PS_HUFF_IID_DF_FINE_NSYMS] = {
    /*  -30 */ PS_PACK(0x0001feb4u, 18),
    /*  -29 */ PS_PACK(0x0001feb5u, 18),
    /*  -28 */ PS_PACK(0x0001fd76u, 18),
    /*  -27 */ PS_PACK(0x0001fd77u, 18),
    /*  -26 */ PS_PACK(0x0001fd74u, 18),
    /*  -25 */ PS_PACK(0x0001fd75u, 18),
    /*  -24 */ PS_PACK(0x0001fe8au, 18),
    /*  -23 */ PS_PACK(0x0001fe8bu, 18),
    /*  -22 */ PS_PACK(0x0001fe88u, 18),
    /*  -21 */ PS_PACK(0x0000fe80u, 17),
    /*  -20 */ PS_PACK(0x0001feb6u, 18),
    /*  -19 */ PS_PACK(0x0000fe82u, 17),
    /*  -18 */ PS_PACK(0x0000feb8u, 17),
    /*  -17 */ PS_PACK(0x00007f42u, 16),
    /*  -16 */ PS_PACK(0x00007faeu, 16),
    /*  -15 */ PS_PACK(0x00003fafu, 15),
    /*  -14 */ PS_PACK(0x00001fd1u, 14),
    /*  -13 */ PS_PACK(0x00001fe9u, 14),
    /*  -12 */ PS_PACK(0x00000fe9u, 13),
    /*  -11 */ PS_PACK(0x000007eau, 12),
    /*  -10 */ PS_PACK(0x000007fbu, 12),
    /*   -9 */ PS_PACK(0x000003fbu, 11),
    /*   -8 */ PS_PACK(0x000001fbu, 10),
    /*   -7 */ PS_PACK(0x000001ffu, 10),
    /*   -6 */ PS_PACK(0x0000007cu, 8),
    /*   -5 */ PS_PACK(0x0000003cu, 7),
    /*   -4 */ PS_PACK(0x0000001cu, 6),
    /*   -3 */ PS_PACK(0x0000000cu, 5),
    /*   -2 */ PS_PACK(0x00000000u, 4),
    /*   -1 */ PS_PACK(0x00000001u, 3),
    /*    0 */ PS_PACK(0x00000001u, 1),
    /*    1 */ PS_PACK(0x00000002u, 3),
    /*    2 */ PS_PACK(0x00000001u, 4),
    /*    3 */ PS_PACK(0x0000000du, 5),
    /*    4 */ PS_PACK(0x0000001du, 6),
    /*    5 */ PS_PACK(0x0000003du, 7),
    /*    6 */ PS_PACK(0x0000007du, 8),
    /*    7 */ PS_PACK(0x000000fcu, 9),
    /*    8 */ PS_PACK(0x000001fcu, 10),
    /*    9 */ PS_PACK(0x000003fcu, 11),
    /*   10 */ PS_PACK(0x000003f4u, 11),
    /*   11 */ PS_PACK(0x000007ebu, 12),
    /*   12 */ PS_PACK(0x00000feau, 13),
    /*   13 */ PS_PACK(0x00001feau, 14),
    /*   14 */ PS_PACK(0x00001fd6u, 14),
    /*   15 */ PS_PACK(0x00003fd0u, 15),
    /*   16 */ PS_PACK(0x00007fafu, 16),
    /*   17 */ PS_PACK(0x00007f43u, 16),
    /*   18 */ PS_PACK(0x0000feb9u, 17),
    /*   19 */ PS_PACK(0x0000fe83u, 17),
    /*   20 */ PS_PACK(0x0001feb7u, 18),
    /*   21 */ PS_PACK(0x0000fe81u, 17),
    /*   22 */ PS_PACK(0x0001fe89u, 18),
    /*   23 */ PS_PACK(0x0001fe8eu, 18),
    /*   24 */ PS_PACK(0x0001fe8fu, 18),
    /*   25 */ PS_PACK(0x0001fe8cu, 18),
    /*   26 */ PS_PACK(0x0001fe8du, 18),
    /*   27 */ PS_PACK(0x0001feb2u, 18),
    /*   28 */ PS_PACK(0x0001feb3u, 18),
    /*   29 */ PS_PACK(0x0001feb0u, 18),
    /*   30 */ PS_PACK(0x0001feb1u, 18),
};

const PsHuffPackedEntry faac_ps_huff_iid_dt_fine_packed[PS_HUFF_IID_DT_FINE_NSYMS] = {
    /*  -30 */ PS_PACK(0x00004ed4u, 16),
    /*  -29 */ PS_PACK(0x00004ed5u, 16),
    /*  -28 */ PS_PACK(0x00004eceu, 16),
    /*  -27 */ PS_PACK(0x00004ecfu, 16),
    /*  -26 */ PS_PACK(0x00004eccu, 16),
    /*  -25 */ PS_PACK(0x00004ed6u, 16),
    /*  -24 */ PS_PACK(0x00004ed8u, 16),
    /*  -23 */ PS_PACK(0x00004f46u, 16),
    /*  -22 */ PS_PACK(0x00004f60u, 16),
    /*  -21 */ PS_PACK(0x00002718u, 15),
    /*  -20 */ PS_PACK(0x00002719u, 15),
    /*  -19 */ PS_PACK(0x00002764u, 15),
    /*  -18 */ PS_PACK(0x00002765u, 15),
    /*  -17 */ PS_PACK(0x0000276du, 15),
    /*  -16 */ PS_PACK(0x000027b1u, 15),
    /*  -15 */ PS_PACK(0x000013b7u, 14),
    /*  -14 */ PS_PACK(0x000013d6u, 14),
    /*  -13 */ PS_PACK(0x000009c7u, 13),
    /*  -12 */ PS_PACK(0x000009e9u, 13),
    /*  -11 */ PS_PACK(0x000009edu, 13),
    /*  -10 */ PS_PACK(0x000004eeu, 12),
    /*   -9 */ PS_PACK(0x000004f7u, 12),
    /*   -8 */ PS_PACK(0x00000278u, 11),
    /*   -7 */ PS_PACK(0x00000139u, 10),
    /*   -6 */ PS_PACK(0x0000009au, 9),
    /*   -5 */ PS_PACK(0x0000009fu, 9),
    /*   -4 */ PS_PACK(0x00000020u, 7),
    /*   -3 */ PS_PACK(0x00000011u, 6),
    /*   -2 */ PS_PACK(0x0000000au, 5),
    /*   -1 */ PS_PACK(0x00000003u, 3),
    /*    0 */ PS_PACK(0x00000001u, 1),
    /*    1 */ PS_PACK(0x00000000u, 2),
    /*    2 */ PS_PACK(0x0000000bu, 5),
    /*    3 */ PS_PACK(0x00000012u, 6),
    /*    4 */ PS_PACK(0x00000021u, 7),
    /*    5 */ PS_PACK(0x0000004cu, 8),
    /*    6 */ PS_PACK(0x0000009bu, 9),
    /*    7 */ PS_PACK(0x0000013au, 10),
    /*    8 */ PS_PACK(0x00000279u, 11),
    /*    9 */ PS_PACK(0x00000270u, 11),
    /*   10 */ PS_PACK(0x000004efu, 12),
    /*   11 */ PS_PACK(0x000004e2u, 12),
    /*   12 */ PS_PACK(0x000009eau, 13),
    /*   13 */ PS_PACK(0x000009d8u, 13),
    /*   14 */ PS_PACK(0x000013d7u, 14),
    /*   15 */ PS_PACK(0x000013d0u, 14),
    /*   16 */ PS_PACK(0x000027b2u, 15),
    /*   17 */ PS_PACK(0x000027a2u, 15),
    /*   18 */ PS_PACK(0x0000271au, 15),
    /*   19 */ PS_PACK(0x0000271bu, 15),
    /*   20 */ PS_PACK(0x00004f66u, 16),
    /*   21 */ PS_PACK(0x00004f67u, 16),
    /*   22 */ PS_PACK(0x00004f61u, 16),
    /*   23 */ PS_PACK(0x00004f47u, 16),
    /*   24 */ PS_PACK(0x00004ed9u, 16),
    /*   25 */ PS_PACK(0x00004ed7u, 16),
    /*   26 */ PS_PACK(0x00004ecdu, 16),
    /*   27 */ PS_PACK(0x00004ed2u, 16),
    /*   28 */ PS_PACK(0x00004ed3u, 16),
    /*   29 */ PS_PACK(0x00004ed0u, 16),
    /*   30 */ PS_PACK(0x00004ed1u, 16),
};

const PsHuffPackedEntry faac_ps_huff_icc_df_packed[PS_HUFF_ICC_DF_NSYMS] = {
    /*   -7 */ PS_PACK(0x00003fffu, 14),
    /*   -6 */ PS_PACK(0x00003ffeu, 14),
    /*   -5 */ PS_PACK(0x00000ffeu, 12),
    /*   -4 */ PS_PACK(0x000003feu, 10),
    /*   -3 */ PS_PACK(0x0000007eu, 7),
    /*   -2 */ PS_PACK(0x0000001eu, 5),
    /*   -1 */ PS_PACK(0x00000006u, 3),
    /*    0 */ PS_PACK(0x00000000u, 1),
    /*    1 */ PS_PACK(0x00000002u, 2),
    /*    2 */ PS_PACK(0x0000000eu, 4),
    /*    3 */ PS_PACK(0x0000003eu, 6),
    /*    4 */ PS_PACK(0x000000feu, 8),
    /*    5 */ PS_PACK(0x000001feu, 9),
    /*    6 */ PS_PACK(0x000007feu, 11),
    /*    7 */ PS_PACK(0x00001ffeu, 13),
};

const PsHuffPackedEntry faac_ps_huff_icc_dt_packed[PS_HUFF_ICC_DT_NSYMS] = {
    /*   -7 */ PS_PACK(0x00003ffeu, 14),
    /*   -6 */ PS_PACK(0x00001ffeu, 13),
    /*   -5 */ PS_PACK(0x000007feu, 11),
    /*   -4 */ PS_PACK(0x000001feu, 9),
    /*   -3 */ PS_PACK(0x0000007eu, 7),
    /*   -2 */ PS_PACK(0x0000001eu, 5),
    /*   -1 */ PS_PACK(0x00000006u, 3),
    /*    0 */ PS_PACK(0x00000000u, 1),
    /*    1 */ PS_PACK(0x00000002u, 2),
    /*    2 */ PS_PACK(0x0000000eu, 4),
    /*    3 */ PS_PACK(0x0000003eu, 6),
    /*    4 */ PS_PACK(0x000000feu, 8),
    /*    5 */ PS_PACK(0x000003feu, 10),
    /*    6 */ PS_PACK(0x00000ffeu, 12),
    /*    7 */ PS_PACK(0x00003fffu, 14),
};

const PsHuffPackedEntry faac_ps_huff_ipd_df_packed[PS_HUFF_IPD_DF_NSYMS] = {
    /*    0 */ PS_PACK(0x00000001u, 1),
    /*    1 */ PS_PACK(0x00000000u, 3),
    /*    2 */ PS_PACK(0x00000006u, 4),
    /*    3 */ PS_PACK(0x00000004u, 4),
    /*    4 */ PS_PACK(0x00000002u, 4),
    /*    5 */ PS_PACK(0x00000003u, 4),
    /*    6 */ PS_PACK(0x00000005u, 4),
    /*    7 */ PS_PACK(0x00000007u, 4),
};

const PsHuffPackedEntry faac_ps_huff_ipd_dt_packed[PS_HUFF_IPD_DT_NSYMS] = {
    /*    0 */ PS_PACK(0x00000001u, 1),
    /*    1 */ PS_PACK(0x00000002u, 3),
    /*    2 */ PS_PACK(0x00000002u, 4),
    /*    3 */ PS_PACK(0x00000003u, 5),
    /*    4 */ PS_PACK(0x00000002u, 5),
    /*    5 */ PS_PACK(0x00000000u, 4),
    /*    6 */ PS_PACK(0x00000003u, 4),
    /*    7 */ PS_PACK(0x00000003u, 3),
};

const PsHuffPackedEntry faac_ps_huff_opd_df_packed[PS_HUFF_OPD_DF_NSYMS] = {
    /*    0 */ PS_PACK(0x00000001u, 1),
    /*    1 */ PS_PACK(0x00000001u, 3),
    /*    2 */ PS_PACK(0x00000006u, 4),
    /*    3 */ PS_PACK(0x00000004u, 4),
    /*    4 */ PS_PACK(0x0000000fu, 5),
    /*    5 */ PS_PACK(0x0000000eu, 5),
    /*    6 */ PS_PACK(0x00000005u, 4),
    /*    7 */ PS_PACK(0x00000000u, 3),
};

const PsHuffPackedEntry faac_ps_huff_opd_dt_packed[PS_HUFF_OPD_DT_NSYMS] = {
    /*    0 */ PS_PACK(0x00000001u, 1),
    /*    1 */ PS_PACK(0x00000002u, 3),
    /*    2 */ PS_PACK(0x00000001u, 4),
    /*    3 */ PS_PACK(0x00000007u, 5),
    /*    4 */ PS_PACK(0x00000006u, 5),
    /*    5 */ PS_PACK(0x00000000u, 4),
    /*    6 */ PS_PACK(0x00000002u, 4),
    /*    7 */ PS_PACK(0x00000003u, 3),
};
#undef PS_PACK
