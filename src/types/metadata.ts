/**
 * TypeScript interfaces for audiobook metadata
 *
 * Field mapping for Plex/Audiobookshelf compatibility:
 * - artist = Author (©ART, also written to aART/AlbumArtist)
 * - composer = Narrator (©wrt/Composer)
 * - series = Series name (©mvn/MVNM)
 * - series_part = Book number in series (©mvi/MVIN)
 * - subseries = Secondary series name (2nd entry in SERIES list)
 * - subseries_part = Secondary series number (2nd entry in SERIES-PART list)
 * - album_sort = Computed TSOA for library sorting ("SERIES PP - TITLE")
 * - date = Publication year (©day)
 */

/**
 * Represents metadata for an audiobook file
 * Matches Rust backend AudiobookMetadata structure
 */
export interface AudiobookMetadata {
	/** Title of the audiobook (©nam) */
	title?: string;
	/** Author of the book (©ART, also written to aART/AlbumArtist) */
	artist?: string;
	/** Album name - typically same as title for audiobooks (©alb) */
	album?: string;
	/** Narrator of the audiobook (©wrt/Composer) */
	composer?: string;
	/** Genre of the book (©gen) */
	genre?: string;
	/** Publication year (©day) */
	date?: number;
	/** Track number (chapter number, total chapters) */
	track?: [number, number | null];
	/** Disk number (rarely used for audiobooks) */
	disk?: [number, number | null];
	/** Comment field (©cmt) - short note, distinct from description */
	comment?: string;
	/** Description or synopsis (desc) */
	description?: string;
	/** Series name (©mvn/MVNM) */
	series?: string;
	/** Book number in series (©mvi/MVIN) */
	series_part?: string;
	/** Sub-series name (secondary series) */
	subseries?: string;
	/** Book number in sub-series */
	subseries_part?: string;
	/** Album sort order for library sorting (soal/TSOA) - computed as "SERIES PP - TITLE" */
	album_sort?: string;
	/** Cover art as raw bytes from backend */
	cover_art?: number[];
}

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

export type MetadataSource = 'audnexus';

export interface OnlineMetadataResult {
	source: MetadataSource;
	sourceId: string;
	title: string;
	authors: string[];
	narrators: string[];
	series?: string;
	seriesPart?: string;
	subseries?: string;
	subseriesPart?: string;
	description?: string;
	publishedYear?: number;
	durationSeconds?: number;
	coverUrl?: string;
	audibleOnly?: boolean;
}
