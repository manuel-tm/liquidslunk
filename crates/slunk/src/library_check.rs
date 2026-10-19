//! The library check: what changed on disk since files
//! were registered, and gaps in the catalog the database can't rule out.
//! Running it writes nothing; the fixes are separate calls. It joins the
//! media context, which reads the disk, and the catalog, which knows what
//! a file's change means for a track.

use std::sync::Arc;

use crate::Result;
use crate::catalog::{Catalog, Release, Track};
use crate::media::{FileCheck, FileMove, FileRecord, Media};

/// What the check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryReport {
    pub files: FileCheck,
    /// Releases with no artist, by title
    pub releases_without_artists: Vec<Release>,
    /// Tracks on various-artists releases that nobody performs
    pub tracks_without_performers: Vec<Track>,
}

pub struct LibraryCheck {
    media: Arc<Media>,
    catalog: Arc<Catalog>,
}

impl LibraryCheck {
    pub fn new(media: Arc<Media>, catalog: Arc<Catalog>) -> LibraryCheck {
        LibraryCheck { media, catalog }
    }

    /// Checks every registered file and the catalog's gaps.
    pub fn run(&self) -> Result<LibraryReport> {
        Ok(LibraryReport {
            files: self.media.check_files()?,
            releases_without_artists: self.catalog.releases_without_artists()?,
            tracks_without_performers: self.catalog.tracks_without_performers()?,
        })
    }

    /// Points files that moved at their new paths, reading each one again.
    pub fn relink_files(&self, moves: &[FileMove]) -> Result<Vec<FileRecord>> {
        self.media.relink_files(moves)
    }

    /// Reads changed files again: their size, mtime and hash, and the
    /// duration of the track whose audio each one is.
    pub fn rehash_files(&self, ids: &[i64]) -> Result<Vec<FileRecord>> {
        let files = self.media.rehash_files(ids)?;
        let ids: Vec<i64> = files.iter().map(|file| file.id).collect();
        self.catalog.read_durations(&ids)?;
        Ok(files)
    }
}
