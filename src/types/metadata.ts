/**
 * TypeScript interfaces for audiobook metadata
 *
 * Field mapping for Plex/Audiobookshelf compatibility:
 * - artist = Author (©ART, also written to aART/AlbumArtist)
 * - composer = Narrator (©wrt/Composer)
 * - series = Series name (©mvn/MVNM)
 * - series_part = Series sequence / book # within a series (©mvi/MVIN)
 * - subseries = Secondary series name (2nd entry in SERIES list)
 * - subseries_part = Series sequence / book # within a sub-series (2nd entry in SERIES-PART list)
 * - album_sort = Computed TSOA for library sorting ("SERIES PP - TITLE")
 * - date = Publication date (YYYY or YYYY-MM in ©day)
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

/**
 * Result type for metadata operations
 */
export interface MetadataResult {
	success: boolean;
	error?: string;
	metadata?: AudiobookMetadata;
}

/**
 * Parameters for writing metadata
 */
export interface WriteMetadataParams {
	filePath: string;
	metadata: AudiobookMetadata;
}

/**
 * Parameters for writing cover art
 */
export interface WriteCoverArtParams {
	filePath: string;
	coverData: number[]; // byte array
}

export type MetadataSource = GeneratedMetadataSource;

export type OnlineMetadataResult = NullToOptionalDeep<GeneratedOnlineMetadataResult>;
