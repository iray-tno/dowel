//! The swappable half: where a snapshot is kept between processes.
//!
//! Everything above this boundary works in terms of `Snapshot`, so the
//! format can change to protobuf, SQLite, or something else without any
//! caller noticing.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Bumped when the on-disk shape changes. A snapshot written by a
/// different version is discarded rather than migrated -- this is a cache,
/// so rebuilding it is cheap and always correct, while a half-understood
/// migration is neither.
pub const SNAPSHOT_VERSION: u32 = 1;

/// What one source file contributed.
///
/// Only the class *names* are stored, never the style properties they
/// resolve to. Names are a durable fact about the source; the properties
/// are derived from Hozo's utility table, and caching them would keep
/// serving the old values after that table changes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileEntry {
    /// Modification time as milliseconds since the epoch, used only to
    /// decide whether a file needs rescanning.
    pub modified_ms: u64,
    pub class_names: Vec<String>,
}

/// The whole cache, as handed to and from a store.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    /// Keyed by source path. `BTreeMap` rather than `HashMap` so iteration
    /// order is stable -- generated CSS is written in this order, and it
    /// shouldn't churn between builds.
    pub files: BTreeMap<String, FileEntry>,
}

impl Snapshot {
    pub fn current() -> Self {
        Snapshot { version: SNAPSHOT_VERSION, files: BTreeMap::new() }
    }
}

/// A place a `Snapshot` can live.
///
/// Deliberately snapshot-oriented rather than per-key: a build always
/// needs every candidate at once (they become one stylesheet), so there's
/// no read pattern a per-key interface would serve better. An
/// implementation that prefers incremental writes can still buffer and
/// write on `store`.
pub trait SnapshotStore: Send + Sync {
    /// Missing or unreadable state is not an error -- it just means an
    /// empty cache, and the build rescans. Only genuine I/O trouble that a
    /// caller might want to report should surface here.
    fn load(&self) -> io::Result<Snapshot>;
    fn store(&self, snapshot: &Snapshot) -> io::Result<()>;
}

/// Keeps the snapshot as JSON on disk.
pub struct JsonFileStore {
    path: PathBuf,
}

impl JsonFileStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        JsonFileStore { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SnapshotStore for JsonFileStore {
    fn load(&self) -> io::Result<Snapshot> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            // No cache yet, which is the normal first-build case.
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Snapshot::current()),
            Err(err) => return Err(err),
        };
        // A corrupt or older-format file is discarded rather than reported:
        // it costs one rescan, where failing the build would cost the user
        // a confusing error about a file they never wrote.
        let snapshot: Snapshot = match serde_json::from_str(&text) {
            Ok(snapshot) => snapshot,
            Err(_) => return Ok(Snapshot::current()),
        };
        if snapshot.version != SNAPSHOT_VERSION {
            return Ok(Snapshot::current());
        }
        Ok(snapshot)
    }

    fn store(&self, snapshot: &Snapshot) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Written to a sibling and renamed, so an interrupted build (Ctrl-C
        // mid-write) can't leave a truncated file behind for the next one
        // to choke on. Rename is atomic on both platforms Hozo targets.
        let temp = self.path.with_extension("json.tmp");
        fs::write(&temp, serde_json::to_string_pretty(snapshot)?)?;
        fs::rename(&temp, &self.path)
    }
}

/// Holds the snapshot in memory only.
///
/// Not just a test double: it's the right store when persistence isn't
/// wanted (a one-shot production build has nothing to resume from), and it
/// keeps the trait honest by proving there's a second implementation.
#[derive(Default)]
pub struct MemoryStore {
    snapshot: std::sync::Mutex<Snapshot>,
}

impl MemoryStore {
    pub fn new() -> Self {
        MemoryStore { snapshot: std::sync::Mutex::new(Snapshot::current()) }
    }
}

impl SnapshotStore for MemoryStore {
    fn load(&self) -> io::Result<Snapshot> {
        Ok(self.snapshot.lock().expect("cache mutex poisoned").clone())
    }

    fn store(&self, snapshot: &Snapshot) -> io::Result<()> {
        *self.snapshot.lock().expect("cache mutex poisoned") = snapshot.clone();
        Ok(())
    }
}
