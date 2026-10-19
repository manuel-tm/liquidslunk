//! The user's library on disk. liquidslunk only ever reads it: nothing here
//! opens a file for writing, or changes one.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use icu_collator::options::CollatorOptions;
use icu_collator::preferences::CollationNumericOrdering;
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use super::file_types::{FileType, file_type_of};
use super::paths::{dirname, join, relative, resolve};
use crate::repo::backups;
use crate::{Error, Result};

/// A file on disk, registered or not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskFile {
    pub name: String,
    /// Relative to the database's folder
    pub path: String,
    pub file_type: FileType,
    pub size_bytes: i64,
}

/// A folder's subfolders, and the files directly in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskFolder {
    /// Relative to the database's folder: `""` is that folder.
    pub path: String,
    pub absolute_path: String,
    /// `None` at the top of the disk
    pub parent: Option<String>,
    pub folders: Vec<DiskSubfolder>,
    pub files: Vec<DiskFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskSubfolder {
    pub name: String,
    pub path: String,
}

/// What liquidslunk reads from a file on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFacts {
    pub size_bytes: i64,
    /// Epoch seconds, as slunk read them
    pub mtime: i64,
    /// Lowercase hex
    pub sha256: String,
}

/// The library around a database: paths are relative to its folder.
#[derive(Debug, Clone)]
pub struct Library {
    /// The database's folder, absolute and resolved
    root: String,
    /// liquidslunk's own files, which aren't part of the library
    own: HashSet<String>,
}

impl Library {
    /// The library around the database at `db`, an absolute path.
    pub fn new(db: &Path) -> Library {
        let db = resolve("/", &db.to_string_lossy());
        let root = dirname(&db);
        let mut own: HashSet<String> = ["", "-wal", "-shm", "-journal"]
            .iter()
            .map(|suffix| format!("{db}{suffix}"))
            .collect();
        own.insert(join(&root, backups::FOLDER));
        Library { root, own }
    }

    /// The database's folder.
    pub fn root(&self) -> &Path {
        Path::new(&self.root)
    }

    /// A path relative to the database's folder, resolved: `""` is that folder,
    /// and paths above it start with `../`. It can be given absolute.
    pub fn normalize(&self, path: &str) -> String {
        relative(&self.root, &resolve(&self.root, path))
    }

    /// A path relative to the database's folder, made absolute.
    pub fn absolute(&self, path: &str) -> PathBuf {
        PathBuf::from(resolve(&self.root, path))
    }

    /// A folder's subfolders and the files directly in it, sorted as a file
    /// browser sorts them. Hidden files and liquidslunk's own are left out.
    /// Symlinks are followed, and a broken one is left out.
    pub fn list_folder(&self, path: &str) -> Result<DiskFolder> {
        let folder_path = self.normalize(path);
        let folder = resolve(&self.root, &folder_path);
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for name in read_folder(&folder, path)? {
            let entry_path = join(&folder, &name);
            if self.skipped(&name, &entry_path) {
                continue;
            }
            match fs::metadata(&entry_path) {
                Ok(info) if info.is_dir() => folders.push(DiskSubfolder {
                    name,
                    path: self.normalize(&entry_path),
                }),
                Ok(info) if info.is_file() => files.push(self.disk_file(name, &entry_path, &info)),
                _ => {}
            }
        }
        folders.sort_by(|a, b| compare(&a.name, &b.name));
        files.sort_by(|a, b| compare(&a.name, &b.name));
        let above = dirname(&folder);
        Ok(DiskFolder {
            parent: (above != folder).then(|| self.normalize(&above)),
            path: folder_path,
            absolute_path: folder,
            folders,
            files,
        })
    }

    /// Every file in a folder and its subfolders, by path. Hidden files and
    /// folders are left out, and symlinked folders aren't followed, so a link
    /// loop can't make it run forever. Symlinked files are listed.
    pub fn scan_folder(&self, path: &str) -> Result<Vec<DiskFile>> {
        let start = resolve(&self.root, &self.normalize(path));
        let unreadable = |folder: &str| {
            let shown = self.normalize(folder);
            Error::Invalid(format!(
                "Can't read the folder {}",
                if shown.is_empty() { "." } else { &shown }
            ))
        };
        // A folder, or a link to one: a file isn't scanned
        if !fs::metadata(&start).is_ok_and(|info| info.is_dir()) {
            return Err(unreadable(&start));
        }
        let mut files = Vec::new();
        let walk = WalkDir::new(&start)
            .min_depth(1)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                let name = entry.file_name().to_string_lossy();
                !self.skipped(&name, &entry.path().to_string_lossy())
            });
        for entry in walk {
            let entry = entry.map_err(|error| {
                let folder = error.path().unwrap_or(Path::new(&start));
                unreadable(&folder.to_string_lossy())
            })?;
            let kind = entry.file_type();
            if !kind.is_file() && !kind.is_symlink() {
                continue;
            }
            let entry_path = entry.path().to_string_lossy().into_owned();
            // Follows a symlink, which is listed only if it's to a file
            if let Ok(info) = fs::metadata(&entry_path)
                && info.is_file()
            {
                let name = entry.file_name().to_string_lossy().into_owned();
                files.push(self.disk_file(name, &entry_path, &info));
            }
        }
        files.sort_by(|a, b| compare(&a.path, &b.path));
        Ok(files)
    }

    /// Reads a file's size, mtime and SHA-256, streaming it through the hash.
    pub fn read_facts(&self, path: &str) -> Result<FileFacts> {
        let file = self.absolute(path);
        let info = fs::metadata(&file)
            .ok()
            .filter(Metadata::is_file)
            .ok_or_else(|| Error::Invalid(format!("There's no file at {path}")))?;
        let mut hash = Sha256::new();
        let mut reader = File::open(&file).map_err(Error::io(&file))?;
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer).map_err(Error::io(&file))?;
            if read == 0 {
                break;
            }
            hash.update(&buffer[..read]);
        }
        Ok(FileFacts {
            size_bytes: size_of(&info),
            mtime: mtime_of(&info),
            sha256: hex(&hash.finalize()),
        })
    }

    /// Whether there's a file at the path.
    pub fn exists(&self, path: &str) -> bool {
        fs::metadata(self.absolute(path)).is_ok_and(|info| info.is_file())
    }

    /// A file's size and mtime, as registering reads them, or `None` if it's
    /// gone.
    pub fn stat_file(&self, path: &str) -> Option<(i64, i64)> {
        let info = fs::metadata(self.absolute(path)).ok()?;
        info.is_file().then(|| (size_of(&info), mtime_of(&info)))
    }

    fn skipped(&self, name: &str, absolute: &str) -> bool {
        name.starts_with('.') || self.own.contains(absolute)
    }

    fn disk_file(&self, name: String, absolute: &str, info: &Metadata) -> DiskFile {
        DiskFile {
            file_type: file_type_of(&name),
            path: self.normalize(absolute),
            size_bytes: size_of(info),
            name,
        }
    }
}

/// The names in a folder.
fn read_folder(folder: &str, shown: &str) -> Result<Vec<String>> {
    let unreadable = || {
        Error::Invalid(format!(
            "Can't read the folder {}",
            if shown.is_empty() { "." } else { shown }
        ))
    };
    fs::read_dir(folder)
        .map_err(|_| unreadable())?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|_| unreadable())
        })
        .collect()
}

/// Like a file browser sorts: "2 Song" before "10 Song". It's what
/// JavaScript's `localeCompare(…, { numeric: true })` does, with ICU's root
/// collation (design §10).
pub fn compare(a: &str, b: &str) -> Ordering {
    static COLLATOR: LazyLock<CollatorBorrowed<'static>> = LazyLock::new(|| {
        let mut preferences = CollatorPreferences::default();
        preferences.numeric_ordering = Some(CollationNumericOrdering::True);
        Collator::try_new(preferences, CollatorOptions::default())
            .expect("the root collation is compiled in")
    });
    COLLATOR.compare(a, b)
}

fn size_of(info: &Metadata) -> i64 {
    i64::try_from(info.len()).unwrap_or(i64::MAX)
}

/// The mtime in whole seconds, as slunk read it: Node's `mtimeMs` is
/// `seconds * 1000 + nanoseconds / 1e6` in floating point, then
/// `Math.floor(mtimeMs / 1000)`. So a time a few nanoseconds before a second
/// rounds up to it (design §10).
#[cfg(unix)]
fn mtime_of(info: &Metadata) -> i64 {
    use std::os::unix::fs::MetadataExt;
    mtime_from(info.mtime(), info.mtime_nsec())
}

#[cfg(not(unix))]
fn mtime_of(info: &Metadata) -> i64 {
    use std::time::UNIX_EPOCH;
    let modified = info.modified().ok();
    let since = modified.and_then(|time| time.duration_since(UNIX_EPOCH).ok());
    since.map_or(0, |since| {
        mtime_from(since.as_secs() as i64, i64::from(since.subsec_nanos()))
    })
}

fn mtime_from(seconds: i64, nanoseconds: i64) -> i64 {
    let milliseconds = seconds as f64 * 1e3 + nanoseconds as f64 / 1e6;
    (milliseconds / 1000.0).floor() as i64
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtimes_floor_to_seconds_as_node_does() {
        // What Node 26 gave for files with these times
        assert_eq!(mtime_from(1_700_000_000, 0), 1_700_000_000);
        assert_eq!(mtime_from(1_700_000_000, 500_000_000), 1_700_000_000);
        assert_eq!(mtime_from(1_700_000_000, 999_999_000), 1_700_000_000);
        assert_eq!(mtime_from(1_700_000_000, 999_999_500), 1_700_000_000);
        assert_eq!(mtime_from(1_700_000_000, 999_999_900), 1_700_000_001);
        assert_eq!(mtime_from(1_700_000_000, 999_999_999), 1_700_000_001);
    }

    #[test]
    fn file_names_sort_numerically_as_a_file_browser_does() {
        let mut names = vec![
            "10 Song.flac",
            "2 Song.flac",
            "b.flac",
            "A.flac",
            "a.flac",
            "é.flac",
            "e.flac",
        ];
        names.sort_by(|a, b| compare(a, b));
        assert_eq!(
            names,
            [
                "2 Song.flac",
                "10 Song.flac",
                "a.flac",
                "A.flac",
                "b.flac",
                "e.flac",
                "é.flac"
            ]
        );
    }
}
