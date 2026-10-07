//! Which decoder reads AAC sources.

use serde::{Deserialize, Serialize};

/// The AAC decoder for every AAC source: import inspection, previews, and
/// exports. Other codecs always use FFmpeg.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum AacDecoder {
    /// FFmpeg's AAC decoders, chosen per file by trial decoding.
    #[default]
    Auto,
    /// Bundled FAAD3. A file it cannot decode fails instead of falling back.
    Faad,
}
