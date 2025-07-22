//! Metadata handling for audiobook files
//! 
//! This module provides functionality to read and write metadata
//! from/to audio files using the Lofty crate.

use serde::{Deserialize, Serialize};

pub mod reader;
pub mod writer;

/// Represents audiobook metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudiobookMetadata {
    /// Title of the audiobook
    pub title: Option<String>,
    /// Author of the book
    pub author: Option<String>,
    /// Album name (book/series name)
    pub album: Option<String>,
    /// Narrator of the audiobook
    pub narrator: Option<String>,
    /// Publication year
    pub year: Option<u32>,
    /// Genre of the book
    pub genre: Option<String>,
    /// Description or synopsis
    pub description: Option<String>,
    /// Cover art as raw bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<Vec<u8>>,
}

impl AudiobookMetadata {
    /// Creates a new empty metadata instance
    pub fn new() -> Self {
        Self {
            title: None,
            author: None,
            album: None,
            narrator: None,
            year: None,
            genre: None,
            description: None,
            cover_art: None,
        }
    }
}

impl Default for AudiobookMetadata {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export main functions for convenience
pub use reader::read_metadata;
pub use writer::write_metadata;