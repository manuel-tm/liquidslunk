//! The records the media context reads and writes, which `repo::files` and
//! `repo::file_links` store.

use super::file_types::FileType;
use super::library::FileFacts;
use crate::paging::Paging;

/// A file the user registered. liquidslunk only reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub id: i64,
    /// Relative to the database's folder (slunk's design §4.1)
    pub path: String,
    pub file_type: FileType,
    pub size_bytes: i64,
    /// Epoch seconds
    pub mtime: i64,
    pub sha256: String,
    pub inserted_at: i64,
    pub updated_at: i64,
}

/// A file to register: its path and type, and what was read from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFile {
    pub path: String,
    pub file_type: FileType,
    pub facts: FileFacts,
}

/// What an update changes. Facts go with a new path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileChanges {
    pub path: Option<String>,
    pub file_type: Option<FileType>,
    pub facts: Option<FileFacts>,
}

/// A row that uses a file, which keeps the file from being deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileUse {
    pub kind: FileUseKind,
    /// The track, release, artist or label
    pub entity_id: i64,
    pub name: String,
    /// A file link's note
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileUseKind {
    TrackAudio,
    ReleaseCover,
    ArtistPicture,
    LabelPicture,
    ReleaseLink,
    ArtistLink,
    TrackLink,
}

impl FileUseKind {
    /// As `uses()` selects it.
    pub fn parse(kind: &str) -> Option<FileUseKind> {
        Some(match kind {
            "track_audio" => FileUseKind::TrackAudio,
            "release_cover" => FileUseKind::ReleaseCover,
            "artist_picture" => FileUseKind::ArtistPicture,
            "label_picture" => FileUseKind::LabelPicture,
            "release_link" => FileUseKind::ReleaseLink,
            "artist_link" => FileUseKind::ArtistLink,
            "track_link" => FileUseKind::TrackLink,
            _ => return None,
        })
    }
}

/// What a page of files is, once the arguments are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileQuery {
    pub paging: Paging,
    /// `None` is `id`.
    pub sort: Option<FileSort>,
    pub descending: bool,
    /// Part of the path, ignoring case
    pub search: Option<String>,
    pub file_type: Option<FileType>,
    /// Only files nothing uses
    pub unused: bool,
    /// Only files that aren't a track's audio
    pub without_track: bool,
}

/// The columns files can be sorted by, besides `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileSort {
    Path,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileCounts {
    pub total: i64,
    /// Files nothing uses yet
    pub unused: i64,
}

/// The track a file is the audio of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTrack {
    pub id: i64,
    pub title: String,
}

/// What a file link belongs to (slunk's design §4.3, File links).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkOwner {
    Release,
    Artist,
    Track,
}

/// A file linked to a release, an artist or a track, with an optional note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileLink {
    pub file_id: i64,
    pub note: Option<String>,
}
