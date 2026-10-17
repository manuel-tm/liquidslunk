//! The media context's commands (slunk's `media.graphql`): registered files,
//! the library on disk, and opening files (design §5.5).
//!
//! Each command is a thin async wrapper over a function that takes the open
//! library, which the write parity script calls too.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use slunk::media::{self as domain, FileListArgs, FileRecord, paths};
use slunk::paging::ListArgs;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::{blocking, on_library};
use crate::error::CommandError;
use crate::library::{Libraries, Library};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Music,
    Image,
    Other,
}

impl From<domain::FileType> for FileType {
    fn from(file_type: domain::FileType) -> Self {
        match file_type {
            domain::FileType::Music => FileType::Music,
            domain::FileType::Image => FileType::Image,
            domain::FileType::Other => FileType::Other,
        }
    }
}

impl From<FileType> for domain::FileType {
    fn from(file_type: FileType) -> Self {
        match file_type {
            FileType::Music => domain::FileType::Music,
            FileType::Image => domain::FileType::Image,
            FileType::Other => domain::FileType::Other,
        }
    }
}

/// A file the user registered. liquidslunk only reads it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub id: i64,
    /// Relative to the database's folder
    pub path: String,
    /// The last part of the path
    pub name: String,
    #[serde(rename = "type")]
    pub file_type: FileType,
    pub size_bytes: i64,
    /// When the file was last modified, in epoch seconds, as read when it was
    /// registered or relinked
    pub mtime: i64,
    pub sha256: String,
    /// Epoch seconds
    pub inserted_at: i64,
    /// Epoch seconds
    pub updated_at: i64,
}

impl From<&FileRecord> for File {
    fn from(file: &FileRecord) -> Self {
        File {
            id: file.id,
            name: paths::basename(&file.path).to_string(),
            path: file.path.clone(),
            file_type: file.file_type.into(),
            size_bytes: file.size_bytes,
            mtime: file.mtime,
            sha256: file.sha256.clone(),
            inserted_at: file.inserted_at,
            updated_at: file.updated_at,
        }
    }
}

/// A file in a list, with the track it's the audio of.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileListItem {
    pub id: i64,
    pub path: String,
    #[serde(rename = "type")]
    pub file_type: FileType,
    pub size_bytes: i64,
    pub mtime: i64,
    pub sha256: String,
    pub inserted_at: i64,
    pub updated_at: i64,
    pub track: Option<FileTrack>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileTrack {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FilePage {
    pub items: Vec<FileListItem>,
    pub total: i64,
}

/// A file, with whether it's there and what uses it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileDetails {
    #[serde(flatten)]
    pub file: File,
    /// Whether there's still a file at the path
    pub on_disk: bool,
    /// Everything that uses the file. While anything does, it can't be
    /// deleted.
    pub uses: Vec<FileUse>,
}

/// Something that uses a file.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileUse {
    pub kind: FileUseKind,
    /// The id of the track, release, artist or label that uses the file
    pub entity_id: i64,
    /// Its title or name
    pub name: String,
    /// A file link's note
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FileUseKind {
    TrackAudio,
    ReleaseCover,
    ArtistPicture,
    LabelPicture,
    ReleaseLink,
    ArtistLink,
    TrackLink,
}

impl From<&domain::FileUse> for FileUse {
    fn from(file_use: &domain::FileUse) -> Self {
        use domain::FileUseKind as Kind;
        FileUse {
            kind: match file_use.kind {
                Kind::TrackAudio => FileUseKind::TrackAudio,
                Kind::ReleaseCover => FileUseKind::ReleaseCover,
                Kind::ArtistPicture => FileUseKind::ArtistPicture,
                Kind::LabelPicture => FileUseKind::LabelPicture,
                Kind::ReleaseLink => FileUseKind::ReleaseLink,
                Kind::ArtistLink => FileUseKind::ArtistLink,
                Kind::TrackLink => FileUseKind::TrackLink,
            },
            entity_id: file_use.entity_id,
            name: file_use.name.clone(),
            note: file_use.note.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileCounts {
    pub total: i64,
    /// Files nothing uses yet
    pub unused: i64,
}

/// A folder, for browsing.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    /// Relative to the database's folder: `""` is that folder, and `..` goes
    /// up.
    pub path: String,
    pub absolute_path: String,
    /// The folder above, or null at the top of the disk
    pub parent: Option<String>,
    pub folders: Vec<Subfolder>,
    pub files: Vec<FolderFile>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Subfolder {
    pub name: String,
    pub path: String,
    /// Whether it, or a folder inside it, holds registered files
    pub has_registered_files: bool,
}

/// A file on disk, registered or not.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderFile {
    pub name: String,
    pub path: String,
    /// From the extension
    #[serde(rename = "type")]
    pub file_type: FileType,
    pub size_bytes: i64,
    /// The file's registration, if it's registered
    pub file: Option<FileRef>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileRef {
    pub id: i64,
}

impl From<domain::FolderFile> for FolderFile {
    fn from(found: domain::FolderFile) -> Self {
        FolderFile {
            name: found.disk.name,
            path: found.disk.path,
            file_type: found.disk.file_type.into(),
            size_bytes: found.disk.size_bytes,
            file: found.file.map(|file| FileRef { id: file.id }),
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileArgs {
    pub id: i64,
}

#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FilesArgs {
    /// From 1. Default 1.
    #[specta(optional)]
    pub page: Option<i64>,
    /// Default 50, at most 200
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// A column, `id` or `path`, with `-` in front for descending. Default
    /// `-id`, newest first.
    #[specta(optional)]
    pub sort: Option<String>,
    /// Part of the path, ignoring case
    #[specta(optional)]
    pub q: Option<String>,
    #[serde(rename = "type")]
    #[specta(optional)]
    pub file_type: Option<FileType>,
    /// Only files nothing uses yet
    #[specta(optional)]
    pub unused: Option<bool>,
    /// Only files that aren't a track's audio yet, to create tracks from
    #[specta(optional)]
    pub without_track: Option<bool>,
}

#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FolderArgs {
    /// Relative to the database's folder. Default `""`, that folder.
    #[specta(optional)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderFilesArgs {
    pub path: String,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RegisterFilesArgs {
    pub paths: Vec<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFileArgs {
    pub id: i64,
    /// A new path relinks a file that moved: its size, mtime and hash are
    /// read again
    #[serde(default)]
    #[specta(optional)]
    pub path: Option<String>,
    #[serde(default, rename = "type")]
    #[specta(optional)]
    pub file_type: Option<FileType>,
}

/// A page of registered files.
#[tauri::command]
#[specta::specta]
pub async fn files(app: AppHandle, args: FilesArgs) -> Result<FilePage, CommandError> {
    on_library(app, move |library| list_files(library, args)).await
}

pub fn list_files(library: &Library, args: FilesArgs) -> Result<FilePage, CommandError> {
    let page = library.media.list_files(&FileListArgs {
        list: ListArgs {
            page: args.page,
            per_page: args.per_page,
            sort: args.sort,
            q: args.q,
        },
        file_type: args.file_type.map(Into::into),
        unused: args.unused.unwrap_or(false),
        without_track: args.without_track.unwrap_or(false),
    })?;
    let ids: Vec<i64> = page.items.iter().map(|file| file.id).collect();
    let mut tracks = library.media.tracks_of_files(&ids)?;
    let items = page
        .items
        .into_iter()
        .map(|file| FileListItem {
            track: tracks.remove(&file.id).map(|track| FileTrack {
                id: track.id,
                title: track.title,
            }),
            id: file.id,
            file_type: file.file_type.into(),
            path: file.path,
            size_bytes: file.size_bytes,
            mtime: file.mtime,
            sha256: file.sha256,
            inserted_at: file.inserted_at,
            updated_at: file.updated_at,
        })
        .collect();
    Ok(FilePage {
        items,
        total: page.total,
    })
}

/// A registered file, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn file(app: AppHandle, args: FileArgs) -> Result<Option<FileDetails>, CommandError> {
    on_library(app, move |library| file_details(library, args)).await
}

pub fn file_details(
    library: &Library,
    args: FileArgs,
) -> Result<Option<FileDetails>, CommandError> {
    let Some(file) = library.media.get_file(args.id)? else {
        return Ok(None);
    };
    let uses = library.media.file_uses(file.id)?;
    Ok(Some(FileDetails {
        on_disk: library.media.file_on_disk(&file),
        uses: uses.iter().map(FileUse::from).collect(),
        file: File::from(&file),
    }))
}

/// How many files are registered, and how many nothing uses.
#[tauri::command]
#[specta::specta]
pub async fn file_counts(app: AppHandle) -> Result<FileCounts, CommandError> {
    on_library(app, counts).await
}

pub fn counts(library: &Library) -> Result<FileCounts, CommandError> {
    let counts = library.media.file_counts()?;
    Ok(FileCounts {
        total: counts.total,
        unused: counts.unused,
    })
}

/// A folder's subfolders and the files directly in it, for browsing. Hidden
/// files are left out.
#[tauri::command]
#[specta::specta]
pub async fn folder(app: AppHandle, args: FolderArgs) -> Result<Folder, CommandError> {
    on_library(app, move |library| browse_folder(library, args)).await
}

pub fn browse_folder(library: &Library, args: FolderArgs) -> Result<Folder, CommandError> {
    let folder = library
        .media
        .browse_folder(args.path.as_deref().unwrap_or(""))?;
    Ok(Folder {
        path: folder.path,
        absolute_path: folder.absolute_path,
        parent: folder.parent,
        folders: folder
            .folders
            .into_iter()
            .map(|sub| Subfolder {
                name: sub.name,
                path: sub.path,
                has_registered_files: sub.has_registered_files,
            })
            .collect(),
        files: folder.files.into_iter().map(FolderFile::from).collect(),
    })
}

/// Every file in a folder and its subfolders, by path.
#[tauri::command]
#[specta::specta]
pub async fn folder_files(
    app: AppHandle,
    args: FolderFilesArgs,
) -> Result<Vec<FolderFile>, CommandError> {
    on_library(app, move |library| scan_folder(library, args)).await
}

pub fn scan_folder(
    library: &Library,
    args: FolderFilesArgs,
) -> Result<Vec<FolderFile>, CommandError> {
    let found = library.media.scan_folder(&args.path)?;
    Ok(found.into_iter().map(FolderFile::from).collect())
}

/// Registers files on disk: reads their size, mtime and hash, and types them
/// by extension. Paths already registered are returned as they are.
#[tauri::command]
#[specta::specta]
pub async fn register_files(
    app: AppHandle,
    args: RegisterFilesArgs,
) -> Result<Vec<File>, CommandError> {
    on_library(app, move |library| register(library, args)).await
}

pub fn register(library: &Library, args: RegisterFilesArgs) -> Result<Vec<File>, CommandError> {
    let files = library.media.register_files(&args.paths)?;
    Ok(files.iter().map(File::from).collect())
}

/// Changes a file's type, or its path to relink a file that moved, in which
/// case its size, mtime and hash are read again.
#[tauri::command]
#[specta::specta]
pub async fn update_file(app: AppHandle, args: UpdateFileArgs) -> Result<File, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(library: &Library, args: UpdateFileArgs) -> Result<File, CommandError> {
    let file = library.media.update_file(
        args.id,
        args.path.as_deref(),
        args.file_type.map(Into::into),
    )?;
    Ok(File::from(&file))
}

/// Deletes a file's record, and returns its id. The file on disk isn't
/// touched. It fails while anything uses the file.
#[tauri::command]
#[specta::specta]
pub async fn delete_file(app: AppHandle, args: FileArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: FileArgs) -> Result<i64, CommandError> {
    library.media.delete_file(args.id)?;
    Ok(args.id)
}

/// Opens a registered file in its default app.
#[tauri::command]
#[specta::specta]
pub async fn open_file(app: AppHandle, args: FileArgs) -> Result<(), CommandError> {
    blocking(move || {
        let path = on_disk(&app, args.id)?;
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}

/// Shows a registered file in Finder.
#[tauri::command]
#[specta::specta]
pub async fn reveal_file(app: AppHandle, args: FileArgs) -> Result<(), CommandError> {
    blocking(move || {
        let path = on_disk(&app, args.id)?;
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}

/// A registered file's absolute path, if it's there.
fn on_disk(app: &AppHandle, id: i64) -> Result<PathBuf, CommandError> {
    let library = app.state::<Libraries>().library()?;
    match library.media.file_path(id)? {
        Some(path) if path.exists() => Ok(path),
        Some(path) => Err(CommandError::not_found(format!(
            "There's no file at {}.",
            path.display()
        ))),
        None => Err(CommandError::not_found("There's no such file.")),
    }
}
