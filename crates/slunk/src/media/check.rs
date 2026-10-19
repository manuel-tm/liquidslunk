//! The library check's file part (slunk's library check): what changed on
//! disk since files were registered, and the fixes, relinking files that
//! moved and reading changed ones again. Checking writes nothing.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use super::library::compare;
use super::paths::basename;
use super::{DiskFile, FileChanges, FileFacts, FileRecord, Media};
use crate::Result;
use crate::repo::files;

/// What the check found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCheck {
    /// How many registered files were checked
    pub file_count: i64,
    /// Registered files that aren't on disk anymore, by path
    pub missing: Vec<MissingFile>,
    /// Registered files whose size or mtime isn't what was read, by path
    pub changed: Vec<ChangedFile>,
    /// Unregistered files in folders that hold registered ones, by folder
    pub unregistered: Vec<UnregisteredFolder>,
}

/// A registered file that's gone, and where it may have moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingFile {
    pub file: FileRecord,
    /// Unregistered files with the same size and hash, those with the same
    /// name first
    pub moved_to: Vec<String>,
    /// The only one, or the only one with the same name. `None` when that's
    /// unclear, or nothing matched.
    pub likely_move: Option<String>,
}

/// A registered file whose size or mtime changed, as it is on disk now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub file: FileRecord,
    pub size_bytes: i64,
    pub mtime: i64,
}

/// A folder's files that aren't registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnregisteredFolder {
    /// `""` for the database's folder
    pub path: String,
    pub files: Vec<DiskFile>,
}

/// A file that moved, and its new path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMove {
    pub file_id: i64,
    pub path: String,
}

impl Media {
    /// Reads each registered file's size and mtime, and lists the folders
    /// they're in. When some are missing, it scans the library and hashes the
    /// unregistered files of a missing file's size, to find where they
    /// moved. It writes nothing, and reads the disk without holding the
    /// connection.
    pub fn check_files(&self) -> Result<FileCheck> {
        let all = files::all(&self.store.conn())?;
        let known: HashSet<&str> = all.iter().map(|file| file.path.as_str()).collect();
        let mut missing: Vec<&FileRecord> = Vec::new();
        let mut changed: Vec<ChangedFile> = Vec::new();
        for file in &all {
            match self.library.stat_file(&file.path) {
                None => missing.push(file),
                Some((size_bytes, mtime))
                    if size_bytes != file.size_bytes || mtime != file.mtime =>
                {
                    changed.push(ChangedFile {
                        file: file.clone(),
                        size_bytes,
                        mtime,
                    })
                }
                Some(_) => {}
            }
        }

        // The folders that hold registered files, and what else is in them
        let mut folders: Vec<&str> = Vec::new();
        for file in &all {
            let folder = folder_of(&file.path);
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
        let listed: Vec<DiskFile> = folders
            .iter()
            .filter_map(|folder| self.library.list_folder(folder).ok())
            .flat_map(|folder| folder.files)
            .collect();

        let moved = if missing.is_empty() {
            Vec::new()
        } else {
            self.moved_files(&all, &missing, &known, &listed)
        };
        let moved_to: HashSet<&str> = moved.iter().map(|(_, path)| path.as_str()).collect();

        let mut unregistered: Vec<UnregisteredFolder> = Vec::new();
        for file in listed {
            if known.contains(file.path.as_str()) || moved_to.contains(file.path.as_str()) {
                continue;
            }
            let folder = folder_of(&file.path);
            match unregistered.iter_mut().find(|found| found.path == folder) {
                Some(found) => found.files.push(file),
                None => unregistered.push(UnregisteredFolder {
                    path: folder.to_string(),
                    files: vec![file],
                }),
            }
        }

        missing.sort_by(|a, b| by_path(&a.path, &b.path));
        changed.sort_by(|a, b| by_path(&a.file.path, &b.file.path));
        unregistered.sort_by(|a, b| by_path(&a.path, &b.path));
        Ok(FileCheck {
            file_count: all.len() as i64,
            missing: missing
                .into_iter()
                .map(|file| candidates_for(file, &moved))
                .collect(),
            changed,
            unregistered,
        })
    }

    /// Unregistered files of a missing file's size, with their hashes, in
    /// the order they're found: in the folders that hold registered files,
    /// then anywhere in the library, which is the database's folder and the
    /// top folders outside it that hold registered files. A file that can't
    /// be read is left out.
    fn moved_files(
        &self,
        all: &[FileRecord],
        missing: &[&FileRecord],
        known: &HashSet<&str>,
        listed: &[DiskFile],
    ) -> Vec<(String, String)> {
        let mut roots: Vec<String> = vec![String::new()];
        for file in all {
            if let Some(root) = outside_root(&file.path)
                && !roots.contains(&root)
            {
                roots.push(root);
            }
        }
        let scanned: Vec<DiskFile> = roots
            .iter()
            .flat_map(|root| self.library.scan_folder(root).unwrap_or_default())
            .collect();
        // By path, in the order first found, each as last found
        let mut order: Vec<&str> = Vec::new();
        let mut on_disk: HashMap<&str, &DiskFile> = HashMap::new();
        for file in listed.iter().chain(&scanned) {
            if on_disk.insert(&file.path, file).is_none() {
                order.push(&file.path);
            }
        }

        // Only files of a missing file's size are worth hashing
        let sizes: HashSet<i64> = missing.iter().map(|file| file.size_bytes).collect();
        order
            .into_iter()
            .map(|path| on_disk[path])
            .filter(|file| !known.contains(file.path.as_str()) && sizes.contains(&file.size_bytes))
            .filter_map(|file| {
                let facts = self.library.read_facts(&file.path).ok()?;
                Some((facts.sha256, file.path.clone()))
            })
            .collect()
    }

    /// Points files that moved at their new paths, one at a time, reading
    /// each one again, as editing a file's path does.
    pub fn relink_files(&self, moves: &[FileMove]) -> Result<Vec<FileRecord>> {
        moves
            .iter()
            .map(|change| self.update_file(change.file_id, Some(&change.path), None))
            .collect()
    }

    /// Reads files again, for files that changed: their size, mtime and
    /// hash. Every file is read before anything is saved, so one that's gone
    /// fails the whole call. Ids that aren't registered are left out.
    pub fn rehash_files(&self, ids: &[i64]) -> Result<Vec<FileRecord>> {
        let found = files::get_many(&self.store.conn(), ids)?;
        let facts: Vec<FileFacts> = found
            .iter()
            .map(|file| self.library.read_facts(&file.path))
            .collect::<Result<_>>()?;
        self.store.transaction(|tx| {
            let mut rehashed = Vec::new();
            for (file, facts) in found.iter().zip(facts) {
                let changes = FileChanges {
                    facts: Some(facts),
                    ..FileChanges::default()
                };
                rehashed.extend(files::update(tx, file.id, &changes)?);
            }
            Ok(rehashed)
        })
    }
}

/// A missing file's candidates, by hash, the ones with its name first.
fn candidates_for(file: &FileRecord, moved: &[(String, String)]) -> MissingFile {
    let paths: Vec<&String> = moved
        .iter()
        .filter(|(sha256, _)| *sha256 == file.sha256)
        .map(|(_, path)| path)
        .collect();
    let name = basename(&file.path);
    let (same_name, others): (Vec<&String>, Vec<&String>) =
        paths.iter().partition(|path| basename(path) == name);
    let likely_move = match (&paths[..], &same_name[..]) {
        ([only], _) => Some((*only).clone()),
        (_, [only]) => Some((*only).clone()),
        _ => None,
    };
    MissingFile {
        file: file.clone(),
        moved_to: same_name.into_iter().chain(others).cloned().collect(),
        likely_move,
    }
}

/// The folder a path is in, as Node's `path.dirname()` gives it: `""` for
/// the database's folder.
fn folder_of(path: &str) -> &str {
    path.rfind('/').map_or("", |slash| &path[..slash])
}

/// `../../music` for a path like `../../music/a.flac`: the top folder
/// outside the database's that holds it, as `^((?:\.\.\/)+)([^/]+)\/`
/// matched it. `None` for a path inside the database's folder, or a file
/// right above it.
fn outside_root(path: &str) -> Option<String> {
    let mut rest = path;
    let mut ups = 0;
    while let Some(after) = rest.strip_prefix("../") {
        rest = after;
        ups += 1;
    }
    let slash = rest.find('/')?;
    (ups > 0 && slash > 0).then(|| format!("{}{}", "../".repeat(ups), &rest[..slash]))
}

/// Paths in the order a file browser lists them, as slunk sorted them with
/// `localeCompare(…, { numeric: true })`.
fn by_path(a: &str, b: &str) -> Ordering {
    compare(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roots_outside_the_library_are_its_top_folders() {
        assert_eq!(
            outside_root("../shared/a.m4a").as_deref(),
            Some("../shared")
        );
        assert_eq!(
            outside_root("../../music/x/a.flac").as_deref(),
            Some("../../music")
        );
        assert_eq!(outside_root("../a.m4a"), None);
        assert_eq!(outside_root("music/a.flac"), None);
        assert_eq!(outside_root(".../a/b"), None);
    }

    #[test]
    fn folders_are_what_dirname_gives() {
        assert_eq!(folder_of("music/a/b.flac"), "music/a");
        assert_eq!(folder_of("b.flac"), "");
        assert_eq!(folder_of("../a.m4a"), "..");
    }
}
