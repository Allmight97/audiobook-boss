/*
 * FAAD - Freeware Advanced Audio Decoder
 * Copyright (C) 2026 Nils Schimmelmann
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 */

#ifndef FAAD_STATS_H
#define FAAD_STATS_H

#ifdef FAAD_STATS
#include <stdbool.h>
#include <stdio.h>

typedef struct faadDecStats {
    unsigned int totalFrames;
    unsigned int elementCounts[8]; /* SCE,CPE,CCE,LFE,DSE,PCE,FIL,END */
    unsigned int nonEndTermination;
    bool haveLastChannels;
    unsigned int lastChannels;
    unsigned int channelCountChanges;
    unsigned int minChannels, maxChannels;
    unsigned int icsCount, tnsActiveFrames, shortBlockIcsCount;
    unsigned int sbrActiveFrames, sbrHeaderCount, sbrEnvelopeSum;
    unsigned int psActiveFrames;
    unsigned long totalBands, msBands, isBands, pnsBands;
    unsigned int escbookMagnitudeEscapes;
    unsigned int fillElementCount, fillElementPadBitsSum, fillElementMaxPad;
    unsigned int errorConcealmentFrames;

    /* This frame's bits by syntax part, reset per frame and written as the
     * 'B' dump record; the parts are disjoint, the remainder is element and
     * ics headers. */
    unsigned int frameSectBits, frameSfBits, frameSpecBits, frameAuxBits;
    unsigned int frameSbrBits, framePsBits, frameFillBits;

    /* Decision dump, opened lazily from FAAD_DUMP. */
    FILE *dumpFile;
    bool dumpOpenTried;
} faadDecStats;

/* Diagnostics are global and intended for one stream at a
 * time. Successful decoder initialization resets the counters. */
extern faadDecStats g_faadStats;
#endif /* FAAD_STATS */

#endif /* FAAD_STATS_H */
