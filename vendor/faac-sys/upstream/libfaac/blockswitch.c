/*
 * FAAC - Freeware Advanced Audio Coder
 * Copyright (C) 2002 Krzysztof Nikiel
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
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "blockswitch.h"
#include "coder.h"
#include "util.h"
#include "faac_internal.h"

typedef float psyfloat;

/* The high-pass energy timeline is held as one contiguous array of per-sub-block
   energies rather than separate prev/curr/next arrays, so the +-2 sub-block
   lookahead around the current frame is a single sliding index instead of
   three-way stitching.
   It holds three 2-frame energy windows back to back: PREV, CUR and the one
   lookahead window NEXT. (Energy windows are 2 frames wide, which is why a single
   "next" window consumes the two-frames-ahead sample slot in the input FIFO.) */
#define SUBBLOCKS_PER_FRAME 8
#define ENG_WIN_PREV (0 * SUBBLOCKS_PER_FRAME)
#define ENG_WIN_CUR  (1 * SUBBLOCKS_PER_FRAME)
#define ENG_WIN_NEXT (2 * SUBBLOCKS_PER_FRAME)

typedef struct
{
  psyfloat eng[3 * SUBBLOCKS_PER_FRAME];
  /* Bit i set: the energy step into sub-block i is a transient. Judged once,
     when the sub-block's energy is produced, so the per-frame decision is a
     mask test rather than a re-walk of the timeline. */
  unsigned attack;
  psyfloat level; /* running level of the sub-block energies so far */
}
psydata_t;

/* The high-pass first difference (d[n]=x[n]-x[n-1]) de-weights bass, whose
 * broadband energy would otherwise mask HF attacks and false-trigger short
 * blocks on stationary music; what's left tracks the band where pre-echo is
 * audible. A sub-block whose energy leaves [level/dropRatio, level*ratio] of
 * the running level before it is a transient. On LC the level spans roughly
 * the last three sub-blocks, so dense stationary texture stops tripping short
 * windows while onsets still do. A drop-out counts on LC only when the level
 * falls by more than PSY_DROP_RATIO_LC: the ordinary decay of a note is
 * better served by a long block, while a hard cut to near silence still needs
 * a short one so the coding noise does not smear into the silence. The
 * bit-starved HE core gains from more short windows than its attacks alone
 * call for, so it judges against the neighbouring sub-block alone, with a
 * tighter band either way. */
#define PSY_LEVEL_RATIO_LC  (2.5f)
#define PSY_DROP_RATIO_LC   (12.0f)
#define PSY_LEVEL_SMOOTH_LC (0.3f)
#define PSY_LEVEL_RATIO_HE  (1.5f)

/* A stationary bass note whose period is longer than the 256-sample analysis
 * window makes the first-difference energy swing with phase by more than the
 * tight HE band, which then trips nearly every sub-block and turns a steady
 * passage into short windows (their sidelobes then leak the bass over the whole
 * spectrum, and they cost bits). That happens when the energy sits below one
 * cycle per window, where the first difference is a fraction of the total of
 * about (2 sin(pi/256))^2 = -32 dB at any sample rate. In such a sub-block the
 * LC band decides instead; a real attack is broadband at its start, so it is not
 * bass dominated and keeps the tight HE test. */
#define PSY_BASS_DOM_HE     (6.3e-4f)

/* Attack anywhere in the frame or its immediate temporal context, sub-blocks
   [cur-2, cur+9], wants a short block. */
static void PsyCheckShort(PsyInfo * psyInfo)
{
  enum {PREVS = 2, NEXTS = 2};
  const psydata_t *psydata = (const psydata_t *)psyInfo->data;
  unsigned span = (1u << (PREVS + SUBBLOCKS_PER_FRAME + NEXTS - 1)) - 1;

  psyInfo->block_type = (psydata->attack >> (ENG_WIN_CUR - PREVS + 1)) & span
                        ? ONLY_SHORT_WINDOW : ONLY_LONG_WINDOW;
}

int PsyInit(GlobalPsyInfo * gpsyInfo, PsyInfo * psyInfo, unsigned int numChannels,
		    unsigned int sampleRate, bool heCore)
{
  unsigned int channel;
  int size;

  gpsyInfo->sampleRate = (float) sampleRate;
  gpsyInfo->levelRatio = heCore ? PSY_LEVEL_RATIO_HE : PSY_LEVEL_RATIO_LC;
  gpsyInfo->dropRatio = heCore ? PSY_LEVEL_RATIO_HE : PSY_DROP_RATIO_LC;
  gpsyInfo->levelSmooth = heCore ? 1.0f : PSY_LEVEL_SMOOTH_LC;
  gpsyInfo->bassDom = heCore ? PSY_BASS_DOM_HE : 0.0f;

  for (channel = 0; channel < numChannels; channel++)
  {
    psydata_t *psydata = (psydata_t *)AllocMemory(sizeof(psydata_t));
    if (!psydata) return 0;
    memset(psydata, 0, sizeof(psydata_t));
    psyInfo[channel].data = psydata;
  }

  size = BLOCK_LEN_LONG;
  for (channel = 0; channel < numChannels; channel++)
  {
    psyInfo[channel].size = size;
  }

  size = BLOCK_LEN_SHORT;
  for (channel = 0; channel < numChannels; channel++)
    psyInfo[channel].sizeS = size;
  return 1;
}

/* Strongest relative energy jump across the sub-blocks of the window the MDCT
   is about to transform. ENG_WIN_PREV is exactly that window -- (FIFO_PAST,
   FIFO_CURR) -- because PsyBufferUpdate has already shifted by the time TNS
   runs.

   Exposed so TNS can gate on the temporal envelope already sitting in
   psydata instead of recomputing it. Returns 0 if PsyBufferUpdate hasn't
   populated the energy windows for this channel yet -- callers must treat
   that as "no basis to judge", not "flat". */
float PsyGetAttack(PsyInfo * psyInfo)
{
  psydata_t *psydata = (psydata_t *)psyInfo->data;
  float strength = 0.0f, total = 0.0f;
  int win;

  if (!psydata)
    return 0.0f;

  for (win = 0; win < SUBBLOCKS_PER_FRAME; win++)
  {
    float e = (float)psydata->eng[ENG_WIN_PREV + win];

    total += e;
    if (win)
    {
      float p = (float)psydata->eng[ENG_WIN_PREV + win - 1];
      float lo = (e < p) ? e : p;
      float s = fabsf(e - p) / lo;      /* IEEE divide covers silence */

      if (s > strength) strength = s;
    }
  }

  return total > 0.0f ? strength : 0.0f;
}

void PsyEnd(PsyInfo * psyInfo, unsigned int numChannels)
{
  unsigned int channel;

  for (channel = 0; channel < numChannels; channel++)
  {
    if (psyInfo[channel].data)
      FreeMemory(psyInfo[channel].data);
    psyInfo[channel].data = NULL;
  }
}

/* Do psychoacoustical analysis */
void PsyCalculate(PsyInfo * psyInfo, const bool * isLfeChannel,
			 unsigned int numChannels)
{
  for (unsigned int channel = 0; channel < numChannels; channel++)
  {
      if (isLfeChannel[channel])
          psyInfo[channel].block_type = ONLY_LONG_WINDOW;
      else
          PsyCheckShort(&psyInfo[channel]);
  }
}

void PsyBufferUpdate(GlobalPsyInfo * gpsyInfo, PsyInfo * psyInfo,
                            float * restrict p_lookahead1,
                            float * restrict p_lookahead2)
{
  int win;
  float * restrict transBuff = gpsyInfo->sharedWorkBuffLong;
  psydata_t *psydata = (psydata_t *)psyInfo->data;
  float level = psydata->level;

  /* Shift the energy windows down by one frame: PREV<-CUR, CUR<-NEXT, freeing
     the NEXT region for the freshly-computed lookahead window below. */
  memmove(psydata->eng, psydata->eng + SUBBLOCKS_PER_FRAME,
          2 * SUBBLOCKS_PER_FRAME * sizeof(psyfloat));
  psydata->attack >>= SUBBLOCKS_PER_FRAME;

  /* Assembly of the newest 2048-sample window for energy analysis */
  memcpy(transBuff, p_lookahead1, BLOCK_LEN_LONG * sizeof(float));
  memcpy(transBuff + BLOCK_LEN_LONG, p_lookahead2, BLOCK_LEN_LONG * sizeof(float));

  /* Sub-block windows are 256 samples at a 128 hop, so each 128-sample half
   * serves two windows: sum every half once. seg[-1] is in bounds (the first
   * block starts >= 448 samples in), so the first difference carries across the
   * block boundary instead of resetting. */
  {
    const float *seg = transBuff + (BLOCK_LEN_LONG - BLOCK_LEN_SHORT) / 2;
    float pe = 0.0f, pt = 0.0f;
    int l;

    for (win = 0; win <= SUBBLOCKS_PER_FRAME; win++, seg += BLOCK_LEN_SHORT)
    {
      float de = 0.0f, dt = 0.0f;

      for (l = 0; l < BLOCK_LEN_SHORT; l++)
      {
        float d = seg[l] - seg[l - 1];
        de += d * d;
      }
      /* Only the HE core judges bass dominance. */
      if (gpsyInfo->bassDom > 0.0f)
        for (l = 0; l < BLOCK_LEN_SHORT; l++)
          dt += seg[l] * seg[l];

      if (win)
      {
        float e = pe + de;
        int trip = e > gpsyInfo->levelRatio * level || e * gpsyInfo->dropRatio < level;

        psydata->eng[ENG_WIN_NEXT + win - 1] = (psyfloat)e;
        if (gpsyInfo->bassDom > 0.0f)
        {
          /* The LC band is wider than the HE one, so a sub-block that does not
           * trip the HE test cannot trip it. */
          if (e < gpsyInfo->bassDom * (pt + dt))
            trip = trip && (e > PSY_LEVEL_RATIO_LC * level || e * PSY_DROP_RATIO_LC < level);
          else if (gpsyInfo->needBass)
            /* Short-only frames are the rule, not the question: only a context
             * that is bass dominated throughout and free of attacks (a bass
             * attack still wants short windows), where a long window costs
             * fewer bits than the short ones it would replace, is let off. */
            trip = 1;
        }
        if (trip)
          psydata->attack |= 1u << (ENG_WIN_NEXT + win - 1);
        level = gpsyInfo->levelSmooth * e + (1.0f - gpsyInfo->levelSmooth) * level;
      }
      pe = de;
      pt = dt;
    }
  }
  psydata->level = level;
}

void BlockSwitch(CoderInfo * coderInfo, PsyInfo * psyInfo, unsigned int numChannels)
{
  unsigned int channel;
  int desire = ONLY_LONG_WINDOW;

  /* Use the same block type for all channels
     If there is 1 channel that wants a short block,
     use a short block on all channels.
   */
  for (channel = 0; channel < numChannels; channel++)
  {
    if (psyInfo[channel].block_type == ONLY_SHORT_WINDOW)
      desire = ONLY_SHORT_WINDOW;
  }

  for (channel = 0; channel < numChannels; channel++)
  {
    int lasttype = coderInfo[channel].block_type;

    if (desire == ONLY_SHORT_WINDOW
	|| coderInfo[channel].desired_block_type == ONLY_SHORT_WINDOW)
    {
      if (lasttype == ONLY_LONG_WINDOW || lasttype == SHORT_LONG_WINDOW)
	coderInfo[channel].block_type = LONG_SHORT_WINDOW;
      else
	coderInfo[channel].block_type = ONLY_SHORT_WINDOW;
    }
    else
    {
      if (lasttype == ONLY_SHORT_WINDOW || lasttype == LONG_SHORT_WINDOW)
	coderInfo[channel].block_type = SHORT_LONG_WINDOW;
      else
	coderInfo[channel].block_type = ONLY_LONG_WINDOW;
    }
    coderInfo[channel].desired_block_type = desire;
  }
}
