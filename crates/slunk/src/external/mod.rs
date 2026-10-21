//! The external libraries context (backend.md, "The external libraries
//! context"): a selection of the library, converted, tagged and given covers
//! from the catalog, for a folder or an iPod. It has the mapping; external
//! libraries with their rules, what they come to, and what that will take;
//! syncing a folder library; and syncing an iPod.
//!
//! - `repository`: the records the context stores.
//! - `tags`: a track's tags, worked out from the catalog.
//! - `template`: path templates, and names on disk.
//! - `estimate`: what a track's file will take on a destination.
//! - `convert`: what each source becomes, and `afconvert`.
//! - `covers`: covers, read, resized, as JPEG.
//! - `write_tags`: tags into files, with lofty.
//! - `manifest`: what's on a destination, and its journal.
//! - `plan`: what a sync would do.
//! - `sync`: running a plan.
//! - `folder`: a destination's files: a folder's, or an iPod's.
//! - `ipod`: the iPod: finding it, its database, its artwork.
//! - `scratch`: a sync's temporary folder.
//! - `volume`: a destination's volume: its free space, and ejecting it.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::catalog::{ReleaseFilters, Track};
use crate::media::{Media, paths};
use crate::repo::external_libraries as repo;
use crate::repo::named::Table;
use crate::repo::store::Store;
use crate::repo::{files, named, releases, tags as tag_definitions, tracks};
use crate::tagging::{TagFilter, TagFilterInput, TagType, tag_filters};
use crate::text::{clean_or_none, clean_text, trim};
use crate::{Error, Result};

pub mod convert;
pub mod covers;
mod estimate;
pub mod folder;
pub mod ipod;
pub mod manifest;
pub mod plan;
mod repository;
pub mod scratch;
pub mod sync;
mod tags;
mod template;
pub mod volume;
pub mod write_tags;

pub use estimate::estimated_size;
pub use folder::Folder;
pub use ipod::Ipod;
pub use plan::Plan;
pub use repository::*;
pub use tags::{SortFields, Tags};
pub use template::{DEFAULT_TEMPLATE, Template, TrackPath, safe_name, unique_paths};

/// AAC's bitrate when none is given, in kbps
pub const DEFAULT_AAC_KBPS: i64 = 256;
/// The largest of an iPod's thumbnails, which its covers are made for
const IPOD_COVER_PX: u32 = 200;
/// An embedded cover's longest side when none is given, in pixels
pub const DEFAULT_COVER_PX: i64 = 600;

/// A track's tags, and its path in a folder library that keeps the original
/// files: what the track page shows of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exported {
    pub tags: Tags,
    /// Relative to the folder library
    pub path: String,
}

/// An external library to save. An iPod's profile is what it plays: CD
/// resolution, no embedded covers and no `cover.jpg`, so those are set for
/// it, and it has no folder or path template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLibraryInput {
    pub name: String,
    pub kind: ExternalKind,
    /// A folder library's, an absolute path
    pub folder: Option<String>,
    pub format: OutputFormat,
    pub aac_kbps: i64,
    pub cd_resolution: bool,
    /// A folder library's. Blank is the default template.
    pub path_template: Option<String>,
    pub embed_covers: bool,
    pub cover_px: i64,
    pub cover_file: bool,
}

/// What's in an external library, and what it will take on its destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contents {
    pub releases: i64,
    pub tracks: i64,
    /// An estimate (`estimated_size()`)
    pub size_bytes: i64,
    /// Tracks in it whose files can't be converted or tagged
    pub left_out: i64,
}

/// What the catalog's pages add to an external library.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Additions {
    pub releases: Vec<i64>,
    pub artists: Vec<i64>,
    pub tracks: Vec<i64>,
}

/// What a page asks the external libraries about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    Release(i64),
    Artist(i64),
    Track(i64),
}

/// Why something is in an external library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// A rule names it
    Own,
    /// A broader rule brings it in: its release, an artist, a tag value,
    /// the whole library
    Through(Rule),
    /// Only some of a release's tracks, through rules of their own
    Tracks,
}

/// An external library something is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub library: ExternalLibrary,
    pub reason: Reason,
    /// Its tracks that are in it
    pub tracks: i64,
    /// Its tracks
    pub total: i64,
}

/// Whether an external library's destination is there to sync (backend.md,
/// "External libraries").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Destination {
    /// It's there, and its manifest names this external library, or it has
    /// none yet
    Connected,
    /// It isn't there: a folder that moved, a card that isn't in, or no
    /// iPod that's its
    Missing,
    /// Its manifest names another external library, by its name
    Elsewhere(String),
    /// Several iPods without a manifest are connected, and it isn't known
    /// which is its
    Several,
}

/// The external library a manifest names, with the manifest's size and
/// mtime when it was read: its UUID, and its name then.
type Identity = ((u64, i64), Option<(String, String)>);

/// A sync's plan, as it was shown, kept until it runs.
#[derive(Debug)]
pub struct SyncPlan {
    /// Which plan it is, so a sync runs the one that was shown
    pub id: u64,
    pub library: ExternalLibrary,
    pub plan: Plan,
    /// The bytes free on the destination's volume
    pub free_bytes: Option<u64>,
    /// The iPod it syncs to
    pub ipod: Option<Ipod>,
    /// The manifest's size and mtime when it was read, and the iTunesDB's
    manifest_stamp: Option<(u64, i64)>,
    database_stamp: Option<(u64, i64)>,
    /// Each release's cover in the library
    cover_paths: HashMap<i64, PathBuf>,
    /// The same, by what a file records of it, for an iPod's artwork
    covers: HashMap<manifest::CoverKey, PathBuf>,
}

impl SyncPlan {
    /// Whether what it writes fits in what's free.
    pub fn fits(&self) -> bool {
        self.free_bytes
            .is_none_or(|free| self.plan.bytes_needed() <= i64::try_from(free).unwrap_or(i64::MAX))
    }
}

pub struct External {
    store: Arc<Store>,
    media: Arc<Media>,
    /// The plan last shown for each external library
    plans: Mutex<HashMap<i64, Arc<SyncPlan>>>,
    /// The sync running, if one is: its external library, and what cancels
    /// it
    running: Mutex<Option<(i64, Arc<AtomicBool>)>>,
    /// Manifests' identities, by folder, with the manifest's size and mtime
    /// when it was read, so it's read again only when it changes
    identities: Mutex<HashMap<PathBuf, Identity>>,
    /// Where iPods are looked for: /Volumes
    volumes: PathBuf,
}

impl External {
    pub fn new(store: Arc<Store>, media: Arc<Media>) -> External {
        External {
            store,
            media,
            plans: Mutex::new(HashMap::new()),
            running: Mutex::new(None),
            identities: Mutex::new(HashMap::new()),
            volumes: PathBuf::from(ipod::VOLUMES),
        }
    }

    /// Looks for iPods in another folder than /Volumes, for tests.
    pub fn with_volumes(self, volumes: impl Into<PathBuf>) -> External {
        External {
            volumes: volumes.into(),
            ..self
        }
    }

    /// Each track's tags, by id. Ids that aren't tracks are left out.
    pub fn tags_of(&self, ids: &[i64]) -> Result<HashMap<i64, Tags>> {
        tags::read(&self.store.conn(), ids)
    }

    /// A track's tags, and its path by a template in a folder library that
    /// keeps the original files, so its file keeps its extension, in
    /// lowercase. `None` for a track that isn't there.
    pub fn exported(&self, id: i64, template: &Template) -> Result<Option<Exported>> {
        let conn = self.store.conn();
        let Some(track) = tracks::get(&conn, id)? else {
            return Ok(None);
        };
        let Some(tags) = tags::read(&conn, &[id])?.remove(&id) else {
            return Ok(None);
        };
        let file = files::path(&conn, track.file_id)?.unwrap_or_default();
        Ok(Some(Exported {
            path: template.path(&tags, &extension(&file)).safe(),
            tags,
        }))
    }

    // External libraries

    /// Every external library, by name ignoring case.
    pub fn list(&self) -> Result<Vec<ExternalLibrary>> {
        repo::list(&self.store.conn())
    }

    pub fn get(&self, id: i64) -> Result<Option<ExternalLibrary>> {
        repo::get(&self.store.conn(), id)
    }

    /// A new external library, with a new UUID for its destination's
    /// manifest to name it by.
    pub fn create(&self, input: &ExternalLibraryInput) -> Result<ExternalLibrary> {
        let uuid = new_uuid()?;
        let conn = self.store.conn();
        let values = self.values_of(&conn, input)?;
        repo::insert(&conn, &uuid, &values)
    }

    /// Saves an external library's name, folder and profile. Its kind can't
    /// change.
    pub fn update(&self, id: i64, input: &ExternalLibraryInput) -> Result<ExternalLibrary> {
        let conn = self.store.conn();
        let current = require(&conn, id)?;
        if current.kind != input.kind {
            return Err(Error::Invalid(
                "An external library can't change from a folder to an iPod, or back".into(),
            ));
        }
        let values = self.values_of(&conn, input)?;
        repo::update(&conn, id, &values)?.ok_or_else(|| not_found(id))
    }

    /// Deletes an external library and its rules. With `remove_files`, it
    /// first removes what liquidslunk wrote on its destination, which has to
    /// be connected: the files its manifest lists, the folders that leaves
    /// empty, and the manifest. Otherwise its destination's files stay where
    /// they are. An unknown id deletes nothing.
    pub fn delete(&self, id: i64, remove_files: bool) -> Result<()> {
        if self.syncing() == Some(id) {
            return Err(Error::Invalid(
                "It's syncing. Cancel the sync first.".into(),
            ));
        }
        if remove_files && let Some(library) = self.get(id)? {
            let folder = self.connected_folder(&library)?;
            let loaded = folder.manifest().load()?;
            let files: Vec<manifest::Entry> = loaded
                .manifest
                .map(|manifest| manifest.files)
                .unwrap_or_default();
            if library.kind == ExternalKind::Ipod {
                self.empty_ipod(&library, &folder, files)?;
            } else {
                let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
                for path in &paths {
                    folder.remove(path)?;
                }
                folder.remove_empty_folders(paths)?;
            }
            folder.manifest().remove()?;
        }
        repo::delete(&self.store.conn(), id)
    }

    /// Removes liquidslunk's tracks from an iPod: their files, and the
    /// database, written again without them. Refused while the iPod has
    /// tracks from somewhere else, which a database of liquidslunk's own
    /// would leave out.
    fn empty_ipod(
        &self,
        library: &ExternalLibrary,
        folder: &Folder,
        files: Vec<manifest::Entry>,
    ) -> Result<()> {
        let ours: HashSet<String> = files.iter().map(|file| file.path.to_lowercase()).collect();
        let others = ipod::sync::read_database(folder)?.map_or(0, |database| {
            database
                .tracks
                .iter()
                .filter(|track| !ours.contains(&ipod::path_of(&track.location).to_lowercase()))
                .count()
        });
        if others > 0 {
            return Err(Error::Invalid(format!(
                "This iPod also has {others} {} from somewhere else, which liquidslunk's database would leave out. Delete the external library without removing its files.",
                if others == 1 { "track" } else { "tracks" }
            )));
        }
        let steps = files
            .iter()
            .map(|file| plan::Step {
                action: plan::Action::Remove,
                subject: match file.written {
                    manifest::Written::Track { track, .. } => plan::Subject::Track(track),
                    manifest::Written::Cover { release, .. } => plan::Subject::Release(release),
                },
                path: file.path.clone(),
                from: None,
                changes: Vec::new(),
                bytes: 0,
            })
            .collect();
        let plan = Plan {
            steps,
            present: files,
            ..Plan::default()
        };
        let scratch = scratch::Scratch::new()?;
        let cancel = AtomicBool::new(false);
        ipod::sync::run(
            ipod::sync::IpodRun {
                plan: &plan,
                folder,
                uuid: &library.uuid,
                name: &library.name,
                covers: HashMap::new(),
                cancel: &cancel,
                scratch: &scratch,
                erase: false,
                make_room_first: false,
            },
            &|_| {},
        )
        .map(|_| ())
    }

    /// Whether its destination is there to sync: a folder library's folder,
    /// or an iPod library's iPod, with a manifest that names it, or none.
    pub fn connected(&self, library: &ExternalLibrary) -> bool {
        self.destination(library) == Destination::Connected
    }

    /// Whether its destination is there, and whose it is.
    pub fn destination(&self, library: &ExternalLibrary) -> Destination {
        if library.kind == ExternalKind::Ipod {
            return self.find_ipod(library).0;
        }
        let Some(folder) = folder_of_library(library) else {
            return Destination::Missing;
        };
        if !folder.is_there() {
            return Destination::Missing;
        }
        match self.identity(&folder) {
            Some((uuid, name)) if uuid != library.uuid => Destination::Elsewhere(name),
            _ => Destination::Connected,
        }
    }

    /// An iPod library's iPod, among those mounted (backend.md, "External
    /// libraries"): the one whose manifest names it, or else the only one
    /// without a manifest that liquidslunk syncs. It takes its manifest at its
    /// first sync.
    pub fn find_ipod(&self, library: &ExternalLibrary) -> (Destination, Option<Ipod>) {
        let mut free = Vec::new();
        let mut elsewhere = None;
        for found in ipod::mounted_in(&self.volumes) {
            match self.identity(&found.folder()) {
                Some((uuid, _)) if uuid == library.uuid => {
                    return (Destination::Connected, Some(found));
                }
                Some((_, name)) => {
                    elsewhere.get_or_insert(name);
                }
                None if found.refusal().is_none() => free.push(found),
                None => {}
            }
        }
        match (free.len(), elsewhere) {
            (1, _) => (Destination::Connected, free.pop()),
            (0, Some(name)) => (Destination::Elsewhere(name), None),
            (0, None) => (Destination::Missing, None),
            _ => (Destination::Several, None),
        }
    }

    /// The iPods mounted now, each with the external library its manifest
    /// names, if it has one.
    pub fn mounted_ipods(&self) -> Vec<(Ipod, Option<String>)> {
        ipod::mounted_in(&self.volumes)
            .into_iter()
            .map(|found| {
                let whose = self.identity(&found.folder()).map(|(_, name)| name);
                (found, whose)
            })
            .collect()
    }

    /// Where its files go, when that's there: a folder library's folder, or
    /// an iPod library's iPod's disk.
    fn destination_root(&self, library: &ExternalLibrary) -> Option<PathBuf> {
        match library.kind {
            ExternalKind::Folder => folder_of_library(library)
                .filter(Folder::is_there)
                .map(|folder| folder.root().to_path_buf()),
            ExternalKind::Ipod => self.find_ipod(library).1.map(|found| found.mount),
        }
    }

    /// The volume its files are on, when it's one to eject.
    pub fn ejectable(&self, library: &ExternalLibrary) -> Option<PathBuf> {
        volume::ejectable(&self.destination_root(library)?)
    }

    /// The bytes free where its files go.
    pub fn free_bytes(&self, library: &ExternalLibrary) -> Option<u64> {
        volume::free_space(&self.destination_root(library)?)
    }

    /// Which external library a manifest names, read again only when the
    /// manifest changes.
    fn identity(&self, folder: &Folder) -> Option<(String, String)> {
        let stamp = folder.manifest().stamp()?;
        let root = folder.root().to_path_buf();
        if let Some((known, identity)) = lock(&self.identities).get(&root)
            && *known == stamp
        {
            return identity.clone();
        }
        let identity = folder.manifest().identity().ok().flatten();
        lock(&self.identities).insert(root, (stamp, identity.clone()));
        identity
    }

    /// Its folder, or its iPod's disk, when it's connected, or why it isn't.
    fn connected_folder(&self, library: &ExternalLibrary) -> Result<Folder> {
        if library.kind == ExternalKind::Ipod {
            return match self.find_ipod(library) {
                (Destination::Connected, Some(found)) => match found.refusal() {
                    Some(refusal) => Err(Error::Invalid(refusal)),
                    None => Ok(found.folder()),
                },
                (Destination::Elsewhere(name), _) => Err(Error::Invalid(format!(
                    "This iPod belongs to another external library, {name}."
                ))),
                (Destination::Several, _) => Err(Error::Invalid(
                    "Several iPods without liquidslunk's manifest are connected. Eject all but the one to sync.".into(),
                )),
                _ => Err(Error::Invalid(
                    "No iPod is connected. Connect it with its disk use on, so it mounts in Finder.".into(),
                )),
            };
        }
        let folder = folder_of_library(library)
            .ok_or_else(|| Error::Invalid("A folder library needs a folder".into()))?;
        match self.destination(library) {
            Destination::Connected => Ok(folder),
            Destination::Missing => Err(Error::Invalid(format!(
                "The folder isn't there: {}. Connect its disk, or find it with Locate.",
                folder.root().display()
            ))),
            Destination::Elsewhere(name) => Err(Error::Invalid(format!(
                "This folder belongs to another external library, {name}."
            ))),
            Destination::Several => Err(Error::Invalid("It isn't known which folder".into())),
        }
    }

    /// Points a folder library at its folder where it is now: one that
    /// moved, or a card that mounted under another name. The folder is
    /// checked as a new one is, and refused when its manifest names another
    /// external library.
    pub fn locate(&self, id: i64, folder: &str) -> Result<ExternalLibrary> {
        let conn = self.store.conn();
        let library = require(&conn, id)?;
        if library.kind != ExternalKind::Folder {
            return Err(Error::Invalid(
                "Only a folder library is located. An iPod is found by its manifest.".into(),
            ));
        }
        let path = self.folder_of(&conn, Some(folder))?;
        drop(conn);
        let found = Folder::new(&path);
        if !found.is_there() {
            return Err(Error::Invalid(format!("There's no folder at {path}")));
        }
        if let Some((uuid, name)) = found.manifest().identity()?
            && uuid != library.uuid
        {
            return Err(Error::Invalid(format!(
                "This folder belongs to another external library, {name}."
            )));
        }
        let values = ExternalLibraryValues {
            name: library.name,
            kind: library.kind,
            folder: Some(path),
            profile: library.profile,
        };
        repo::update(&self.store.conn(), id, &values)?.ok_or_else(|| not_found(id))
    }

    /// Ejects the volume its files are on, so it can be unplugged.
    pub fn eject(&self, id: i64) -> Result<()> {
        let library = self.get(id)?.ok_or_else(|| not_found(id))?;
        if self.syncing().is_some() {
            return Err(Error::Invalid(
                "Wait for the sync to end, or cancel it.".into(),
            ));
        }
        let mount = self
            .ejectable(&library)
            .ok_or_else(|| Error::Invalid("Its files aren't on a disk that's ejected".into()))?;
        volume::eject(&mount)
    }

    // Rules

    /// An external library's rules, in the order they were added.
    pub fn rules(&self, id: i64) -> Result<Vec<Rule>> {
        let conn = self.store.conn();
        require(&conn, id)?;
        rules_of(&conn, id)
    }

    /// Replaces an external library's rules. A rule given twice is kept
    /// once. Refused: leaving out anything but a release or a track, both
    /// including and leaving out the same thing, a tag value rule whose
    /// filter can't be read, and something that isn't there.
    pub fn set_rules(&self, id: i64, rules: &[RuleValues]) -> Result<Vec<Rule>> {
        let rules = checked_rules(rules)?;
        self.store.transaction(|tx| {
            require(tx, id)?;
            for rule in &rules {
                check_target(tx, &rule.target)?;
            }
            repo::set_rules(tx, id, &rules)?;
            rules_of(tx, id)
        })
    }

    /// Adds to an external library from the catalog's pages, in one
    /// transaction. Ids that aren't there are skipped.
    /// - A release is added whole: its exclusion goes, and its tracks', and
    ///   it gets a rule of its own.
    /// - An artist gets a rule.
    /// - A track's exclusion goes, and it gets a rule of its own. An
    ///   exclusion of its release would still leave it out, so the release's
    ///   other tracks are left out one by one instead.
    pub fn add(&self, id: i64, additions: &Additions) -> Result<()> {
        self.store.transaction(|tx| {
            require(tx, id)?;
            let mut rules = Editing::load(tx, id)?;
            let known: HashSet<i64> = releases::get_many(tx, &additions.releases)?
                .iter()
                .map(|release| release.id)
                .collect();
            let release_tracks = tracks::of_releases(tx, &additions.releases)?;
            for &release in additions.releases.iter().filter(|id| known.contains(id)) {
                rules.remove(true, RuleTarget::Release(release))?;
                for track in release_tracks.get(&release).into_iter().flatten() {
                    rules.remove(true, RuleTarget::Track(track.id))?;
                }
                rules.insert(false, RuleTarget::Release(release))?;
            }

            let mut artists = named::get_many(tx, Table::Artists, &additions.artists)?;
            artists.sort_by_key(|artist| artist.id);
            for artist in artists {
                rules.insert(false, RuleTarget::Artist(artist.id))?;
            }

            let adding: HashSet<i64> = additions.tracks.iter().copied().collect();
            let mut found = tracks::get_many(tx, &additions.tracks)?;
            found.sort_by_key(|track| track.id);
            for track in found {
                rules.remove(true, RuleTarget::Track(track.id))?;
                let release = RuleTarget::Release(track.release_id);
                if rules.has(true, &release) {
                    rules.remove(true, release)?;
                    let others = tracks::of_releases(tx, &[track.release_id])?
                        .remove(&track.release_id)
                        .unwrap_or_default();
                    for other in others.iter().filter(|other| !adding.contains(&other.id)) {
                        rules.insert(true, RuleTarget::Track(other.id))?;
                    }
                }
                rules.insert(false, RuleTarget::Track(track.id))?;
            }
            Ok(())
        })
    }

    /// Takes something out of an external library, from the catalog's
    /// pages: its own rule goes, and when a broader rule still brings it
    /// in, it's left out. A release's tracks' own rules go with it, those
    /// that leave them out too. An artist's rule, a tag value's or an
    /// everything rule just goes.
    pub fn remove(&self, id: i64, target: &RuleTarget) -> Result<()> {
        self.store.transaction(|tx| {
            require(tx, id)?;
            let mut rules = Editing::load(tx, id)?;
            match target {
                RuleTarget::Release(release) => {
                    rules.remove(false, RuleTarget::Release(*release))?;
                    let ids: Vec<i64> = tracks::of_releases(tx, &[*release])?
                        .remove(release)
                        .unwrap_or_default()
                        .iter()
                        .map(|track| track.id)
                        .collect();
                    // Leaving out the release covers its tracks
                    for track in &ids {
                        rules.remove(false, RuleTarget::Track(*track))?;
                        rules.remove(true, RuleTarget::Track(*track))?;
                    }
                    if !members(tx, id, Some(&ids))?.is_empty() {
                        rules.insert(true, RuleTarget::Release(*release))?;
                    }
                }
                RuleTarget::Track(track) => {
                    rules.remove(false, RuleTarget::Track(*track))?;
                    if !members(tx, id, Some(&[*track]))?.is_empty() {
                        rules.insert(true, RuleTarget::Track(*track))?;
                    }
                }
                RuleTarget::Artist(_) | RuleTarget::Tag { .. } | RuleTarget::Everything => {
                    rules.remove(false, target.clone())?;
                }
            }
            Ok(())
        })
    }

    // What's in one

    /// The tracks an external library's rules come to, by release title
    /// ignoring case, then release, disc and number.
    pub fn track_ids(&self, id: i64) -> Result<Vec<i64>> {
        let conn = self.store.conn();
        require(&conn, id)?;
        members(&conn, id, None)
    }

    /// How many releases and tracks an external library has, and what
    /// they'll take on its destination.
    pub fn contents(&self, id: i64) -> Result<Contents> {
        let conn = self.store.conn();
        let library = require(&conn, id)?;
        let ids = members(&conn, id, None)?;
        let mut contents = Contents {
            tracks: ids.len() as i64,
            ..Contents::default()
        };
        let mut releases = HashSet::new();
        for facts in repo::file_facts(&conn, &ids)? {
            releases.insert(facts.release_id);
            match estimated_size(
                &extension(&facts.path),
                facts.size_bytes,
                facts.duration_ms,
                library.kind,
                &library.profile,
            ) {
                Some(size) => contents.size_bytes += size,
                None => contents.left_out += 1,
            }
        }
        contents.releases = releases.len() as i64;
        Ok(contents)
    }

    /// A track to show an external library's path template with: its first
    /// track, or else the catalog's newest.
    pub fn sample_track(&self, id: i64) -> Result<Option<i64>> {
        let conn = self.store.conn();
        require(&conn, id)?;
        if let Some(first) = members(&conn, id, None)?.first() {
            return Ok(Some(*first));
        }
        Ok(conn.query_row(r#"select max("id") from "tracks""#, [], |row| row.get(0))?)
    }

    /// The external libraries a release, an artist or a track is in, by
    /// name, and why. An artist is in those with a rule for it.
    pub fn memberships(&self, subject: Subject) -> Result<Vec<Membership>> {
        let conn = self.store.conn();
        let libraries = repo::list(&conn)?;
        let library_ids: Vec<i64> = libraries.iter().map(|library| library.id).collect();
        let mut all_rules = repo::rules_of(&conn, &library_ids)?;
        let subject_tracks: Vec<Track> = match subject {
            Subject::Release(id) => tracks::of_releases(&conn, &[id])?
                .remove(&id)
                .unwrap_or_default(),
            Subject::Track(id) => tracks::get(&conn, id)?.into_iter().collect(),
            Subject::Artist(id) => {
                let filters = ReleaseFilters {
                    artist: Some(id),
                    ..ReleaseFilters::default()
                };
                let ids = releases::ids(&conn, &filters)?;
                tracks::of_releases(&conn, &ids)?
                    .into_values()
                    .flatten()
                    .collect()
            }
        };
        let track_ids: Vec<i64> = subject_tracks.iter().map(|track| track.id).collect();
        let release_ids: Vec<i64> = subject_tracks
            .iter()
            .map(|track| track.release_id)
            .chain(match subject {
                Subject::Release(id) => Some(id),
                _ => None,
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let release_artists = releases::artists_of(&conn, &release_ids)?;

        let mut memberships = Vec::new();
        for library in libraries {
            let rules = all_rules.remove(&library.id).unwrap_or_default();
            let included: HashSet<i64> = members(&conn, library.id, Some(&track_ids))?
                .into_iter()
                .collect();
            let reasons = Reasons {
                conn: &conn,
                rules: &rules,
                release_artists: &release_artists,
            };
            let reason = match subject {
                Subject::Artist(artist) => reasons
                    .included(&RuleTarget::Artist(artist))
                    .map(|_| Reason::Own),
                Subject::Track(track) => match subject_tracks.first() {
                    Some(found) if included.contains(&track) => Some(reasons.of_track(found)?),
                    _ => None,
                },
                Subject::Release(release) => reasons.of_release(release, &track_ids, &included)?,
            };
            if let Some(reason) = reason {
                memberships.push(Membership {
                    library,
                    reason,
                    tracks: included.len() as i64,
                    total: track_ids.len() as i64,
                });
            }
        }
        Ok(memberships)
    }

    // Syncing

    /// Works out what a sync would do, and keeps it, for `sync()` to
    /// run. What should be there is read in one read transaction, then the
    /// connection is let go before any file is read: each source's format,
    /// its cover, the manifest, and the destination: a folder library's
    /// folder, or an iPod's database.
    pub fn plan_sync(&self, id: i64) -> Result<Arc<SyncPlan>> {
        let snapshot = self.store.read(|tx| {
            let library = require(tx, id)?;
            let ids = members(tx, id, None)?;
            let tags = tags::read(tx, &ids)?;
            let facts: HashMap<i64, repo::TrackFileFacts> = repo::file_facts(tx, &ids)?
                .into_iter()
                .map(|facts| (facts.track_id, facts))
                .collect();
            let cover_ids: Vec<i64> = tags
                .values()
                .filter_map(|tags| tags.cover)
                .collect::<HashSet<_>>()
                .into_iter()
                .collect();
            let covers: HashMap<i64, crate::media::FileRecord> = files::get_many(tx, &cover_ids)?
                .into_iter()
                .map(|file| (file.id, file))
                .collect();
            Ok((library, ids, tags, facts, covers))
        })?;
        let (library, ids, mut tags, facts, cover_files) = snapshot;
        let folder = self.connected_folder(&library)?;
        let on_ipod = library.kind == ExternalKind::Ipod;
        let found_ipod = if on_ipod {
            self.find_ipod(&library).1
        } else {
            None
        };
        let profile = &library.profile;
        let template =
            Template::parse(profile.path_template.as_deref().unwrap_or(DEFAULT_TEMPLATE))?;
        let root = self.media.library();

        // Each source's format, read from its headers, several at once
        let sources: Vec<(i64, PathBuf)> = ids
            .iter()
            .filter_map(|id| Some((*id, root.absolute(&facts.get(id)?.path))))
            .collect();
        let audio = read_sources(&sources);

        // Each release's cover, checked from its headers
        let mut inputs = plan::Inputs::default();
        let mut cover_keys: HashMap<i64, manifest::CoverKey> = HashMap::new();
        let mut cover_paths: HashMap<i64, PathBuf> = HashMap::new();
        let cover_px = if on_ipod {
            IPOD_COVER_PX
        } else {
            u32::try_from(profile.cover_px).unwrap_or(600)
        };
        if profile.embed_covers || profile.cover_file || on_ipod {
            let mut checked: HashSet<i64> = HashSet::new();
            for track in &ids {
                let (Some(tags), Some(facts)) = (tags.get(track), facts.get(track)) else {
                    continue;
                };
                let release = facts.release_id;
                let Some(cover) = tags.cover.and_then(|id| cover_files.get(&id)) else {
                    continue;
                };
                if !checked.insert(release) {
                    continue;
                }
                let path = root.absolute(&cover.path);
                let checked = if path.is_file() {
                    covers::check(&path)
                } else {
                    Err(format!(
                        "Its cover is missing from the library: {}",
                        cover.path
                    ))
                };
                match checked {
                    Ok(()) => {
                        cover_keys.insert(
                            release,
                            manifest::CoverKey {
                                source: cover.sha256.clone(),
                                px: cover_px,
                            },
                        );
                        cover_paths.insert(release, path);
                    }
                    Err(reason) => inputs.left_out.push(plan::LeftOut {
                        subject: plan::Subject::Release(release),
                        reason,
                    }),
                }
            }
        }
        if profile.cover_file {
            inputs.cover_files = cover_keys.clone();
        }

        for track in &ids {
            let (Some(facts), Some(tags)) = (facts.get(track), tags.remove(track)) else {
                continue;
            };
            let release = facts.release_id;
            inputs.labels.insert(
                plan::Subject::Track(*track),
                plan::Label {
                    title: tags.title.clone(),
                    artist: tags.artist.clone(),
                    album: Some(tags.album.clone()),
                },
            );
            inputs
                .labels
                .entry(plan::Subject::Release(release))
                .or_insert_with(|| plan::Label {
                    title: tags.album.clone(),
                    artist: tags.album_artist.clone(),
                    album: None,
                });
            let left_out = |reason: String| plan::LeftOut {
                subject: plan::Subject::Track(*track),
                reason,
            };
            let source = root.absolute(&facts.path);
            let Some(Some(read)) = audio.get(track) else {
                // Its copy on the destination stays as it is
                inputs.kept.insert(*track, release);
                inputs.left_out.push(left_out(format!(
                    "Its file is missing from the library: {}",
                    facts.path
                )));
                continue;
            };
            let becomes = match convert::becomes(read, library.kind, profile) {
                Ok(becomes) => becomes,
                Err(reason) => {
                    inputs.left_out.push(left_out(reason));
                    continue;
                }
            };
            let source_extension = extension(&facts.path);
            inputs.wanted.push(plan::WantedTrack {
                track: *track,
                release,
                path: template.path(&tags, &becomes.extension(&source_extension)),
                bytes: becomes.estimated_bytes(read, facts.size_bytes, facts.duration_ms),
                audio: manifest::AudioKey {
                    source: facts.sha256.clone(),
                    becomes: becomes.key(),
                    conversion: convert::CONVERSION_VERSION,
                    encoder: convert::encoder().to_string(),
                },
                // An iPod's, for its artwork
                cover: (profile.embed_covers || on_ipod)
                    .then(|| cover_keys.get(&release).cloned())
                    .flatten(),
                source,
                becomes,
                tags,
            });
        }

        // What's there: the manifest, checked against the disk, and on an
        // iPod, against its database
        let manifest_stamp = folder.manifest().stamp();
        let database_stamp = folder::stat(&folder.path(ipod::ITUNES_DB));
        let loaded = folder.manifest().load()?;
        if let Some(manifest) = &loaded.manifest
            && manifest.external_library != library.uuid
        {
            return Err(Error::Invalid(format!(
                "This {} belongs to another external library, {}.",
                if on_ipod { "iPod" } else { "folder" },
                manifest.name
            )));
        }
        let database = if on_ipod {
            ipod::sync::read_database(&folder)?
        } else {
            None
        };
        let listed: HashSet<String> = database
            .iter()
            .flat_map(|database| &database.tracks)
            .map(|track| ipod::path_of(&track.location).to_lowercase())
            .collect();
        for mut entry in loaded
            .manifest
            .map(|manifest| manifest.files)
            .unwrap_or_default()
        {
            let mut as_written = manifest::as_written(&entry, folder.stat(&entry.path));
            // A move interrupted between the journal and the rename
            if !as_written
                && let Some(from) = loaded.moved_from.get(&entry.path)
                && manifest::as_written(&entry, folder.stat(from))
            {
                entry.path = from.clone();
                as_written = true;
            }
            let listed = !on_ipod || listed.contains(&entry.path.to_lowercase());
            inputs.present.push(plan::PresentFile {
                entry,
                as_written,
                listed,
            });
        }

        let covers: HashMap<manifest::CoverKey, PathBuf> = cover_keys
            .iter()
            .filter_map(|(release, key)| Some((key.clone(), cover_paths.get(release)?.clone())))
            .collect();
        let plan = if on_ipod {
            // Tracks on the iPod that liquidslunk didn't write
            let ours: HashSet<String> = inputs
                .present
                .iter()
                .map(|file| file.entry.path.to_lowercase())
                .collect();
            let other: Vec<plan::OtherTrack> = database
                .iter()
                .flat_map(|database| &database.tracks)
                .filter_map(|track| {
                    let path = ipod::path_of(&track.location);
                    (!ours.contains(&path.to_lowercase())).then(|| plan::OtherTrack {
                        label: plan::Label {
                            title: if track.title.is_empty() {
                                path.clone()
                            } else {
                                track.title.clone()
                            },
                            artist: track.artist.clone(),
                            album: track.album.clone(),
                        },
                        path,
                    })
                })
                .collect();
            let present: Vec<&str> = inputs
                .present
                .iter()
                .map(|file| file.entry.path.as_str())
                .chain(other.iter().map(|track| track.path.as_str()))
                .collect();
            let mut names = ipod::Names::new(folder.root(), present);
            let mut plan = plan::plan_ipod(inputs, &mut |extension| {
                names.next(extension, &|path| folder.exists(path))
            });
            plan.other = other;
            plan
        } else {
            plan::plan(inputs, &|path| folder.exists(path))
        };

        static NEXT: AtomicU64 = AtomicU64::new(1);
        let planned = Arc::new(SyncPlan {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            free_bytes: volume::free_space(folder.root()),
            ipod: found_ipod,
            plan,
            library,
            manifest_stamp,
            database_stamp,
            cover_paths,
            covers,
        });
        lock(&self.plans).insert(id, planned.clone());
        Ok(planned)
    }

    /// Runs the plan last shown, with its progress. One sync runs at
    /// a time, and the library can be browsed and edited meanwhile: it works
    /// from the plan's snapshot. Refused: a plan that isn't the one last
    /// shown, one that doesn't fit, a destination that changed since, and on
    /// an iPod with tracks from somewhere else, a sync the user didn't agree
    /// to erase them for.
    pub fn sync(
        &self,
        id: i64,
        plan_id: u64,
        erase: bool,
        progress: &dyn Fn(sync::Progress),
    ) -> Result<sync::Outcome> {
        let planned = lock(&self.plans)
            .get(&id)
            .filter(|planned| planned.id == plan_id)
            .cloned()
            .ok_or_else(|| Error::Invalid("The plan is out of date. Look at it again.".into()))?;
        if !planned.plan.other.is_empty() && !erase {
            return Err(Error::Invalid(format!(
                "This iPod has {} from somewhere else. Agree to erase them to sync it.",
                count_text(planned.plan.other.len(), "track")
            )));
        }
        if !planned.fits() {
            return Err(Error::Invalid(format!(
                "It needs {} more than the {} has free.",
                bytes_text(
                    planned.plan.bytes_needed()
                        - i64::try_from(planned.free_bytes.unwrap_or(0)).unwrap_or(i64::MAX)
                ),
                if planned.ipod.is_some() {
                    "iPod"
                } else {
                    "folder"
                }
            )));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut running = lock(&self.running);
            if running.is_some() {
                return Err(Error::Invalid("A sync is already running".into()));
            }
            *running = Some((id, cancel.clone()));
        }
        let _running = Running(&self.running);
        let library = self.get(id)?.ok_or_else(|| not_found(id))?;
        let folder = self.connected_folder(&library)?;
        let changed = folder.manifest().stamp() != planned.manifest_stamp
            || (library.kind == ExternalKind::Ipod
                && folder::stat(&folder.path(ipod::ITUNES_DB)) != planned.database_stamp);
        if changed {
            return Err(Error::Invalid(format!(
                "The {} changed since the plan was made. Look at it again.",
                if library.kind == ExternalKind::Ipod {
                    "iPod"
                } else {
                    "folder"
                }
            )));
        }
        let scratch = scratch::Scratch::new()?;
        let outcome = match library.kind {
            ExternalKind::Folder => sync::run(
                sync::Run {
                    plan: &planned.plan,
                    folder: &folder,
                    uuid: &library.uuid,
                    name: &library.name,
                    cover_paths: planned.cover_paths.clone(),
                    cover_px: u32::try_from(library.profile.cover_px).unwrap_or(600),
                    cancel: &cancel,
                    scratch: &scratch,
                },
                progress,
            ),
            ExternalKind::Ipod => ipod::sync::run(
                ipod::sync::IpodRun {
                    plan: &planned.plan,
                    folder: &folder,
                    uuid: &library.uuid,
                    name: &library.name,
                    covers: planned.covers.clone(),
                    cancel: &cancel,
                    scratch: &scratch,
                    erase,
                    // Room for the new files only once the old ones are gone
                    make_room_first: planned.free_bytes.is_some_and(|free| {
                        planned.plan.added > i64::try_from(free).unwrap_or(i64::MAX)
                    }),
                },
                progress,
            ),
        };
        lock(&self.plans).remove(&id);
        let outcome = match outcome {
            // A card pulled out midway
            Err(_) if !folder.is_there() => return Err(folder::gone(folder.root())),
            outcome => outcome?,
        };
        if !outcome.cancelled {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_secs() as i64);
            repo::set_last_synced(&self.store.conn(), id, now)?;
        }
        Ok(outcome)
    }

    /// Cancels the sync running, if one is: it stops after the file in
    /// hand.
    pub fn cancel_sync(&self) {
        if let Some((_, cancel)) = lock(&self.running).as_ref() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// The external library a sync is running for, if one is.
    pub fn syncing(&self) -> Option<i64> {
        lock(&self.running).as_ref().map(|(id, _)| *id)
    }

    /// An external library's columns, as they're saved, once they're
    /// checked.
    fn values_of(
        &self,
        conn: &Connection,
        input: &ExternalLibraryInput,
    ) -> Result<ExternalLibraryValues> {
        let name = clean_text(&input.name);
        if name.is_empty() {
            return Err(Error::Invalid("The name can't be empty".into()));
        }
        if !(128..=320).contains(&input.aac_kbps) {
            return Err(Error::Invalid("AAC's bitrate is 128 to 320 kbps".into()));
        }
        let (folder, profile) = match input.kind {
            ExternalKind::Folder => {
                if !(100..=3000).contains(&input.cover_px) {
                    return Err(Error::Invalid(
                        "Embedded covers are 100 to 3000 pixels".into(),
                    ));
                }
                let template = clean_or_none(input.path_template.as_deref())
                    .unwrap_or_else(|| DEFAULT_TEMPLATE.to_string());
                Template::parse(&template)?;
                (
                    Some(self.folder_of(conn, input.folder.as_deref())?),
                    Profile {
                        format: input.format,
                        aac_kbps: input.aac_kbps,
                        cd_resolution: input.cd_resolution,
                        path_template: Some(template),
                        embed_covers: input.embed_covers,
                        cover_px: input.cover_px,
                        cover_file: input.cover_file,
                    },
                )
            }
            ExternalKind::Ipod => {
                if !matches!(input.format, OutputFormat::Alac | OutputFormat::Aac) {
                    return Err(Error::Invalid("An iPod plays Apple Lossless or AAC".into()));
                }
                (
                    None,
                    Profile {
                        format: input.format,
                        aac_kbps: input.aac_kbps,
                        cd_resolution: true,
                        path_template: None,
                        embed_covers: false,
                        cover_px: DEFAULT_COVER_PX,
                        cover_file: false,
                    },
                )
            }
        };
        Ok(ExternalLibraryValues {
            name,
            kind: input.kind,
            folder,
            profile,
        })
    }

    /// A folder library's folder, resolved, once it's checked: absolute,
    /// and never inside the library's folder, holding it, or holding a
    /// registered file, since that's where the user's own files are.
    fn folder_of(&self, conn: &Connection, folder: Option<&str>) -> Result<String> {
        let typed = folder
            .map(trim)
            .filter(|folder| !folder.is_empty())
            .ok_or_else(|| Error::Invalid("A folder library needs a folder".into()))?;
        if !typed.starts_with('/') {
            return Err(Error::Invalid(
                "The folder needs an absolute path, like /Volumes/CARD/Music".into(),
            ));
        }
        let folder = paths::resolve("/", typed);
        let root = self.media.library().root().to_string_lossy().to_string();
        let inside = |path: &str, of: &str| of == "/" || path.starts_with(&format!("{of}/"));
        let refused =
            |message: &str| Err(Error::Invalid(format!("{message}. Choose one outside it.")));
        if folder == root {
            return refused("That's the library's own folder");
        }
        if inside(&folder, &root) {
            return refused("That folder is inside the library's folder");
        }
        if inside(&root, &folder) {
            return refused("That folder holds the library");
        }
        let relative = paths::relative(&root, &folder);
        if files::has_path_starting_with(conn, &format!("{relative}/"), false)? {
            return Err(Error::Invalid(
                "That folder holds files the library has registered. Choose one without them."
                    .into(),
            ));
        }
        Ok(folder)
    }
}

/// Why something is in an external library, from its rules: the narrowest
/// rule that brings it in.
struct Reasons<'a> {
    conn: &'a Connection,
    rules: &'a [Rule],
    release_artists: &'a HashMap<i64, Vec<crate::catalog::ArtistLink>>,
}

impl Reasons<'_> {
    fn included(&self, target: &RuleTarget) -> Option<&Rule> {
        self.rules
            .iter()
            .find(|rule| !rule.exclude && rule.target == *target)
    }

    fn excluded(&self, target: &RuleTarget) -> bool {
        self.rules
            .iter()
            .any(|rule| rule.exclude && rule.target == *target)
    }

    /// An artist rule for one of a release's artists.
    fn artist_rule(&self, release: i64) -> Option<&Rule> {
        let artists: HashSet<i64> = self
            .release_artists
            .get(&release)
            .into_iter()
            .flatten()
            .map(|link| link.artist_id)
            .collect();
        self.rules.iter().find(|rule| {
            !rule.exclude
                && matches!(rule.target, RuleTarget::Artist(artist) if artists.contains(&artist))
        })
    }

    /// A tag value rule that includes every one of these tracks.
    fn tag_rule(&self, tracks: &[i64]) -> Result<Option<&Rule>> {
        if tracks.is_empty() {
            return Ok(None);
        }
        for rule in self.rules {
            if let Some(filter) = filter_of(&rule.target)? {
                let matches = repo::tag_value_matches(self.conn, &filter, tracks)?;
                if tracks.iter().all(|id| matches.contains(id)) {
                    return Ok(Some(rule));
                }
            }
        }
        Ok(None)
    }

    /// A track that's in: its own rule, its release's, an artist's, a tag
    /// value's, or an everything rule.
    fn of_track(&self, track: &Track) -> Result<Reason> {
        if self.included(&RuleTarget::Track(track.id)).is_some() {
            return Ok(Reason::Own);
        }
        let broader = self
            .included(&RuleTarget::Release(track.release_id))
            .or_else(|| self.artist_rule(track.release_id));
        let rule = match broader {
            Some(rule) => Some(rule),
            None => self.tag_rule(&[track.id])?,
        }
        .or_else(|| self.included(&RuleTarget::Everything));
        Ok(rule.map_or(Reason::Own, |rule| Reason::Through(rule.clone())))
    }

    /// A release: by its own rule or an artist's, even with no tracks yet,
    /// unless it's left out; or a tag value's, when that brings in all its
    /// tracks; or an everything rule, even with no tracks yet, unless it's
    /// left out; or else by some of its tracks.
    fn of_release(
        &self,
        release: i64,
        tracks: &[i64],
        included: &HashSet<i64>,
    ) -> Result<Option<Reason>> {
        let target = RuleTarget::Release(release);
        if !self.excluded(&target) {
            if self.included(&target).is_some() {
                return Ok(Some(Reason::Own));
            }
            if let Some(rule) = self.artist_rule(release) {
                return Ok(Some(Reason::Through(rule.clone())));
            }
        }
        if !included.is_empty()
            && let Some(rule) = self.tag_rule(tracks)?
        {
            return Ok(Some(Reason::Through(rule.clone())));
        }
        if !self.excluded(&target)
            && let Some(rule) = self.included(&RuleTarget::Everything)
        {
            return Ok(Some(Reason::Through(rule.clone())));
        }
        Ok((!included.is_empty()).then_some(Reason::Tracks))
    }
}

/// An external library's rules, as they're edited one at a time, each
/// added or removed once.
struct Editing<'a> {
    tx: &'a Connection,
    library_id: i64,
    rules: HashMap<RuleValues, i64>,
}

impl<'a> Editing<'a> {
    fn load(tx: &'a Connection, library_id: i64) -> Result<Editing<'a>> {
        let rules = rules_of(tx, library_id)?
            .into_iter()
            .map(|rule| (rule.values(), rule.id))
            .collect();
        Ok(Editing {
            tx,
            library_id,
            rules,
        })
    }

    fn has(&self, exclude: bool, target: &RuleTarget) -> bool {
        self.rules.contains_key(&RuleValues {
            exclude,
            target: target.clone(),
        })
    }

    fn insert(&mut self, exclude: bool, target: RuleTarget) -> Result<()> {
        let values = RuleValues { exclude, target };
        if !self.rules.contains_key(&values) {
            let id = repo::insert_rule(self.tx, self.library_id, &values)?;
            self.rules.insert(values, id);
        }
        Ok(())
    }

    fn remove(&mut self, exclude: bool, target: RuleTarget) -> Result<()> {
        if let Some(id) = self.rules.remove(&RuleValues { exclude, target }) {
            repo::delete_rule(self.tx, id)?;
        }
        Ok(())
    }
}

/// The tracks an external library's rules come to, with its tag value
/// rules read as filters.
fn members(conn: &Connection, id: i64, within: Option<&[i64]>) -> Result<Vec<i64>> {
    let mut filters = Vec::new();
    for rule in rules_of(conn, id)? {
        if let Some(filter) = filter_of(&rule.target)? {
            filters.push(filter);
        }
    }
    repo::members(conn, id, &filters, within)
}

fn rules_of(conn: &Connection, id: i64) -> Result<Vec<Rule>> {
    Ok(repo::rules_of(conn, &[id])?.remove(&id).unwrap_or_default())
}

/// A tag value rule's filter, as the lists' tag filters read it. `None` for
/// other rules.
fn filter_of(target: &RuleTarget) -> Result<Option<TagFilter>> {
    let RuleTarget::Tag {
        kind,
        tag_id,
        equals,
        min,
        max,
    } = target
    else {
        return Ok(None);
    };
    let input = TagFilterInput {
        kind: *kind,
        tag_id: *tag_id,
        equals: equals.clone(),
        contains: None,
        min: min.clone(),
        max: max.clone(),
    };
    Ok(tag_filters(&[input])?.pop())
}

/// Rules as they're saved: a tag value rule's filter cleaned like text, and
/// checked, and each rule once.
fn checked_rules(rules: &[RuleValues]) -> Result<Vec<RuleValues>> {
    let mut seen: HashMap<RuleTarget, bool> = HashMap::new();
    let mut checked = Vec::new();
    for rule in rules {
        let target = match &rule.target {
            RuleTarget::Tag {
                kind,
                tag_id,
                equals,
                min,
                max,
            } => {
                let (equals, min, max) = (
                    clean_or_none(equals.as_deref()),
                    clean_or_none(min.as_deref()),
                    clean_or_none(max.as_deref()),
                );
                let ranged = min.is_some() || max.is_some();
                let valid = match kind {
                    TagType::Text => equals.is_some() && !ranged,
                    TagType::Number | TagType::Date => equals.is_some() != ranged,
                };
                if !valid {
                    return Err(Error::Invalid(match kind {
                        TagType::Text => "A text tag value rule takes a value".into(),
                        _ => format!(
                            "A {} tag value rule takes a value, or a range",
                            kind.as_str()
                        ),
                    }));
                }
                let target = RuleTarget::Tag {
                    kind: *kind,
                    tag_id: *tag_id,
                    equals,
                    min,
                    max,
                };
                // The values have to read as the type's
                filter_of(&target)?;
                target
            }
            target => target.clone(),
        };
        if rule.exclude && !matches!(target, RuleTarget::Release(_) | RuleTarget::Track(_)) {
            return Err(Error::Invalid(
                "Only a release or a track can be left out".into(),
            ));
        }
        // Text values compare ignoring case, as the database compares them
        let key = match &target {
            RuleTarget::Tag {
                kind: TagType::Text,
                tag_id,
                equals,
                ..
            } => RuleTarget::Tag {
                kind: TagType::Text,
                tag_id: *tag_id,
                equals: equals.as_ref().map(|value| value.to_ascii_lowercase()),
                min: None,
                max: None,
            },
            target => target.clone(),
        };
        match seen.get(&key) {
            Some(exclude) if *exclude == rule.exclude => {}
            Some(_) => {
                return Err(Error::Invalid(
                    "A rule can't both include something and leave it out".into(),
                ));
            }
            None => {
                seen.insert(key, rule.exclude);
                checked.push(RuleValues {
                    exclude: rule.exclude,
                    target,
                });
            }
        }
    }
    Ok(checked)
}

/// Refuses a rule for something that isn't there.
fn check_target(conn: &Connection, target: &RuleTarget) -> Result<()> {
    let missing = |what: &str, id: i64| Err(Error::NotFound(format!("There's no {what} {id}")));
    match target {
        RuleTarget::Release(id) if releases::get(conn, *id)?.is_none() => missing("release", *id),
        RuleTarget::Artist(id) if named::get(conn, Table::Artists, *id)?.is_none() => {
            missing("artist", *id)
        }
        RuleTarget::Track(id) if tracks::get(conn, *id)?.is_none() => missing("track", *id),
        RuleTarget::Tag { kind, tag_id, .. }
            if tag_definitions::get(conn, *kind, *tag_id)?.is_none() =>
        {
            missing(&format!("{} tag", kind.as_str()), *tag_id)
        }
        _ => Ok(()),
    }
}

fn require(conn: &Connection, id: i64) -> Result<ExternalLibrary> {
    repo::get(conn, id)?.ok_or_else(|| not_found(id))
}

fn not_found(id: i64) -> Error {
    Error::NotFound(format!("There's no external library {id}"))
}

/// Clears the sync running when it ends, however it ends.
struct Running<'a>(&'a Mutex<Option<(i64, Arc<AtomicBool>)>>);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        *lock(self.0) = None;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A folder library's folder.
fn folder_of_library(library: &ExternalLibrary) -> Option<Folder> {
    match library.kind {
        ExternalKind::Folder => library.folder.as_deref().map(Folder::new),
        ExternalKind::Ipod => None,
    }
}

/// What each source holds, read several at once. `None` for a file that
/// isn't there.
fn read_sources(sources: &[(i64, PathBuf)]) -> HashMap<i64, Option<convert::SourceAudio>> {
    let workers = std::thread::available_parallelism().map_or(1, |n| n.get());
    let chunk = sources.len().div_ceil(workers).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = sources
            .chunks(chunk)
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .map(|(id, path)| (*id, path.is_file().then(|| convert::read_source(path))))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().unwrap_or_default())
            .collect()
    })
}

/// A count of things in words: "1 track", "1,234 tracks".
fn count_text(count: usize, noun: &str) -> String {
    let digits = count.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    format!("{grouped} {noun}{}", if count == 1 { "" } else { "s" })
}

/// A size in words, as Finder shows it: "3.2 GB".
fn bytes_text(bytes: i64) -> String {
    let units = ["bytes", "KB", "MB", "GB", "TB"];
    let mut value = bytes.max(0) as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < units.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value} bytes")
    } else {
        format!("{value:.1} {}", units[unit])
    }
}

/// A path's extension, without its dot, in lowercase.
fn extension(path: &str) -> String {
    paths::extname(path).trim_start_matches('.').to_lowercase()
}

/// A random UUID, version 4, from the system's random numbers.
fn new_uuid() -> Result<String> {
    let random = Path::new("/dev/urandom");
    let mut bytes = [0u8; 16];
    File::open(random)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(Error::io(random))?;
    // The version, 4, and the variant, 10 in binary
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}
