use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::app_settings::{EditStamp, LatestEdit};

#[derive(Default)]
pub(crate) struct DefaultEdits {
    audio: u64,
    output: Arc<AtomicU64>,
    reset_revision: u64,
}

impl DefaultEdits {
    pub(crate) fn stamp(&self) -> EditStamp {
        EditStamp {
            audio: self.audio,
            output: self.output.load(Ordering::SeqCst),
        }
    }

    pub(crate) fn note_audio_edit(&mut self) -> EditStamp {
        self.audio += 1;
        self.stamp()
    }

    pub(crate) fn note_output_edit(&mut self) -> (EditStamp, LatestEdit) {
        let output = self.output.fetch_add(1, Ordering::SeqCst) + 1;
        (
            EditStamp {
                audio: self.audio,
                output,
            },
            LatestEdit(Arc::clone(&self.output), output),
        )
    }
    pub(crate) fn take_reset(&mut self, revision: u64) -> bool {
        if revision <= self.reset_revision {
            return false;
        }
        self.reset_revision = revision;
        true
    }
}

#[derive(Default)]
pub(crate) struct Claim(bool);

impl Claim {
    pub(crate) fn take(&mut self) -> bool {
        !std::mem::replace(&mut self.0, true)
    }

    pub(crate) fn release(&mut self) {
        self.0 = false;
    }

    pub(crate) fn held(&self) -> bool {
        self.0
    }
}
