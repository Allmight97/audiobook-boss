/**
 * TypeScript interfaces for audiobook metadata
 */

/**
 * Represents metadata for an audiobook file
 * Updated to match Rust backend AudiobookMetadata structure
 */
export interface AudiobookMetadata {
  /** Title of the audiobook */
  title?: string;
  /** Artist (author) of the book - maps to artist field in backend */
  artist?: string;
  /** Album name (book/series name) */
  album?: string;
  /** Composer (narrator) of the audiobook - maps to composer field in backend */
  composer?: string;
  /** Genre of the book */
  genre?: string;
  /** Publication date/year - maps to date field in backend */
  date?: number;
  /** Track number (chapter number, total chapters) */
  track?: [number, number | null];
  /** Disk number (rarely used for audiobooks) */
  disk?: [number, number | null];
  /** Comment field */
  comment?: string;
  /** Description or synopsis */
  description?: string;
  /** Cover art as base64 encoded string (optional in responses) */
  coverArt?: string;
  /** Cover art as raw bytes from backend (snake_case field name) */
  cover_art?: number[];
  
  // Legacy fields for backward compatibility (deprecated)
  /** @deprecated Use artist instead */
  author?: string;
  /** @deprecated Use composer instead */
  narrator?: string;
  /** @deprecated Use date instead */
  year?: number;
  /** Series name - may be combined with album */
  series?: string;
}

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
