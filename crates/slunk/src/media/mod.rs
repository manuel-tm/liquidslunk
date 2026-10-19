//! The media context: registered files, and the library on disk they come
//! from. liquidslunk only reads the files: it records them in its database.
//!
//! - `library`: the library on disk, read-only.
//! - `paths`: Node's path functions, which slunk's paths were made with.
//! - `file_types`: a file's type, from its extension.
//! - `audio`: an audio file's duration and embedded tags.
//! - `check`: the library check's file part, and its fixes.
//! - `repository`: the records `repo::files` and `repo::file_links` store.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use rusqlite::Connection;

use crate::paging::{ListArgs, Page, paging_of, search_of, sort_of};
use crate::repo::store::Store;
use crate::repo::{file_links, files};
use crate::{Error, Result};

pub mod audio;
mod check;
pub mod file_types;
pub mod library;
pub mod paths;
pub mod repository;

pub use audio::{Audio, AudioTags};
pub use check::{ChangedFile, FileCheck, FileMove, MissingFile, UnregisteredFolder};
pub use file_types::{FileType, file_type_of};
pub use library::{DiskFile, DiskFolder, DiskSubfolder, FileFacts, Library};
pub use repository::*;

/// Files read from disk and ready to insert, and those already registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedFiles {
    pub existing: Vec<FileRecord>,
    pub fresh: Vec<NewFile>,
}

/// What a page of files takes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileListArgs {
    pub list: ListArgs,
    pub file_type: Option<FileType>,
    /// Only files nothing uses yet
    pub unused: bool,
    /// Only files that aren't a track's audio yet, to create tracks from
    pub without_track: bool,
}

/// A folder's subfolders and files, marking what's registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowsedFolder {
    /// Relative to the database's folder: `""` is that folder, and `..`
    /// goes up.
    pub path: String,
    pub absolute_path: String,
    /// The folder above, or `None` at the top of the disk
    pub parent: Option<String>,
    pub folders: Vec<BrowsedSubfolder>,
    pub files: Vec<FolderFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowsedSubfolder {
    pub name: String,
    pub path: String,
    /// Whether it, or a folder inside it, holds registered files
    pub has_registered_files: bool,
}

/// A file on disk, with its registration if it's registered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderFile {
    pub disk: DiskFile,
    pub file: Option<FileRecord>,
}

pub struct Media {
    store: Arc<Store>,
    library: Library,
}

impl Media {
    pub fn new(store: Arc<Store>, library: Library) -> Media {
        Media { store, library }
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    /// A path relative to the database's folder, resolved.
    pub fn normalize_path(&self, path: &str) -> String {
        self.library.normalize(path)
    }

    /// Reads the files at these paths that aren't registered yet. It writes
    /// nothing, and reads the files without holding the connection.
    pub fn prepare_files(&self, paths: &[String]) -> Result<PreparedFiles> {
        let wanted = self.unique_paths(paths);
        let known = registered(&self.store.conn(), &wanted)?;
        let mut fresh = Vec::new();
        for path in wanted.iter().filter(|path| !known.contains_key(*path)) {
            if path.is_empty() {
                return Err(Error::Invalid("That's a folder, not a file".into()));
            }
            let facts = self.library.read_facts(path)?;
            fresh.push(NewFile {
                path: path.clone(),
                file_type: file_type_of(path),
                facts,
            });
        }
        Ok(PreparedFiles {
            existing: known.into_values().collect(),
            fresh,
        })
    }

    /// Inserts prepared files, in a transaction the caller started. Returns
    /// them all, by path.
    pub fn insert_prepared(
        conn: &Connection,
        prepared: PreparedFiles,
    ) -> Result<HashMap<String, FileRecord>> {
        let mut by_path: HashMap<String, FileRecord> = prepared
            .existing
            .into_iter()
            .map(|file| (file.path.clone(), file))
            .collect();
        for file in files::insert(conn, &prepared.fresh)? {
            by_path.insert(file.path.clone(), file);
        }
        Ok(by_path)
    }

    pub fn files_by_ids(&self, ids: &[i64]) -> Result<Vec<FileRecord>> {
        files::get_many(&self.store.conn(), ids)
    }

    pub fn file_links_of(
        &self,
        owner: LinkOwner,
        ids: &[i64],
    ) -> Result<HashMap<i64, Vec<FileLink>>> {
        file_links::of(&self.store.conn(), owner, ids)
    }

    /// An audio file's duration and embedded tags.
    pub fn read_audio(&self, file: &FileRecord) -> Audio {
        audio::read_audio(&self.library.absolute(&file.path))
    }

    pub fn read_audio_at(&self, path: &str) -> Audio {
        audio::read_audio(&self.library.absolute(path))
    }

    /// A page of files.
    pub fn list_files(&self, args: &FileListArgs) -> Result<Page<FileRecord>> {
        let paging = paging_of(&args.list)?;
        let sort = sort_of(
            args.list.sort.as_deref(),
            &[("path", FileSort::Path)],
            "Files",
        )?;
        let query = FileQuery {
            paging,
            sort: sort.column,
            descending: sort.descending,
            search: search_of(args.list.q.as_deref()),
            file_type: args.file_type,
            unused: args.unused,
            without_track: args.without_track,
        };
        let (items, total) = files::list(&self.store.conn(), &query)?;
        Ok(Page { items, total })
    }

    pub fn get_file(&self, id: i64) -> Result<Option<FileRecord>> {
        files::get(&self.store.conn(), id)
    }

    pub fn file_counts(&self) -> Result<FileCounts> {
        files::counts(&self.store.conn())
    }

    pub fn file_uses(&self, id: i64) -> Result<Vec<FileUse>> {
        files::uses(&self.store.conn(), id)
    }

    /// The track each of these files is the audio of, by file id.
    pub fn tracks_of_files(&self, ids: &[i64]) -> Result<HashMap<i64, FileTrack>> {
        files::tracks_of(&self.store.conn(), ids)
    }

    pub fn file_on_disk(&self, file: &FileRecord) -> bool {
        self.library.exists(&file.path)
    }

    pub fn absolute_path(&self, file: &FileRecord) -> PathBuf {
        self.library.absolute(&file.path)
    }

    /// A registered file's absolute path, or `None` for an unknown id. The
    /// file may not be there.
    pub fn file_path(&self, id: i64) -> Result<Option<PathBuf>> {
        let path = files::path(&self.store.conn(), id)?;
        Ok(path.map(|path| self.library.absolute(&path)))
    }

    /// A folder's subfolders and files, marking what's registered.
    pub fn browse_folder(&self, path: &str) -> Result<BrowsedFolder> {
        let folder = self.library.list_folder(path)?;
        let conn = self.store.conn();
        let paths: Vec<String> = folder.files.iter().map(|file| file.path.clone()).collect();
        let known = registered(&conn, &paths)?;
        let folders = folder
            .folders
            .into_iter()
            .map(|sub| {
                Ok(BrowsedSubfolder {
                    has_registered_files: holds_registered_files(&conn, &sub.path)?,
                    name: sub.name,
                    path: sub.path,
                })
            })
            .collect::<Result<_>>()?;
        Ok(BrowsedFolder {
            path: folder.path,
            absolute_path: folder.absolute_path,
            parent: folder.parent,
            folders,
            files: with_registrations(folder.files, &known),
        })
    }

    /// Every file in a folder and its subfolders, with its registration.
    pub fn scan_folder(&self, path: &str) -> Result<Vec<FolderFile>> {
        let found = self.library.scan_folder(path)?;
        let paths: Vec<String> = found.iter().map(|file| file.path.clone()).collect();
        let known = registered(&self.store.conn(), &paths)?;
        Ok(with_registrations(found, &known))
    }

    /// Changes a file's type, or its path to relink a file that moved, in
    /// which case its size, mtime and hash are read again.
    pub fn update_file(
        &self,
        id: i64,
        path: Option<&str>,
        file_type: Option<FileType>,
    ) -> Result<FileRecord> {
        let no_file = || Error::NotFound(format!("There's no file {id}"));
        let file = self.get_file(id)?.ok_or_else(no_file)?;
        let mut changes = FileChanges::default();
        if let Some(path) = path {
            let path = self.library.normalize(path);
            if path != file.path {
                let taken = files::find_by_paths(&self.store.conn(), std::slice::from_ref(&path))?;
                if let Some(taken) = taken.first() {
                    return Err(Error::Invalid(format!(
                        "File {} is already registered at {path}",
                        taken.id
                    )));
                }
                // Without the connection
                changes.facts = Some(self.library.read_facts(&path)?);
                changes.path = Some(path);
            }
        }
        changes.file_type = file_type;
        files::update(&self.store.conn(), id, &changes)?.ok_or_else(no_file)
    }

    /// Registers files on disk: reads their size, mtime and hash, and types
    /// them by extension. Paths already registered are returned as they are.
    /// They come back in the order given, each once.
    pub fn register_files(&self, paths: &[String]) -> Result<Vec<FileRecord>> {
        let prepared = self.prepare_files(paths)?;
        let mut by_path = self
            .store
            .transaction(|tx| Media::insert_prepared(tx, prepared))?;
        Ok(self
            .unique_paths(paths)
            .iter()
            .filter_map(|path| by_path.remove(path))
            .collect())
    }

    /// Deletes a file's record. The file on disk isn't touched. It fails
    /// while anything uses the file, and says what.
    pub fn delete_file(&self, id: i64) -> Result<()> {
        let conn = self.store.conn();
        let uses = files::uses(&conn, id)?;
        if !uses.is_empty() {
            let uses: Vec<String> = uses.iter().map(describe_use).collect();
            return Err(Error::Invalid(format!(
                "The file is still used: {}",
                uses.join(", ")
            )));
        }
        files::delete(&conn, id)
    }

    /// The paths, normalized, each once, in the order given.
    fn unique_paths(&self, paths: &[String]) -> Vec<String> {
        let mut seen = HashSet::new();
        paths
            .iter()
            .map(|path| self.library.normalize(path))
            .filter(|path| seen.insert(path.clone()))
            .collect()
    }
}

/// Registered files at these paths, by path.
fn registered(conn: &Connection, paths: &[String]) -> Result<HashMap<String, FileRecord>> {
    Ok(files::find_by_paths(conn, paths)?
        .into_iter()
        .map(|file| (file.path.clone(), file))
        .collect())
}

/// Whether any registered file is in the folder or its subfolders.
fn holds_registered_files(conn: &Connection, folder: &str) -> Result<bool> {
    // A folder at or above the database's folder holds everything but the
    // paths that climb out of it: "../.." holds all but "../../../…"
    if folder.is_empty() || folder.split('/').all(|part| part == "..") {
        let ups = if folder.is_empty() {
            0
        } else {
            folder.split('/').count()
        };
        return files::has_path_starting_with(conn, &"../".repeat(ups + 1), true);
    }
    files::has_path_starting_with(conn, &format!("{folder}/"), false)
}

fn with_registrations(
    found: Vec<DiskFile>,
    known: &HashMap<String, FileRecord>,
) -> Vec<FolderFile> {
    found
        .into_iter()
        .map(|disk| FolderFile {
            file: known.get(&disk.path).cloned(),
            disk,
        })
        .collect()
}

fn describe_use(file_use: &FileUse) -> String {
    let what = match file_use.kind {
        FileUseKind::TrackAudio => "the audio of track",
        FileUseKind::ReleaseCover => "the cover of",
        FileUseKind::ArtistPicture => "the picture of",
        FileUseKind::LabelPicture => "the logo of",
        FileUseKind::ReleaseLink => "linked to release",
        FileUseKind::ArtistLink => "linked to artist",
        FileUseKind::TrackLink => "linked to track",
    };
    format!("{what} \"{}\"", file_use.name)
}
