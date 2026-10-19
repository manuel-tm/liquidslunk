//! The import's reads (slunk's design §7.9): what a folder suggests for a new
//! release and for its tracks, and the files to link from it. The import's
//! writes are the release's own: `create_release`, `set_release_tracks`,
//! `set_release_files`, and `add_artist_name` for a tag's other names.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slunk::catalog as domain;
use specta::Type;
use tauri::AppHandle;

use super::releases::{self, ReleaseFiles, ReleaseHeader};
use super::{ArtistRef, CreditedArtist, Related};
use crate::commands::media::{FileRef, FolderFile, FolderFilesArgs};
use crate::commands::{media, on_library};
use crate::error::CommandError;
use crate::library::Library;

/// An artist tag, and the artist it names. It's matched in turn by main
/// name, by alternate name, and by either one ignoring case. The first step
/// that finds anyone decides, and finding more than one artist picks none.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistSuggestion {
    /// The name matched: the tag, or its first part when it has " = " in it,
    /// like "Ryo Fukui = 福居良"
    pub name: String,
    /// The artist, when exactly one matches
    pub artist: Option<ArtistRef>,
    /// The name to credit the artist with, when it isn't the main name: the
    /// alternate name it matched, or the tag when only case differs
    pub credited_as: Option<String>,
    /// How many artists matched equally well. More than one means none is
    /// picked.
    pub matches: i64,
    /// The tag's other parts, after " = ", that the artist doesn't have as
    /// names yet, to offer as its alternate names
    pub other_names: Vec<String>,
}

impl From<domain::ArtistSuggestion> for ArtistSuggestion {
    fn from(suggestion: domain::ArtistSuggestion) -> Self {
        ArtistSuggestion {
            name: suggestion.name,
            artist: suggestion.artist.map(|artist| ArtistRef {
                id: artist.id,
                name: artist.name,
                name_language: artist.name_language,
            }),
            credited_as: suggestion.credited_as,
            matches: suggestion.matches,
            other_names: suggestion.other_names,
        }
    }
}

/// A release with the title a folder suggests already.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SameTitle {
    pub id: i64,
    pub title: String,
    /// As credited
    pub artists: Vec<CreditedArtist>,
}

/// What a folder suggests for a new release, from its files' embedded tags,
/// or else its name.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSuggestion {
    /// The album tag, or else the folder's name
    pub title: String,
    pub released: Option<String>,
    /// The album artist tags, or the artist tags
    pub artists: Vec<ArtistSuggestion>,
    /// Releases with that title already, so the folder isn't imported twice
    pub same_title: Vec<SameTitle>,
    pub music: i64,
    pub images: i64,
    pub others: i64,
}

/// The track a file is already the audio of, on any release.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedTrack {
    pub id: i64,
    pub release: ReleaseHeader,
}

/// A row for an audio file in a folder or its subfolders: from its embedded
/// tags, or else its path (`CD2/03 Title.flac` is disc 2, track 3).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackSuggestion {
    pub path: String,
    /// The file's registration, if it's registered
    pub file: Option<FileRef>,
    pub track: Option<SuggestedTrack>,
    pub cd_number: i64,
    /// The disc's title tag: DISCSUBTITLE, or TSST in MP3s
    pub disc_title: Option<String>,
    pub track_number: i64,
    pub title: String,
    pub artists: Vec<ArtistSuggestion>,
    pub duration_ms: Option<i64>,
}

/// A release's cover and links, and the folder's files to link.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportFiles {
    #[serde(flatten)]
    pub release: ReleaseFiles,
    /// Every file in the folder and its subfolders, by path
    pub folder_files: Vec<FolderFile>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSuggestionArgs {
    /// Relative to the database's folder
    pub folder: String,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportFilesArgs {
    pub id: i64,
    pub folder: String,
}

/// What a folder suggests for a new release: the most common album, date
/// and album artist tags, or else the folder's name, the releases with that
/// title already, and how many audio files, images and other files it holds.
#[tauri::command]
#[specta::specta]
pub async fn release_suggestion(
    app: AppHandle,
    args: ReleaseSuggestionArgs,
) -> Result<ReleaseSuggestion, CommandError> {
    on_library(app, move |library| suggest_release(library, args)).await
}

pub fn suggest_release(
    library: &Library,
    args: ReleaseSuggestionArgs,
) -> Result<ReleaseSuggestion, CommandError> {
    let suggestion = library.catalog.suggest_release(&args.folder)?;
    let ids: Vec<i64> = suggestion
        .same_title
        .iter()
        .map(|release| release.id)
        .collect();
    let mut artists = library.catalog.release_artists_of(&ids)?;
    let mut related = Related::default();
    related.load_artists(library, artists.values().flatten(), false)?;
    Ok(ReleaseSuggestion {
        same_title: suggestion
            .same_title
            .into_iter()
            .map(|release| SameTitle {
                artists: related.credited_all(&artists.remove(&release.id).unwrap_or_default()),
                id: release.id,
                title: release.title,
            })
            .collect(),
        title: suggestion.title,
        released: suggestion.released,
        artists: suggestion.artists.into_iter().map(Into::into).collect(),
        music: suggestion.music,
        images: suggestion.images,
        others: suggestion.others,
    })
}

/// The import's rows for a folder's audio files, with the release each one
/// is already a track on, if any.
pub(crate) fn track_suggestions(
    library: &Library,
    folder: &str,
) -> Result<Vec<TrackSuggestion>, CommandError> {
    let rows = library.catalog.suggest_tracks(folder)?;
    let release_ids: Vec<i64> = rows
        .iter()
        .filter_map(|row| row.track.as_ref().map(|track| track.release_id))
        .collect();
    let releases: HashMap<i64, domain::Release> = library
        .catalog
        .releases_by_ids(&release_ids)?
        .into_iter()
        .map(|release| (release.id, release))
        .collect();
    Ok(rows
        .into_iter()
        .map(|row| TrackSuggestion {
            track: row.track.and_then(|track| {
                let release = releases.get(&track.release_id)?;
                Some(SuggestedTrack {
                    id: track.id,
                    release: ReleaseHeader {
                        id: release.id,
                        title: release.title.clone(),
                        title_language: release.title_language.clone(),
                    },
                })
            }),
            file: row.file.map(|file| FileRef { id: file.id }),
            path: row.path,
            cd_number: row.cd_number,
            disc_title: row.disc_title,
            track_number: row.track_number,
            title: row.title,
            artists: row.artists.into_iter().map(Into::into).collect(),
            duration_ms: row.duration_ms,
        })
        .collect())
}

/// The import's files step: the release's cover and links, and the folder's
/// files, or null for an unknown release. A folder that can't be read is an
/// error.
#[tauri::command]
#[specta::specta]
pub async fn import_files(
    app: AppHandle,
    args: ImportFilesArgs,
) -> Result<Option<ImportFiles>, CommandError> {
    on_library(app, move |library| files(library, args)).await
}

pub fn files(
    library: &Library,
    args: ImportFilesArgs,
) -> Result<Option<ImportFiles>, CommandError> {
    let folder_files = media::scan_folder(library, FolderFilesArgs { path: args.folder })?;
    Ok(
        releases::files(library, super::IdArgs { id: args.id })?.map(|release| ImportFiles {
            release,
            folder_files,
        }),
    )
}
