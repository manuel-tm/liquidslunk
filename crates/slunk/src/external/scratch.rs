//! Where a sync makes its files before they're copied to the destination: a
//! folder of its own in the system's temporary folder (backend.md, "Syncing a
//! folder library"). It's removed when the sync ends, and whatever an earlier
//! run left there is removed at launch.
//!
//! Each folder holds a lock while its sync runs, so the leftovers removed at
//! launch are never those of a sync still running in another copy of the
//! app, like a development build.

use std::fs::{self, File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, Result};

const PREFIX: &str = "liquidslunk-sync-";
const LOCK: &str = ".lock";

/// A sync's temporary folder, removed when it's dropped.
#[derive(Debug)]
pub struct Scratch {
    dir: PathBuf,
    // Held for the folder's life
    _lock: File,
}

impl Scratch {
    pub fn new() -> Result<Scratch> {
        Scratch::in_folder(&std::env::temp_dir())
    }

    pub fn in_folder(parent: &Path) -> Result<Scratch> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = parent.join(format!(
            "{PREFIX}{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
        let lock_path = dir.join(LOCK);
        let lock = File::create(&lock_path).map_err(Error::io(&lock_path))?;
        lock.lock().map_err(Error::io(&lock_path))?;
        Ok(Scratch { dir, _lock: lock })
    }

    /// A path for a file in it.
    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Removes the temporary folders earlier syncs left, those whose lock no
/// sync holds.
pub fn remove_leftovers() {
    remove_leftovers_in(&std::env::temp_dir());
}

pub fn remove_leftovers_in(parent: &Path) {
    let Ok(entries) = fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(PREFIX) {
            continue;
        }
        let dir = entry.path();
        let held = File::open(dir.join(LOCK))
            .map(|lock| matches!(lock.try_lock(), Err(TryLockError::WouldBlock)))
            .unwrap_or(false);
        if !held {
            let _ = fs::remove_dir_all(&dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leftovers_go_and_running_syncs_stay() {
        let parent = tempfile::tempdir().unwrap();
        let running = Scratch::in_folder(parent.path()).unwrap();
        fs::write(running.path("a.m4a"), b"").unwrap();
        let left = parent.path().join(format!("{PREFIX}1-1"));
        fs::create_dir(&left).unwrap();
        fs::write(left.join(LOCK), b"").unwrap();
        remove_leftovers_in(parent.path());
        assert!(running.path("a.m4a").exists());
        assert!(!left.exists());
        let dir = running.path("");
        drop(running);
        assert!(!dir.exists());
    }
}
