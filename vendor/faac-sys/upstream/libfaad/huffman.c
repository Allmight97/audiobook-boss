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
#include "sfb_tables.h"
#include <math.h>

static float pow_4_3_lut[128];
static float sf_scale_lut[256];

void init_dequant_tables(void)
{
    for (int i = 0; i < 128; i++) {
        pow_4_3_lut[i] = powf((float)i, 4.0f / 3.0f);
    }
    for (int i = 0; i < 256; i++) {
        sf_scale_lut[i] = powf(2.0f, 0.25f * (i - 100));
    }
}

void setup_sfb_offsets(ICSInfo *ics, uint32_t sample_rate)
{
    memset(ics->sfb_offsets, 0, sizeof(ics->sfb_offsets));
    int sr_idx = get_sr_index(sample_rate);
    ics->sample_rate_index = (int8_t)sr_idx;
    if (sr_idx < 0) {
        ics->num_sfbs = 0;      /* faad_decoder_init() rejects such rates */
        return;
    }
    if (ics->window_sequence == EIGHT_SHORT_SEQUENCE) {
        ics->num_sfbs = num_sfbs_128[sr_idx];
        const uint16_t *offsets = sfb_offsets_128[sr_idx];
        for (int i = 0; i <= ics->num_sfbs && i < 68; i++) {
            ics->sfb_offsets[i] = offsets[i];
        }
    } else {
        ics->num_sfbs = num_sfbs_1024[sr_idx];
        const uint16_t *offsets = sfb_offsets_1024[sr_idx];
        for (int i = 0; i <= ics->num_sfbs && i < 68; i++) {
            ics->sfb_offsets[i] = offsets[i];
        }
    }
}

typedef struct { uint16_t sym : 9; uint16_t len : 5; } HuffDesc;
_Static_assert(sizeof(HuffDesc) == 2, "Huffman descriptor size");
/* Canonical symbol order and lengths rebuild the existing lookup tables
 * without retaining full codewords in decoder ROM. */
static const HuffDesc huff_desc[] = {
    /* Book 1 */
    {40, 1}, {67, 5}, {13, 5}, {39, 5}, {49, 5}, {41, 5}, {37, 5}, {43, 5},
    {31, 5}, {58, 7}, {22, 7}, {38, 7}, {46, 7}, {34, 7}, {42, 7}, {76, 7},
    {36, 7}, {4, 7}, {28, 7}, {64, 7}, {48, 7}, {16, 7}, {44, 7}, {70, 7},
    {32, 7}, {52, 7}, {50, 7}, {10, 7}, {68, 7}, {12, 7}, {66, 7}, {14, 7},
    {30, 7}, {73, 9}, {19, 9}, {61, 9}, {51, 9}, {47, 9}, {35, 9}, {33, 9},
    {55, 9}, {65, 9}, {45, 9}, {25, 9}, {15, 9}, {7, 9}, {29, 9}, {59, 9},
    {57, 9}, {21, 9}, {1, 9}, {27, 9}, {53, 9}, {69, 9}, {77, 9}, {23, 9},
    {79, 9}, {5, 10}, {9, 10}, {75, 10}, {63, 10}, {11, 10}, {3, 10}, {17, 10},
    {71, 10}, {60, 11}, {20, 11}, {24, 11}, {56, 11}, {80, 11}, {8, 11}, {72, 11},
    {6, 11}, {0, 11}, {74, 11}, {62, 11}, {26, 11}, {18, 11}, {2, 11}, {54, 11},
    {78, 11},
    /* Book 2 */
    {40, 3}, {67, 4}, {13, 5}, {41, 5}, {37, 5}, {39, 5}, {31, 5}, {43, 5},
    {49, 5}, {34, 6}, {22, 6}, {46, 6}, {42, 6}, {48, 6}, {38, 6}, {12, 6},
    {58, 6}, {64, 6}, {4, 6}, {36, 6}, {70, 6}, {68, 6}, {32, 6}, {16, 6},
    {50, 6}, {28, 6}, {14, 6}, {30, 6}, {10, 6}, {76, 6}, {52, 6}, {44, 6},
    {66, 6}, {47, 7}, {65, 7}, {19, 7}, {33, 7}, {61, 7}, {75, 7}, {71, 7},
    {25, 7}, {29, 7}, {79, 7}, {15, 7}, {1, 7}, {11, 7}, {55, 7}, {73, 7},
    {59, 8}, {21, 8}, {7, 8}, {17, 8}, {5, 8}, {3, 8}, {27, 8}, {69, 8},
    {63, 8}, {45, 8}, {53, 8}, {23, 8}, {9, 8}, {51, 8}, {57, 8}, {35, 8},
    {77, 8}, {60, 8}, {20, 8}, {56, 9}, {0, 9}, {24, 9}, {26, 9}, {80, 9},
    {6, 9}, {62, 9}, {18, 9}, {8, 9}, {72, 9}, {54, 9}, {2, 9}, {74, 9},
    {78, 9},
    /* Book 3 */
    {0, 1}, {27, 4}, {1, 4}, {9, 4}, {3, 4}, {36, 5}, {4, 5}, {12, 6},
    {10, 6}, {30, 6}, {13, 6}, {28, 6}, {39, 6}, {40, 7}, {31, 7}, {37, 7},
    {54, 8}, {2, 8}, {5, 8}, {63, 8}, {48, 8}, {7, 9}, {16, 9}, {45, 9},
    {14, 9}, {66, 9}, {6, 9}, {21, 9}, {15, 9}, {18, 9}, {11, 9}, {57, 9},
    {49, 9}, {22, 9}, {42, 9}, {43, 9}, {46, 10}, {33, 10}, {34, 10}, {19, 10},
    {67, 10}, {41, 10}, {64, 10}, {32, 10}, {8, 10}, {17, 10}, {75, 10}, {51, 10},
    {29, 10}, {55, 10}, {25, 10}, {72, 11}, {52, 11}, {38, 11}, {58, 11}, {44, 11},
    {76, 11}, {24, 11}, {23, 11}, {35, 12}, {73, 12}, {69, 12}, {78, 12}, {26, 12},
    {79, 12}, {70, 12}, {50, 12}, {53, 12}, {20, 13}, {60, 13}, {47, 13}, {61, 14},
    {68, 14}, {65, 14}, {80, 15}, {77, 15}, {71, 15}, {59, 15}, {56, 15}, {74, 16},
    {62, 16},
    /* Book 4 */
    {40, 4}, {13, 4}, {37, 4}, {39, 4}, {31, 4}, {27, 4}, {36, 4}, {0, 4},
    {4, 4}, {30, 4}, {28, 5}, {12, 5}, {1, 5}, {10, 5}, {3, 5}, {9, 5},
    {67, 7}, {43, 7}, {49, 7}, {41, 7}, {66, 7}, {64, 7}, {48, 7}, {58, 7},
    {16, 7}, {14, 8}, {42, 8}, {22, 8}, {32, 8}, {46, 8}, {38, 8}, {34, 8},
    {63, 8}, {57, 8}, {45, 8}, {55, 8}, {11, 8}, {21, 8}, {5, 8}, {15, 8},
    {19, 8}, {29, 8}, {7, 8}, {33, 8}, {54, 8}, {2, 8}, {18, 9}, {6, 9},
    {52, 9}, {76, 9}, {70, 9}, {44, 9}, {50, 9}, {68, 9}, {51, 10}, {75, 10},
    {69, 10}, {25, 10}, {17, 10}, {73, 10}, {23, 10}, {61, 10}, {35, 10}, {79, 10},
    {47, 10}, {59, 10}, {65, 10}, {53, 10}, {71, 11}, {77, 11}, {24, 11}, {72, 11},
    {8, 11}, {60, 11}, {20, 11}, {56, 11}, {80, 11}, {26, 11}, {78, 11}, {74, 12},
    {62, 12},
    /* Book 5 */
    {40, 1}, {31, 4}, {49, 4}, {41, 4}, {39, 4}, {48, 5}, {32, 5}, {30, 5},
    {50, 5}, {22, 7}, {42, 7}, {58, 7}, {38, 7}, {21, 8}, {59, 8}, {29, 8},
    {51, 8}, {23, 8}, {57, 8}, {33, 8}, {47, 8}, {13, 8}, {67, 8}, {37, 8},
    {43, 8}, {12, 9}, {52, 9}, {68, 9}, {28, 9}, {14, 9}, {66, 9}, {46, 9},
    {34, 9}, {24, 9}, {60, 9}, {20, 9}, {56, 9}, {11, 10}, {65, 10}, {25, 10},
    {55, 10}, {69, 10}, {61, 10}, {15, 10}, {19, 10}, {36, 10}, {4, 10}, {77, 10},
    {76, 10}, {3, 11}, {44, 11}, {75, 11}, {27, 11}, {53, 11}, {35, 11}, {5, 11},
    {45, 11}, {64, 11}, {10, 11}, {16, 11}, {26, 11}, {2, 11}, {78, 11}, {54, 11},
    {62, 11}, {70, 11}, {6, 11}, {18, 12}, {74, 12}, {63, 12}, {1, 12}, {7, 12},
    {71, 12}, {17, 12}, {79, 12}, {73, 12}, {9, 12}, {72, 13}, {8, 13}, {80, 13},
    {0, 13},
    /* Book 6 */
    {40, 4}, {49, 4}, {39, 4}, {41, 4}, {31, 4}, {50, 4}, {32, 4}, {48, 4},
    {30, 4}, {57, 6}, {59, 6}, {23, 6}, {21, 6}, {22, 6}, {33, 6}, {58, 6},
    {47, 6}, {51, 6}, {38, 6}, {29, 6}, {42, 6}, {56, 6}, {24, 6}, {20, 6},
    {60, 6}, {14, 7}, {68, 7}, {66, 7}, {34, 7}, {12, 7}, {52, 7}, {46, 7},
    {28, 7}, {67, 7}, {13, 7}, {37, 7}, {43, 7}, {69, 7}, {11, 8}, {25, 8},
    {61, 8}, {65, 8}, {55, 8}, {19, 8}, {15, 8}, {70, 8}, {64, 9}, {10, 9},
    {16, 9}, {45, 9}, {27, 9}, {77, 9}, {5, 9}, {3, 9}, {53, 9}, {75, 9},
    {35, 9}, {36, 9}, {6, 9}, {2, 9}, {62, 9}, {18, 9}, {4, 9}, {78, 9},
    {74, 9}, {26, 9}, {76, 9}, {54, 9}, {44, 9}, {9, 10}, {17, 10}, {63, 10},
    {73, 10}, {71, 10}, {79, 10}, {7, 10}, {1, 10}, {80, 11}, {8, 11}, {0, 11},
    {72, 11},
    /* Book 7 */
    {0, 1}, {8, 3}, {1, 3}, {9, 4}, {17, 6}, {10, 6}, {16, 6}, {2, 6},
    {25, 7}, {11, 7}, {18, 7}, {24, 7}, {3, 7}, {19, 8}, {26, 8}, {12, 8},
    {33, 8}, {13, 8}, {41, 8}, {27, 8}, {20, 8}, {4, 8}, {32, 8}, {34, 9},
    {21, 9}, {42, 9}, {5, 9}, {49, 9}, {40, 9}, {14, 9}, {35, 9}, {29, 9},
    {28, 9}, {43, 9}, {22, 9}, {50, 9}, {15, 9}, {30, 10}, {6, 10}, {48, 10},
    {36, 10}, {57, 10}, {37, 10}, {58, 10}, {44, 10}, {51, 10}, {23, 10}, {59, 10},
    {52, 10}, {45, 10}, {38, 10}, {31, 10}, {56, 11}, {7, 11}, {53, 11}, {46, 11},
    {60, 11}, {39, 11}, {47, 11}, {61, 11}, {62, 12}, {54, 12}, {55, 12}, {63, 12},
    /* Book 8 */
    {9, 3}, {17, 4}, {8, 4}, {10, 4}, {1, 4}, {18, 4}, {0, 5}, {16, 5},
    {2, 5}, {25, 5}, {11, 5}, {26, 5}, {19, 5}, {27, 6}, {33, 6}, {12, 6},
    {34, 6}, {20, 6}, {24, 6}, {3, 6}, {35, 6}, {28, 6}, {42, 6}, {41, 7},
    {21, 7}, {13, 7}, {43, 7}, {29, 7}, {36, 7}, {44, 7}, {4, 7}, {37, 7},
    {32, 7}, {22, 7}, {50, 7}, {49, 7}, {14, 7}, {30, 8}, {51, 8}, {45, 8},
    {40, 8}, {52, 8}, {5, 8}, {38, 8}, {57, 8}, {58, 8}, {23, 8}, {53, 8},
    {59, 8}, {15, 8}, {46, 8}, {31, 8}, {54, 9}, {60, 9}, {48, 9}, {39, 9},
    {6, 9}, {61, 9}, {62, 9}, {55, 9}, {47, 10}, {56, 10}, {7, 10}, {63, 10},
    /* Book 9 */
    {0, 1}, {13, 3}, {1, 3}, {14, 4}, {27, 6}, {15, 6}, {26, 6}, {2, 6},
    {40, 7}, {28, 7}, {16, 7}, {39, 8}, {3, 8}, {29, 8}, {41, 8}, {17, 8},
    {53, 8}, {30, 8}, {18, 8}, {54, 9}, {42, 9}, {4, 9}, {52, 9}, {66, 9},
    {31, 9}, {19, 9}, {43, 9}, {67, 9}, {79, 9}, {55, 9}, {5, 10}, {32, 10},
    {65, 10}, {20, 10}, {44, 10}, {21, 10}, {105, 10}, {56, 10}, {68, 10}, {80, 10},
    {92, 10}, {6, 10}, {106, 10}, {34, 10}, {45, 10}, {33, 10}, {57, 10}, {118, 10},
    {22, 10}, {93, 10}, {78, 11}, {69, 11}, {81, 11}, {107, 11}, {7, 11}, {119, 11},
    {47, 11}, {58, 11}, {46, 11}, {8, 11}, {131, 11}, {82, 11}, {35, 11}, {70, 11},
    {104, 11}, {91, 11}, {94, 11}, {132, 11}, {120, 11}, {108, 11}, {23, 11}, {95, 11},
    {83, 11}, {71, 11}, {60, 11}, {59, 11}, {48, 11}, {144, 11}, {73, 11}, {117, 11},
    {109, 11}, {133, 12}, {36, 12}, {9, 12}, {145, 12}, {121, 12}, {84, 12}, {157, 12},
    {61, 12}, {110, 12}, {24, 12}, {122, 12}, {134, 12}, {72, 12}, {96, 12}, {37, 12},
    {25, 12}, {158, 12}, {146, 12}, {49, 12}, {74, 12}, {85, 12}, {111, 12}, {147, 12},
    {10, 12}, {97, 12}, {159, 12}, {130, 12}, {135, 12}, {62, 12}, {86, 12}, {38, 12},
    {123, 12}, {124, 12}, {63, 12}, {143, 12}, {87, 12}, {50, 12}, {75, 12}, {112, 13},
    {99, 13}, {161, 13}, {51, 13}, {148, 13}, {98, 13}, {160, 13}, {149, 13}, {136, 13},
    {64, 13}, {100, 13}, {76, 13}, {11, 13}, {162, 13}, {88, 13}, {156, 13}, {137, 13},
    {77, 13}, {101, 13}, {125, 13}, {12, 13}, {150, 13}, {113, 13}, {126, 13}, {138, 13},
    {102, 13}, {163, 13}, {89, 13}, {115, 13}, {151, 13}, {103, 13}, {90, 13}, {114, 14},
    {139, 14}, {116, 14}, {127, 14}, {128, 14}, {129, 14}, {141, 14}, {165, 14}, {140, 14},
    {152, 14}, {164, 14}, {153, 14}, {166, 14}, {167, 14}, {142, 15}, {154, 15}, {155, 15},
    {168, 15},
    /* Book 10 */
    {14, 4}, {15, 4}, {27, 4}, {28, 5}, {13, 5}, {1, 5}, {16, 5}, {41, 5},
    {40, 5}, {29, 5}, {42, 5}, {26, 6}, {2, 6}, {30, 6}, {54, 6}, {17, 6},
    {53, 6}, {0, 6}, {55, 6}, {43, 6}, {39, 6}, {3, 6}, {56, 6}, {31, 6},
    {67, 6}, {18, 7}, {66, 7}, {68, 7}, {44, 7}, {69, 7}, {57, 7}, {80, 7},
    {32, 7}, {81, 7}, {52, 7}, {79, 7}, {4, 7}, {19, 7}, {45, 7}, {70, 7},
    {82, 7}, {58, 7}, {83, 8}, {93, 8}, {46, 8}, {33, 8}, {71, 8}, {106, 8},
    {94, 8}, {65, 8}, {92, 8}, {5, 8}, {105, 8}, {20, 8}, {107, 8}, {95, 8},
    {59, 8}, {34, 8}, {84, 8}, {96, 8}, {21, 8}, {47, 8}, {108, 8}, {60, 8},
    {72, 8}, {109, 8}, {73, 8}, {97, 9}, {85, 9}, {119, 9}, {78, 9}, {86, 9},
    {120, 9}, {48, 9}, {118, 9}, {35, 9}, {6, 9}, {110, 9}, {121, 9}, {61, 9},
    {132, 9}, {22, 9}, {98, 9}, {111, 9}, {122, 9}, {99, 9}, {133, 9}, {74, 9},
    {134, 9}, {36, 9}, {131, 9}, {49, 9}, {123, 9}, {87, 9}, {104, 9}, {62, 9},
    {91, 9}, {145, 9}, {100, 10}, {146, 10}, {136, 10}, {23, 10}, {144, 10}, {124, 10},
    {7, 10}, {112, 10}, {135, 10}, {50, 10}, {75, 10}, {113, 10}, {148, 10}, {8, 10},
    {147, 10}, {37, 10}, {101, 10}, {88, 10}, {137, 10}, {63, 10}, {24, 10}, {158, 10},
    {125, 10}, {159, 10}, {149, 10}, {76, 10}, {160, 10}, {150, 10}, {161, 10}, {51, 10},
    {89, 10}, {117, 10}, {138, 10}, {130, 10}, {157, 10}, {9, 10}, {64, 10}, {126, 10},
    {162, 10}, {38, 10}, {114, 10}, {127, 11}, {25, 11}, {151, 11}, {163, 11}, {102, 11},
    {77, 11}, {90, 11}, {139, 11}, {115, 11}, {164, 11}, {10, 11}, {103, 11}, {143, 11},
    {140, 11}, {152, 11}, {153, 11}, {11, 11}, {154, 11}, {128, 11}, {141, 11}, {156, 11},
    {116, 11}, {165, 12}, {142, 12}, {129, 12}, {155, 12}, {167, 12}, {12, 12}, {166, 12},
    {168, 12},
    /* Book 11 */
    {0, 4}, {18, 4}, {288, 5}, {17, 5}, {1, 5}, {35, 5}, {19, 5}, {36, 5},
    {20, 6}, {52, 6}, {53, 6}, {34, 6}, {37, 6}, {2, 6}, {54, 6}, {69, 7},
    {21, 7}, {70, 7}, {38, 7}, {71, 7}, {55, 7}, {51, 7}, {3, 7}, {86, 7},
    {87, 7}, {39, 7}, {72, 7}, {22, 7}, {88, 7}, {56, 7}, {89, 7}, {73, 8},
    {104, 8}, {40, 8}, {103, 8}, {105, 8}, {57, 8}, {23, 8}, {84, 8}, {67, 8},
    {277, 8}, {275, 8}, {276, 8}, {106, 8}, {278, 8}, {68, 8}, {74, 8}, {4, 8},
    {50, 8}, {90, 8}, {101, 8}, {279, 8}, {274, 8}, {280, 8}, {41, 8}, {121, 8},
    {58, 8}, {107, 8}, {91, 8}, {118, 8}, {282, 8}, {122, 8}, {120, 8}, {281, 8},
    {135, 8}, {33, 8}, {24, 8}, {75, 8}, {283, 8}, {123, 8}, {284, 8}, {152, 8},
    {273, 8}, {108, 8}, {169, 8}, {42, 8}, {92, 8}, {186, 8}, {285, 8}, {139, 8},
    {138, 8}, {59, 8}, {85, 8}, {286, 8}, {203, 8}, {124, 8}, {76, 8}, {109, 8},
    {125, 8}, {5, 8}, {140, 9}, {287, 9}, {220, 9}, {25, 9}, {137, 9}, {254, 9},
    {93, 9}, {237, 9}, {60, 9}, {141, 9}, {126, 9}, {43, 9}, {142, 9}, {155, 9},
    {156, 9}, {271, 9}, {77, 9}, {110, 9}, {102, 9}, {157, 9}, {94, 9}, {143, 9},
    {127, 9}, {26, 9}, {173, 9}, {6, 9}, {172, 9}, {154, 9}, {158, 9}, {78, 9},
    {44, 9}, {159, 9}, {61, 9}, {111, 9}, {174, 9}, {144, 9}, {175, 9}, {160, 9},
    {190, 9}, {27, 9}, {119, 9}, {176, 9}, {128, 9}, {62, 9}, {95, 9}, {171, 9},
    {79, 9}, {189, 9}, {223, 9}, {112, 9}, {224, 9}, {45, 9}, {272, 9}, {96, 9},
    {192, 9}, {191, 10}, {161, 10}, {129, 10}, {145, 10}, {16, 10}, {81, 10}, {7, 10},
    {64, 10}, {193, 10}, {222, 10}, {225, 10}, {207, 10}, {47, 10}, {226, 10}, {146, 10},
    {113, 10}, {178, 10}, {177, 10}, {240, 10}, {208, 10}, {28, 10}, {80, 10}, {188, 10},
    {63, 10}, {30, 10}, {206, 10}, {130, 10}, {65, 10}, {97, 10}, {98, 10}, {242, 10},
    {82, 10}, {194, 10}, {241, 10}, {209, 10}, {227, 10}, {210, 10}, {136, 10}, {195, 10},
    {46, 10}, {162, 10}, {243, 10}, {115, 10}, {180, 10}, {257, 10}, {147, 10}, {163, 10},
    {244, 10}, {179, 10}, {99, 10}, {196, 10}, {239, 10}, {48, 10}, {114, 10}, {29, 10},
    {229, 10}, {8, 10}, {228, 10}, {131, 10}, {211, 10}, {132, 10}, {258, 10}, {205, 10},
    {116, 10}, {49, 10}, {260, 10}, {259, 10}, {31, 10}, {164, 10}, {83, 10}, {245, 10},
    {149, 10}, {230, 10}, {148, 10}, {100, 10}, {66, 10}, {181, 10}, {197, 10}, {212, 10},
    {261, 10}, {262, 10}, {150, 10}, {256, 10}, {133, 10}, {153, 10}, {9, 10}, {166, 10},
    {165, 10}, {213, 10}, {246, 10}, {183, 10}, {247, 10}, {214, 10}, {117, 10}, {134, 10},
    {167, 11}, {263, 11}, {198, 11}, {201, 11}, {32, 11}, {182, 11}, {184, 11}, {232, 11},
    {231, 11}, {200, 11}, {199, 11}, {151, 11}, {249, 11}, {233, 11}, {217, 11}, {264, 11},
    {248, 11}, {170, 11}, {215, 11}, {168, 11}, {10, 11}, {216, 11}, {187, 11}, {218, 11},
    {185, 11}, {234, 11}, {13, 11}, {250, 11}, {265, 11}, {266, 11}, {202, 11}, {251, 11},
    {221, 11}, {11, 11}, {235, 11}, {267, 11}, {268, 11}, {219, 11}, {238, 11}, {252, 11},
    {236, 11}, {204, 11}, {253, 11}, {14, 12}, {12, 12}, {269, 12}, {255, 12}, {15, 12},
    {270, 12},
    /* Book 12 */
    {60, 1}, {59, 3}, {61, 4}, {58, 4}, {62, 4}, {57, 5}, {63, 5}, {56, 6},
    {64, 6}, {55, 6}, {65, 6}, {66, 7}, {54, 7}, {67, 7}, {53, 8}, {68, 8},
    {52, 8}, {69, 8}, {51, 8}, {70, 9}, {50, 9}, {49, 9}, {71, 9}, {72, 10},
    {48, 10}, {73, 10}, {47, 10}, {74, 10}, {46, 10}, {76, 11}, {75, 11}, {77, 11},
    {78, 11}, {45, 11}, {43, 11}, {44, 12}, {79, 12}, {42, 12}, {41, 12}, {80, 12},
    {40, 12}, {81, 13}, {39, 13}, {82, 13}, {38, 13}, {83, 13}, {37, 14}, {35, 14},
    {85, 14}, {33, 14}, {36, 14}, {34, 14}, {84, 14}, {32, 14}, {87, 15}, {89, 15},
    {30, 15}, {31, 15}, {86, 16}, {29, 16}, {26, 16}, {27, 16}, {28, 16}, {24, 16},
    {88, 16}, {25, 17}, {22, 17}, {23, 17}, {90, 18}, {21, 18}, {19, 18}, {3, 18},
    {1, 18}, {2, 18}, {0, 18}, {98, 19}, {99, 19}, {100, 19}, {101, 19}, {102, 19},
    {117, 19}, {97, 19}, {91, 19}, {92, 19}, {93, 19}, {94, 19}, {95, 19}, {96, 19},
    {104, 19}, {111, 19}, {112, 19}, {113, 19}, {114, 19}, {115, 19}, {116, 19}, {110, 19},
    {105, 19}, {106, 19}, {107, 19}, {108, 19}, {109, 19}, {118, 19}, {6, 19}, {8, 19},
    {9, 19}, {10, 19}, {5, 19}, {103, 19}, {120, 19}, {119, 19}, {4, 19}, {7, 19},
    {15, 19}, {16, 19}, {18, 19}, {20, 19}, {17, 19}, {11, 19}, {12, 19}, {14, 19},
    {13, 19},
};
static const uint16_t huffbook_sizes[13] = { 0, 81, 81, 81, 81, 81, 81, 64, 64, 169, 169, 289, 121 };
/* pair books: index = x * base + y */
static const uint8_t book_base[12] = { 0, 0, 0, 0, 0, 9, 9, 8, 8, 13, 13, 17 };

/* Decoding is a lookup on the next 8 bits, which resolves the short codes
 * that carry most of the symbols directly; a longer code's 8-bit prefix
 * leads to a second table covering the rest of its subtree, so every code
 * takes at most two lookups. A lookup yields the decoded tuple, not a
 * symbol index: four 2-bit magnitudes for a quad book, two 6-bit values
 * for a pair book (the escape magnitude 16 included), the index itself
 * for the scalefactor book. Rows 0..10 are the spectral books 1..11,
 * row 11 the scalefactor book. */
#define HUFF_LUT_BITS 8
#define HUFF_MAX_LEN  19
#define HUFF_SUBTREES 200  /* 8-bit prefixes shared by longer codes, all books */
#define HUFF_SUB_ENTRIES 3206 /* their second-level entries */
typedef uint16_t HuffEntry; /* len (bits beyond the prefix at level two) | tuple << 4; level one: 0 | subtree << 4 */
static HuffEntry huff_lut[12][1 << HUFF_LUT_BITS];
static HuffEntry huff_sub[HUFF_SUB_ENTRIES];
static struct { uint16_t start; uint8_t depth; } huff_subtree[HUFF_SUBTREES];

static uint32_t huff_tuple(int book, int sym)
{
    if (book == 12) return (uint32_t)sym;
    if (book <= 4) { /* v w x y, base 3 */
        return (uint32_t)((sym / 27) | ((sym / 9 % 3) << 2) | ((sym / 3 % 3) << 4) | ((sym % 3) << 6));
    }
    int base = book_base[book];
    return (uint32_t)((sym / base) | ((sym % base) << 6));
}

void init_huffman_luts(void)
{
    int n_sub = 0, n_entries = 0;
    const HuffDesc *tab = huff_desc;
    for (int book = 1; book <= 12; book++) {
        int n = huffbook_sizes[book];
        uint32_t code = 0;
        int prev_len = 0;
        HuffEntry *lut = huff_lut[book - 1];
        memset(lut, 0, sizeof(huff_lut[0]));

        /* short codes fill their share of the first level; each longer
         * code's prefix gets a subtree as deep as its longest code */
        for (int i = 0; i < n; i++) {
            int len = (int)tab[i].len;
            code = (code + (i != 0)) << (len - prev_len);
            prev_len = len;
            if (len == 0) continue;
            if (len <= HUFF_LUT_BITS) {
                uint32_t start = code << (HUFF_LUT_BITS - len);
                for (uint32_t k = 0; k < (1U << (HUFF_LUT_BITS - len)); k++)
                    lut[start + k] = (HuffEntry)(len | (huff_tuple(book, tab[i].sym) << 4));
            } else {
                uint32_t prefix = code >> (len - HUFF_LUT_BITS);
                if (lut[prefix] == 0) {
                    lut[prefix] = (HuffEntry)(n_sub << 4);
                    huff_subtree[n_sub].depth = 0;
                    n_sub++;
                }
                int t = lut[prefix] >> 4;
                if (len - HUFF_LUT_BITS > huff_subtree[t].depth) huff_subtree[t].depth = (uint8_t)(len - HUFF_LUT_BITS);
            }
        }
        code = 0;
        prev_len = 0;
        for (int i = 0; i < n; i++) {
            int len = (int)tab[i].len;
            code = (code + (i != 0)) << (len - prev_len);
            prev_len = len;
            if (len <= HUFF_LUT_BITS) continue;
            uint32_t prefix = code >> (len - HUFF_LUT_BITS);
            int t = lut[prefix] >> 4;
            if (!(huff_subtree[t].depth & 0x80)) {
                /* first code of this subtree: allocate it */
                huff_subtree[t].start = (uint16_t)n_entries;
                n_entries += 1 << huff_subtree[t].depth;
                memset(huff_sub + huff_subtree[t].start, 0, sizeof(HuffEntry) << huff_subtree[t].depth);
                huff_subtree[t].depth |= 0x80;
            }
            int depth = huff_subtree[t].depth & 0x7F;
            int rest = len - HUFF_LUT_BITS;
            uint32_t tail = code & ((1U << rest) - 1);
            uint32_t start = huff_subtree[t].start + (tail << (depth - rest));
            for (uint32_t k = 0; k < (1U << (depth - rest)); k++)
                huff_sub[start + k] = (HuffEntry)(rest | (huff_tuple(book, tab[i].sym) << 4));
        }
        tab += n;
    }
    for (int t = 0; t < n_sub; t++) huff_subtree[t].depth &= 0x7F;
}

/* One codeword of book (1..12): the decoded tuple. */
static inline uint32_t huff_decode(BitReader *bs, int book)
{
    /* Reuse the same lookahead when a long code needs the second table. */
    uint32_t lookahead = bits_show_fast(bs, HUFF_MAX_LEN);
    HuffEntry e = huff_lut[book - 1][lookahead >> (HUFF_MAX_LEN - HUFF_LUT_BITS)];
    if (e & 15) {
        bits_skip(bs, e & 15);
        return e >> 4;
    }
    int t = e >> 4;
    if (t < 0 || t >= HUFF_SUBTREES) return 0;
    int depth = huff_subtree[t].depth;
    uint32_t rest = (lookahead >> (HUFF_MAX_LEN - HUFF_LUT_BITS - depth)) & ((1U << depth) - 1);
    e = huff_sub[huff_subtree[t].start + rest];
    uint32_t skip = e & 15;
    if (skip == 0 && depth > 0) return 0;
    bits_skip(bs, HUFF_LUT_BITS + skip);
    return e >> 4;
}

#define DECODE_HUFF_SF(bs) ((int)huff_decode((bs), 12))

static inline void decode_quad(BitReader *bs, int book, int *v, int *w, int *x, int *y)
{
    /* Quad books (1..4) are bounded to {-1,0,1}; there is no escape path to
     * instrument here, unlike decode_pair's book 11. */
    uint32_t t = huff_decode(bs, book);
    int v_val = (int)(t & 3), w_val = (int)((t >> 2) & 3), x_val = (int)((t >> 4) & 3), y_val = (int)((t >> 6) & 3);
    if (book <= 2) {
        /* signed 4-tuple: values in {-1, 0, 1} */
        *v = v_val - 1; *w = w_val - 1; *x = x_val - 1; *y = y_val - 1;
    } else {
        /* unsigned 4-tuple: a sign bit follows for each non-zero value */
        uint32_t count = (v_val != 0) + (w_val != 0) + (x_val != 0) + (y_val != 0);
        uint32_t signs = count ? bits_get_fast(bs, count) : 0;
        uint32_t remaining = count;
        int sign;
        /* Zero stays zero under either sign, avoiding per-value branches. */
        remaining -= v_val != 0;
        sign = (int)((signs >> remaining) & 1U);
        v_val = (v_val ^ -sign) + sign;
        remaining -= w_val != 0;
        sign = (int)((signs >> remaining) & 1U);
        w_val = (w_val ^ -sign) + sign;
        remaining -= x_val != 0;
        sign = (int)((signs >> remaining) & 1U);
        x_val = (x_val ^ -sign) + sign;
        remaining -= y_val != 0;
        sign = (int)((signs >> remaining) & 1U);
        y_val = (y_val ^ -sign) + sign;
        *v = v_val; *w = w_val; *x = x_val; *y = y_val;
    }
}

/* The escape prefix has at most 8 ones, so a magnitude never exceeds 8191; a
 * longer run is corrupt and is clamped instead of shifting out of range. */
static inline int decode_escape(BitReader *bs)
{
    int prefix = 0;
    while (prefix < 8 && bits_get_1(bs)) prefix++;
    if (prefix == 8) bits_get_1(bs);
    return (1 << (prefix + 4)) + (int)bits_get_fast(bs, (uint32_t)prefix + 4);
}

static inline void decode_pair(BitReader *bs, int book, int *x, int *y)
{
    uint32_t t = huff_decode(bs, book);
    *x = (int)(t & 63);
    *y = (int)(t >> 6);

    if (book == 5 || book == 6) {
        /* signed 2-tuples in [-4, 4], no sign bits */
        *x -= 4;
        *y -= 4;
    } else if (book == 11) {
        /* the sign bits of both values come right after the codeword, the
         * escape sequences (for a magnitude of 16) after those */
        int abs_x = *x, abs_y = *y;
        bool neg_x = abs_x && bits_get_1(bs);
        bool neg_y = abs_y && bits_get_1(bs);
        if (abs_x == 16) {
            abs_x = decode_escape(bs);
#ifdef FAAD_STATS
            g_faadStats.escbookMagnitudeEscapes++;
#endif
        }
        if (abs_y == 16) {
            abs_y = decode_escape(bs);
#ifdef FAAD_STATS
            g_faadStats.escbookMagnitudeEscapes++;
#endif
        }
        *x = neg_x ? -abs_x : abs_x;
        *y = neg_y ? -abs_y : abs_y;
    } else {
        /* unsigned 2-tuple: a sign bit follows for each non-zero value */
        if (*x) if (bits_get_1(bs)) *x = -*x;
        if (*y) if (bits_get_1(bs)) *y = -*y;
    }
}

faad_status decode_scale_factor_data(BitReader *bs, ICSInfo *ics, uint32_t sample_rate)
{
    setup_sfb_offsets(ics, sample_rate);

    int sf = ics->global_gain;
    int is_pos = 0;
    int pns_energy = ics->global_gain - 90; /* §4.6.13.3: noise_nrg starts from global_gain, not the running sf */
    bool is_first_pns = true;

    int max_sfb = ics->max_sfb < ics->num_sfbs ? ics->max_sfb : ics->num_sfbs;
    if (max_sfb > MAX_SFB) max_sfb = MAX_SFB;
    for (int g = 0; g < ics->num_window_groups && g < 8; g++) {
        for (int sfb = 0; sfb < max_sfb; sfb++) {
            int cb = ics->sfb_cb[g][sfb];
#ifdef FAAD_STATS
            g_faadStats.totalBands++;
#endif
            if (cb == 0) {
                continue;
            } else if (cb == 13) { /* PNS */
#ifdef FAAD_STATS
                g_faadStats.pnsBands++;
#endif
                if (is_first_pns) {
                    pns_energy += (int)bits_get(bs, 9) - 256;
                    is_first_pns = false;
                } else {
                    int dpns = DECODE_HUFF_SF(bs);
                    pns_energy += dpns - 60;
                }
                ics->scalefactors[g][sfb] = (int16_t)pns_energy;
            } else if (cb == 14 || cb == 15) { /* Intensity stereo */
#ifdef FAAD_STATS
                g_faadStats.isBands++;
#endif
                int dis = DECODE_HUFF_SF(bs);
                is_pos += dis - 60; /* signed: negative positions boost the right channel */
                ics->scalefactors[g][sfb] = (int16_t)is_pos;
            } else {
                int dsf = DECODE_HUFF_SF(bs);
                sf += dsf - 60;
                if (sf < 0) sf = 0;
                if (sf > 255) sf = 255;
                ics->scalefactors[g][sfb] = (int16_t)sf;
            }
        }
    }
    return FAAD_OK;
}

static inline float pow_4_3_fast(int x)
{
    int abs_x = abs(x);
    if (abs_x < 128) {
        return copysignf(pow_4_3_lut[abs_x], (float)x);
    }
    return copysignf(powf((float)abs_x, 4.0f / 3.0f), (float)x);
}

/* §4.6.4.3: a pulse adds to the quantised value, away from zero, before
 * inverse quantisation; the line is re-quantised from its dequantised
 * value, which is exact for the integer magnitudes the books carry. */
static void apply_pulses(const ICSInfo *ics, float *spec, int max_sfb)
{
    for (int i = 0; i < ics->pulse_count; i++) {
        int k = ics->pulse_pos[i];
        int sfb = 0;
        while (sfb < max_sfb && ics->sfb_offsets[sfb + 1] <= k) sfb++;
        if (sfb >= max_sfb) continue;
        int cb = ics->sfb_cb[0][sfb];
        if (cb == 0 || cb >= 13) continue;
        float scale = sf_scale_lut[ics->scalefactors[0][sfb]]; /* clamped to 0..255 when read */
        float q = floorf(powf(fabsf(spec[k]) / scale, 0.75f) + 0.5f);
        if (spec[k] < 0.0f) q = -q;
        q += (q > 0.0f) ? ics->pulse_amp[i] : -(float)ics->pulse_amp[i];
        spec[k] = copysignf(powf(fabsf(q), 4.0f / 3.0f), q) * scale;
    }
}

faad_status decode_spectral_data(BitReader *bs, ICSInfo *ics, float *spec)
{
    int window_offset = 0;
    const uint16_t * restrict sfb_offsets = ics->sfb_offsets;
    int max_sfb = ics->max_sfb < ics->num_sfbs ? ics->max_sfb : ics->num_sfbs;
    if (max_sfb > MAX_SFB) max_sfb = MAX_SFB;

    for (int g = 0; g < ics->num_window_groups && g < 8; g++) {
        int win_group_len = ics->window_group_length[g];
        for (int sfb = 0; sfb < max_sfb; sfb++) {
            int cb = ics->sfb_cb[g][sfb];
            if (cb == 0 || cb >= 13) continue;
            {
                int sf = ics->scalefactors[g][sfb];
                float scale = (sf >= 0 && sf < 256) ? sf_scale_lut[sf] : powf(2.0f, 0.25f * (sf - 100));

                int start_k = sfb_offsets[sfb];
                int end_k = sfb_offsets[sfb + 1];
                if (start_k >= FRAME_LEN_LONG) continue;
                if (end_k > FRAME_LEN_LONG) end_k = FRAME_LEN_LONG;

                for (int w = 0; w < win_group_len; w++) {
                    float * restrict ptr = spec + (window_offset + w) * 128 + start_k;
                    int k = start_k;
                    if (cb <= 4) {
                        while (k < end_k) {
                            int v, w_val, x, y;
                            decode_quad(bs, cb, &v, &w_val, &x, &y);
                            ptr[0] = pow_4_3_fast(v) * scale;
                            ptr[1] = pow_4_3_fast(w_val) * scale;
                            ptr[2] = pow_4_3_fast(x) * scale;
                            ptr[3] = pow_4_3_fast(y) * scale;
                            ptr += 4;
                            k += 4;
                        }
                    } else {
                        while (k < end_k) {
                            int x, y;
                            decode_pair(bs, cb, &x, &y);
                            ptr[0] = pow_4_3_fast(x) * scale;
                            ptr[1] = pow_4_3_fast(y) * scale;
                            ptr += 2;
                            k += 2;
                        }
                    }
                }
            }
        }
        window_offset += win_group_len;
    }

    if (ics->pulse_count) apply_pulses(ics, spec, max_sfb);
    return FAAD_OK;
}
