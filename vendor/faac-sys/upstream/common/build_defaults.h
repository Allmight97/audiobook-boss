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

#ifndef FAAC_BUILD_DEFAULTS_H
#define FAAC_BUILD_DEFAULTS_H

/* Meson force-includes config.h with these; a build without it gets meson_options.txt's defaults. */
#ifndef MAX_CHANNELS
#define MAX_CHANNELS 8
#endif
#ifndef FAAC_SBR_DECIMATION
#define FAAC_SBR_DECIMATION 1
#endif

#endif
