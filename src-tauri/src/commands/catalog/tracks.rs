//! Tracks: the track list, the track page and form, a release's Tracks tab,
//! which the import's tracks step is too, and what the player shows.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use slunk::catalog::{self as domain, CreditOwner};
use slunk::media::LinkOwner;
use slunk::paging::ListArgs;
use slunk::tagging::{TagOwner, tag_filters};
use specta::Type;
use tauri::AppHandle;

use super::import::{TrackSuggestion, track_suggestions};
use super::releases::{
    AlternateTitleInput, CreditInput, CreditedArtistInput, DiscTitle, Given, Release,
    ReleaseHeader, TrackFile,
};
use super::{
    AlternateTitle, Credit, CreditedArtist, FileLink, FileLinkInput, IdArgs, NamedCreditedArtist,
    NamedItem, OPTIONS, OptionsArgs, Related, files_by_id,
};
use crate::commands::media::{File, FileRef};
use crate::commands::on_library;
use crate::commands::tagging::{TagFilterInput, TagValues, TagValuesInput, owned};
use crate::error::CommandError;
use crate::library::Library;

/// A track, as a save returns it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub id: i64,
    pub title: String,
    /// Null: the release's
    pub title_language: Option<String>,
    /// Read from the audio file. Null when it can't be read.
    pub duration_ms: Option<i64>,
    pub cd_number: i64,
    pub track_number: i64,
    pub release_id: i64,
    /// The audio file
    pub file_id: i64,
    /// Epoch seconds
    pub inserted_at: i64,
    /// Epoch seconds
    pub updated_at: i64,
}

impl From<domain::Track> for Track {
    fn from(track: domain::Track) -> Self {
        Track {
            id: track.id,
            title: track.title,
            title_language: track.title_language,
            duration_ms: track.duration_ms,
            cd_number: track.cd_number,
            track_number: track.track_number,
            release_id: track.release_id,
            file_id: track.file_id,
            inserted_at: track.inserted_at,
            updated_at: track.updated_at,
        }
    }
}

/// A track in the track list.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackListItem {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    pub duration_ms: Option<i64>,
    pub cd_number: i64,
    pub track_number: i64,
    pub inserted_at: i64,
    pub updated_at: i64,
    pub release: ReleaseHeader,
    /// Who performs it: its own artists; on a regular release with none, the
    /// release's, as credited there; on a various-artists release, nobody.
    pub performers: Vec<CreditedArtist>,
    pub file: TrackFile,
}

/// A page of tracks, and the release filter's title, for its picker: null
/// when it wasn't asked for, or isn't there.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackPage {
    pub items: Vec<TrackListItem>,
    pub total: i64,
    pub filter_release: Option<NamedItem>,
}

/// The track page: everything it shows.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackView {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    pub duration_ms: Option<i64>,
    pub cd_number: i64,
    pub track_number: i64,
    pub release: ReleaseHeader,
    /// Who performs it, each with its alternate names
    pub performers: Vec<NamedCreditedArtist>,
    /// Guests, shown as "(feat. …)"
    pub features: Vec<CreditedArtist>,
    pub credits: Vec<Credit>,
    pub file: TrackFile,
    /// By tag name
    pub tags: Vec<TagValues>,
    pub files: Vec<FileLink>,
}

/// What the player shows of a track (2_player.md §6.1).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTrack {
    pub id: i64,
    pub title: String,
    /// Null: the release's
    pub title_language: Option<String>,
    /// Read from the audio file. Null when it can't be read.
    pub duration_ms: Option<i64>,
    /// Who performs it: its own artists; on a regular release with none, the
    /// release's, as credited there; on a various-artists release, nobody.
    pub performers: Vec<CreditedArtist>,
    pub release: ReleaseHeader,
    /// The release's cover
    pub cover: Option<FileRef>,
    /// The audio file
    pub file: TrackFile,
}

/// A track, as its form edits it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackDetails {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    pub cd_number: i64,
    pub track_number: i64,
    pub release: ReleaseHeader,
    pub file: TrackFile,
    /// Its own performers, without features
    pub artists: Vec<CreditedArtist>,
    pub features: Vec<CreditedArtist>,
    pub credits: Vec<Credit>,
    pub files: Vec<FileLink>,
    pub tags: Vec<TagValues>,
}

/// What the track form starts from: the track it edits, or what a new one
/// is prefilled with. Each is null when it wasn't asked for, or isn't there.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackForm {
    pub track: Option<TrackDetails>,
    pub prefill_release: Option<ReleaseHeader>,
    pub prefill_file: Option<File>,
}

/// A track, as the Tracks tab edits it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditedTrack {
    pub id: i64,
    pub cd_number: i64,
    pub track_number: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
    pub duration_ms: Option<i64>,
    pub file: TrackFile,
    pub artists: Vec<CreditedArtist>,
    pub features: Vec<CreditedArtist>,
}

/// A release's tracks, as its Tracks tab edits them.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTracks {
    pub id: i64,
    pub title: String,
    pub various_artists: bool,
    /// As credited
    pub artists: Vec<CreditedArtist>,
    /// By disc number. Only discs with a title are listed.
    pub disc_titles: Vec<DiscTitle>,
    /// In order: disc, then number
    pub tracks: Vec<EditedTrack>,
    /// With a folder, the import's: a row for each of its audio files. Null
    /// without one.
    pub track_suggestions: Option<Vec<TrackSuggestion>>,
}

#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct TracksArgs {
    /// From 1. Default 1.
    #[specta(optional)]
    pub page: Option<i64>,
    /// Default 50, at most 200
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// `title` or `id`, with `-` in front for descending. Default `-id`,
    /// newest first.
    #[specta(optional)]
    pub sort: Option<String>,
    /// Part of the title or of an alternate title, ignoring case
    #[specta(optional)]
    pub q: Option<String>,
    #[specta(optional)]
    pub release: Option<i64>,
    /// Every filter has to match.
    #[specta(optional)]
    pub tags: Vec<TagFilterInput>,
}

/// The track to edit, or what a new one is prefilled with.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct TrackFormArgs {
    /// The track to edit. Left out for a new one.
    #[specta(optional)]
    pub id: Option<i64>,
    /// A new track's release
    #[specta(optional)]
    pub release: Option<i64>,
    /// A new track's audio file
    #[specta(optional)]
    pub file: Option<i64>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTracksArgs {
    pub id: i64,
    /// The import's folder: its audio files get rows too
    #[serde(default)]
    #[specta(optional)]
    pub folder: Option<String>,
}

/// A track, with everything saved with it.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackInput {
    pub release_id: i64,
    /// A registered audio file. Its duration is read from it.
    pub file_id: i64,
    /// Default 1
    #[serde(default = "first")]
    #[specta(optional)]
    pub cd_number: i64,
    pub track_number: i64,
    pub title: String,
    /// A BCP 47 tag. Blank: the release's title language. Left out, an
    /// update keeps it as it is.
    #[serde(default)]
    #[specta(optional, type = Option<String>)]
    pub title_language: Given<String>,
    /// Replaces the alternate titles, in order. Left out, they stay as they
    /// are, except one that's now the main title.
    #[serde(default)]
    #[specta(optional)]
    pub alternate_titles: Option<Vec<AlternateTitleInput>>,
    #[serde(default)]
    #[specta(optional)]
    pub artists: Vec<CreditedArtistInput>,
    #[serde(default)]
    #[specta(optional)]
    pub features: Vec<CreditedArtistInput>,
    #[serde(default)]
    #[specta(optional)]
    pub credits: Vec<CreditInput>,
    #[serde(default)]
    #[specta(optional)]
    pub files: Vec<FileLinkInput>,
    /// Replaces the track's tags. Left out, they stay as they are.
    #[serde(default)]
    #[specta(optional)]
    pub tags: Option<Vec<TagValuesInput>>,
}

impl From<TrackInput> for domain::TrackInput {
    fn from(input: TrackInput) -> Self {
        domain::TrackInput {
            release_id: input.release_id,
            file_id: input.file_id,
            cd_number: input.cd_number,
            track_number: input.track_number,
            title: input.title,
            title_language: input.title_language.0,
            alternate_titles: input
                .alternate_titles
                .map(|titles| titles.into_iter().map(Into::into).collect()),
            artists: input.artists.into_iter().map(Into::into).collect(),
            features: input.features.into_iter().map(Into::into).collect(),
            credits: input.credits.into_iter().map(Into::into).collect(),
            files: input.files.into_iter().map(Into::into).collect(),
            tags: input
                .tags
                .map(|tags| tags.into_iter().map(Into::into).collect()),
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTrackArgs {
    pub id: i64,
    #[serde(flatten)]
    pub input: TrackInput,
}

/// A row of a release's tracks: `id` for one it has, else `fileId` or
/// `path` for a new one.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTrackInput {
    #[serde(default)]
    #[specta(optional)]
    pub id: Option<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub file_id: Option<i64>,
    /// A file to register first
    #[serde(default)]
    #[specta(optional)]
    pub path: Option<String>,
    /// Default 1
    #[serde(default = "first")]
    #[specta(optional)]
    pub cd_number: i64,
    pub track_number: i64,
    pub title: String,
    /// Left out, it stays as it is. Blank: the release's title language.
    #[serde(default)]
    #[specta(optional, type = Option<String>)]
    pub title_language: Given<String>,
    /// Replaces the alternate titles, in order. Left out, they stay as they
    /// are, except one that's now the main title.
    #[serde(default)]
    #[specta(optional)]
    pub alternate_titles: Option<Vec<AlternateTitleInput>>,
    #[serde(default)]
    #[specta(optional)]
    pub artists: Vec<CreditedArtistInput>,
    #[serde(default)]
    #[specta(optional)]
    pub features: Vec<CreditedArtistInput>,
}

impl From<ReleaseTrackInput> for domain::ReleaseTrackInput {
    fn from(input: ReleaseTrackInput) -> Self {
        domain::ReleaseTrackInput {
            id: input.id,
            file: domain::FileRef {
                file_id: input.file_id,
                path: input.path,
            },
            cd_number: input.cd_number,
            track_number: input.track_number,
            title: input.title,
            title_language: input.title_language.0,
            alternate_titles: input
                .alternate_titles
                .map(|titles| titles.into_iter().map(Into::into).collect()),
            artists: input.artists.into_iter().map(Into::into).collect(),
            features: input.features.into_iter().map(Into::into).collect(),
        }
    }
}

/// A disc's title, to save.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiscTitleInput {
    pub cd_number: i64,
    /// Trimmed. Blank means no title, and then it can't have alternate
    /// titles.
    pub title: String,
    /// Blank: the release's title language
    #[serde(default)]
    #[specta(optional)]
    pub language: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub alternate_titles: Vec<AlternateTitleInput>,
}

impl From<DiscTitleInput> for domain::DiscTitleInput {
    fn from(input: DiscTitleInput) -> Self {
        domain::DiscTitleInput {
            cd_number: input.cd_number,
            title: input.title,
            language: input.language,
            alternate_titles: input.alternate_titles.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetReleaseTracksArgs {
    pub id: i64,
    /// The whole list: rows with an id update those tracks, the others
    /// create tracks, and tracks left out are deleted. Their files stay
    /// registered.
    pub tracks: Vec<ReleaseTrackInput>,
    /// Replaces the disc titles. A blank title is left out. Left out, they
    /// stay as they are, except on a disc left without tracks, which loses
    /// its title.
    #[serde(default)]
    #[specta(optional)]
    pub disc_titles: Option<Vec<DiscTitleInput>>,
}

fn first() -> i64 {
    1
}

/// The tracks the player's queue holds.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerTracksArgs {
    pub ids: Vec<i64>,
}

/// Tracks, with the release filter's title. The filters combine.
#[tauri::command]
#[specta::specta]
pub async fn tracks(app: AppHandle, args: TracksArgs) -> Result<TrackPage, CommandError> {
    on_library(app, move |library| list(library, args)).await
}

pub fn list(library: &Library, args: TracksArgs) -> Result<TrackPage, CommandError> {
    let tags: Vec<slunk::tagging::TagFilterInput> = args.tags.into_iter().map(Into::into).collect();
    let filters = domain::TrackFilterArgs {
        release: args.release,
        tags: tag_filters(&tags)?,
    };
    let catalog = &library.catalog;
    let page = catalog.list_tracks(
        &ListArgs {
            page: args.page,
            per_page: args.per_page,
            sort: args.sort,
            q: args.q,
        },
        &filters,
    )?;
    let filter_release = match args.release {
        Some(id) => catalog.get_release(id)?.map(|release| NamedItem {
            id: release.id,
            name: release.title,
        }),
        None => None,
    };
    Ok(TrackPage {
        items: list_items(library, page.items)?,
        total: page.total,
        filter_release,
    })
}

/// Tracks as a list shows them, in the order given.
pub(crate) fn list_items(
    library: &Library,
    tracks: Vec<domain::Track>,
) -> Result<Vec<TrackListItem>, CommandError> {
    let catalog = &library.catalog;
    let ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    let mut titles = catalog.track_titles_of(&ids)?;
    let file_ids: Vec<i64> = tracks.iter().map(|track| track.file_id).collect();
    let files = files_by_id(library, &file_ids)?;
    let Performing {
        releases,
        performers: mut performing,
    } = performers_of(library, &tracks)?;
    let mut related = Related::default();
    related.load_artists(library, performing.values().flatten(), false)?;
    Ok(tracks
        .into_iter()
        .filter_map(|track| {
            let release = releases.get(&track.release_id)?;
            Some(TrackListItem {
                alternate_titles: alternate_titles(titles.remove(&track.id)),
                release: header(release),
                performers: related.credited_all(&performing.remove(&track.id).unwrap_or_default()),
                file: track_file(&files, track.file_id),
                id: track.id,
                title: track.title,
                title_language: track.title_language,
                duration_ms: track.duration_ms,
                cd_number: track.cd_number,
                track_number: track.track_number,
                inserted_at: track.inserted_at,
                updated_at: track.updated_at,
            })
        })
        .collect())
}

/// A track, as the track picker offers it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackOption {
    pub id: i64,
    pub title: String,
    /// Null: the release's
    pub title_language: Option<String>,
    pub release: ReleaseHeader,
    /// The release's cover
    pub cover: Option<FileRef>,
}

/// The first 20 tracks whose title or alternate title has `q` in it,
/// ignoring case, by title, for the track picker.
#[tauri::command]
#[specta::specta]
pub async fn track_options(
    app: AppHandle,
    args: OptionsArgs,
) -> Result<Vec<TrackOption>, CommandError> {
    on_library(app, move |library| options(library, args)).await
}

pub fn options(library: &Library, args: OptionsArgs) -> Result<Vec<TrackOption>, CommandError> {
    let catalog = &library.catalog;
    let page = catalog.list_tracks(
        &ListArgs {
            per_page: Some(OPTIONS),
            sort: Some("title".into()),
            q: args.q,
            ..ListArgs::default()
        },
        &domain::TrackFilterArgs::default(),
    )?;
    let mut release_ids: Vec<i64> = page.items.iter().map(|track| track.release_id).collect();
    release_ids.sort_unstable();
    release_ids.dedup();
    let releases: HashMap<i64, domain::Release> = catalog
        .releases_by_ids(&release_ids)?
        .into_iter()
        .map(|release| (release.id, release))
        .collect();
    Ok(page
        .items
        .into_iter()
        .filter_map(|track| {
            let release = releases.get(&track.release_id)?;
            Some(TrackOption {
                id: track.id,
                title: track.title,
                title_language: track.title_language,
                release: header(release),
                cover: release.cover_picture.map(|id| FileRef { id }),
            })
        })
        .collect())
}

/// The track page, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn track_page(app: AppHandle, args: IdArgs) -> Result<Option<TrackView>, CommandError> {
    on_library(app, move |library| page(library, args)).await
}

pub fn page(library: &Library, args: IdArgs) -> Result<Option<TrackView>, CommandError> {
    let catalog = &library.catalog;
    let Some(track) = catalog.get_track(args.id)? else {
        return Ok(None);
    };
    let Some(release) = catalog.get_release(track.release_id)? else {
        return Ok(None);
    };
    let id = track.id;
    let own = catalog
        .track_artists_of(&[id])?
        .remove(&id)
        .unwrap_or_default();
    let release_artists = catalog
        .release_artists_of(&[release.id])?
        .remove(&release.id)
        .unwrap_or_default();
    let performers = domain::performers(&release, &own, &release_artists);
    let (_, features) = split(&own);
    let credits = catalog
        .credits_of(CreditOwner::Track, &[id])?
        .remove(&id)
        .unwrap_or_default();
    let mut related = Related::default();
    related.load_artists(library, performers.iter(), true)?;
    related.load_artists(
        library,
        features
            .iter()
            .chain(credits.iter().flat_map(|credit| credit.artists.iter())),
        false,
    )?;
    let files = files_by_id(library, &[track.file_id])?;
    Ok(Some(TrackView {
        alternate_titles: alternate_titles(catalog.track_titles_of(&[id])?.remove(&id)),
        release: header(&release),
        performers: related.credited_named(&performers),
        features: related.credited_all(&features),
        credits: related.credits(&credits),
        file: track_file(&files, track.file_id),
        tags: owned(library, TagOwner::Track, id)?,
        files: file_links(library, id)?,
        id,
        title: track.title,
        title_language: track.title_language,
        duration_ms: track.duration_ms,
        cd_number: track.cd_number,
        track_number: track.track_number,
    }))
}

/// What the player shows of each of these tracks: one per id that's a
/// track, each once, in the order asked. An id that isn't a track is left
/// out, since a queue can name a track deleted since.
#[tauri::command]
#[specta::specta]
pub async fn player_tracks(
    app: AppHandle,
    args: PlayerTracksArgs,
) -> Result<Vec<PlayerTrack>, CommandError> {
    on_library(app, move |library| for_player(library, args)).await
}

pub fn for_player(
    library: &Library,
    args: PlayerTracksArgs,
) -> Result<Vec<PlayerTrack>, CommandError> {
    let mut seen = HashSet::new();
    let ids: Vec<i64> = args.ids.into_iter().filter(|id| seen.insert(*id)).collect();
    let mut found: HashMap<i64, domain::Track> = library
        .catalog
        .tracks_by_ids(&ids)?
        .into_iter()
        .map(|track| (track.id, track))
        .collect();
    let tracks: Vec<domain::Track> = ids.iter().filter_map(|id| found.remove(id)).collect();
    let file_ids: Vec<i64> = tracks.iter().map(|track| track.file_id).collect();
    let files = files_by_id(library, &file_ids)?;
    let Performing {
        releases,
        performers: mut performing,
    } = performers_of(library, &tracks)?;
    let mut related = Related::default();
    related.load_artists(library, performing.values().flatten(), false)?;
    Ok(tracks
        .into_iter()
        .filter_map(|track| {
            let release = releases.get(&track.release_id)?;
            Some(PlayerTrack {
                performers: related.credited_all(&performing.remove(&track.id).unwrap_or_default()),
                release: header(release),
                cover: release.cover_picture.map(|id| FileRef { id }),
                file: track_file(&files, track.file_id),
                id: track.id,
                title: track.title,
                title_language: track.title_language,
                duration_ms: track.duration_ms,
            })
        })
        .collect())
}

/// A release's tracks' ids, by disc, then by number, as its tracklist orders
/// them, for the release table's Play (2_player.md §6.2). A release with no
/// tracks, or one that isn't there, has none.
#[tauri::command]
#[specta::specta]
pub async fn release_track_ids(app: AppHandle, args: IdArgs) -> Result<Vec<i64>, CommandError> {
    on_library(app, move |library| track_ids(library, args)).await
}

pub fn track_ids(library: &Library, args: IdArgs) -> Result<Vec<i64>, CommandError> {
    Ok(library
        .catalog
        .tracks_of_releases(&[args.id])?
        .remove(&args.id)
        .unwrap_or_default()
        .iter()
        .map(|track| track.id)
        .collect())
}

/// What the track form starts from: the track to edit, or a new one's
/// release and audio file.
#[tauri::command]
#[specta::specta]
pub async fn track_form(app: AppHandle, args: TrackFormArgs) -> Result<TrackForm, CommandError> {
    on_library(app, move |library| form(library, args)).await
}

pub fn form(library: &Library, args: TrackFormArgs) -> Result<TrackForm, CommandError> {
    let catalog = &library.catalog;
    let track = match args.id {
        Some(id) => details(library, id)?,
        None => None,
    };
    let prefill_release = match args.release {
        Some(id) => catalog.get_release(id)?.as_ref().map(header),
        None => None,
    };
    let prefill_file = match args.file {
        Some(id) => library.media.get_file(id)?.as_ref().map(File::from),
        None => None,
    };
    Ok(TrackForm {
        track,
        prefill_release,
        prefill_file,
    })
}

fn details(library: &Library, id: i64) -> Result<Option<TrackDetails>, CommandError> {
    let catalog = &library.catalog;
    let Some(track) = catalog.get_track(id)? else {
        return Ok(None);
    };
    let Some(release) = catalog.get_release(track.release_id)? else {
        return Ok(None);
    };
    let own = catalog
        .track_artists_of(&[id])?
        .remove(&id)
        .unwrap_or_default();
    let (artists, features) = split(&own);
    let credits = catalog
        .credits_of(CreditOwner::Track, &[id])?
        .remove(&id)
        .unwrap_or_default();
    let mut related = Related::default();
    related.load_artists(
        library,
        artists
            .iter()
            .chain(&features)
            .chain(credits.iter().flat_map(|credit| credit.artists.iter())),
        false,
    )?;
    let files = files_by_id(library, &[track.file_id])?;
    Ok(Some(TrackDetails {
        alternate_titles: alternate_titles(catalog.track_titles_of(&[id])?.remove(&id)),
        release: header(&release),
        file: track_file(&files, track.file_id),
        artists: related.credited_all(&artists),
        features: related.credited_all(&features),
        credits: related.credits(&credits),
        files: file_links(library, id)?,
        tags: owned(library, TagOwner::Track, id)?,
        id,
        title: track.title,
        title_language: track.title_language,
        cd_number: track.cd_number,
        track_number: track.track_number,
    }))
}

/// Creates a track from a registered audio file, reading its duration.
#[tauri::command]
#[specta::specta]
pub async fn create_track(app: AppHandle, args: TrackInput) -> Result<Track, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(library: &Library, args: TrackInput) -> Result<Track, CommandError> {
    Ok(library.catalog.create_track(&args.into())?.into())
}

/// Saves a track and everything saved with it. A new audio file is read for
/// its duration.
#[tauri::command]
#[specta::specta]
pub async fn update_track(app: AppHandle, args: UpdateTrackArgs) -> Result<Track, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(library: &Library, args: UpdateTrackArgs) -> Result<Track, CommandError> {
    Ok(library
        .catalog
        .update_track(args.id, &args.input.into())?
        .into())
}

/// Deletes a track, its artists, credits and file links, and returns its id.
/// Its audio file stays registered.
#[tauri::command]
#[specta::specta]
pub async fn delete_track(app: AppHandle, args: IdArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: IdArgs) -> Result<i64, CommandError> {
    library.catalog.delete_track(args.id)?;
    Ok(args.id)
}

/// A release's tracks and disc titles, as its Tracks tab edits them, or
/// null for an unknown release. With a folder, the import's rows for its
/// audio files too. A folder that can't be read is an error.
#[tauri::command]
#[specta::specta]
pub async fn release_tracks(
    app: AppHandle,
    args: ReleaseTracksArgs,
) -> Result<Option<ReleaseTracks>, CommandError> {
    on_library(app, move |library| of_release(library, args)).await
}

pub fn of_release(
    library: &Library,
    args: ReleaseTracksArgs,
) -> Result<Option<ReleaseTracks>, CommandError> {
    let suggestions = args
        .folder
        .map(|folder| track_suggestions(library, &folder))
        .transpose()?;
    let catalog = &library.catalog;
    let Some(release) = catalog.get_release(args.id)? else {
        return Ok(None);
    };
    let id = release.id;
    let tracks = catalog
        .tracks_of_releases(&[id])?
        .remove(&id)
        .unwrap_or_default();
    let ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    let mut titles = catalog.track_titles_of(&ids)?;
    let mut own = catalog.track_artists_of(&ids)?;
    let file_ids: Vec<i64> = tracks.iter().map(|track| track.file_id).collect();
    let files = files_by_id(library, &file_ids)?;
    let artists = catalog
        .release_artists_of(&[id])?
        .remove(&id)
        .unwrap_or_default();
    let mut related = Related::default();
    related.load_artists(library, artists.iter(), false)?;
    let links: Vec<domain::ArtistLink> = own
        .values()
        .flat_map(|own| {
            let (artists, features) = split(own);
            artists.into_iter().chain(features)
        })
        .collect();
    related.load_artists(library, links.iter(), false)?;
    Ok(Some(ReleaseTracks {
        artists: related.credited_all(&artists),
        disc_titles: catalog
            .disc_titles_of(&[id])?
            .remove(&id)
            .unwrap_or_default()
            .into_iter()
            .map(|disc| DiscTitle {
                cd_number: disc.cd_number,
                title: disc.title,
                language: disc.language,
                alternate_titles: alternate_titles(Some(disc.alternate_titles)),
            })
            .collect(),
        tracks: tracks
            .into_iter()
            .map(|track| {
                let (artists, features) = split(&own.remove(&track.id).unwrap_or_default());
                EditedTrack {
                    alternate_titles: alternate_titles(titles.remove(&track.id)),
                    file: track_file(&files, track.file_id),
                    artists: related.credited_all(&artists),
                    features: related.credited_all(&features),
                    id: track.id,
                    cd_number: track.cd_number,
                    track_number: track.track_number,
                    title: track.title,
                    title_language: track.title_language,
                    duration_ms: track.duration_ms,
                }
            })
            .collect(),
        track_suggestions: suggestions,
        id,
        title: release.title,
        various_artists: release.various_artists,
    }))
}

/// Saves the whole track list, registering files given by path, then the
/// disc titles.
#[tauri::command]
#[specta::specta]
pub async fn set_release_tracks(
    app: AppHandle,
    args: SetReleaseTracksArgs,
) -> Result<Release, CommandError> {
    on_library(app, move |library| set_tracks(library, args)).await
}

pub fn set_tracks(library: &Library, args: SetReleaseTracksArgs) -> Result<Release, CommandError> {
    let rows: Vec<domain::ReleaseTrackInput> = args.tracks.into_iter().map(Into::into).collect();
    let disc_titles: Option<Vec<domain::DiscTitleInput>> = args
        .disc_titles
        .map(|titles| titles.into_iter().map(Into::into).collect());
    Ok(library
        .catalog
        .set_release_tracks(args.id, &rows, disc_titles.as_deref())?
        .into())
}

/// Tracks' releases, by id, and who performs each track, by track id.
struct Performing {
    releases: HashMap<i64, domain::Release>,
    performers: HashMap<i64, Vec<domain::ArtistLink>>,
}

fn performers_of(library: &Library, tracks: &[domain::Track]) -> Result<Performing, CommandError> {
    let catalog = &library.catalog;
    let ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
    let mut release_ids: Vec<i64> = tracks.iter().map(|track| track.release_id).collect();
    release_ids.sort_unstable();
    release_ids.dedup();
    let releases: HashMap<i64, domain::Release> = catalog
        .releases_by_ids(&release_ids)?
        .into_iter()
        .map(|release| (release.id, release))
        .collect();
    let release_artists = catalog.release_artists_of(&release_ids)?;
    let own = catalog.track_artists_of(&ids)?;
    let performers = tracks
        .iter()
        .filter_map(|track| {
            let release = releases.get(&track.release_id)?;
            let links = domain::performers(
                release,
                own.get(&track.id).map(Vec::as_slice).unwrap_or_default(),
                release_artists
                    .get(&track.release_id)
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
            );
            Some((track.id, links))
        })
        .collect();
    Ok(Performing {
        releases,
        performers,
    })
}

/// A track's own artists, then its features.
fn split(own: &[domain::TrackArtist]) -> (Vec<domain::ArtistLink>, Vec<domain::ArtistLink>) {
    let link = |artist: &domain::TrackArtist| domain::ArtistLink {
        artist_id: artist.artist_id,
        credited_name_id: artist.credited_name_id,
    };
    (
        own.iter()
            .filter(|artist| !artist.featured)
            .map(link)
            .collect(),
        own.iter()
            .filter(|artist| artist.featured)
            .map(link)
            .collect(),
    )
}

/// A track's file links, in the order they were added.
fn file_links(library: &Library, id: i64) -> Result<Vec<FileLink>, CommandError> {
    let links = library
        .media
        .file_links_of(LinkOwner::Track, &[id])?
        .remove(&id)
        .unwrap_or_default();
    let ids: Vec<i64> = links.iter().map(|link| link.file_id).collect();
    let files = files_by_id(library, &ids)?;
    Ok(links
        .into_iter()
        .filter_map(|link| {
            Some(FileLink {
                file: files.get(&link.file_id)?.clone(),
                note: link.note,
            })
        })
        .collect())
}

fn header(release: &domain::Release) -> ReleaseHeader {
    ReleaseHeader {
        id: release.id,
        title: release.title.clone(),
        title_language: release.title_language.clone(),
    }
}

fn track_file(files: &HashMap<i64, File>, id: i64) -> TrackFile {
    TrackFile {
        id,
        path: files
            .get(&id)
            .map(|file| file.path.clone())
            .unwrap_or_default(),
    }
}

fn alternate_titles(titles: Option<Vec<domain::AlternateTitle>>) -> Vec<AlternateTitle> {
    titles
        .unwrap_or_default()
        .into_iter()
        .map(AlternateTitle::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use slunk::repo::{connection, migrator};

    use super::*;

    const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// A library with a regular release, whose second track only has a
    /// feature, a various-artists one, whose second track has nobody, and one
    /// on two discs, its tracks added out of order.
    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrator::migrate(&db, 10).unwrap();
        let library = Library::new(&db, connection::open(&db).unwrap());
        library
            .store
            .conn()
            .execute_batch(&format!(
                r#"
                insert into files (id, path, type, size_bytes, mtime, sha256) values
                    (1, 'a/1.flac', 'music', 1, 0, '{HASH}'),
                    (2, 'a/2.flac', 'music', 1, 0, '{HASH}'),
                    (3, 'b/1.flac', 'music', 1, 0, '{HASH}'),
                    (4, 'b/2.flac', 'music', 1, 0, '{HASH}'),
                    (5, 'a/cover.jpg', 'image', 1, 0, '{HASH}'),
                    (6, 'c/2-1.flac', 'music', 1, 0, '{HASH}'),
                    (7, 'c/1-2.flac', 'music', 1, 0, '{HASH}'),
                    (8, 'c/1-1.flac', 'music', 1, 0, '{HASH}');
                insert into artists (id, name) values (1, 'Alika'), (2, 'Guest'), (3, 'Solo');
                insert into releases (id, title, cover_picture, various_artists) values
                    (1, 'Regular', 5, 0),
                    (2, 'Various', null, 1),
                    (3, 'Double', null, 0);
                insert into artists_releases (artist_id, release_id, position) values (1, 1, 0);
                insert into tracks (id, title, track_number, cd_number, release_id, file_id) values
                    (1, 'One', 1, 1, 1, 1),
                    (2, 'Two', 2, 1, 1, 2),
                    (3, 'Three', 1, 1, 2, 3),
                    (4, 'Four', 2, 1, 2, 4),
                    (5, 'Two One', 1, 2, 3, 6),
                    (6, 'One Two', 2, 1, 3, 7),
                    (7, 'One One', 1, 1, 3, 8);
                insert into artists_tracks (artist_id, track_id, featured, position) values
                    (2, 2, 1, 0),
                    (3, 3, 0, 0);
                "#
            ))
            .unwrap();
        (dir, library)
    }

    fn names(track: &PlayerTrack) -> Vec<&str> {
        track
            .performers
            .iter()
            .map(|performer| performer.artist.name.as_str())
            .collect()
    }

    #[test]
    fn player_tracks_come_once_each_in_the_order_asked() {
        let (_dir, library) = library();
        let tracks = for_player(
            &library,
            PlayerTracksArgs {
                ids: vec![2, 99, 1, 2, 3, 4],
            },
        )
        .unwrap();
        let ids: Vec<i64> = tracks.iter().map(|track| track.id).collect();
        assert_eq!(ids, [2, 1, 3, 4]);

        let [two, one, three, four] = &tracks[..] else {
            unreachable!()
        };
        // A regular release's track without its own artists is the
        // release's, and a feature doesn't perform it
        assert_eq!(names(one), ["Alika"]);
        assert_eq!(names(two), ["Alika"]);
        // On a various-artists release, only its own, or nobody
        assert_eq!(names(three), ["Solo"]);
        assert!(names(four).is_empty());

        assert_eq!(one.release.title, "Regular");
        assert_eq!(one.cover.as_ref().map(|cover| cover.id), Some(5));
        assert!(three.cover.is_none());
        assert_eq!((one.file.id, one.file.path.as_str()), (1, "a/1.flac"));

        assert!(
            for_player(&library, PlayerTracksArgs { ids: vec![] })
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_releases_track_ids_come_by_disc_then_number() {
        let (_dir, library) = library();
        let ids = |id| track_ids(&library, IdArgs { id }).unwrap();
        assert_eq!(ids(3), [7, 6, 5]);
        assert_eq!(ids(1), [1, 2]);
        assert!(ids(99).is_empty());
    }
}
