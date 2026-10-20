//! The manifest (4_external_libraries.md §8.1): what liquidslunk wrote on a
//! destination, kept on the destination itself, at
//! `.liquidslunk/manifest.json` in a folder library. The database says what
//! should be there, and the manifest what is.
//!
//! A sync appends each file it finishes to `.liquidslunk/journal.jsonl`, one
//! line at a time, and rewrites the manifest from it when it ends. So a sync
//! that's interrupted, by a card pulled out midway, carries on from its last
//! file at the next one.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::folder::without_apple_double;
use crate::{Error, Result};

/// The manifest's format
pub const VERSION: u32 = 1;

const MANIFEST: &str = "manifest.json";
const JOURNAL: &str = "journal.jsonl";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub version: u32,
    /// The external library's UUID
    pub external_library: String,
    /// Its name when it was written, for a refusal to say whose it is
    pub name: String,
    pub files: Vec<Entry>,
}

/// A file liquidslunk wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// Relative to the destination, with `/`
    pub path: String,
    pub size: u64,
    /// In nanoseconds since the epoch, as read back once it was written
    pub mtime: i64,
    #[serde(flatten)]
    pub written: Written,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Written {
    /// A track's file
    #[serde(rename_all = "camelCase")]
    Track {
        track: i64,
        audio: AudioKey,
        /// Its tags as written (`Tags::fields()`)
        tags: BTreeMap<String, Vec<String>>,
        /// Its embedded cover, if it has one
        cover: Option<CoverKey>,
    },
    /// A `cover.jpg`, in a folder that holds one release's tracks
    #[serde(rename_all = "camelCase")]
    Cover { release: i64, cover: CoverKey },
}

/// What a file's audio was made from, and how: when it changes, the file is
/// converted again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioKey {
    /// The source file's SHA-256
    pub source: String,
    /// What it became: "copy", "aac 256 kbps", "alac 16-bit 44100 Hz"
    pub becomes: String,
    /// liquidslunk's way of converting (`convert::CONVERSION_VERSION`)
    pub conversion: u32,
    /// The encoder, recorded but not compared, so a macOS update doesn't
    /// convert everything again
    pub encoder: String,
}

impl AudioKey {
    /// Whether a file made this way is still what `wanted` asks for.
    pub fn matches(&self, wanted: &AudioKey) -> bool {
        self.source == wanted.source
            && self.becomes == wanted.becomes
            && self.conversion == wanted.conversion
    }
}

/// What a cover was made from, and at what size.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverKey {
    /// The cover file's SHA-256
    pub source: String,
    /// The longest side it was made to fit
    pub px: u32,
}

/// A line of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Change {
    /// A file is about to be put at `entry.path`. When it moves there from
    /// another path, that's `from`, so a sync interrupted between the
    /// journal and the rename finds it where it still is.
    Put { entry: Entry, from: Option<String> },
    /// A file was removed.
    Removed { path: String },
}

/// A destination's manifest, with what its journal adds.
#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub manifest: Option<Manifest>,
    /// Where a file was moved from, by its new path, for files moved since
    /// the manifest was written
    pub moved_from: HashMap<String, String>,
}

/// A destination's `.liquidslunk` folder.
#[derive(Debug, Clone)]
pub struct ManifestFolder {
    dir: PathBuf,
}

impl ManifestFolder {
    pub fn new(dir: PathBuf) -> ManifestFolder {
        ManifestFolder { dir }
    }

    /// The manifest, with its journal replayed. `None` for a destination
    /// liquidslunk hasn't written yet. A journal's last line, cut short when
    /// the destination went away, is left out.
    pub fn load(&self) -> Result<Loaded> {
        let path = self.dir.join(MANIFEST);
        let mut manifest = match fs::read(&path) {
            Ok(bytes) => Some(serde_json::from_slice::<Manifest>(&bytes).map_err(|error| {
                Error::Invalid(format!(
                    "The manifest at {} can't be read: {error}",
                    path.display()
                ))
            })?),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(Error::io(&path)(error)),
        };
        if manifest
            .as_ref()
            .is_some_and(|manifest| manifest.version > VERSION)
        {
            return Err(Error::Invalid(
                "This destination was synced by a newer version of liquidslunk".into(),
            ));
        }
        let mut moved_from = HashMap::new();
        let journal = self.dir.join(JOURNAL);
        let text = match fs::read_to_string(&journal) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
            Err(error) => return Err(Error::io(&journal)(error)),
        };
        for line in text.lines() {
            let Ok(change) = serde_json::from_str::<Change>(line) else {
                continue;
            };
            let Some(manifest) = manifest.as_mut() else {
                continue;
            };
            apply(&mut manifest.files, &change);
            if let Change::Put {
                entry,
                from: Some(from),
            } = change
            {
                moved_from.insert(entry.path, from);
            }
        }
        Ok(Loaded {
            manifest,
            moved_from,
        })
    }

    /// Which external library the manifest names, and its name then, read
    /// without its files. `None` without a manifest.
    pub fn identity(&self) -> Result<Option<(String, String)>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity {
            external_library: String,
            name: String,
        }
        let path = self.dir.join(MANIFEST);
        match fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice::<Identity>(&bytes)
                .ok()
                .map(|identity| (identity.external_library, identity.name))),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(Error::io(&path)(error)),
        }
    }

    /// The manifest file's size and mtime, to tell when it changed.
    pub fn stamp(&self) -> Option<(u64, i64)> {
        super::folder::stat(&self.dir.join(MANIFEST))
    }

    /// Writes the manifest, atomically: to a temporary file, then renamed
    /// over the old one. The journal it now holds is removed.
    pub fn write(&self, manifest: &Manifest) -> Result<()> {
        self.create_dir()?;
        let path = self.dir.join(MANIFEST);
        let temporary = self.dir.join(format!("{MANIFEST}.tmp"));
        let bytes = serde_json::to_vec(manifest)
            .map_err(|error| Error::Invalid(format!("The manifest can't be written: {error}")))?;
        let mut file = File::create(&temporary).map_err(Error::io(&temporary))?;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(Error::io(&temporary))?;
        drop(file);
        without_apple_double(&temporary)?;
        fs::rename(&temporary, &path).map_err(Error::io(&path))?;
        let journal = self.dir.join(JOURNAL);
        match fs::remove_file(&journal) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
            Err(error) => Err(Error::io(&journal)(error)),
        }
    }

    /// Appends a change to the journal, and waits until it's on the disk.
    pub fn append(&self, change: &Change) -> Result<()> {
        self.create_dir()?;
        let journal = self.dir.join(JOURNAL);
        let mut line = serde_json::to_vec(change)
            .map_err(|error| Error::Invalid(format!("The journal can't be written: {error}")))?;
        line.push(b'\n');
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&journal)
            .map_err(Error::io(&journal))?;
        file.write_all(&line)
            .and_then(|()| file.sync_all())
            .map_err(Error::io(&journal))?;
        drop(file);
        // Opening it for writing gives it an AppleDouble file again
        without_apple_double(&journal)
    }

    /// Removes the manifest and the journal, and the folder when nothing
    /// else is left in it.
    pub fn remove(&self) -> Result<()> {
        for name in [MANIFEST, JOURNAL] {
            let path = self.dir.join(name);
            match fs::remove_file(&path) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(Error::io(&path)(error)),
            }
        }
        super::folder::remove_empty(&self.dir.join("tmp"))?;
        super::folder::remove_empty(&self.dir)?;
        Ok(())
    }

    fn create_dir(&self) -> Result<()> {
        // Only inside the destination's folder, while it's there
        let root = self.dir.parent().unwrap_or(&self.dir);
        super::folder::create_dirs(&self.dir, root)
    }
}

/// Applies a change to a manifest's files.
pub fn apply(files: &mut Vec<Entry>, change: &Change) {
    match change {
        Change::Put { entry, from } => {
            files.retain(|file| file.path != entry.path && Some(&file.path) != from.as_ref());
            files.push(entry.clone());
        }
        Change::Removed { path } => files.retain(|file| &file.path != path),
    }
}

/// Whether a file on disk is as liquidslunk wrote it: the same size, and the
/// same mtime. FAT32 keeps local time, so a change of time zone, or of
/// daylight saving time, moves every mtime by whole quarter-hours, which
/// doesn't count.
pub fn as_written(entry: &Entry, on_disk: Option<(u64, i64)>) -> bool {
    const QUARTER_HOUR: i64 = 15 * 60 * 1_000_000_000;
    const DAY: i64 = 24 * 60 * 60 * 1_000_000_000;
    let Some((size, mtime)) = on_disk else {
        return false;
    };
    let shift = (mtime - entry.mtime).abs();
    size == entry.size && (shift == 0 || (shift % QUARTER_HOUR == 0 && shift <= DAY))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cover(release: i64, path: &str) -> Entry {
        Entry {
            path: path.into(),
            size: 10,
            mtime: 1_000,
            written: Written::Cover {
                release,
                cover: CoverKey {
                    source: "a".repeat(64),
                    px: 600,
                },
            },
        }
    }

    #[test]
    fn a_journal_carries_on_from_the_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let folder = ManifestFolder::new(dir.path().join(".liquidslunk"));
        assert!(folder.load().unwrap().manifest.is_none());
        let manifest = Manifest {
            version: VERSION,
            external_library: "u".into(),
            name: "Car".into(),
            files: vec![cover(1, "A/cover.jpg"), cover(2, "B/cover.jpg")],
        };
        folder.write(&manifest).unwrap();
        folder
            .append(&Change::Removed {
                path: "A/cover.jpg".into(),
            })
            .unwrap();
        folder
            .append(&Change::Put {
                entry: cover(2, "C/cover.jpg"),
                from: Some("B/cover.jpg".into()),
            })
            .unwrap();
        // A line cut short
        let journal = dir.path().join(".liquidslunk").join(JOURNAL);
        let mut file = OpenOptions::new().append(true).open(&journal).unwrap();
        file.write_all(b"{\"removed\":{\"pa").unwrap();
        let loaded = folder.load().unwrap();
        let files = loaded.manifest.unwrap().files;
        assert_eq!(files, vec![cover(2, "C/cover.jpg")]);
        assert_eq!(loaded.moved_from["C/cover.jpg"], "B/cover.jpg");
        assert_eq!(
            folder.identity().unwrap(),
            Some(("u".to_string(), "Car".to_string()))
        );
        // Writing it again starts a new journal
        folder.write(&Manifest { files, ..manifest }).unwrap();
        assert!(!journal.exists());
    }

    #[test]
    fn a_file_as_written_has_its_size_and_mtime() {
        let entry = cover(1, "A/cover.jpg");
        let hour = 3_600_000_000_000;
        assert!(as_written(&entry, Some((10, 1_000))));
        assert!(!as_written(&entry, Some((11, 1_000))));
        assert!(!as_written(&entry, Some((10, 2_000))));
        assert!(!as_written(&entry, None));
        // A time zone's shift
        assert!(as_written(&entry, Some((10, 1_000 + hour))));
        assert!(as_written(&entry, Some((10, 1_000 - 5 * hour - hour / 2))));
        assert!(!as_written(&entry, Some((10, 1_000 + 30 * hour))));
    }
}
