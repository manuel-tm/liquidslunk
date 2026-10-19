//! Artists, with their alternate names, pictures and file links, and the
//! artist page's tabs: releases, appearances and credits.

use serde::{Deserialize, Serialize};
use slunk::catalog::{self as domain, ListKind};
use slunk::media::LinkOwner;
use slunk::paging::ListArgs;
use specta::Type;
use tauri::AppHandle;

use super::releases::{self, ReleaseFiltersArgs, ReleasePage, ReleasesArgs};
use super::{
    CreditedArtist, FileLink, FileLinkInput, IdArgs, NameRef, NamedItem, OptionsArgs, PageArgs,
    Related, SetPictureArgs, files_by_id,
};
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

/// An alternate name to add after the artist's others.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AddArtistNameArgs {
    pub id: i64,
    /// Trimmed. Compared exactly, case included.
    pub name: String,
    /// A BCP 47 tag, stored in canonical form. Blank means unknown.
    #[serde(default)]
    #[specta(optional)]
    pub language: Option<String>,
}

/// Adds an alternate name after the others, like one the import offers from
/// a tag's other parts. Fails if the artist has the name already.
#[tauri::command]
#[specta::specta]
pub async fn add_artist_name(
    app: AppHandle,
    args: AddArtistNameArgs,
) -> Result<Artist, CommandError> {
    on_library(app, move |library| add_name(library, args)).await
}

pub fn add_name(library: &Library, args: AddArtistNameArgs) -> Result<Artist, CommandError> {
    Ok(library
        .catalog
        .add_artist_name(args.id, &args.name, args.language.as_deref())?
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

/// The artist page's tabs, counted.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCounts {
    pub releases: i64,
    pub appearances: i64,
    pub credits: i64,
    /// The picture, and the linked files
    pub files: i64,
}

/// What every tab of the artist page shows above it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistHeader {
    pub id: i64,
    pub name: String,
    pub name_language: Option<String>,
    pub alternate_names: Vec<ArtistName>,
    pub picture: Option<FileRef>,
    /// What keeps it from being deleted, in words
    pub blockers: Vec<String>,
    pub counts: ArtistCounts,
}

/// How many of the artist's releases have a format.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatCount {
    pub format: NamedItem,
    pub count: i64,
}

/// The artist's releases, and how many have each format.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistReleases {
    pub format_counts: Vec<FormatCount>,
    pub releases: ReleasePage,
}

#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistReleasesArgs {
    pub id: i64,
    #[serde(default)]
    #[specta(optional)]
    pub page: Option<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// `title`, `released` or `id`, with `-` in front for descending
    #[serde(default)]
    #[specta(optional)]
    pub sort: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub q: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub format: Option<i64>,
}

/// A release, as a row of the artist's appearances.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseSummary {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub released: Option<String>,
    pub cover: Option<FileRef>,
    pub artists: Vec<CreditedArtist>,
}

/// A track the artist performs or is featured on.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceTrack {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub artists: Vec<CreditedArtist>,
    pub features: Vec<CreditedArtist>,
}

/// A release the artist is on as a track artist only, with those tracks.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Appearance {
    pub release: ReleaseSummary,
    pub tracks: Vec<AppearanceTrack>,
}

/// The release of a credit line.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreditRelease {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub released: Option<String>,
}

/// The track of a credit line.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreditTrack {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
}

/// A credit line the artist is on.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistCredit {
    /// As printed
    pub role: String,
    /// The name the artist is credited with on the line, when it isn't the
    /// main name
    pub credited_as: Option<NameRef>,
    pub release: CreditRelease,
    /// Null for a line on the release itself
    pub track: Option<CreditTrack>,
}

/// What every tab of the artist page shows above it, or null for an unknown
/// id.
#[tauri::command]
#[specta::specta]
pub async fn artist_header(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<ArtistHeader>, CommandError> {
    on_library(app, move |library| header(library, args)).await
}

pub fn header(library: &Library, args: IdArgs) -> Result<Option<ArtistHeader>, CommandError> {
    let catalog = &library.catalog;
    let Some(artist) = catalog.get_artist(args.id)? else {
        return Ok(None);
    };
    let id = artist.id;
    let releases = catalog
        .list_releases(
            &ListArgs {
                per_page: Some(1),
                ..ListArgs::default()
            },
            &domain::ReleaseFilterArgs {
                artist: Some(id),
                ..domain::ReleaseFilterArgs::default()
            },
        )?
        .total;
    let files = library
        .media
        .file_links_of(LinkOwner::Artist, &[id])?
        .remove(&id)
        .unwrap_or_default()
        .len() as i64;
    Ok(Some(ArtistHeader {
        alternate_names: catalog
            .artist_names_of(&[id])?
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(ArtistName::from)
            .collect(),
        blockers: catalog.artist_blockers(id)?,
        counts: ArtistCounts {
            releases,
            appearances: catalog.artist_appearances(id)?.len() as i64,
            credits: catalog.artist_credits(id)?.len() as i64,
            files: files + i64::from(artist.picture.is_some()),
        },
        picture: artist.picture.map(|id| FileRef { id }),
        id,
        name: artist.name,
        name_language: artist.name_language,
    }))
}

/// The releases the artist is a release artist on, and how many have each
/// format.
#[tauri::command]
#[specta::specta]
pub async fn artist_releases(
    app: AppHandle,
    args: ArtistReleasesArgs,
) -> Result<ArtistReleases, CommandError> {
    on_library(app, move |library| releases_of(library, args)).await
}

pub fn releases_of(
    library: &Library,
    args: ArtistReleasesArgs,
) -> Result<ArtistReleases, CommandError> {
    let counts = library.catalog.artist_format_counts(args.id)?;
    let ids: Vec<i64> = counts.iter().map(|count| count.format_id).collect();
    let mut related = Related::default();
    related.load_items(library, ListKind::ReleaseFormats, &ids)?;
    let format_counts = counts
        .iter()
        .filter_map(|count| {
            let format = related
                .items(ListKind::ReleaseFormats, &[count.format_id])
                .pop()?;
            Some(FormatCount {
                format,
                count: count.count,
            })
        })
        .collect();
    let releases = releases::list(
        library,
        ReleasesArgs {
            page: args.page,
            per_page: args.per_page,
            sort: args.sort,
            filters: ReleaseFiltersArgs {
                q: args.q,
                format: args.format,
                artist: Some(args.id),
                ..ReleaseFiltersArgs::default()
            },
        },
    )?;
    Ok(ArtistReleases {
        format_counts,
        releases,
    })
}

/// Releases where the artist performs or is featured on tracks, without
/// being a release artist, with those tracks.
#[tauri::command]
#[specta::specta]
pub async fn artist_appearances(
    app: AppHandle,
    args: IdArgs,
) -> Result<Vec<Appearance>, CommandError> {
    on_library(app, move |library| appearances(library, args)).await
}

pub fn appearances(library: &Library, args: IdArgs) -> Result<Vec<Appearance>, CommandError> {
    let catalog = &library.catalog;
    let appearances = catalog.artist_appearances(args.id)?;
    let release_ids: Vec<i64> = appearances.iter().map(|a| a.release_id).collect();
    let track_ids: Vec<i64> = appearances
        .iter()
        .flat_map(|a| a.track_ids.clone())
        .collect();
    let releases: std::collections::HashMap<i64, domain::Release> = catalog
        .releases_by_ids(&release_ids)?
        .into_iter()
        .map(|release| (release.id, release))
        .collect();
    let tracks: std::collections::HashMap<i64, domain::Track> = catalog
        .tracks_by_ids(&track_ids)?
        .into_iter()
        .map(|track| (track.id, track))
        .collect();
    let release_artists = catalog.release_artists_of(&release_ids)?;
    let track_artists = catalog.track_artists_of(&track_ids)?;
    let link = |artist: &domain::TrackArtist| domain::ArtistLink {
        artist_id: artist.artist_id,
        credited_name_id: artist.credited_name_id,
    };
    let track_links: Vec<domain::ArtistLink> = track_artists.values().flatten().map(link).collect();
    let mut related = Related::default();
    related.load_artists(
        library,
        release_artists.values().flatten().chain(track_links.iter()),
        false,
    )?;
    let own = |track: i64, featured: bool| -> Vec<CreditedArtist> {
        let links: Vec<domain::ArtistLink> = track_artists
            .get(&track)
            .map(|list| {
                list.iter()
                    .filter(|artist| artist.featured == featured)
                    .map(link)
                    .collect()
            })
            .unwrap_or_default();
        related.credited_all(&links)
    };
    Ok(appearances
        .iter()
        .filter_map(|appearance| {
            let release = releases.get(&appearance.release_id)?;
            Some(Appearance {
                release: ReleaseSummary {
                    id: release.id,
                    title: release.title.clone(),
                    title_language: release.title_language.clone(),
                    released: domain::released(release),
                    cover: release.cover_picture.map(|id| FileRef { id }),
                    artists: related.credited_all(
                        release_artists
                            .get(&release.id)
                            .map(Vec::as_slice)
                            .unwrap_or_default(),
                    ),
                },
                tracks: appearance
                    .track_ids
                    .iter()
                    .filter_map(|id| tracks.get(id))
                    .map(|track| AppearanceTrack {
                        id: track.id,
                        title: track.title.clone(),
                        title_language: track.title_language.clone(),
                        artists: own(track.id, false),
                        features: own(track.id, true),
                    })
                    .collect(),
            })
        })
        .collect())
}

/// The credit lines the artist is on.
#[tauri::command]
#[specta::specta]
pub async fn artist_credits(
    app: AppHandle,
    args: IdArgs,
) -> Result<Vec<ArtistCredit>, CommandError> {
    on_library(app, move |library| credits(library, args)).await
}

pub fn credits(library: &Library, args: IdArgs) -> Result<Vec<ArtistCredit>, CommandError> {
    let catalog = &library.catalog;
    let credits = catalog.artist_credits(args.id)?;
    let release_ids: Vec<i64> = credits.iter().map(|credit| credit.release_id).collect();
    let track_ids: Vec<i64> = credits
        .iter()
        .filter_map(|credit| credit.track_id)
        .collect();
    let name_ids: Vec<i64> = credits
        .iter()
        .filter_map(|credit| credit.credited_name_id)
        .collect();
    let releases: std::collections::HashMap<i64, domain::Release> = catalog
        .releases_by_ids(&release_ids)?
        .into_iter()
        .map(|release| (release.id, release))
        .collect();
    let tracks: std::collections::HashMap<i64, domain::Track> = catalog
        .tracks_by_ids(&track_ids)?
        .into_iter()
        .map(|track| (track.id, track))
        .collect();
    let names: std::collections::HashMap<i64, domain::AlternateName> = catalog
        .artist_names_by_ids(&name_ids)?
        .into_iter()
        .map(|name| (name.id, name))
        .collect();
    Ok(credits
        .into_iter()
        .filter_map(|credit| {
            let release = releases.get(&credit.release_id)?;
            Some(ArtistCredit {
                credited_as: credit
                    .credited_name_id
                    .and_then(|id| names.get(&id))
                    .map(|name| NameRef {
                        name: name.name.clone(),
                        language: name.language.clone(),
                    }),
                release: CreditRelease {
                    id: release.id,
                    title: release.title.clone(),
                    title_language: release.title_language.clone(),
                    released: domain::released(release),
                },
                track: credit
                    .track_id
                    .and_then(|id| tracks.get(&id))
                    .map(|track| CreditTrack {
                        id: track.id,
                        title: track.title.clone(),
                        title_language: track.title_language.clone(),
                    }),
                role: credit.role,
            })
        })
        .collect())
}
