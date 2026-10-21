//! The external libraries context's commands (backend.md, "Commands"):
//! external libraries, their rules, what they come to and what that will
//! take, and where the catalog's releases, artists and tracks are in them;
//! a track's tags as an external library writes them, with the path a
//! template gives it; and syncing a folder library: its plan, the sync with
//! its progress, cancelling it, ejecting, and finding a folder that moved.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slunk::external::plan::{Action, Subject};
use slunk::external::sync::{Outcome, Progress, Stage};
use slunk::external::{self as domain, DEFAULT_TEMPLATE, Destination, Template};
use specta::Type;
use tauri::AppHandle;
use tauri::ipc::Channel;

use super::catalog::releases::ReleaseHeader;
use super::catalog::tracks::{TrackListItem, list_items};
use super::catalog::{ArtistRef, CreditedArtist, IdArgs, Related, files_by_id};
use super::media::{File, FileRef};
use super::on_library;
use super::tagging::{Tag, TagArgs, tags_by_id};
use crate::error::CommandError;
use crate::library::Library;

/// Where an external library's files go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExternalKind {
    /// A folder, with its files laid out by a path template
    Folder,
    /// An iPod with video, 5th or 5.5th generation
    Ipod,
}

impl From<ExternalKind> for domain::ExternalKind {
    fn from(kind: ExternalKind) -> Self {
        match kind {
            ExternalKind::Folder => domain::ExternalKind::Folder,
            ExternalKind::Ipod => domain::ExternalKind::Ipod,
        }
    }
}

impl From<domain::ExternalKind> for ExternalKind {
    fn from(kind: domain::ExternalKind) -> Self {
        match kind {
            domain::ExternalKind::Folder => ExternalKind::Folder,
            domain::ExternalKind::Ipod => ExternalKind::Ipod,
        }
    }
}

/// What lossless sources become. Lossy ones are copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// Kept as they are: a folder library only
    Original,
    /// A folder library only
    Flac,
    /// Apple Lossless
    Alac,
    Aac,
}

impl From<OutputFormat> for domain::OutputFormat {
    fn from(format: OutputFormat) -> Self {
        match format {
            OutputFormat::Original => domain::OutputFormat::Original,
            OutputFormat::Flac => domain::OutputFormat::Flac,
            OutputFormat::Alac => domain::OutputFormat::Alac,
            OutputFormat::Aac => domain::OutputFormat::Aac,
        }
    }
}

impl From<domain::OutputFormat> for OutputFormat {
    fn from(format: domain::OutputFormat) -> Self {
        match format {
            domain::OutputFormat::Original => OutputFormat::Original,
            domain::OutputFormat::Flac => OutputFormat::Flac,
            domain::OutputFormat::Alac => OutputFormat::Alac,
            domain::OutputFormat::Aac => OutputFormat::Aac,
        }
    }
}

/// An external library: its destination and its profile.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLibrary {
    pub id: i64,
    pub name: String,
    pub kind: ExternalKind,
    /// A folder library's, an absolute path. Null for an iPod.
    pub folder: Option<String>,
    /// What lossless sources become
    pub format: OutputFormat,
    pub aac_kbps: i64,
    /// Lossless at most 16-bit and 48 kHz. Always on an iPod.
    pub cd_resolution: bool,
    /// A folder library's. Null for an iPod.
    pub path_template: Option<String>,
    pub embed_covers: bool,
    /// An embedded cover's longest side, in pixels
    pub cover_px: i64,
    /// Also a `cover.jpg` in each folder that holds one release's tracks
    pub cover_file: bool,
    /// When its last sync ended, in epoch seconds. Null before its first.
    pub last_synced_at: Option<i64>,
    /// Whether its destination is there to sync: a folder library's folder,
    /// or an iPod library's iPod, with a manifest that names it, or none
    /// yet.
    pub connected: bool,
    /// Epoch seconds
    pub inserted_at: i64,
    /// Epoch seconds
    pub updated_at: i64,
}

fn library_dto(library: &Library, external: domain::ExternalLibrary) -> ExternalLibrary {
    let connected = library.external.connected(&external);
    let profile = external.profile;
    ExternalLibrary {
        id: external.id,
        name: external.name,
        kind: external.kind.into(),
        folder: external.folder,
        format: profile.format.into(),
        aac_kbps: profile.aac_kbps,
        cd_resolution: profile.cd_resolution,
        path_template: profile.path_template,
        embed_covers: profile.embed_covers,
        cover_px: profile.cover_px,
        cover_file: profile.cover_file,
        last_synced_at: external.last_synced_at,
        connected,
        inserted_at: external.inserted_at,
        updated_at: external.updated_at,
    }
}

/// What's in an external library, and what it will take.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Contents {
    pub releases: i64,
    pub tracks: i64,
    /// An estimate: AAC from durations and the bitrate, and everything else
    /// at its own size, without covers
    pub size_bytes: i64,
    /// Tracks whose files can't be converted or tagged, which a sync leaves
    /// out
    pub left_out: i64,
}

/// An external library's page.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLibraryDetails {
    #[serde(flatten)]
    pub library: ExternalLibrary,
    pub contents: Contents,
    /// A track to show the path template with: its first, or else the
    /// catalog's newest. Null with no tracks at all.
    pub sample_track_id: Option<i64>,
    /// Whether its destination is there, and whose it is
    pub destination: DestinationStatus,
    /// The bytes free where its files go, while it's there
    pub free_bytes: Option<i64>,
    /// Whether its files are on a disk that's ejected, like a card or an
    /// iPod
    pub ejectable: bool,
    /// Whether it's syncing now
    pub syncing: bool,
    /// An iPod library's iPod, while it's connected
    pub ipod: Option<IpodInfo>,
}

/// A mounted iPod.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IpodInfo {
    /// Its volume's name, as Finder shows it
    pub name: String,
    /// Where it's mounted
    pub mount: String,
    /// What it is: "iPod with video (5.5th generation), 30 GB, black", or
    /// "An iPod"
    pub description: String,
    /// Why liquidslunk won't sync it: it's another iPod
    pub refusal: Option<String>,
    /// That its model can't be read
    pub warning: Option<String>,
}

impl From<&domain::Ipod> for IpodInfo {
    fn from(ipod: &domain::Ipod) -> Self {
        IpodInfo {
            name: ipod.name.clone(),
            mount: ipod.mount.to_string_lossy().into_owned(),
            description: ipod.description(),
            refusal: ipod.refusal(),
            warning: ipod.warning(),
        }
    }
}

/// An iPod mounted now, for a new iPod library.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MountedIpod {
    #[serde(flatten)]
    pub ipod: IpodInfo,
    /// The bytes free on it
    pub free_bytes: Option<i64>,
    /// The external library its manifest names, if it has one
    pub external_library: Option<String>,
}

/// Whether an external library's destination is there to sync.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DestinationStatus {
    /// It's there, and its manifest names this external library, or it has
    /// none yet
    Connected,
    /// It isn't there: a folder that moved, a card that isn't in, or no
    /// iPod that's its
    Missing,
    /// Its manifest names another external library
    Elsewhere { name: String },
    /// Several iPods without a manifest are connected, and it isn't known
    /// which is an iPod library's
    Several,
}

/// An external library to save. An iPod's are what it plays: it has no
/// folder or template, it's always at CD resolution, and it has no embedded
/// covers or `cover.jpg`, so those are ignored.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLibraryInput {
    /// Unique ignoring case
    pub name: String,
    /// It can't change once it's created
    pub kind: ExternalKind,
    /// A folder library's: an absolute path, outside the library's folder,
    /// with no registered files in it
    #[serde(default)]
    #[specta(optional)]
    pub folder: Option<String>,
    /// On an iPod, Apple Lossless or AAC
    pub format: OutputFormat,
    /// 128 to 320
    pub aac_kbps: i64,
    pub cd_resolution: bool,
    /// A folder library's. Blank is the default template.
    #[serde(default)]
    #[specta(optional)]
    pub path_template: Option<String>,
    pub embed_covers: bool,
    /// 100 to 3000
    pub cover_px: i64,
    pub cover_file: bool,
}

impl From<ExternalLibraryInput> for domain::ExternalLibraryInput {
    fn from(input: ExternalLibraryInput) -> Self {
        domain::ExternalLibraryInput {
            name: input.name,
            kind: input.kind.into(),
            folder: input.folder,
            format: input.format.into(),
            aac_kbps: input.aac_kbps,
            cd_resolution: input.cd_resolution,
            path_template: input.path_template,
            embed_covers: input.embed_covers,
            cover_px: input.cover_px,
            cover_file: input.cover_file,
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExternalLibraryArgs {
    pub id: i64,
    #[serde(flatten)]
    pub input: ExternalLibraryInput,
}

/// A release a rule names.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleRelease {
    pub id: i64,
    pub title: String,
    pub title_language: Option<String>,
    pub cover: Option<FileRef>,
    /// As credited
    pub artists: Vec<CreditedArtist>,
}

/// A track a rule names.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleTrack {
    pub id: i64,
    pub title: String,
    /// Null: the release's
    pub title_language: Option<String>,
    pub release: ReleaseHeader,
}

/// What a rule names.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuleTarget {
    /// Its tracks
    Release {
        release: RuleRelease,
    },
    /// The tracks of every release the artist is a release artist of
    Artist {
        artist: ArtistRef,
    },
    Track {
        track: RuleTrack,
    },
    /// The releases with the value, whole, and the tracks with it. Text
    /// matches `equals`, ignoring case; numbers and dates `equals`, or the
    /// range from `min` to `max`, both included.
    Tag {
        tag: Tag,
        equals: Option<String>,
        min: Option<String>,
        max: Option<String>,
    },
    /// Every track in the library, now and later
    Everything,
}

/// A rule of an external library's selection.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRule {
    /// Left out, whatever else brings it in: a release or a track
    pub exclude: bool,
    pub target: RuleTarget,
}

/// What a rule names, to save it.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RuleTargetInput {
    Release {
        id: i64,
    },
    Artist {
        id: i64,
    },
    Track {
        id: i64,
    },
    /// A tag filter, as the lists take them: text `equals`, and numbers and
    /// dates `equals`, or a range from `min` to `max`
    Tag {
        tag: TagArgs,
        equals: Option<String>,
        min: Option<String>,
        max: Option<String>,
    },
    /// The whole library, now and later
    Everything,
}

impl From<RuleTargetInput> for domain::RuleTarget {
    fn from(target: RuleTargetInput) -> Self {
        match target {
            RuleTargetInput::Release { id } => domain::RuleTarget::Release(id),
            RuleTargetInput::Artist { id } => domain::RuleTarget::Artist(id),
            RuleTargetInput::Track { id } => domain::RuleTarget::Track(id),
            RuleTargetInput::Tag {
                tag,
                equals,
                min,
                max,
            } => domain::RuleTarget::Tag {
                kind: tag.kind.into(),
                tag_id: tag.id,
                equals,
                min,
                max,
            },
            RuleTargetInput::Everything => domain::RuleTarget::Everything,
        }
    }
}

/// A rule to save.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    /// Left out: a release or a track only. Default false.
    #[serde(default)]
    #[specta(optional)]
    pub exclude: bool,
    pub target: RuleTargetInput,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetRulesArgs {
    pub id: i64,
    /// In order. A rule given twice is kept once.
    pub rules: Vec<RuleInput>,
}

/// What the catalog's pages add to an external library.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AddArgs {
    /// The external library
    pub id: i64,
    /// Each comes in whole
    #[serde(default)]
    #[specta(optional)]
    pub releases: Vec<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub artists: Vec<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub tracks: Vec<i64>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoveArgs {
    /// The external library
    pub id: i64,
    pub target: RuleTargetInput,
}

/// A release, an artist or a track: exactly one.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct MembershipsArgs {
    #[specta(optional)]
    pub release: Option<i64>,
    #[specta(optional)]
    pub artist: Option<i64>,
    #[specta(optional)]
    pub track: Option<i64>,
}

/// An external library, by name.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalLibraryRef {
    pub id: i64,
    pub name: String,
    pub kind: ExternalKind,
}

/// Why something is in an external library.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MembershipReason {
    /// A rule names it
    Own,
    /// A broader rule brings it in: its release, an artist, a tag value,
    /// the whole library
    Through { rule: ExternalRule },
    /// Only some of a release's tracks, through rules of their own
    Tracks,
}

/// An external library something is in.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalMembership {
    pub library: ExternalLibraryRef,
    pub reason: MembershipReason,
    /// Its tracks that are in it
    pub tracks: i64,
    /// Its tracks
    pub total: i64,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTracksArgs {
    pub id: i64,
    /// From 1. Default 1.
    #[serde(default)]
    #[specta(optional)]
    pub page: Option<i64>,
    /// Default 50, at most 200
    #[serde(default)]
    #[specta(optional)]
    pub per_page: Option<i64>,
}

/// A page of what's in an external library.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrackPage {
    pub items: Vec<TrackListItem>,
    pub total: i64,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportedTagsArgs {
    /// A track
    pub id: i64,
    /// A path template. Default: the default template.
    #[serde(default)]
    #[specta(optional)]
    pub template: Option<String>,
}

/// A track's tags, as every external library writes them, whatever its
/// format (backend.md, "A track's tags"). A field with no value isn't
/// written: it's null or empty.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportedTags {
    /// With its features: "Bridges (feat. Mari, Sander)"
    pub title: String,
    /// Who performs it, as credited, joined with ", "
    pub artist: Option<String>,
    /// Each performer, then each feature, as credited
    pub artists: Vec<String>,
    /// The release's title
    pub album: String,
    /// The release's artists, as credited, joined with ", "
    pub album_artist: Option<String>,
    /// A various-artists release
    pub compilation: bool,
    /// `2006`, `2006-01` or `2006-01-10`
    pub date: Option<String>,
    pub track_number: i64,
    /// The last track number on its disc
    pub track_total: i64,
    pub disc_number: i64,
    /// The release's last disc
    pub disc_total: i64,
    pub disc_title: Option<String>,
    pub labels: Vec<String>,
    pub catalog_number: Option<String>,
    pub media: Vec<String>,
    /// The release's formats
    pub release_types: Vec<String>,
    /// What players sort the artist by, when it isn't in Latin script
    pub artist_sort: Option<String>,
    pub album_artist_sort: Option<String>,
    pub album_sort: Option<String>,
    pub title_sort: Option<String>,
    /// The release's cover
    pub cover: Option<File>,
    /// The template the path comes from
    pub template: String,
    /// Where a folder library that keeps the original files puts the track,
    /// by the template
    pub path: String,
}

/// Every external library, by name ignoring case, for the sidebar and the
/// list.
#[tauri::command]
#[specta::specta]
pub async fn external_libraries(app: AppHandle) -> Result<Vec<ExternalLibrary>, CommandError> {
    on_library(app, list).await
}

pub fn list(library: &Library) -> Result<Vec<ExternalLibrary>, CommandError> {
    Ok(library
        .external
        .list()?
        .into_iter()
        .map(|external| library_dto(library, external))
        .collect())
}

/// An external library's page: what's in it, and what it will take. Null for
/// an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn external_library(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<ExternalLibraryDetails>, CommandError> {
    on_library(app, move |library| details(library, args)).await
}

pub fn details(
    library: &Library,
    args: IdArgs,
) -> Result<Option<ExternalLibraryDetails>, CommandError> {
    let external = &library.external;
    let Some(found) = external.get(args.id)? else {
        return Ok(None);
    };
    let contents = external.contents(args.id)?;
    Ok(Some(ExternalLibraryDetails {
        sample_track_id: external.sample_track(args.id)?,
        destination: match external.destination(&found) {
            Destination::Connected => DestinationStatus::Connected,
            Destination::Missing => DestinationStatus::Missing,
            Destination::Elsewhere(name) => DestinationStatus::Elsewhere { name },
            Destination::Several => DestinationStatus::Several,
        },
        ipod: match found.kind {
            domain::ExternalKind::Ipod => external.find_ipod(&found).1.as_ref().map(IpodInfo::from),
            domain::ExternalKind::Folder => None,
        },
        free_bytes: external
            .free_bytes(&found)
            .map(|free| i64::try_from(free).unwrap_or(i64::MAX)),
        ejectable: external.ejectable(&found).is_some(),
        syncing: external.syncing() == Some(args.id),
        contents: Contents {
            releases: contents.releases,
            tracks: contents.tracks,
            size_bytes: contents.size_bytes,
            left_out: contents.left_out,
        },
        library: library_dto(library, found),
    }))
}

/// A new external library, with no rules yet.
#[tauri::command]
#[specta::specta]
pub async fn create_external_library(
    app: AppHandle,
    args: ExternalLibraryInput,
) -> Result<ExternalLibrary, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(
    library: &Library,
    args: ExternalLibraryInput,
) -> Result<ExternalLibrary, CommandError> {
    let created = library.external.create(&args.into())?;
    Ok(library_dto(library, created))
}

/// Saves an external library's name, folder and profile. Its kind can't
/// change.
#[tauri::command]
#[specta::specta]
pub async fn update_external_library(
    app: AppHandle,
    args: UpdateExternalLibraryArgs,
) -> Result<ExternalLibrary, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(
    library: &Library,
    args: UpdateExternalLibraryArgs,
) -> Result<ExternalLibrary, CommandError> {
    let saved = library.external.update(args.id, &args.input.into())?;
    Ok(library_dto(library, saved))
}

/// An external library to delete.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeleteExternalLibraryArgs {
    pub id: i64,
    /// Also remove what liquidslunk wrote on its destination, which has to
    /// be connected. Otherwise its files stay there.
    #[serde(default)]
    #[specta(optional)]
    pub remove_files: Option<bool>,
}

/// Deletes an external library and its rules, and with `removeFiles`, what
/// liquidslunk wrote on its destination: the files its manifest lists, the
/// folders that leaves empty, and the manifest. Returns the id.
#[tauri::command]
#[specta::specta]
pub async fn delete_external_library(
    app: AppHandle,
    args: DeleteExternalLibraryArgs,
) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: DeleteExternalLibraryArgs) -> Result<i64, CommandError> {
    library
        .external
        .delete(args.id, args.remove_files.unwrap_or(false))?;
    Ok(args.id)
}

/// A folder library's folder, where it is now.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocateArgs {
    pub id: i64,
    /// An absolute path
    pub folder: String,
}

/// Points a folder library at its folder where it is now: one that moved,
/// or a card that mounted under another name. Refused when the folder's
/// manifest names another external library.
#[tauri::command]
#[specta::specta]
pub async fn locate_external_library(
    app: AppHandle,
    args: LocateArgs,
) -> Result<ExternalLibrary, CommandError> {
    on_library(app, move |library| {
        let located = library.external.locate(args.id, &args.folder)?;
        Ok(library_dto(library, located))
    })
    .await
}

/// Ejects the disk an external library's files are on, like a card or an
/// iPod, so it can be unplugged.
#[tauri::command]
#[specta::specta]
pub async fn eject_external_library(app: AppHandle, args: IdArgs) -> Result<(), CommandError> {
    on_library(app, move |library| Ok(library.external.eject(args.id)?)).await
}

/// The iPods mounted now, each with its free space and the external library
/// it belongs to, for a new iPod library (backend.md, "Syncing an iPod").
#[tauri::command]
#[specta::specta]
pub async fn mounted_ipods(app: AppHandle) -> Result<Vec<MountedIpod>, CommandError> {
    on_library(app, move |library| {
        Ok(library
            .external
            .mounted_ipods()
            .into_iter()
            .map(|(ipod, whose)| MountedIpod {
                free_bytes: domain::volume::free_space(&ipod.mount)
                    .map(|free| i64::try_from(free).unwrap_or(i64::MAX)),
                ipod: IpodInfo::from(&ipod),
                external_library: whose,
            })
            .collect())
    })
    .await
}

/// What a sync does, as `sync_plan` shows it.
#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncAction {
    /// A file that should be there and isn't
    Add,
    /// Converted again: its audio file or its format changed. A `cover.jpg`
    /// whose cover changed.
    Convert,
    /// Its file is missing on the destination, or not as liquidslunk wrote
    /// it
    Restore,
    /// Its tags or cover changed
    Retag,
    /// Its path changed, and its tags too when `changes` says so
    Move,
    /// It's left the selection, or been deleted
    Remove,
}

impl From<Action> for SyncAction {
    fn from(action: Action) -> Self {
        match action {
            Action::Add => SyncAction::Add,
            Action::Convert => SyncAction::Convert,
            Action::Restore => SyncAction::Restore,
            Action::Retag => SyncAction::Retag,
            Action::Move => SyncAction::Move,
            Action::Remove => SyncAction::Remove,
        }
    }
}

/// How a plan names a track, by its title, artist and album, or a release,
/// by its title and artist.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncLabel {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
}

/// A field a retag changes: "Title: Shoka → 初夏".
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagChange {
    pub field: String,
    pub was: Vec<String>,
    pub now: Vec<String>,
}

/// A step of a sync's plan.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    pub action: SyncAction,
    /// A track's file. Null for a release's `cover.jpg`.
    pub track_id: Option<i64>,
    /// A `cover.jpg`'s release
    pub release_id: Option<i64>,
    pub label: SyncLabel,
    /// Where the file goes, or a removal's, where it is, relative to the
    /// folder or the iPod
    pub path: String,
    /// Where it is now, when that's elsewhere
    pub from: Option<String>,
    /// What a retag changes, or a move that retags too
    pub changes: Vec<TagChange>,
    /// What it writes, estimated, or a removal's, what it frees
    pub bytes: i64,
}

/// A track on an iPod that liquidslunk didn't write, which its first sync
/// erases once the user agrees.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanOtherTrack {
    pub label: SyncLabel,
    /// Relative to the iPod
    pub path: String,
}

/// A track or a cover a sync leaves out, and why.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanLeftOut {
    pub track_id: Option<i64>,
    /// A release whose cover is left out
    pub release_id: Option<i64>,
    pub label: SyncLabel,
    pub reason: String,
}

/// What a sync would do, before it does anything.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    /// Which plan it is: `sync` runs it by this id
    pub id: i64,
    pub library_id: i64,
    pub steps: Vec<PlanStep>,
    pub left_out: Vec<PlanLeftOut>,
    /// Files that stay as they are
    pub unchanged: i64,
    /// What it writes, estimated
    pub added_bytes: i64,
    /// What the files it removes or replaces take
    pub freed_bytes: i64,
    /// What it needs: what it writes, less what it frees
    pub needed_bytes: i64,
    /// What's free on the folder's disk, or the iPod's
    pub free_bytes: Option<i64>,
    /// Whether it fits. A sync that doesn't is refused.
    pub fits: bool,
    /// The iPod it syncs to
    pub ipod: Option<IpodInfo>,
    /// On an iPod, the tracks liquidslunk didn't write: `sync` erases them
    /// first, with `erase`, and is refused without it
    pub other: Vec<PlanOtherTrack>,
}

fn label(plan: &domain::Plan, subject: Subject, path: &str) -> SyncLabel {
    match plan.labels.get(&subject) {
        Some(label) => SyncLabel {
            title: label.title.clone(),
            artist: label.artist.clone(),
            album: label.album.clone(),
        },
        None => SyncLabel {
            title: path.to_string(),
            artist: None,
            album: None,
        },
    }
}

fn subject_ids(subject: Subject) -> (Option<i64>, Option<i64>) {
    match subject {
        Subject::Track(id) => (Some(id), None),
        Subject::Release(id) => (None, Some(id)),
    }
}

/// Works out what a sync would do, without doing anything: what it adds,
/// converts again, restores, retags, moves and removes, what it leaves out
/// and why, whether it fits, and on an iPod, the tracks liquidslunk didn't
/// write. `sync` runs it. Refused when its folder or iPod isn't there, or
/// belongs to another external library.
#[tauri::command]
#[specta::specta]
pub async fn sync_plan(app: AppHandle, args: IdArgs) -> Result<SyncPlan, CommandError> {
    on_library(app, move |library| {
        let planned = library.external.plan_sync(args.id)?;
        let plan = &planned.plan;
        Ok(SyncPlan {
            id: i64::try_from(planned.id).unwrap_or(i64::MAX),
            library_id: planned.library.id,
            steps: plan
                .steps
                .iter()
                .map(|step| {
                    let (track_id, release_id) = subject_ids(step.subject);
                    PlanStep {
                        action: step.action.into(),
                        track_id,
                        release_id,
                        label: label(plan, step.subject, &step.path),
                        path: step.path.clone(),
                        from: step.from.clone(),
                        changes: step
                            .changes
                            .iter()
                            .map(|change| TagChange {
                                field: change.field.clone(),
                                was: change.was.clone(),
                                now: change.now.clone(),
                            })
                            .collect(),
                        bytes: step.bytes,
                    }
                })
                .collect(),
            left_out: plan
                .left_out
                .iter()
                .map(|left| {
                    let (track_id, release_id) = subject_ids(left.subject);
                    PlanLeftOut {
                        track_id,
                        release_id,
                        label: label(plan, left.subject, ""),
                        reason: left.reason.clone(),
                    }
                })
                .collect(),
            unchanged: plan.unchanged as i64,
            added_bytes: plan.added,
            freed_bytes: plan.freed,
            needed_bytes: plan.bytes_needed(),
            free_bytes: planned
                .free_bytes
                .map(|free| i64::try_from(free).unwrap_or(i64::MAX)),
            fits: planned.fits(),
            ipod: planned.ipod.as_ref().map(IpodInfo::from),
            other: plan
                .other
                .iter()
                .map(|other| PlanOtherTrack {
                    label: SyncLabel {
                        title: other.label.title.clone(),
                        artist: other.label.artist.clone(),
                        album: other.label.album.clone(),
                    },
                    path: other.path.clone(),
                })
                .collect(),
        })
    })
    .await
}

/// The plan to run.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncArgs {
    pub id: i64,
    /// The plan `sync_plan` showed
    pub plan_id: i64,
    /// On an iPod, erase the tracks liquidslunk didn't write, which the
    /// user agreed to
    #[serde(default)]
    #[specta(optional)]
    pub erase: Option<bool>,
}

/// Where a sync is.
#[derive(Debug, Clone, Copy, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SyncStage {
    /// On an iPod, erasing the tracks liquidslunk didn't write
    Erasing,
    Removing,
    Moving,
    /// Adding, converting again and restoring
    Writing,
    Retagging,
    Covers,
    /// On an iPod, writing its database and artwork
    Database,
    Finishing,
}

/// What a running sync says about itself.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub stage: SyncStage,
    /// The plan's steps done, and all of them
    pub done: i64,
    pub total: i64,
    /// Written to the folder or the iPod so far
    pub bytes: i64,
    /// What's in hand: a track's title, or a path
    pub current: Option<String>,
}

impl From<Progress> for SyncProgress {
    fn from(progress: Progress) -> Self {
        SyncProgress {
            stage: match progress.stage {
                Stage::Erasing => SyncStage::Erasing,
                Stage::Removing => SyncStage::Removing,
                Stage::Moving => SyncStage::Moving,
                Stage::Writing => SyncStage::Writing,
                Stage::Retagging => SyncStage::Retagging,
                Stage::Covers => SyncStage::Covers,
                Stage::Database => SyncStage::Database,
                Stage::Finishing => SyncStage::Finishing,
            },
            done: progress.done as i64,
            total: progress.total as i64,
            bytes: i64::try_from(progress.bytes).unwrap_or(i64::MAX),
            current: progress.current,
        }
    }
}

/// A file a sync couldn't write, and why.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncFailure {
    pub track_id: Option<i64>,
    pub release_id: Option<i64>,
    pub path: String,
    pub reason: String,
}

/// How a sync went: the steps done, by action, and the files it couldn't
/// write.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncOutcome {
    pub added: i64,
    pub converted: i64,
    pub restored: i64,
    pub retagged: i64,
    pub moved: i64,
    pub removed: i64,
    pub failed: Vec<SyncFailure>,
    /// Stopped by `cancel_sync`, after the file in hand
    pub cancelled: bool,
    /// Written to the folder or the iPod
    pub bytes: i64,
    /// On an iPod, the tracks liquidslunk didn't write that it erased
    pub erased: i64,
}

impl From<Outcome> for SyncOutcome {
    fn from(outcome: Outcome) -> Self {
        let done = |action| outcome.done.get(&action).copied().unwrap_or(0) as i64;
        SyncOutcome {
            added: done(Action::Add),
            converted: done(Action::Convert),
            restored: done(Action::Restore),
            retagged: done(Action::Retag),
            moved: done(Action::Move),
            removed: done(Action::Remove),
            failed: outcome
                .failed
                .into_iter()
                .map(|failure| {
                    let (track_id, release_id) = subject_ids(failure.subject);
                    SyncFailure {
                        track_id,
                        release_id,
                        path: failure.path,
                        reason: failure.reason,
                    }
                })
                .collect(),
            cancelled: outcome.cancelled,
            bytes: i64::try_from(outcome.bytes).unwrap_or(i64::MAX),
            erased: outcome.erased as i64,
        }
    }
}

/// Runs the plan `sync_plan` last showed, with its progress. One sync runs
/// at a time. Refused: a plan that isn't the one last shown, one that
/// doesn't fit, a folder or an iPod that changed since, and on an iPod with
/// tracks liquidslunk didn't write, a sync without `erase`.
#[tauri::command]
#[specta::specta]
pub async fn sync(
    app: AppHandle,
    args: SyncArgs,
    progress: Channel<SyncProgress>,
) -> Result<SyncOutcome, CommandError> {
    on_library(app, move |library| {
        let plan_id = u64::try_from(args.plan_id).unwrap_or(0);
        let erase = args.erase.unwrap_or(false);
        let outcome = library.external.sync(args.id, plan_id, erase, &|step| {
            let _ = progress.send(step.into());
        })?;
        Ok(outcome.into())
    })
    .await
}

/// Cancels the sync running, if one is: it stops after the file in hand,
/// and keeps what's done.
#[tauri::command]
#[specta::specta]
pub async fn cancel_sync(app: AppHandle) -> Result<(), CommandError> {
    on_library(app, move |library| {
        library.external.cancel_sync();
        Ok(())
    })
    .await
}

/// An external library's rules, in the order they were added.
#[tauri::command]
#[specta::specta]
pub async fn external_library_rules(
    app: AppHandle,
    args: IdArgs,
) -> Result<Vec<ExternalRule>, CommandError> {
    on_library(app, move |library| rules(library, args)).await
}

pub fn rules(library: &Library, args: IdArgs) -> Result<Vec<ExternalRule>, CommandError> {
    let rules = library.external.rules(args.id)?;
    rule_dtos(library, &rules)
}

/// Replaces an external library's rules, and returns them as saved.
#[tauri::command]
#[specta::specta]
pub async fn set_external_library_rules(
    app: AppHandle,
    args: SetRulesArgs,
) -> Result<Vec<ExternalRule>, CommandError> {
    on_library(app, move |library| set_rules(library, args)).await
}

pub fn set_rules(library: &Library, args: SetRulesArgs) -> Result<Vec<ExternalRule>, CommandError> {
    let rules: Vec<domain::RuleValues> = args
        .rules
        .into_iter()
        .map(|rule| domain::RuleValues {
            exclude: rule.exclude,
            target: rule.target.into(),
        })
        .collect();
    let saved = library.external.set_rules(args.id, &rules)?;
    rule_dtos(library, &saved)
}

/// Adds releases, artists or tracks to an external library, from the
/// catalog's pages and the release list's bulk edits. A release comes in
/// whole. Ids that aren't there are skipped.
#[tauri::command]
#[specta::specta]
pub async fn add_to_external_library(app: AppHandle, args: AddArgs) -> Result<(), CommandError> {
    on_library(app, move |library| add(library, args)).await
}

pub fn add(library: &Library, args: AddArgs) -> Result<(), CommandError> {
    let additions = domain::Additions {
        releases: args.releases,
        artists: args.artists,
        tracks: args.tracks,
    };
    Ok(library.external.add(args.id, &additions)?)
}

/// Takes a release, an artist, a track, a tag value or the whole library out
/// of an external library: its own rule goes, and a release or track that a
/// broader rule still brings in is left out.
#[tauri::command]
#[specta::specta]
pub async fn remove_from_external_library(
    app: AppHandle,
    args: RemoveArgs,
) -> Result<(), CommandError> {
    on_library(app, move |library| remove(library, args)).await
}

pub fn remove(library: &Library, args: RemoveArgs) -> Result<(), CommandError> {
    Ok(library.external.remove(args.id, &args.target.into())?)
}

/// The external libraries a release, an artist or a track is in, by name,
/// and why. An artist is in those with a rule for it.
#[tauri::command]
#[specta::specta]
pub async fn external_memberships(
    app: AppHandle,
    args: MembershipsArgs,
) -> Result<Vec<ExternalMembership>, CommandError> {
    on_library(app, move |library| memberships(library, args)).await
}

pub fn memberships(
    library: &Library,
    args: MembershipsArgs,
) -> Result<Vec<ExternalMembership>, CommandError> {
    let subject = match (args.release, args.artist, args.track) {
        (Some(id), None, None) => domain::Subject::Release(id),
        (None, Some(id), None) => domain::Subject::Artist(id),
        (None, None, Some(id)) => domain::Subject::Track(id),
        _ => {
            return Err(CommandError::invalid(
                "Ask about one release, artist or track",
            ));
        }
    };
    let found = library.external.memberships(subject)?;
    let rules: Vec<domain::Rule> = found
        .iter()
        .filter_map(|membership| match &membership.reason {
            domain::Reason::Through(rule) => Some(rule.clone()),
            _ => None,
        })
        .collect();
    let mut named = Named::load(library, &rules)?;
    Ok(found
        .into_iter()
        .filter_map(|membership| {
            let reason = match membership.reason {
                domain::Reason::Own => MembershipReason::Own,
                domain::Reason::Tracks => MembershipReason::Tracks,
                domain::Reason::Through(rule) => MembershipReason::Through {
                    rule: named.rule(&rule)?,
                },
            };
            Some(ExternalMembership {
                library: ExternalLibraryRef {
                    id: membership.library.id,
                    name: membership.library.name,
                    kind: membership.library.kind.into(),
                },
                reason,
                tracks: membership.tracks,
                total: membership.total,
            })
        })
        .collect())
}

/// A page of what's in an external library, by release title, then disc and
/// number, as the track list shows tracks.
#[tauri::command]
#[specta::specta]
pub async fn external_library_tracks(
    app: AppHandle,
    args: LibraryTracksArgs,
) -> Result<LibraryTrackPage, CommandError> {
    on_library(app, move |library| tracks(library, args)).await
}

pub fn tracks(
    library: &Library,
    args: LibraryTracksArgs,
) -> Result<LibraryTrackPage, CommandError> {
    let paging = slunk::paging::paging_of(&slunk::paging::ListArgs {
        page: args.page,
        per_page: args.per_page,
        ..slunk::paging::ListArgs::default()
    })?;
    let ids = library.external.track_ids(args.id)?;
    let page: Vec<i64> = ids
        .iter()
        .skip(paging.offset as usize)
        .take(paging.limit as usize)
        .copied()
        .collect();
    let mut found: HashMap<i64, slunk::catalog::Track> = library
        .catalog
        .tracks_by_ids(&page)?
        .into_iter()
        .map(|track| (track.id, track))
        .collect();
    let ordered = page.iter().filter_map(|id| found.remove(id)).collect();
    Ok(LibraryTrackPage {
        items: list_items(library, ordered)?,
        total: ids.len() as i64,
    })
}

/// A track's tags as an external library writes them, and its path by a
/// template, the default one unless one is given, or null for an unknown id.
/// A template that can't be used is `invalid`, with the reason.
#[tauri::command]
#[specta::specta]
pub async fn exported_tags(
    app: AppHandle,
    args: ExportedTagsArgs,
) -> Result<Option<ExportedTags>, CommandError> {
    on_library(app, move |library| exported(library, args)).await
}

pub fn exported(
    library: &Library,
    args: ExportedTagsArgs,
) -> Result<Option<ExportedTags>, CommandError> {
    let template = args.template.unwrap_or_else(|| DEFAULT_TEMPLATE.into());
    let parsed = Template::parse(&template)?;
    let Some(exported) = library.external.exported(args.id, &parsed)? else {
        return Ok(None);
    };
    let tags = exported.tags;
    let cover = match tags.cover {
        Some(id) => files_by_id(library, &[id])?.remove(&id),
        None => None,
    };
    Ok(Some(ExportedTags {
        title: tags.title,
        artist: tags.artist,
        artists: tags.artists,
        album: tags.album,
        album_artist: tags.album_artist,
        compilation: tags.compilation,
        date: tags.date,
        track_number: tags.track_number,
        track_total: tags.track_total,
        disc_number: tags.disc_number,
        disc_total: tags.disc_total,
        disc_title: tags.disc_title,
        labels: tags.labels,
        catalog_number: tags.catalog_number,
        media: tags.media,
        release_types: tags.release_types,
        artist_sort: tags.sort.artist,
        album_artist_sort: tags.sort.album_artist,
        album_sort: tags.sort.album,
        title_sort: tags.sort.title,
        cover,
        template,
        path: exported.path,
    }))
}

/// Rules as the pages show them, in order.
fn rule_dtos(library: &Library, rules: &[domain::Rule]) -> Result<Vec<ExternalRule>, CommandError> {
    let mut named = Named::load(library, rules)?;
    Ok(rules.iter().filter_map(|rule| named.rule(rule)).collect())
}

/// What rules name, loaded with one query per kind.
struct Named {
    releases: HashMap<i64, slunk::catalog::Release>,
    release_artists: HashMap<i64, Vec<slunk::catalog::ArtistLink>>,
    tracks: HashMap<i64, slunk::catalog::Track>,
    track_releases: HashMap<i64, slunk::catalog::Release>,
    tags: HashMap<(slunk::tagging::TagType, i64), Tag>,
    related: Related,
}

impl Named {
    fn load(library: &Library, rules: &[domain::Rule]) -> Result<Named, CommandError> {
        let catalog = &library.catalog;
        let (mut release_ids, mut artist_ids, mut track_ids, mut tag_refs) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for rule in rules {
            match &rule.target {
                domain::RuleTarget::Release(id) => release_ids.push(*id),
                domain::RuleTarget::Artist(id) => artist_ids.push(*id),
                domain::RuleTarget::Track(id) => track_ids.push(*id),
                domain::RuleTarget::Tag { kind, tag_id, .. } => tag_refs.push((*kind, *tag_id)),
                domain::RuleTarget::Everything => {}
            }
        }
        let by_id = |releases: Vec<slunk::catalog::Release>| -> HashMap<i64, _> {
            releases
                .into_iter()
                .map(|release| (release.id, release))
                .collect()
        };
        let releases = by_id(catalog.releases_by_ids(&release_ids)?);
        let release_artists = catalog.release_artists_of(&release_ids)?;
        let tracks: HashMap<i64, slunk::catalog::Track> = catalog
            .tracks_by_ids(&track_ids)?
            .into_iter()
            .map(|track| (track.id, track))
            .collect();
        let track_release_ids: Vec<i64> = tracks.values().map(|track| track.release_id).collect();
        let track_releases = by_id(catalog.releases_by_ids(&track_release_ids)?);
        let mut related = Related::default();
        let artist_links: Vec<slunk::catalog::ArtistLink> = artist_ids
            .iter()
            .map(|&artist_id| slunk::catalog::ArtistLink {
                artist_id,
                credited_name_id: None,
            })
            .collect();
        related.load_artists(
            library,
            release_artists.values().flatten().chain(&artist_links),
            false,
        )?;
        Ok(Named {
            releases,
            release_artists,
            tracks,
            track_releases,
            tags: tags_by_id(library, &tag_refs)?,
            related,
        })
    }

    fn rule(&mut self, rule: &domain::Rule) -> Option<ExternalRule> {
        let target = match &rule.target {
            domain::RuleTarget::Release(id) => {
                let release = self.releases.get(id)?;
                RuleTarget::Release {
                    release: RuleRelease {
                        id: release.id,
                        title: release.title.clone(),
                        title_language: release.title_language.clone(),
                        cover: release.cover_picture.map(|id| FileRef { id }),
                        artists: self.related.credited_all(
                            self.release_artists
                                .get(id)
                                .map(Vec::as_slice)
                                .unwrap_or_default(),
                        ),
                    },
                }
            }
            domain::RuleTarget::Artist(id) => RuleTarget::Artist {
                artist: self
                    .related
                    .credited(&slunk::catalog::ArtistLink {
                        artist_id: *id,
                        credited_name_id: None,
                    })?
                    .artist,
            },
            domain::RuleTarget::Track(id) => {
                let track = self.tracks.get(id)?;
                let release = self.track_releases.get(&track.release_id)?;
                RuleTarget::Track {
                    track: RuleTrack {
                        id: track.id,
                        title: track.title.clone(),
                        title_language: track.title_language.clone(),
                        release: ReleaseHeader {
                            id: release.id,
                            title: release.title.clone(),
                            title_language: release.title_language.clone(),
                        },
                    },
                }
            }
            domain::RuleTarget::Tag {
                kind,
                tag_id,
                equals,
                min,
                max,
            } => RuleTarget::Tag {
                tag: self.tags.get(&(*kind, *tag_id))?.clone(),
                equals: equals.clone(),
                min: min.clone(),
                max: max.clone(),
            },
            domain::RuleTarget::Everything => RuleTarget::Everything,
        };
        Some(ExternalRule {
            exclude: rule.exclude,
            target,
        })
    }
}

#[cfg(test)]
mod tests {
    use slunk::repo::{connection, migrator};

    use super::*;

    const HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("library").join("slunk.db");
        std::fs::create_dir_all(db.parent().unwrap()).unwrap();
        migrator::migrate(&db, 10).unwrap();
        let library = Library::new(&db, connection::open(&db).unwrap());
        library
            .store
            .conn()
            .execute_batch(&format!(
                r#"
                insert into files (id, path, type, size_bytes, mtime, sha256) values
                    (1, 'Scenery/01.flac', 'music', 30000000, 0, '{HASH}'),
                    (2, 'Scenery/cover.jpg', 'image', 1, 0, '{HASH}'),
                    (3, 'Scenery/02.flac', 'music', 30000000, 0, '{HASH}');
                insert into artists (id, name) values (1, 'Ryo Fukui');
                insert into artist_alternate_names (id, artist_id, name, position)
                    values (5, 1, '福居良', 0);
                insert into releases (id, title, cover_picture, various_artists)
                    values (1, 'Scenery', 2, 0);
                insert into artists_releases (artist_id, release_id, position, credited_name_id)
                    values (1, 1, 0, 5);
                insert into tracks (id, title, track_number, cd_number, release_id, file_id, duration_ms) values
                    (1, 'It Could Happen to You', 1, 1, 1, 1, 240000),
                    (2, 'Early Summer', 2, 1, 1, 3, 240000);
                insert into text_tags (id, name, multiple) values (1, 'Genre', 1);
                insert into release_text_tags (release_id, tag_id, value) values (1, 1, 'Jazz');
                "#
            ))
            .unwrap();
        (dir, library)
    }

    fn input(name: &str, folder: &str) -> ExternalLibraryInput {
        ExternalLibraryInput {
            name: name.into(),
            kind: ExternalKind::Folder,
            folder: Some(folder.into()),
            format: OutputFormat::Aac,
            aac_kbps: 256,
            cd_resolution: false,
            path_template: None,
            embed_covers: true,
            cover_px: 600,
            cover_file: false,
        }
    }

    #[test]
    fn exported_tags_come_with_the_cover_and_the_templates_path() {
        let (_dir, library) = library();
        let args = |template: Option<&str>| ExportedTagsArgs {
            id: 1,
            template: template.map(str::to_string),
        };

        let tags = exported(&library, args(None)).unwrap().unwrap();
        assert_eq!(tags.artist.as_deref(), Some("福居良"));
        assert_eq!(
            tags.cover.map(|cover| cover.path).as_deref(),
            Some("Scenery/cover.jpg")
        );
        assert_eq!(tags.template, DEFAULT_TEMPLATE);
        assert_eq!(tags.path, "福居良/Scenery/01 It Could Happen to You.flac");

        let tags = exported(&library, args(Some("${track_title}.${format}")))
            .unwrap()
            .unwrap();
        assert_eq!(tags.path, "It Could Happen to You.flac");

        let error = exported(&library, args(Some("${genre}.${format}"))).unwrap_err();
        assert_eq!(error.message, "${genre} isn't a template variable");

        let unknown = ExportedTagsArgs {
            id: 99,
            template: None,
        };
        assert!(exported(&library, unknown).unwrap().is_none());
    }

    #[test]
    fn an_external_library_shows_its_rules_contents_and_memberships() {
        let (dir, library) = library();
        let folder = dir.path().join("car").to_string_lossy().to_string();
        let car = create(&library, input("Car", &folder)).unwrap();
        assert!(!car.connected);
        assert_eq!(list(&library).unwrap().len(), 1);

        let saved = set_rules(
            &library,
            SetRulesArgs {
                id: car.id,
                rules: vec![
                    RuleInput {
                        exclude: false,
                        target: RuleTargetInput::Tag {
                            tag: TagArgs {
                                kind: super::super::tagging::TagType::Text,
                                id: 1,
                            },
                            equals: Some("jazz".into()),
                            min: None,
                            max: None,
                        },
                    },
                    RuleInput {
                        exclude: true,
                        target: RuleTargetInput::Track { id: 2 },
                    },
                ],
            },
        )
        .unwrap();
        // Rules come with what they name
        let [jazz, left_out] = &saved[..] else {
            panic!("{saved:?}")
        };
        assert!(
            matches!(&jazz.target, RuleTarget::Tag { tag, equals, .. } if tag.name == "Genre" && equals.as_deref() == Some("jazz"))
        );
        assert!(
            matches!(&left_out.target, RuleTarget::Track { track } if left_out.exclude && track.release.title == "Scenery")
        );

        let page = details(&library, IdArgs { id: car.id }).unwrap().unwrap();
        assert_eq!(
            (
                page.contents.releases,
                page.contents.tracks,
                page.contents.size_bytes
            ),
            (1, 1, 7_680_000)
        );
        assert_eq!(page.sample_track_id, Some(1));
        let page = tracks(
            &library,
            LibraryTracksArgs {
                id: car.id,
                page: None,
                per_page: None,
            },
        )
        .unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].title, "It Could Happen to You");

        // Through the tag value, which names its tag
        let on = memberships(
            &library,
            MembershipsArgs {
                track: Some(1),
                ..MembershipsArgs::default()
            },
        )
        .unwrap();
        assert_eq!(on.len(), 1);
        assert!(matches!(
            &on[0].reason,
            MembershipReason::Through { rule } if matches!(&rule.target, RuleTarget::Tag { tag, .. } if tag.name == "Genre")
        ));
        let none = memberships(&library, MembershipsArgs::default()).unwrap_err();
        assert_eq!(none.message, "Ask about one release, artist or track");

        // Adding the artist names it, as its main name
        add(
            &library,
            AddArgs {
                id: car.id,
                artists: vec![1],
                ..AddArgs::default()
            },
        )
        .unwrap();
        let named = rules(&library, IdArgs { id: car.id }).unwrap();
        assert!(matches!(
            &named.last().unwrap().target,
            RuleTarget::Artist { artist } if artist.name == "Ryo Fukui"
        ));

        let args = DeleteExternalLibraryArgs {
            id: car.id,
            remove_files: None,
        };
        assert_eq!(delete(&library, args).unwrap(), car.id);
        assert!(details(&library, IdArgs { id: car.id }).unwrap().is_none());
    }
}
