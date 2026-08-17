//! Build cache for Dowel's candidate-class scan.
//!
//! Two layers, split so the second can be replaced without touching the
//! first:
//!
//! - `CandidateCache` -- the API callers use. Shaped around what the scan
//!   actually needs (is this file still current? what's the union of every
//!   file's candidates?) rather than being a general-purpose key/value
//!   store, because a general one would have to answer questions nothing
//!   asks and would make the staleness rule someone else's problem.
//! - `SnapshotStore` -- where the data rests between processes. JSON on
//!   disk today; swapping in protobuf, SQLite, or anything else means
//!   writing one impl and changing one constructor call.
//!
//! **Ownership note.** The scan is a main-process job: Vite's plugin
//! container is single-process, and Metro runs `transform` in `jest-worker`
//! subprocesses but its config layer is not. Keeping scanning out of
//! per-file transforms means there is exactly one writer, which is why a
//! single shared file is safe here without locking.

mod store;

use std::collections::{BTreeMap, HashSet};

pub use store::{FileEntry, JsonFileStore, MemoryStore, Snapshot, SnapshotStore, SNAPSHOT_VERSION};

/// Tracks which candidate classes each source file contributes, and which
/// files still need scanning.
pub struct CandidateCache {
    store: Box<dyn SnapshotStore>,
    snapshot: Snapshot,
    /// Candidate -> number of source files contributing it. Derived from the
    /// snapshot on open, so the durable format remains source facts only.
    references: BTreeMap<String, usize>,
    dirty: bool,
}

impl CandidateCache {
    /// Reads whatever the store has. A missing, corrupt, or older-format
    /// snapshot yields an empty cache rather than an error -- rebuilding is
    /// cheap and always correct.
    pub fn open(store: Box<dyn SnapshotStore>) -> Self {
        let snapshot = store.load().unwrap_or_else(|_| Snapshot::current());
        let mut references = BTreeMap::new();
        for entry in snapshot.files.values() {
            for class_name in &entry.class_names {
                *references.entry(class_name.clone()).or_insert(0) += 1;
            }
        }
        CandidateCache { store, snapshot, references, dirty: false }
    }

    /// Whether `path`'s entry is present and matches `modified_ms`, i.e.
    /// the file hasn't changed since it was scanned.
    pub fn is_current(&self, path: &str, modified_ms: u64) -> bool {
        self.snapshot.files.get(path).is_some_and(|e| e.modified_ms == modified_ms)
    }

    /// Records what a scan of `path` found, replacing any earlier entry.
    ///
    /// Returns whether the file's *candidates* changed -- not whether
    /// anything was written. Saving a file with no class edits bumps its
    /// mtime, which the cache must store but which leaves the generated
    /// stylesheet byte-identical; callers use this to skip rewriting it.
    pub fn record(&mut self, path: &str, modified_ms: u64, class_names: Vec<String>) -> bool {
        let entry = FileEntry { modified_ms, class_names };
        if self.snapshot.files.get(path) == Some(&entry) {
            return false;
        }

        let previous = self.snapshot.files.insert(path.to_string(), entry.clone());
        let changed = previous.as_ref().map(|old| &old.class_names) != Some(&entry.class_names);
        if changed {
            if let Some(old) = previous {
                self.remove_references(&old.class_names);
            }
            self.add_references(&entry.class_names);
        }
        self.dirty = true;
        changed
    }

    /// Drops a file's entry -- for when a source file is deleted, so its
    /// candidates stop appearing in the union. Returns whether anything was
    /// there to drop.
    pub fn forget(&mut self, path: &str) -> bool {
        if let Some(removed) = self.snapshot.files.remove(path) {
            self.remove_references(&removed.class_names);
            self.dirty = true;
            true
        } else {
            false
        }
    }

    /// Drops entries for files that were not present in the latest complete
    /// project walk. This closes the restart gap left by file-watcher delete
    /// events: a file removed while the bundler was stopped must not keep
    /// contributing candidates forever.
    pub fn retain_files(&mut self, paths: Vec<String>) -> usize {
        let present: HashSet<String> = paths.into_iter().collect();
        let missing: Vec<String> = self
            .snapshot
            .files
            .keys()
            .filter(|path| !present.contains(*path))
            .cloned()
            .collect();
        for path in &missing {
            self.forget(path);
        }
        missing.len()
    }

    /// Every candidate class across every file, deduplicated and sorted.
    ///
    /// Sorted rather than in insertion order so the generated stylesheet
    /// is byte-identical between builds that saw the same files in a
    /// different order.
    pub fn union(&self) -> Vec<String> {
        self.references.keys().cloned().collect()
    }

    /// Number of files tracked. Mostly for diagnostics.
    pub fn len(&self) -> usize {
        self.snapshot.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.snapshot.files.is_empty()
    }

    fn add_references(&mut self, class_names: &[String]) {
        for class_name in class_names {
            *self.references.entry(class_name.clone()).or_insert(0) += 1;
        }
    }

    fn remove_references(&mut self, class_names: &[String]) {
        for class_name in class_names {
            let remove = match self.references.get_mut(class_name) {
                Some(count) if *count > 1 => {
                    *count -= 1;
                    false
                }
                Some(_) => true,
                None => false,
            };
            if remove {
                self.references.remove(class_name);
            }
        }
    }

    /// Writes the snapshot back if anything changed. A no-op otherwise, so
    /// callers can persist freely without rewriting an unchanged file on
    /// every rebuild.
    pub fn persist(&mut self) -> std::io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        self.store.store(&self.snapshot)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn in_memory() -> CandidateCache {
        CandidateCache::open(Box::new(MemoryStore::new()))
    }

    #[test]
    fn union_deduplicates_across_files_and_is_sorted() {
        let mut cache = in_memory();
        cache.record("b.tsx", 1, vec!["p-4".into(), "gap-2".into()]);
        cache.record("a.tsx", 1, vec!["p-4".into(), "flex-1".into()]);
        assert_eq!(cache.union(), vec!["flex-1", "gap-2", "p-4"]);
    }

    #[test]
    fn a_shared_candidate_survives_until_its_last_file_leaves() {
        let mut cache = in_memory();
        cache.record("a.tsx", 1, vec!["p-4".into()]);
        cache.record("b.tsx", 1, vec!["p-4".into()]);

        cache.forget("a.tsx");
        assert_eq!(cache.union(), vec!["p-4"]);
        cache.forget("b.tsx");
        assert!(cache.union().is_empty());
    }

    #[test]
    fn replacing_a_files_candidates_updates_only_its_references() {
        let mut cache = in_memory();
        cache.record("a.tsx", 1, vec!["p-4".into(), "gap-2".into()]);
        cache.record("b.tsx", 1, vec!["p-4".into()]);

        cache.record("a.tsx", 2, vec!["m-4".into()]);
        assert_eq!(cache.union(), vec!["m-4", "p-4"]);
    }

    #[test]
    fn is_current_tracks_modification_time() {
        let mut cache = in_memory();
        assert!(!cache.is_current("a.tsx", 1), "unknown file is never current");
        cache.record("a.tsx", 1, vec!["p-4".into()]);
        assert!(cache.is_current("a.tsx", 1));
        assert!(!cache.is_current("a.tsx", 2), "a newer mtime means rescan");
    }

    #[test]
    fn forget_removes_a_deleted_file_from_the_union() {
        let mut cache = in_memory();
        cache.record("a.tsx", 1, vec!["p-4".into()]);
        cache.forget("a.tsx");
        assert!(cache.union().is_empty());
    }

    #[test]
    fn retain_files_sweeps_entries_missing_from_a_complete_walk() {
        let mut cache = in_memory();
        cache.record("a.tsx", 1, vec!["p-4".into()]);
        cache.record("b.tsx", 1, vec!["gap-2".into()]);

        assert_eq!(cache.retain_files(vec!["b.tsx".into()]), 1);
        assert_eq!(cache.union(), vec!["gap-2"]);
        assert_eq!(cache.retain_files(vec!["b.tsx".into()]), 0);
    }

    #[test]
    fn record_reports_candidate_changes_not_mere_rewrites() {
        let mut cache = in_memory();
        assert!(cache.record("a.tsx", 1, vec!["p-4".into()]), "a new file is a change");
        assert!(
            !cache.record("a.tsx", 2, vec!["p-4".into()]),
            "a touched file with the same classes leaves the stylesheet identical"
        );
        assert!(cache.is_current("a.tsx", 2), "...but the new mtime is still stored");
        assert!(cache.record("a.tsx", 3, vec!["p-8".into()]));
    }

    #[test]
    fn persist_is_a_no_op_when_nothing_changed() {
        let mut cache = in_memory();
        cache.record("a.tsx", 1, vec!["p-4".into()]);
        cache.persist().unwrap();
        // Re-recording identical content must not mark it dirty again.
        cache.record("a.tsx", 1, vec!["p-4".into()]);
        assert!(!cache.dirty);
    }

    #[test]
    fn a_snapshot_survives_a_round_trip_through_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidates.json");

        let mut cache = CandidateCache::open(Box::new(JsonFileStore::new(&path)));
        cache.record("a.tsx", 7, vec!["p-4".into()]);
        cache.persist().unwrap();

        let reopened = CandidateCache::open(Box::new(JsonFileStore::new(&path)));
        assert_eq!(reopened.union(), vec!["p-4"]);
        assert!(reopened.is_current("a.tsx", 7));
    }

    #[test]
    fn a_corrupt_snapshot_is_discarded_rather_than_failing_the_build() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidates.json");
        std::fs::write(&path, "{ this is not json").unwrap();

        let cache = CandidateCache::open(Box::new(JsonFileStore::new(&path)));
        assert!(cache.is_empty(), "should fall back to an empty cache");
    }

    #[test]
    fn a_snapshot_from_another_version_is_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("candidates.json");
        std::fs::write(
            &path,
            r#"{"version":999,"files":{"a.tsx":{"modified_ms":1,"class_names":["p-4"]}}}"#,
        )
        .unwrap();

        let cache = CandidateCache::open(Box::new(JsonFileStore::new(&path)));
        assert!(cache.is_empty(), "a format we don't understand must not be trusted");
    }

    #[test]
    fn the_same_cache_works_over_a_different_store() {
        // The point of the split: callers never mention the format.
        for store in [
            Box::new(MemoryStore::new()) as Box<dyn SnapshotStore>,
            Box::new(JsonFileStore::new(
                tempfile::tempdir().unwrap().path().join("c.json"),
            )),
        ] {
            let mut cache = CandidateCache::open(store);
            cache.record("a.tsx", 1, vec!["p-4".into()]);
            assert_eq!(cache.union(), vec!["p-4"]);
            cache.persist().unwrap();
        }
    }
}
