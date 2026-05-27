/**
 * TypeScript interfaces for audiobook metadata
 *
 * Field mapping for Plex/Audiobookshelf compatibility:
 * - artist = Author (©ART, also written to aART/AlbumArtist)
 * - composer = Narrator (©wrt/Composer)
 * - series = Series name (series/series-part tags plus mirrored iTunes freeform atoms)
 * - series_part = Series sequence / book # within a series (series-part/freeform SERIES-PART)
 * - subseries = Secondary series name (2nd entry in SERIES list)
 * - subseries_part = Series sequence / book # within a sub-series (2nd entry in SERIES-PART list)
 * - album_sort = TSOA library sort value; preserved unless explicit set/clear/recompute intent is sent
 * - date = Publication date (YYYY or YYYY-MM in ©day)
 *
 * `track`, `disk`, and `comment` remain readable for compatibility, but ABB does
 * not expose them as supported UI draft write fields. `album_sort` is writable
 * only through explicit backend intent.
 */

import type {
	AudiobookMetadata as GeneratedAudiobookMetadata,
	MetadataSaveBatchResult as GeneratedMetadataSaveBatchResult,
	MetadataSaveRequest as GeneratedMetadataSaveRequest,
	MetadataSaveResultEntry as GeneratedMetadataSaveResultEntry,
	MetadataSaveResultStatus as GeneratedMetadataSaveResultStatus,
	MetadataSource as GeneratedMetadataSource,
	OnlineMetadataResult as GeneratedOnlineMetadataResult,
} from '../lib/generated/tauri';
import type { AppErrorEnvelope } from '../lib/tauri/appError';
import type { NullToOptionalDeep } from './ipc';

/**
 * Represents metadata for an audiobook file
 * Matches Rust backend AudiobookMetadata structure
 */
export type AudiobookMetadata = NullToOptionalDeep<GeneratedAudiobookMetadata>;

/** Per-file metadata map keyed by input path */
export type AudiobookMetadataMap = Record<string, AudiobookMetadata>;

export type MetadataSource = GeneratedMetadataSource;

export type OnlineMetadataResult = NullToOptionalDeep<GeneratedOnlineMetadataResult>;

export type MetadataSaveRequest = GeneratedMetadataSaveRequest;

export type MetadataSaveResultError = AppErrorEnvelope;

export type MetadataSaveResultEntry = Omit<
	NullToOptionalDeep<GeneratedMetadataSaveResultEntry>,
	'error'
> & {
	error?: MetadataSaveResultError | null;
};

export type MetadataSaveBatchResult = Omit<
	NullToOptionalDeep<GeneratedMetadataSaveBatchResult>,
	'results'
> & {
	results: MetadataSaveResultEntry[];
};

export type MetadataSaveResultStatus = GeneratedMetadataSaveResultStatus;
