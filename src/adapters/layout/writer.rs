//! Serialized layout writes shared by background saving and explicit close flush.
//! Snapshots arrive as values; this module never reads live GPUI entities.
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::Result;

use crate::adapters::layout as storage;
use crate::workspace::boards::BoardSetFile;

pub(crate) type SharedWriter = Arc<Mutex<WriteSession>>;

pub(crate) struct WriteSession {
    path: PathBuf,
    saved_revision: Option<u64>,
}

impl WriteSession {
    pub(crate) fn shared(path: PathBuf) -> SharedWriter {
        Arc::new(Mutex::new(Self {
            path,
            saved_revision: None,
        }))
    }
}

/// A dropped async task can leave blocking filesystem work already running.
/// Serialize commits and reject older requests after a newer flush has won.
pub(crate) fn save(writer: &SharedWriter, revision: u64, state: &BoardSetFile) -> Result<()> {
    let mut session = writer
        .lock()
        .map_err(|_| anyhow::anyhow!("布局保存任务状态不可用"))?;
    if session
        .saved_revision
        .is_some_and(|saved| revision <= saved)
    {
        return Ok(());
    }
    // Keep existing validation, atomic replacement and corrupt/backup retention.
    // A failed write never acknowledges its revision, so the same request can retry.
    storage::write_board_set(&session.path, state)?;
    session.saved_revision = Some(revision);
    Ok(())
}

/// Tracks which live changes a completed save can acknowledge. Filesystem order
/// and UI callback order are separate: both need their own revision check.
#[derive(Default)]
pub(crate) struct SaveRevision {
    current: u64,
    pending: Option<u64>,
}

impl SaveRevision {
    pub(crate) fn changed(&mut self) {
        self.current = self
            .current
            .checked_add(1)
            .expect("layout save revision exhausted");
    }

    pub(crate) fn begin(&mut self) -> u64 {
        // A snapshot may include inputs/viewports that emit no layout event.
        // Give every submitted snapshot a fresh identity, especially close flush.
        self.changed();
        self.pending = Some(self.current);
        self.current
    }

    /// None means obsolete callback. Some(false) means this save completed, but
    /// newer live edits must retain dirty state. Some(true) acknowledges all edits.
    pub(crate) fn finish(&mut self, revision: u64) -> Option<bool> {
        if self.pending != Some(revision) {
            return None;
        }
        self.pending = None;
        Some(self.current == revision)
    }
}

#[cfg(test)]
mod tests;
