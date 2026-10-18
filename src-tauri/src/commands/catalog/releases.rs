//! Releases: the list and its filters, the release page, the edit tabs'
//! reads and writes, the release picker, credit roles and the catalog's
//! totals.

use std::collections::HashSet;

use serde::{Deserialize, Deserializer, Serialize};
use slunk::catalog::{self as domain, CreditOwner, ListKind, ReleaseLink};
use slunk::media::LinkOwner;
use slunk::paging::ListArgs;
use slunk::tagging::{TagOwner, tag_filters};
use specta::Type;
use tauri::AppHandle;

use super::{
    AlternateTitle, Credit, CreditedArtist, FileLink, FileLinkInput, FileRefInput, IdArgs,
    LabelRef, NamedCreditedArtist, NamedItem, OPTIONS, OptionsArgs, Related, files_by_id,
};
use crate::commands::media::{File, FileRef};
use crate::commands::on_library;
use crate::commands::tagging::{TagFilterInput, TagValues, owned};
use crate::error::CommandError;
use crate::library::Library;

/// A release, as a save returns it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub id: i64,
    /// The main title
    pub title: String,
    /// The main title's language, and its discs' and tracks' unless they
    /// have their own: a BCP 47 tag. Null when it isn't known.
    pub title_language: Option<String>,
    /// With only its known parts: `2006`, `2006-01` or `2006-01-10`
    pub released: Option<String>,
    pub catalog_number: Option<String>,
    /// Each track names its own performers: compilations, soundtracks, cast
    /// recordings.
    pub various_artists: bool,
    pub cover: Option<FileRef>,
    /// Epoch seconds
    pub inserted_at: i64,
    /// Epoch seconds
    pub updated_at: i64,
}

impl From<domain::Release> for Release {
    fn from(release: domain::Release) -> Self {
        Release {
            released: domain::released(&release),
            id: release.id,
            title: release.title,
            title_language: release.title_language,
            catalog_number: release.catalog_number,
            various_artists: release.various_artists,
            cover: release.cover_picture.map(|id| FileRef { id }),
            inserted_at: release.inserted_at,
            updated_at: release.updated_at,
        }
    }
}

/// A release in a list: the release list, an artist's or a label's
/// releases, the dashboard's newest.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseListItem {
    #[serde(flatten)]
    pub release: Release,
    pub alternate_titles: Vec<AlternateTitle>,
    /// Who the release is by, in order, as credited
    pub artists: Vec<CreditedArtist>,
    pub formats: Vec<NamedItem>,
    pub media: Vec<NamedItem>,
    pub labels: Vec<LabelRef>,
}

/// A page of releases, and the names of the filters it was asked for, for
/// their pickers. A filter that wasn't asked for, or isn't there, is null.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleasePage {
    pub items: Vec<ReleaseListItem>,
    pub total: i64,
    pub filter_format: Option<NamedItem>,
    pub filter_medium: Option<NamedItem>,
    pub filter_label: Option<NamedItem>,
}

/// A release's title, for the edit tabs' header.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseHeader {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
}

/// A disc's own title, as printed.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscTitle {
    pub cd_number: i64,
    pub title: String,
    /// Null: the release's title language
    pub language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
}

/// A track's audio file.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackFile {
    pub id: i64,
    pub path: String,
}

/// A track, as the release page shows it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTrack {
    pub id: i64,
    pub title: String,
    /// Null: the release's
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    /// Null when it can't be read
    pub duration_ms: Option<i64>,
    pub cd_number: i64,
    pub track_number: i64,
    pub file: TrackFile,
    /// Its own performers, without features, each with its alternate names
    pub artists: Vec<NamedCreditedArtist>,
    /// Guests, shown as "(feat. …)"
    pub features: Vec<CreditedArtist>,
    pub credits: Vec<Credit>,
}

/// The release page: everything it shows.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseView {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    pub released: Option<String>,
    pub catalog_number: Option<String>,
    pub various_artists: bool,
    /// The cover and the linked files
    pub file_count: i64,
    /// The tracks' total playing time, or null with no durations
    pub duration_ms: Option<i64>,
    pub blockers: Vec<String>,
    pub cover: Option<FileRef>,
    /// As credited, each with its alternate names
    pub artists: Vec<NamedCreditedArtist>,
    pub labels: Vec<LabelRef>,
    pub formats: Vec<NamedItem>,
    pub media: Vec<NamedItem>,
    pub credits: Vec<Credit>,
    /// By tag name
    pub tags: Vec<TagValues>,
    /// By disc number. Only discs with a title are listed.
    pub disc_titles: Vec<DiscTitle>,
    /// In order: disc, then number
    pub tracks: Vec<ReleaseTrack>,
}

/// A release, as its Details tab edits it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseDetails {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    pub released: Option<String>,
    pub various_artists: bool,
    pub catalog_number: Option<String>,
    pub artists: Vec<CreditedArtist>,
    pub labels: Vec<LabelRef>,
    pub formats: Vec<NamedItem>,
    pub media: Vec<NamedItem>,
}

/// A release's cover and linked files: its files page and its Files tab.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseFiles {
    pub id: i64,
    pub title: String,
    pub cover: Option<File>,
    pub files: Vec<FileLink>,
}

/// A track's credit lines, as the Credits tab edits them.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackCredits {
    pub id: i64,
    pub cd_number: i64,
    pub track_number: i64,
    pub title: String,
    pub credits: Vec<Credit>,
}

/// A release's credit lines and its tracks', as the Credits tab edits them.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCredits {
    pub id: i64,
    pub title: String,
    pub credits: Vec<Credit>,
    pub tracks: Vec<TrackCredits>,
}

/// A release in the release picker.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseOption {
    pub id: i64,
    pub title: String,
    pub cover: Option<FileRef>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTotals {
    pub releases: i64,
    pub artists: i64,
    pub tracks: i64,
    pub labels: i64,
    /// The tracks' playing time
    pub duration_ms: i64,
}

/// A release list's search and filters. They all have to match.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct ReleaseFiltersArgs {
    /// Part of the title or of an alternate title, ignoring case
    #[specta(optional)]
    pub q: Option<String>,
    #[specta(optional)]
    pub format: Option<i64>,
    #[specta(optional)]
    pub medium: Option<i64>,
    #[specta(optional)]
    pub label: Option<i64>,
    /// Releases the artist is a release artist on
    #[specta(optional)]
    pub artist: Option<i64>,
    /// Every filter has to match: Genre is Rock and Score is 8 or more.
    #[specta(optional)]
    pub tags: Vec<TagFilterInput>,
}

impl ReleaseFiltersArgs {
    /// The filters, with the tag filters checked.
    fn filters(&self) -> Result<domain::ReleaseFilterArgs, CommandError> {
        let tags: Vec<slunk::tagging::TagFilterInput> =
            self.tags.iter().cloned().map(Into::into).collect();
        Ok(domain::ReleaseFilterArgs {
            format: self.format,
            medium: self.medium,
            label: self.label,
            artist: self.artist,
            tags: tag_filters(&tags)?,
        })
    }
}

#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct ReleasesArgs {
    /// From 1. Default 1.
    #[specta(optional)]
    pub page: Option<i64>,
    /// Default 50, at most 200
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// `title`, `released` (undated last) or `id`, with `-` in front for
    /// descending. Default `-id`, newest first.
    #[specta(optional)]
    pub sort: Option<String>,
    #[serde(flatten)]
    pub filters: ReleaseFiltersArgs,
}

/// An alternate title to save.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AlternateTitleInput {
    pub title: String,
    /// A BCP 47 tag, stored in canonical form. Blank means unknown.
    #[serde(default)]
    #[specta(optional)]
    pub language: Option<String>,
    /// On the release as published. Default true.
    #[serde(default)]
    #[specta(optional)]
    pub printed: Option<bool>,
}

impl From<AlternateTitleInput> for domain::AlternateTitleInput {
    fn from(input: AlternateTitleInput) -> Self {
        domain::AlternateTitleInput {
            title: input.title,
            language: input.language,
            printed: input.printed,
        }
    }
}

/// An artist as credited: `creditedAs` is the name printed. Blank, or the
/// artist's main name, means the main name; a name the artist doesn't have
/// yet is added to its alternate names.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreditedArtistInput {
    pub artist_id: i64,
    #[serde(default)]
    #[specta(optional)]
    pub credited_as: Option<String>,
}

impl From<CreditedArtistInput> for domain::CreditedArtistInput {
    fn from(input: CreditedArtistInput) -> Self {
        domain::CreditedArtistInput {
            artist_id: input.artist_id,
            credited_as: input.credited_as,
        }
    }
}

/// A credit line: a role, as printed, and its artists, in order.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreditInput {
    pub role: String,
    pub artists: Vec<CreditedArtistInput>,
}

impl From<CreditInput> for domain::CreditInput {
    fn from(input: CreditInput) -> Self {
        domain::CreditInput {
            role: input.role,
            artists: input.artists.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackCreditsInput {
    pub track_id: i64,
    pub credits: Vec<CreditInput>,
}

/// A release's details, to save.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInput {
    pub title: String,
    /// A BCP 47 tag, stored in canonical form. Blank means unknown. Left
    /// out, an update keeps it as it is.
    #[serde(default)]
    #[specta(optional, type = Option<String>)]
    pub title_language: Given<String>,
    /// Replaces the alternate titles, in order. Left out, they stay as they
    /// are, except one that's now the main title.
    #[serde(default)]
    #[specta(optional)]
    pub alternate_titles: Option<Vec<AlternateTitleInput>>,
    /// `2006`, `2006-01` or `2006-01-10`. Blank means unknown.
    #[serde(default)]
    #[specta(optional)]
    pub released: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub catalog_number: Option<String>,
    /// Default false
    #[serde(default)]
    #[specta(optional)]
    pub various_artists: bool,
    pub artists: Vec<CreditedArtistInput>,
    #[serde(default)]
    #[specta(optional)]
    pub label_ids: Vec<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub format_ids: Vec<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub medium_ids: Vec<i64>,
}

/// A field an update can leave out: left out, it's `Given(None)`, and given,
/// even as null, it's `Given(Some(…))`.
#[derive(Debug, Default)]
pub struct Given<T>(pub Option<Option<T>>);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Given<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(|value| Given(Some(value)))
    }
}

impl From<ReleaseInput> for domain::ReleaseInput {
    fn from(input: ReleaseInput) -> Self {
        domain::ReleaseInput {
            title: input.title,
            title_language: input.title_language.0,
            alternate_titles: input
                .alternate_titles
                .map(|titles| titles.into_iter().map(Into::into).collect()),
            released: input.released,
            catalog_number: input.catalog_number,
            various_artists: input.various_artists,
            artists: input.artists.into_iter().map(Into::into).collect(),
            label_ids: input.label_ids,
            format_ids: input.format_ids,
            medium_ids: input.medium_ids,
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReleaseArgs {
    pub id: i64,
    #[serde(flatten)]
    pub input: ReleaseInput,
}

/// Sets only the cover.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetReleaseCoverArgs {
    pub id: i64,
    /// A file given by path is registered first, in the same transaction.
    pub cover: FileRefInput,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetReleaseFilesArgs {
    pub id: i64,
    /// Null: no cover
    #[serde(default)]
    #[specta(optional)]
    pub cover: Option<FileRefInput>,
    /// Replaces the file links, in order. Files given by path are registered
    /// first.
    pub files: Vec<FileLinkInput>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetReleaseCreditsArgs {
    pub id: i64,
    /// The release's lines, in order
    pub credits: Vec<CreditInput>,
    /// Its tracks' lines. A track left out keeps its own.
    #[serde(default)]
    #[specta(optional)]
    pub tracks: Vec<TrackCreditsInput>,
}

/// Releases, with the filters' names. The filters combine: `format` and
/// `medium` give "EPs on vinyl".
#[tauri::command]
#[specta::specta]
pub async fn releases(app: AppHandle, args: ReleasesArgs) -> Result<ReleasePage, CommandError> {
    on_library(app, move |library| list(library, args)).await
}

pub fn list(library: &Library, args: ReleasesArgs) -> Result<ReleasePage, CommandError> {
    let filters = args.filters.filters()?;
    let list_args = ListArgs {
        page: args.page,
        per_page: args.per_page,
        sort: args.sort,
        q: args.filters.q.clone(),
    };
    let page = library.catalog.list_releases(&list_args, &filters)?;
    let catalog = &library.catalog;
    let item = |kind: ListKind, id: Option<i64>| -> Result<Option<NamedItem>, CommandError> {
        Ok(match id {
            Some(id) => catalog.get_list_item(kind, id)?.map(|item| NamedItem {
                id: item.id,
                name: item.name,
            }),
            None => None,
        })
    };
    let filter_label = match filters.label {
        Some(id) => catalog.get_label(id)?.map(|label| NamedItem {
            id: label.id,
            name: label.name,
        }),
        None => None,
    };
    Ok(ReleasePage {
        items: list_items(library, page.items)?,
        total: page.total,
        filter_format: item(ListKind::ReleaseFormats, filters.format)?,
        filter_medium: item(ListKind::ReleaseMedia, filters.medium)?,
        filter_label,
    })
}

/// Every release the same search and filters as `releases` match, newest
/// first: selecting them all for a bulk edit.
#[tauri::command]
#[specta::specta]
pub async fn release_ids(
    app: AppHandle,
    args: ReleaseFiltersArgs,
) -> Result<Vec<i64>, CommandError> {
    on_library(app, move |library| ids(library, args)).await
}

pub fn ids(library: &Library, args: ReleaseFiltersArgs) -> Result<Vec<i64>, CommandError> {
    let filters = args.filters()?;
    Ok(library.catalog.release_ids(args.q.as_deref(), &filters)?)
}

/// The release page, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn release_page(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<ReleaseView>, CommandError> {
    on_library(app, move |library| page(library, args)).await
}

pub fn page(library: &Library, args: IdArgs) -> Result<Option<ReleaseView>, CommandError> {
    let catalog = &library.catalog;
    let Some(release) = catalog.get_release(args.id)? else {
        return Ok(None);
    };
    let id = release.id;
    let ids = [id];
    let tracks = catalog
        .tracks_of_releases(&ids)?
        .remove(&id)
        .unwrap_or_default();
    let track_ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    let mut track_titles = catalog.track_titles_of(&track_ids)?;
    let mut track_artists = catalog.track_artists_of(&track_ids)?;
    let mut track_credits = catalog.credits_of(CreditOwner::Track, &track_ids)?;
    let file_ids: Vec<i64> = tracks.iter().map(|track| track.file_id).collect();
    let mut files = files_by_id(library, &file_ids)?;
    let artists = catalog
        .release_artists_of(&ids)?
        .remove(&id)
        .unwrap_or_default();
    let credits = catalog
        .credits_of(CreditOwner::Release, &ids)?
        .remove(&id)
        .unwrap_or_default();
    let labels = catalog
        .release_links_of(ReleaseLink::Labels, &ids)?
        .remove(&id)
        .unwrap_or_default();
    let formats = catalog
        .release_links_of(ReleaseLink::Formats, &ids)?
        .remove(&id)
        .unwrap_or_default();
    let media = catalog
        .release_links_of(ReleaseLink::Media, &ids)?
        .remove(&id)
        .unwrap_or_default();
    let links = library
        .media
        .file_links_of(LinkOwner::Release, &ids)?
        .remove(&id)
        .unwrap_or_default();

    let mut related = Related::default();
    let as_links = |list: &[domain::TrackArtist], featured: bool| -> Vec<domain::ArtistLink> {
        list.iter()
            .filter(|artist| artist.featured == featured)
            .map(|artist| domain::ArtistLink {
                artist_id: artist.artist_id,
                credited_name_id: artist.credited_name_id,
            })
            .collect()
    };
    let mut performing: Vec<domain::ArtistLink> = artists.clone();
    let mut featuring: Vec<domain::ArtistLink> = Vec::new();
    for list in track_artists.values() {
        performing.extend(as_links(list, false));
        featuring.extend(as_links(list, true));
    }
    let credited: Vec<domain::ArtistLink> = credits
        .iter()
        .chain(track_credits.values().flatten())
        .flat_map(|credit| credit.artists.iter().copied())
        .collect();
    related.load_artists(library, performing.iter(), true)?;
    related.load_artists(library, featuring.iter().chain(credited.iter()), false)?;
    related.load_labels(library, &labels)?;
    related.load_items(library, ListKind::ReleaseFormats, &formats)?;
    related.load_items(library, ListKind::ReleaseMedia, &media)?;

    let durations: Vec<i64> = tracks
        .iter()
        .filter_map(|track| track.duration_ms)
        .collect();
    let mut file_count: HashSet<i64> = links.iter().map(|link| link.file_id).collect();
    file_count.extend(release.cover_picture);
    let tracks = tracks
        .into_iter()
        .map(|track| {
            let own = track_artists.remove(&track.id).unwrap_or_default();
            ReleaseTrack {
                alternate_titles: titles(track_titles.remove(&track.id)),
                artists: related.credited_named(&as_links(&own, false)),
                features: related.credited_all(&as_links(&own, true)),
                credits: related.credits(&track_credits.remove(&track.id).unwrap_or_default()),
                file: TrackFile {
                    id: track.file_id,
                    path: files
                        .remove(&track.file_id)
                        .map(|file| file.path)
                        .unwrap_or_default(),
                },
                id: track.id,
                title: track.title,
                title_language: track.title_language,
                duration_ms: track.duration_ms,
                cd_number: track.cd_number,
                track_number: track.track_number,
            }
        })
        .collect();
    Ok(Some(ReleaseView {
        id,
        alternate_titles: titles(catalog.release_titles_of(&ids)?.remove(&id)),
        released: domain::released(&release),
        file_count: file_count.len() as i64,
        duration_ms: (!durations.is_empty()).then(|| durations.iter().sum()),
        blockers: catalog.release_blockers(id)?,
        cover: release.cover_picture.map(|id| FileRef { id }),
        artists: related.credited_named(&artists),
        labels: related.labels(&labels),
        formats: related.items(ListKind::ReleaseFormats, &formats),
        media: related.items(ListKind::ReleaseMedia, &media),
        credits: related.credits(&credits),
        tags: owned(library, TagOwner::Release, id)?,
        disc_titles: catalog
            .disc_titles_of(&ids)?
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(|disc| DiscTitle {
                cd_number: disc.cd_number,
                title: disc.title,
                language: disc.language,
                alternate_titles: titles(Some(disc.alternate_titles)),
            })
            .collect(),
        tracks,
        title: release.title,
        title_language: release.title_language,
        catalog_number: release.catalog_number,
        various_artists: release.various_artists,
    }))
}

/// A release's title, for the edit tabs' header, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn release_header(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<ReleaseHeader>, CommandError> {
    on_library(app, move |library| header(library, args)).await
}

pub fn header(library: &Library, args: IdArgs) -> Result<Option<ReleaseHeader>, CommandError> {
    Ok(library
        .catalog
        .get_release(args.id)?
        .map(|release| ReleaseHeader {
            id: release.id,
            title: release.title,
            title_language: release.title_language,
        }))
}

/// A release, as its Details tab edits it, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn release(app: AppHandle, args: IdArgs) -> Result<Option<ReleaseDetails>, CommandError> {
    on_library(app, move |library| details(library, args)).await
}

pub fn details(library: &Library, args: IdArgs) -> Result<Option<ReleaseDetails>, CommandError> {
    let catalog = &library.catalog;
    let Some(release) = catalog.get_release(args.id)? else {
        return Ok(None);
    };
    let ids = [release.id];
    let artists = catalog
        .release_artists_of(&ids)?
        .remove(&release.id)
        .unwrap_or_default();
    let labels = catalog
        .release_links_of(ReleaseLink::Labels, &ids)?
        .remove(&release.id)
        .unwrap_or_default();
    let formats = catalog
        .release_links_of(ReleaseLink::Formats, &ids)?
        .remove(&release.id)
        .unwrap_or_default();
    let media = catalog
        .release_links_of(ReleaseLink::Media, &ids)?
        .remove(&release.id)
        .unwrap_or_default();
    let mut related = Related::default();
    related.load_artists(library, artists.iter(), false)?;
    related.load_labels(library, &labels)?;
    related.load_items(library, ListKind::ReleaseFormats, &formats)?;
    related.load_items(library, ListKind::ReleaseMedia, &media)?;
    Ok(Some(ReleaseDetails {
        id: release.id,
        alternate_titles: titles(catalog.release_titles_of(&ids)?.remove(&release.id)),
        released: domain::released(&release),
        artists: related.credited_all(&artists),
        labels: related.labels(&labels),
        formats: related.items(ListKind::ReleaseFormats, &formats),
        media: related.items(ListKind::ReleaseMedia, &media),
        title: release.title,
        title_language: release.title_language,
        various_artists: release.various_artists,
        catalog_number: release.catalog_number,
    }))
}

/// A release's cover and linked files, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn release_files(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<ReleaseFiles>, CommandError> {
    on_library(app, move |library| files(library, args)).await
}

pub fn files(library: &Library, args: IdArgs) -> Result<Option<ReleaseFiles>, CommandError> {
    let Some(release) = library.catalog.get_release(args.id)? else {
        return Ok(None);
    };
    let links = library
        .media
        .file_links_of(LinkOwner::Release, &[release.id])?
        .remove(&release.id)
        .unwrap_or_default();
    let mut ids: Vec<i64> = links.iter().map(|link| link.file_id).collect();
    ids.extend(release.cover_picture);
    let files = files_by_id(library, &ids)?;
    Ok(Some(ReleaseFiles {
        id: release.id,
        title: release.title,
        cover: release.cover_picture.and_then(|id| files.get(&id).cloned()),
        files: links
            .into_iter()
            .filter_map(|link| {
                Some(FileLink {
                    file: files.get(&link.file_id)?.clone(),
                    note: link.note,
                })
            })
            .collect(),
    }))
}

/// A release's credit lines and its tracks', or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn release_credits(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<ReleaseCredits>, CommandError> {
    on_library(app, move |library| credits(library, args)).await
}

pub fn credits(library: &Library, args: IdArgs) -> Result<Option<ReleaseCredits>, CommandError> {
    let catalog = &library.catalog;
    let Some(release) = catalog.get_release(args.id)? else {
        return Ok(None);
    };
    let id = release.id;
    let tracks = catalog
        .tracks_of_releases(&[id])?
        .remove(&id)
        .unwrap_or_default();
    let track_ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    let mut track_credits = catalog.credits_of(CreditOwner::Track, &track_ids)?;
    let credits = catalog
        .credits_of(CreditOwner::Release, &[id])?
        .remove(&id)
        .unwrap_or_default();
    let mut related = Related::default();
    related.load_artists(
        library,
        credits
            .iter()
            .chain(track_credits.values().flatten())
            .flat_map(|credit| credit.artists.iter()),
        false,
    )?;
    Ok(Some(ReleaseCredits {
        id,
        title: release.title,
        credits: related.credits(&credits),
        tracks: tracks
            .into_iter()
            .map(|track| TrackCredits {
                credits: related.credits(&track_credits.remove(&track.id).unwrap_or_default()),
                id: track.id,
                cd_number: track.cd_number,
                track_number: track.track_number,
                title: track.title,
            })
            .collect(),
    }))
}

/// The first releases with `q` in a title, by title, for a picker.
#[tauri::command]
#[specta::specta]
pub async fn release_options(
    app: AppHandle,
    args: OptionsArgs,
) -> Result<Vec<ReleaseOption>, CommandError> {
    on_library(app, move |library| options(library, args)).await
}

pub fn options(library: &Library, args: OptionsArgs) -> Result<Vec<ReleaseOption>, CommandError> {
    let page = library.catalog.list_releases(
        &ListArgs {
            per_page: Some(OPTIONS),
            sort: Some("title".into()),
            q: args.q,
            ..ListArgs::default()
        },
        &domain::ReleaseFilterArgs::default(),
    )?;
    Ok(page
        .items
        .into_iter()
        .map(|release| ReleaseOption {
            id: release.id,
            title: release.title,
            cover: release.cover_picture.map(|id| FileRef { id }),
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_release(app: AppHandle, args: ReleaseInput) -> Result<Release, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(library: &Library, args: ReleaseInput) -> Result<Release, CommandError> {
    Ok(library.catalog.create_release(&args.into())?.into())
}

/// Saves the details: title, date, artists, labels, formats, media, catalog
/// number.
#[tauri::command]
#[specta::specta]
pub async fn update_release(
    app: AppHandle,
    args: UpdateReleaseArgs,
) -> Result<Release, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(library: &Library, args: UpdateReleaseArgs) -> Result<Release, CommandError> {
    Ok(library
        .catalog
        .update_release(args.id, &args.input.into())?
        .into())
}

/// Deletes a release, and returns its id. It fails while the release has
/// tracks.
#[tauri::command]
#[specta::specta]
pub async fn delete_release(app: AppHandle, args: IdArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: IdArgs) -> Result<i64, CommandError> {
    library.catalog.delete_release(args.id)?;
    Ok(args.id)
}

/// Sets only the cover, keeping the file links. A file given by path is
/// registered first, in the same transaction.
#[tauri::command]
#[specta::specta]
pub async fn set_release_cover(
    app: AppHandle,
    args: SetReleaseCoverArgs,
) -> Result<Release, CommandError> {
    on_library(app, move |library| set_cover(library, args)).await
}

pub fn set_cover(library: &Library, args: SetReleaseCoverArgs) -> Result<Release, CommandError> {
    Ok(library
        .catalog
        .set_release_cover(args.id, &args.cover.into())?
        .into())
}

/// Sets the cover and replaces the file links. Files given by path are
/// registered first.
#[tauri::command]
#[specta::specta]
pub async fn set_release_files(
    app: AppHandle,
    args: SetReleaseFilesArgs,
) -> Result<Release, CommandError> {
    on_library(app, move |library| set_files(library, args)).await
}

pub fn set_files(library: &Library, args: SetReleaseFilesArgs) -> Result<Release, CommandError> {
    let cover: Option<domain::FileRef> = args.cover.map(Into::into);
    let files: Vec<domain::FileLinkInput> = args.files.into_iter().map(Into::into).collect();
    Ok(library
        .catalog
        .set_release_files(args.id, cover.as_ref(), &files)?
        .into())
}

/// Replaces the release's credit lines and its tracks'.
#[tauri::command]
#[specta::specta]
pub async fn set_release_credits(
    app: AppHandle,
    args: SetReleaseCreditsArgs,
) -> Result<Release, CommandError> {
    on_library(app, move |library| set_credits(library, args)).await
}

pub fn set_credits(
    library: &Library,
    args: SetReleaseCreditsArgs,
) -> Result<Release, CommandError> {
    let credits: Vec<domain::CreditInput> = args.credits.into_iter().map(Into::into).collect();
    let tracks: Vec<domain::TrackCreditsInput> = args
        .tracks
        .into_iter()
        .map(|track| domain::TrackCreditsInput {
            track_id: track.track_id,
            credits: track.credits.into_iter().map(Into::into).collect(),
        })
        .collect();
    Ok(library
        .catalog
        .set_release_credits(args.id, &credits, &tracks)?
        .into())
}

/// Roles in use on releases and tracks, for suggestions: the first 20 with
/// `q` in them, ignoring case, the most used first. Each spelling is its own
/// suggestion, so "Mixed By" and "Mixed by" are both offered.
#[tauri::command]
#[specta::specta]
pub async fn credit_roles(app: AppHandle, args: OptionsArgs) -> Result<Vec<String>, CommandError> {
    on_library(app, move |library| roles(library, args)).await
}

pub fn roles(library: &Library, args: OptionsArgs) -> Result<Vec<String>, CommandError> {
    Ok(library.catalog.credit_roles(args.q.as_deref())?)
}

/// How many releases, artists, tracks and labels there are, and the tracks'
/// playing time.
#[tauri::command]
#[specta::specta]
pub async fn catalog_totals(app: AppHandle) -> Result<CatalogTotals, CommandError> {
    on_library(app, totals).await
}

pub fn totals(library: &Library) -> Result<CatalogTotals, CommandError> {
    let totals = library.catalog.totals()?;
    Ok(CatalogTotals {
        releases: totals.releases,
        artists: totals.artists,
        tracks: totals.tracks,
        labels: totals.labels,
        duration_ms: totals.duration_ms,
    })
}

/// Releases as list rows, with what they refer to loaded once.
pub(crate) fn list_items(
    library: &Library,
    releases: Vec<domain::Release>,
) -> Result<Vec<ReleaseListItem>, CommandError> {
    let catalog = &library.catalog;
    let ids: Vec<i64> = releases.iter().map(|release| release.id).collect();
    let mut alternate_titles = catalog.release_titles_of(&ids)?;
    let mut artists = catalog.release_artists_of(&ids)?;
    let mut labels = catalog.release_links_of(ReleaseLink::Labels, &ids)?;
    let mut formats = catalog.release_links_of(ReleaseLink::Formats, &ids)?;
    let mut media = catalog.release_links_of(ReleaseLink::Media, &ids)?;
    let mut related = Related::default();
    related.load_artists(library, artists.values().flatten(), false)?;
    related.load_labels(
        library,
        &labels.values().flatten().copied().collect::<Vec<_>>(),
    )?;
    related.load_items(
        library,
        ListKind::ReleaseFormats,
        &formats.values().flatten().copied().collect::<Vec<_>>(),
    )?;
    related.load_items(
        library,
        ListKind::ReleaseMedia,
        &media.values().flatten().copied().collect::<Vec<_>>(),
    )?;
    Ok(releases
        .into_iter()
        .map(|release| {
            let id = release.id;
            ReleaseListItem {
                alternate_titles: titles(alternate_titles.remove(&id)),
                artists: related.credited_all(&artists.remove(&id).unwrap_or_default()),
                formats: related.items(
                    ListKind::ReleaseFormats,
                    &formats.remove(&id).unwrap_or_default(),
                ),
                media: related.items(
                    ListKind::ReleaseMedia,
                    &media.remove(&id).unwrap_or_default(),
                ),
                labels: related.labels(&labels.remove(&id).unwrap_or_default()),
                release: release.into(),
            }
        })
        .collect())
}

fn titles(titles: Option<Vec<domain::AlternateTitle>>) -> Vec<AlternateTitle> {
    titles
        .unwrap_or_default()
        .into_iter()
        .map(AlternateTitle::from)
        .collect()
}
