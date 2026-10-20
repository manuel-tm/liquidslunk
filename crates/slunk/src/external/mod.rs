//! The external libraries context (4_external_libraries.md): a selection of
//! the library, converted, tagged and given covers from the catalog, for a
//! folder or an iPod. So far it has the mapping (phase 0), and external
//! libraries with their rules, what they come to, and what that will take
//! (phase 1). Nothing is written to a destination yet.
//!
//! - `repository`: the records the context stores.
//! - `tags`: a track's tags, worked out from the catalog (§4).
//! - `template`: path templates, and names on disk (§6).
//! - `estimate`: what a track's file will take on a destination.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;

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

mod estimate;
mod repository;
mod tags;
mod template;

pub use estimate::estimated_size;
pub use repository::*;
pub use tags::{SortFields, Tags};
pub use template::{DEFAULT_TEMPLATE, Template, TrackPath, safe_name, unique_paths};

/// AAC's bitrate when none is given, in kbps
pub const DEFAULT_AAC_KBPS: i64 = 256;
/// An embedded cover's longest side when none is given, in pixels
pub const DEFAULT_COVER_PX: i64 = 600;

/// A track's tags, and its path in a folder library that keeps the original
/// files: what the track page shows of it (phase 0).
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
    /// Tracks in it whose files can't be converted or tagged (§5.2)
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
    /// A broader rule brings it in: its release, an artist, a tag value
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

pub struct External {
    store: Arc<Store>,
    media: Arc<Media>,
}

impl External {
    pub fn new(store: Arc<Store>, media: Arc<Media>) -> External {
        External { store, media }
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

    /// Deletes an external library and its rules. Its destination's files
    /// stay where they are. An unknown id deletes nothing.
    pub fn delete(&self, id: i64) -> Result<()> {
        repo::delete(&self.store.conn(), id)
    }

    /// Whether its destination is there: a folder library's folder. An iPod
    /// library isn't tied to an iPod yet, so it isn't.
    pub fn connected(&self, library: &ExternalLibrary) -> bool {
        match library.kind {
            ExternalKind::Folder => library
                .folder
                .as_deref()
                .is_some_and(|folder| Path::new(folder).is_dir()),
            ExternalKind::Ipod => false,
        }
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
    /// that leave them out too. An artist's rule, or a tag value's, just
    /// goes.
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
                RuleTarget::Artist(_) | RuleTarget::Tag { .. } => {
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
    /// registered file, since that's where the user's own files are (§3.1).
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

    /// A track that's in: its own rule, its release's, an artist's, or a tag
    /// value's.
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
        };
        Ok(rule.map_or(Reason::Own, |rule| Reason::Through(rule.clone())))
    }

    /// A release: by its own rule or an artist's, even with no tracks yet,
    /// unless it's left out; or a tag value's, when that brings in all its
    /// tracks; or else by some of its tracks.
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
        if included.is_empty() {
            return Ok(None);
        }
        Ok(Some(match self.tag_rule(tracks)? {
            Some(rule) => Reason::Through(rule.clone()),
            None => Reason::Tracks,
        }))
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
