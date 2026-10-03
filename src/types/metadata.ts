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
 * - album_sort = TSOA library sort value; processing derives it from series/book #/title
 *   (Rust `processing_album_sort`), saves preserve it unless explicit intent is sent
 * - date = Publication date (YYYY or YYYY-MM in ©day)
 *
 * `track`, `disk`, and `comment` remain readable for compatibility, but ABB does
 * not expose them as supported UI draft write fields. `album_sort` has no UI
 * draft field; Rust owns its value.
 */

import type {
	AudiobookMetadata as GeneratedAudiobookMetadata,
	MetadataSource as GeneratedMetadataSource,
	OnlineMetadataResult as GeneratedOnlineMetadataResult,
} from '../lib/generated/tauri';
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
