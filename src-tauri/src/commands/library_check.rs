//! The library check's commands: running it, and its fixes, relinking files
//! that moved and reading changed ones again.

use serde::{Deserialize, Serialize};
use slunk::media::{self as domain, FileMove};
use specta::Type;
use tauri::AppHandle;

use super::catalog::releases::ReleaseHeader;
use super::media::{File, FileDetails, FileUse, FolderFile};
use super::on_library;
use crate::error::CommandError;
use crate::library::Library;

/// A registered file that's gone, and where it may have moved.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MissingFile {
    /// With what uses it
    pub file: FileDetails,
    /// Unregistered files with the same content (size and hash), where it
    /// may have moved. Those with the same name come first.
    pub moved_to: Vec<String>,
    /// Where it most likely moved: the only candidate, or the only one with
    /// the same name. Null when that's unclear, or nothing matched.
    pub likely_move: Option<String>,
}

/// A registered file whose size or mtime changed since it was read.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    /// As it was read
    pub file: File,
    /// The size on disk now
    pub size_bytes: i64,
    /// The mtime on disk now, in epoch seconds
    pub mtime: i64,
}

/// A folder's files that aren't registered.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnregisteredFolder {
    /// `""` for the database's folder
    pub path: String,
    pub files: Vec<FolderFile>,
}

/// A track nobody performs.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnperformedTrack {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub cd_number: i64,
    pub track_number: i64,
    pub release: ReleaseHeader,
}

/// The library check's results, by kind of problem.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryReport {
    /// How many registered files were checked
    pub file_count: i64,
    /// Registered files that aren't on disk anymore
    pub missing_files: Vec<MissingFile>,
    /// Registered files whose size or mtime changed since they were read
    pub changed_files: Vec<ChangedFile>,
    /// Files that aren't registered, in folders that hold registered files
    pub unregistered_folders: Vec<UnregisteredFolder>,
    pub releases_without_artists: Vec<ReleaseHeader>,
    /// Tracks on various-artists releases with no artist of their own but
    /// features, so nobody performs them
    pub tracks_without_performers: Vec<UnperformedTrack>,
}

/// A file that moved, and its new path.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileMoveInput {
    pub file_id: i64,
    /// Relative to the database's folder, or absolute
    pub path: String,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelinkFilesArgs {
    pub moves: Vec<FileMoveInput>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RehashFilesArgs {
    pub ids: Vec<i64>,
}

/// Runs the library check: reads every registered file's size and mtime,
/// and lists the folders they're in. When files are missing, it also scans
/// the library and hashes files of the same size, to find where they moved.
/// Nothing is written.
#[tauri::command]
#[specta::specta]
pub async fn library_check(app: AppHandle) -> Result<LibraryReport, CommandError> {
    on_library(app, run).await
}

pub fn run(library: &Library) -> Result<LibraryReport, CommandError> {
    let report = library.library_check.run()?;
    let catalog = &library.catalog;
    let release_ids: Vec<i64> = report
        .tracks_without_performers
        .iter()
        .map(|track| track.release_id)
        .collect();
    let releases = catalog.releases_by_ids(&release_ids)?;
    let header = |release: &slunk::catalog::Release| ReleaseHeader {
        id: release.id,
        title: release.title.clone(),
        title_language: release.title_language.clone(),
    };
    let missing_files = report
        .files
        .missing
        .into_iter()
        .map(|missing| {
            Ok(MissingFile {
                file: FileDetails {
                    uses: library
                        .media
                        .file_uses(missing.file.id)?
                        .iter()
                        .map(FileUse::from)
                        .collect(),
                    on_disk: false,
                    file: File::from(&missing.file),
                },
                moved_to: missing.moved_to,
                likely_move: missing.likely_move,
            })
        })
        .collect::<Result<_, CommandError>>()?;
    Ok(LibraryReport {
        file_count: report.files.file_count,
        missing_files,
        changed_files: report
            .files
            .changed
            .into_iter()
            .map(|changed| ChangedFile {
                file: File::from(&changed.file),
                size_bytes: changed.size_bytes,
                mtime: changed.mtime,
            })
            .collect(),
        unregistered_folders: report
            .files
            .unregistered
            .into_iter()
            .map(|folder| UnregisteredFolder {
                path: folder.path,
                files: folder
                    .files
                    .into_iter()
                    .map(|disk| FolderFile::from(domain::FolderFile { disk, file: None }))
                    .collect(),
            })
            .collect(),
        releases_without_artists: report.releases_without_artists.iter().map(header).collect(),
        tracks_without_performers: report
            .tracks_without_performers
            .into_iter()
            .filter_map(|track| {
                let release = releases
                    .iter()
                    .find(|release| release.id == track.release_id)?;
                Some(UnperformedTrack {
                    release: header(release),
                    id: track.id,
                    title: track.title,
                    title_language: track.title_language,
                    cd_number: track.cd_number,
                    track_number: track.track_number,
                })
            })
            .collect(),
    })
}

/// Points files that moved at their new paths, reading each one again, one
/// at a time: a move that fails leaves the ones before it saved.
#[tauri::command]
#[specta::specta]
pub async fn relink_files(
    app: AppHandle,
    args: RelinkFilesArgs,
) -> Result<Vec<File>, CommandError> {
    on_library(app, move |library| relink(library, args)).await
}

pub fn relink(library: &Library, args: RelinkFilesArgs) -> Result<Vec<File>, CommandError> {
    let moves: Vec<FileMove> = args
        .moves
        .into_iter()
        .map(|change| FileMove {
            file_id: change.file_id,
            path: change.path,
        })
        .collect();
    Ok(library
        .library_check
        .relink_files(&moves)?
        .iter()
        .map(File::from)
        .collect())
}

/// Reads changed files again: their size, mtime and hash, and the duration
/// of the track each one is the audio of. A file that's gone fails the call
/// before anything is saved.
#[tauri::command]
#[specta::specta]
pub async fn rehash_files(
    app: AppHandle,
    args: RehashFilesArgs,
) -> Result<Vec<File>, CommandError> {
    on_library(app, move |library| rehash(library, args)).await
}

pub fn rehash(library: &Library, args: RehashFilesArgs) -> Result<Vec<File>, CommandError> {
    Ok(library
        .library_check
        .rehash_files(&args.ids)?
        .iter()
        .map(File::from)
        .collect())
}
