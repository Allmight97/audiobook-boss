/**
 * TypeScript interfaces for audiobook metadata
 */

/**
 * Represents metadata for an audiobook file
 */
export interface AudiobookMetadata {
  /** Title of the audiobook */
  title?: string;
  /** Author of the book */
  author?: string;
  /** Album name (book/series name) */
  album?: string;
  /** Narrator of the audiobook */
  narrator?: string;
  /** Publication year */
  year?: number;
  /** Genre of the book */
  genre?: string;
  /** Description or synopsis */
  description?: string;
  /** Cover art as base64 encoded string (optional in responses) */
  coverArt?: string;
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