//! Artists, with their alternate names, pictures and file links.

use serde::{Deserialize, Serialize};
use slunk::catalog as domain;
use specta::Type;
use tauri::AppHandle;

use super::{FileLink, FileLinkInput, IdArgs, OptionsArgs, PageArgs, SetPictureArgs, files_by_id};
use crate::commands::media::{File, FileRef};
use crate::commands::on_library;
use crate::error::CommandError;
use crate::library::Library;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: i64,
    /// The main name
    pub name: String,
    /// The main name's language: a BCP 47 tag, like ja or ja-Latn. Null when
    /// it isn't known.
    pub name_language: Option<String>,
    /// The primary image. Other images are file links.
    pub picture: Option<FileRef>,
    /// Epoch seconds
    pub inserted_at: i64,
    /// Epoch seconds
    pub updated_at: i64,
}

impl From<domain::Artist> for Artist {
    fn from(artist: domain::Artist) -> Self {
        Artist {
            id: artist.id,
            name: artist.name,
            name_language: artist.name_language,
            picture: artist.picture.map(|id| FileRef { id }),
            inserted_at: artist.inserted_at,
            updated_at: artist.updated_at,
        }
    }
}

/// A name an artist goes by, besides its main one: another spelling (ALIKA),
/// a name in another script (福居良), a real name.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistName {
    pub id: i64,
    /// As written. Compared exactly, case included.
    pub name: String,
    /// A BCP 47 tag. Null when it isn't known.
    pub language: Option<String>,
    /// A legal or birth name
    pub real_name: bool,
    pub note: Option<String>,
}

impl From<domain::AlternateName> for ArtistName {
    fn from(name: domain::AlternateName) -> Self {
        ArtistName {
            id: name.id,
            name: name.name,
            language: name.language,
            real_name: name.real_name,
            note: name.note,
        }
    }
}

/// An alternate name, with the links credited with it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UsedArtistName {
    #[serde(flatten)]
    pub name: ArtistName,
    /// Artist links credited with it: as a release artist, a track artist or
    /// feature, or on a credit line.
    pub uses: i64,
}

/// An artist in a list or a picker, with its alternate names, in order.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistListItem {
    #[serde(flatten)]
    pub artist: Artist,
    pub alternate_names: Vec<ArtistName>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistPage {
    pub items: Vec<ArtistListItem>,
    pub total: i64,
}

/// An artist, as its form edits it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistDetails {
    pub id: i64,
    pub name: String,
    pub name_language: Option<String>,
    /// Artist links that show the main name: the ones a rename changes,
    /// unless it keeps the old name on them.
    pub main_name_uses: i64,
    pub alternate_names: Vec<UsedArtistName>,
    pub picture: Option<File>,
    pub files: Vec<FileLink>,
}

/// An artist to save.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInput {
    pub name: String,
    /// A BCP 47 tag, stored in canonical form: JA-latn becomes ja-Latn.
    /// Blank means unknown.
    #[serde(default)]
    #[specta(optional)]
    pub name_language: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub picture_id: Option<i64>,
    /// Replaces the file links. Default none.
    #[serde(default)]
    #[specta(optional)]
    pub files: Option<Vec<FileLinkInput>>,
    /// Replaces the alternate names, in order. Left out, they stay as they
    /// are, except one the main name is renamed to: it's promoted, and goes
    /// unless it's a real name. Removing a name that's credited credits its
    /// links with the main name.
    #[serde(default)]
    #[specta(optional)]
    pub alternate_names: Option<Vec<ArtistNameInput>>,
    /// On a rename, credits the links that showed the old main name with it,
    /// as an alternate name, so they keep showing it. Default false.
    #[serde(default)]
    #[specta(optional)]
    pub keep_old_name: Option<bool>,
}

impl From<ArtistInput> for domain::ArtistInput {
    fn from(input: ArtistInput) -> Self {
        domain::ArtistInput {
            name: input.name,
            name_language: input.name_language,
            picture_id: input.picture_id,
            files: input
                .files
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            alternate_names: input
                .alternate_names
                .map(|names| names.into_iter().map(Into::into).collect()),
            keep_old_name: input.keep_old_name.unwrap_or(false),
        }
    }
}

/// An alternate name. Names are trimmed, a blank one is left out, and none
/// can be given twice or be the main name, except a real name.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistNameInput {
    /// One of the artist's alternate names. Without an id, the name is added.
    #[serde(default)]
    #[specta(optional)]
    pub id: Option<i64>,
    pub name: String,
    #[serde(default)]
    #[specta(optional)]
    pub language: Option<String>,
    /// Default false
    #[serde(default)]
    #[specta(optional)]
    pub real_name: Option<bool>,
    #[serde(default)]
    #[specta(optional)]
    pub note: Option<String>,
}

impl From<ArtistNameInput> for domain::AlternateNameInput {
    fn from(name: ArtistNameInput) -> Self {
        domain::AlternateNameInput {
            id: name.id,
            name: name.name,
            language: name.language,
            real_name: name.real_name.unwrap_or(false),
            note: name.note,
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArtistArgs {
    pub id: i64,
    #[serde(flatten)]
    pub input: ArtistInput,
}

/// Artists, with their alternate names. `q` matches part of the main name or
/// of an alternate name.
#[tauri::command]
#[specta::specta]
pub async fn artists(app: AppHandle, args: PageArgs) -> Result<ArtistPage, CommandError> {
    on_library(app, move |library| list(library, args)).await
}

pub fn list(library: &Library, args: PageArgs) -> Result<ArtistPage, CommandError> {
    let page = library.catalog.list_artists(&args.into())?;
    Ok(ArtistPage {
        items: with_names(library, page.items)?,
        total: page.total,
    })
}

/// An artist, as its form edits it, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn artist(app: AppHandle, args: IdArgs) -> Result<Option<ArtistDetails>, CommandError> {
    on_library(app, move |library| details(library, args)).await
}

pub fn details(library: &Library, args: IdArgs) -> Result<Option<ArtistDetails>, CommandError> {
    let catalog = &library.catalog;
    let Some(artist) = catalog.get_artist(args.id)? else {
        return Ok(None);
    };
    let names = catalog
        .artist_names_of(&[artist.id])?
        .remove(&artist.id)
        .unwrap_or_default();
    let ids: Vec<i64> = names.iter().map(|name| name.id).collect();
    let uses = catalog.artist_name_uses(&ids)?;
    let links = catalog.artist_files(artist.id)?;
    let file_ids: Vec<i64> = links
        .iter()
        .map(|link| link.file_id)
        .chain(artist.picture)
        .collect();
    let files = files_by_id(library, &file_ids)?;
    Ok(Some(ArtistDetails {
        main_name_uses: catalog.main_name_uses(artist.id)?,
        alternate_names: names
            .into_iter()
            .map(|name| UsedArtistName {
                uses: uses.get(&name.id).copied().unwrap_or(0),
                name: name.into(),
            })
            .collect(),
        picture: artist.picture.and_then(|id| files.get(&id).cloned()),
        files: links
            .into_iter()
            .filter_map(|link| {
                Some(FileLink {
                    file: files.get(&link.file_id).cloned()?,
                    note: link.note,
                })
            })
            .collect(),
        id: artist.id,
        name: artist.name,
        name_language: artist.name_language,
    }))
}

/// The first artists with `q` in a name, by name, for a picker.
#[tauri::command]
#[specta::specta]
pub async fn artist_options(
    app: AppHandle,
    args: OptionsArgs,
) -> Result<Vec<ArtistListItem>, CommandError> {
    on_library(app, move |library| options(library, args)).await
}

pub fn options(library: &Library, args: OptionsArgs) -> Result<Vec<ArtistListItem>, CommandError> {
    let page = library.catalog.list_artists(&args.list())?;
    with_names(library, page.items)
}

/// An artist's alternate names, in order, to suggest the name it's credited
/// with. An unknown artist has none.
#[tauri::command]
#[specta::specta]
pub async fn artist_names(app: AppHandle, args: IdArgs) -> Result<Vec<ArtistName>, CommandError> {
    on_library(app, move |library| names(library, args)).await
}

pub fn names(library: &Library, args: IdArgs) -> Result<Vec<ArtistName>, CommandError> {
    let names = library
        .catalog
        .artist_names_of(&[args.id])?
        .remove(&args.id);
    Ok(names
        .unwrap_or_default()
        .into_iter()
        .map(ArtistName::from)
        .collect())
}

/// Creates an artist. Files given by path are registered first, in the same
/// transaction.
#[tauri::command]
#[specta::specta]
pub async fn create_artist(app: AppHandle, args: ArtistInput) -> Result<Artist, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(library: &Library, args: ArtistInput) -> Result<Artist, CommandError> {
    Ok(library.catalog.create_artist(&args.into())?.into())
}

/// Saves an artist, its alternate names and its file links. Files given by
/// path are registered first, in the same transaction.
#[tauri::command]
#[specta::specta]
pub async fn update_artist(app: AppHandle, args: UpdateArtistArgs) -> Result<Artist, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(library: &Library, args: UpdateArtistArgs) -> Result<Artist, CommandError> {
    Ok(library
        .catalog
        .update_artist(args.id, &args.input.into())?
        .into())
}

/// Sets only the picture. A file given by path is registered first, in the
/// same transaction.
#[tauri::command]
#[specta::specta]
pub async fn set_artist_picture(
    app: AppHandle,
    args: SetPictureArgs,
) -> Result<Artist, CommandError> {
    on_library(app, move |library| set_picture(library, args)).await
}

pub fn set_picture(library: &Library, args: SetPictureArgs) -> Result<Artist, CommandError> {
    Ok(library
        .catalog
        .set_artist_picture(args.id, &args.picture.into())?
        .into())
}

/// Deletes an artist, with its alternate names and file links, and returns
/// its id. It fails while the artist is a release or track artist, or
/// credited.
#[tauri::command]
#[specta::specta]
pub async fn delete_artist(app: AppHandle, args: IdArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: IdArgs) -> Result<i64, CommandError> {
    library.catalog.delete_artist(args.id)?;
    Ok(args.id)
}

/// Artists, with their alternate names, loaded in one query.
fn with_names(
    library: &Library,
    artists: Vec<domain::Artist>,
) -> Result<Vec<ArtistListItem>, CommandError> {
    let ids: Vec<i64> = artists.iter().map(|artist| artist.id).collect();
    let mut names = library.catalog.artist_names_of(&ids)?;
    Ok(artists
        .into_iter()
        .map(|artist| ArtistListItem {
            alternate_names: names
                .remove(&artist.id)
                .unwrap_or_default()
                .into_iter()
                .map(ArtistName::from)
                .collect(),
            artist: artist.into(),
        })
        .collect())
}
