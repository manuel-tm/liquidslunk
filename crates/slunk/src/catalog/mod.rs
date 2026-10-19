//! The catalog context, what the user catalogs: artists and labels, with their
//! alternate names, pictures and logos, and an artist's file links; the two
//! lists the user manages, release formats and release media; the languages in
//! use (phase 2); releases, with their credits (phase 4); and tracks, with
//! their disc titles, and what a folder suggests for importing it (phase 5).
//!
//! - `names`: saving alternate names, and the links credited with them.
//! - `releases`: releases, their links, cover, file links and credits.
//! - `tracks`: tracks, and a release's whole track list with its disc titles.
//! - `suggestions`: what the import makes of a folder's tags and names.
//! - `credited`: artists as credited, and credit lines as they're saved.
//! - `titles`: alternate titles as they're saved.
//! - `repository`: the records the repositories store.

use std::collections::HashMap;
use std::sync::Arc;

use rusqlite::Connection;

use crate::languages::language_tag;
use crate::media::{FileLink, FileRecord, LinkOwner, Media, PreparedFiles};
use crate::paging::{ListArgs, Page, paging_of, search_of, sort_of};
use crate::repo::artists::NewAlternateName;
use crate::repo::named::Table;
use crate::repo::store::Store;
use crate::repo::{artists, file_links, labels, lists, named, stats};
use crate::tagging::{TagFilter, TagValuesInput};
use crate::text::{clean_or_none, clean_text};
use crate::{Error, Result};

mod credited;
mod names;
mod releases;
pub mod repository;
mod suggestions;
mod titles;
mod tracks;

pub use releases::released;
pub use repository::*;
pub use suggestions::equals_parts;
pub use tracks::performers;

/// A file by id, or by a path to register first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileRef {
    pub file_id: Option<i64>,
    pub path: Option<String>,
}

/// A file to link, with an optional note.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileLinkInput {
    pub file: FileRef,
    pub note: Option<String>,
}

/// An alternate name. With an id, one the owner has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlternateNameInput {
    pub id: Option<i64>,
    pub name: String,
    /// A BCP 47 tag. Blank means unknown.
    pub language: Option<String>,
    /// Artists only
    pub real_name: bool,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtistInput {
    pub name: String,
    pub name_language: Option<String>,
    pub picture_id: Option<i64>,
    pub files: Vec<FileLinkInput>,
    /// Replaces the alternate names. Left out, they stay as they are.
    pub alternate_names: Option<Vec<AlternateNameInput>>,
    /// On a rename, credits the links that showed the old main name with
    /// it, so they keep showing it
    pub keep_old_name: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LabelInput {
    pub name: String,
    pub name_language: Option<String>,
    pub picture_id: Option<i64>,
    /// Replaces the alternate names. Left out, they stay as they are.
    pub alternate_names: Option<Vec<AlternateNameInput>>,
}

/// An artist as credited: `credited_as` is the name printed. Blank, or the
/// artist's main name, means the main name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreditedArtistInput {
    pub artist_id: i64,
    pub credited_as: Option<String>,
}

/// A credit line: a role as printed, and its artists in order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreditInput {
    pub role: String,
    pub artists: Vec<CreditedArtistInput>,
}

/// A track's credit lines, saved with its release's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackCreditsInput {
    pub track_id: i64,
    pub credits: Vec<CreditInput>,
}

/// An alternate title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlternateTitleInput {
    pub title: String,
    /// A BCP 47 tag. Blank means unknown.
    pub language: Option<String>,
    /// On the release as published. True when left out.
    pub printed: Option<bool>,
}

/// A release's details.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleaseInput {
    pub title: String,
    /// Left out (`None`), an update keeps it as it is. `Some(None)` is
    /// unknown.
    pub title_language: Option<Option<String>>,
    /// Replaces the alternate titles. Left out, they stay as they are,
    /// except one that's now the main title.
    pub alternate_titles: Option<Vec<AlternateTitleInput>>,
    /// `2006`, `2006-01` or `2006-01-10`. Blank means unknown.
    pub released: Option<String>,
    pub catalog_number: Option<String>,
    pub various_artists: bool,
    pub artists: Vec<CreditedArtistInput>,
    pub label_ids: Vec<i64>,
    pub format_ids: Vec<i64>,
    pub medium_ids: Vec<i64>,
}

/// A row of a release's tracks: a track it has, by id, or a new one from a
/// file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReleaseTrackInput {
    /// A track the release has
    pub id: Option<i64>,
    /// A new track's audio: a registered file, or a path to register first
    pub file: FileRef,
    pub cd_number: i64,
    pub track_number: i64,
    pub title: String,
    /// Left out (`None`), it stays as it is. `Some(None)` is the release's.
    pub title_language: Option<Option<String>>,
    /// Replaces the alternate titles. Left out, they stay as they are,
    /// except one that's now the main title.
    pub alternate_titles: Option<Vec<AlternateTitleInput>>,
    pub artists: Vec<CreditedArtistInput>,
    pub features: Vec<CreditedArtistInput>,
}

/// A disc's own title, its language, and its alternate titles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiscTitleInput {
    pub cd_number: i64,
    /// Blank means no title, and then it can't have alternate titles.
    pub title: String,
    /// Blank: the release's title language
    pub language: Option<String>,
    pub alternate_titles: Vec<AlternateTitleInput>,
}

/// A track, with everything that's saved with it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrackInput {
    pub release_id: i64,
    /// A registered audio file
    pub file_id: i64,
    pub cd_number: i64,
    pub track_number: i64,
    pub title: String,
    /// Left out (`None`), an update keeps it as it is. `Some(None)` is the
    /// release's.
    pub title_language: Option<Option<String>>,
    /// Replaces the alternate titles. Left out, they stay as they are,
    /// except one that's now the main title.
    pub alternate_titles: Option<Vec<AlternateTitleInput>>,
    pub artists: Vec<CreditedArtistInput>,
    pub features: Vec<CreditedArtistInput>,
    pub credits: Vec<CreditInput>,
    pub files: Vec<FileLinkInput>,
    /// Replaces the track's tags. Left out, they stay as they are.
    pub tags: Option<Vec<TagValuesInput>>,
}

/// A track list's filters besides its search. The tag filters are checked
/// already (`tagging::tag_filters`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrackFilterArgs {
    pub release: Option<i64>,
    pub tags: Vec<TagFilter>,
}

/// A release list's filters besides its search. The tag filters are checked
/// already (`tagging::tag_filters`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReleaseFilterArgs {
    pub format: Option<i64>,
    pub medium: Option<i64>,
    pub label: Option<i64>,
    /// Releases the artist is a release artist on
    pub artist: Option<i64>,
    pub tags: Vec<TagFilter>,
}

pub struct Catalog {
    store: Arc<Store>,
    media: Arc<Media>,
}

impl Catalog {
    pub fn new(store: Arc<Store>, media: Arc<Media>) -> Catalog {
        Catalog { store, media }
    }

    // Lists: release formats, release media

    pub fn list_items(&self, kind: ListKind, args: &ListArgs) -> Result<Page<ListItem>> {
        let query = name_query(args, "Lists")?;
        let (items, total) = lists::list(&self.store.conn(), kind, &query)?;
        Ok(Page { items, total })
    }

    pub fn get_list_item(&self, kind: ListKind, id: i64) -> Result<Option<ListItem>> {
        lists::get(&self.store.conn(), kind, id)
    }

    /// The rows of these ids that exist, in no particular order.
    pub fn list_items_by_ids(&self, kind: ListKind, ids: &[i64]) -> Result<Vec<ListItem>> {
        lists::get_many(&self.store.conn(), kind, ids)
    }

    /// How many releases have each format or medium.
    pub fn list_item_uses(&self, kind: ListKind, ids: &[i64]) -> Result<HashMap<i64, i64>> {
        lists::uses_of(&self.store.conn(), kind, ids)
    }

    /// A new format or medium. Names are unique ignoring case, which the
    /// database enforces.
    pub fn create_list_item(&self, kind: ListKind, name: &str) -> Result<ListItem> {
        lists::insert(&self.store.conn(), kind, &clean_text(name))
    }

    pub fn rename_list_item(&self, kind: ListKind, id: i64, name: &str) -> Result<ListItem> {
        lists::update(&self.store.conn(), kind, id, &clean_text(name))?
            .ok_or_else(|| not_found(kind.noun(), id))
    }

    /// What keeps a format or medium from being deleted, in words.
    pub fn list_item_blockers(&self, kind: ListKind, id: i64) -> Result<Vec<String>> {
        let uses = lists::uses(&self.store.conn(), kind, id)?;
        Ok(if uses == 0 {
            Vec::new()
        } else {
            vec![format!("Used by {}", count(uses, "release"))]
        })
    }

    /// Fails while a release has it.
    pub fn delete_list_item(&self, kind: ListKind, id: i64) -> Result<()> {
        let conn = self.store.conn();
        let uses = lists::uses(&conn, kind, id)?;
        if uses > 0 {
            return Err(Error::Invalid(format!(
                "The {} is used by {}",
                kind.noun(),
                count(uses, "release")
            )));
        }
        lists::delete(&conn, kind, id)
    }

    // Artists

    /// A page of artists. The search matches the main name or an alternate
    /// name.
    pub fn list_artists(&self, args: &ListArgs) -> Result<Page<Artist>> {
        let query = name_query(args, "Artists")?;
        let (items, total) = named::list(&self.store.conn(), Table::Artists, &query)?;
        Ok(Page { items, total })
    }

    pub fn get_artist(&self, id: i64) -> Result<Option<Artist>> {
        named::get(&self.store.conn(), Table::Artists, id)
    }

    /// The artists of these ids that exist, in no particular order.
    pub fn artists_by_ids(&self, ids: &[i64]) -> Result<Vec<Artist>> {
        named::get_many(&self.store.conn(), Table::Artists, ids)
    }

    /// Artists' alternate names, by their ids, in no particular order: the
    /// names links are credited with.
    pub fn artist_names_by_ids(&self, ids: &[i64]) -> Result<Vec<AlternateName>> {
        artists::alternate_names_by_ids(&self.store.conn(), ids)
    }

    /// Releases where the artist performs or is featured on tracks, without
    /// being a release artist, with those tracks, in order.
    pub fn artist_appearances(&self, id: i64) -> Result<Vec<Appearance>> {
        let mut appearances: Vec<Appearance> = Vec::new();
        for (release_id, track_id) in artists::appearances(&self.store.conn(), id)? {
            match appearances.last_mut() {
                Some(last) if last.release_id == release_id => last.track_ids.push(track_id),
                _ => appearances.push(Appearance {
                    release_id,
                    track_ids: vec![track_id],
                }),
            }
        }
        Ok(appearances)
    }

    /// The credit lines the artist is on: the releases', then the tracks'.
    pub fn artist_credits(&self, id: i64) -> Result<Vec<ArtistCredit>> {
        artists::credits(&self.store.conn(), id)
    }

    /// How many of the artist's releases have each format.
    pub fn artist_format_counts(&self, id: i64) -> Result<Vec<FormatCount>> {
        artists::format_counts(&self.store.conn(), id)
    }

    /// Creates an artist with its alternate names and file links,
    /// registering files given by path, in one transaction.
    pub fn create_artist(&self, input: &ArtistInput) -> Result<Artist> {
        let values = names::named_values(
            &input.name,
            input.name_language.as_deref(),
            input.picture_id,
        )?;
        let prepared = self.prepare_refs(input.files.iter().map(|link| &link.file))?;
        self.store.transaction(|tx| {
            let registered = Media::insert_prepared(tx, prepared)?;
            let artist = named::insert(tx, Table::Artists, &values)?;
            names::save_artist_names(tx, None, &artist, input.alternate_names.as_deref(), false)?;
            self.set_file_links(tx, LinkOwner::Artist, artist.id, &input.files, &registered)?;
            Ok(artist)
        })
    }

    /// Saves an artist, its alternate names and its file links, in one
    /// transaction. Its links keep showing what they showed (`names.rs`).
    pub fn update_artist(&self, id: i64, input: &ArtistInput) -> Result<Artist> {
        let values = names::named_values(
            &input.name,
            input.name_language.as_deref(),
            input.picture_id,
        )?;
        let prepared = self.prepare_refs(input.files.iter().map(|link| &link.file))?;
        self.store.transaction(|tx| {
            let before =
                named::get(tx, Table::Artists, id)?.ok_or_else(|| not_found("artist", id))?;
            let registered = Media::insert_prepared(tx, prepared)?;
            let artist = named::update(tx, Table::Artists, id, &values)?
                .ok_or_else(|| not_found("artist", id))?;
            names::save_artist_names(
                tx,
                Some(&before),
                &artist,
                input.alternate_names.as_deref(),
                input.keep_old_name,
            )?;
            self.set_file_links(tx, LinkOwner::Artist, id, &input.files, &registered)?;
            Ok(artist)
        })
    }

    /// Adds an alternate name after the artist's others, like one the import
    /// offers. Refuses one the artist has already, and its main name.
    pub fn add_artist_name(&self, id: i64, name: &str, language: Option<&str>) -> Result<Artist> {
        let name = clean_text(name);
        if name.is_empty() {
            return Err(Error::Invalid("The name is blank".into()));
        }
        let language = language_tag(language)?;
        self.store.transaction(|tx| {
            let artist =
                named::get(tx, Table::Artists, id)?.ok_or_else(|| not_found("artist", id))?;
            if name == artist.name {
                return Err(Error::Invalid(format!(
                    "\"{name}\" is already the artist's main name"
                )));
            }
            let names = named::alternate_names_of(tx, Table::Artists, &[id])?
                .remove(&id)
                .unwrap_or_default();
            if names.iter().any(|known| known.name == name) {
                return Err(Error::Invalid(format!(
                    "{} already has the name \"{name}\"",
                    artist.name
                )));
            }
            artists::add_alternate_name(
                tx,
                id,
                &NewAlternateName {
                    name: name.clone(),
                    language: language.clone(),
                    real_name: false,
                    note: None,
                },
            )?;
            Ok(artist)
        })
    }

    /// Alternate names by artist, in order.
    pub fn artist_names_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<AlternateName>>> {
        named::alternate_names_of(&self.store.conn(), Table::Artists, ids)
    }

    /// How many artist links are credited with each name.
    pub fn artist_name_uses(&self, name_ids: &[i64]) -> Result<HashMap<i64, i64>> {
        artists::name_uses(&self.store.conn(), name_ids)
    }

    /// How many of an artist's links show its main name.
    pub fn main_name_uses(&self, id: i64) -> Result<i64> {
        artists::main_name_uses(&self.store.conn(), id)
    }

    /// An artist's file links, in the order they were added.
    pub fn artist_files(&self, id: i64) -> Result<Vec<FileLink>> {
        let mut links = file_links::of(&self.store.conn(), LinkOwner::Artist, &[id])?;
        Ok(links.remove(&id).unwrap_or_default())
    }

    /// Sets only an artist's picture, registering a path.
    pub fn set_artist_picture(&self, id: i64, picture: &FileRef) -> Result<Artist> {
        self.set_image(Table::Artists, "artist", id, picture)
    }

    /// What keeps an artist from being deleted, in words.
    pub fn artist_blockers(&self, id: i64) -> Result<Vec<String>> {
        artist_blockers(&self.store.conn(), id)
    }

    /// Fails while the artist is a release or track artist, or credited.
    pub fn delete_artist(&self, id: i64) -> Result<()> {
        let conn = self.store.conn();
        let blockers = artist_blockers(&conn, id)?;
        if !blockers.is_empty() {
            return Err(Error::Invalid(format!(
                "The artist is still used: {}",
                blockers.join(", ").to_lowercase()
            )));
        }
        named::delete(&conn, Table::Artists, id)
    }

    // Labels

    /// A page of labels. The search matches the main name or an alternate
    /// name.
    pub fn list_labels(&self, args: &ListArgs) -> Result<Page<Label>> {
        let query = name_query(args, "Labels")?;
        let (items, total) = named::list(&self.store.conn(), Table::Labels, &query)?;
        Ok(Page { items, total })
    }

    pub fn get_label(&self, id: i64) -> Result<Option<Label>> {
        named::get(&self.store.conn(), Table::Labels, id)
    }

    /// The labels of these ids that exist, in no particular order.
    pub fn labels_by_ids(&self, ids: &[i64]) -> Result<Vec<Label>> {
        named::get_many(&self.store.conn(), Table::Labels, ids)
    }

    pub fn create_label(&self, input: &LabelInput) -> Result<Label> {
        let values = names::named_values(
            &input.name,
            input.name_language.as_deref(),
            input.picture_id,
        )?;
        self.store.transaction(|tx| {
            let label = named::insert(tx, Table::Labels, &values)?;
            names::save_label_names(tx, &label, input.alternate_names.as_deref())?;
            Ok(label)
        })
    }

    pub fn update_label(&self, id: i64, input: &LabelInput) -> Result<Label> {
        let values = names::named_values(
            &input.name,
            input.name_language.as_deref(),
            input.picture_id,
        )?;
        self.store.transaction(|tx| {
            let label = named::update(tx, Table::Labels, id, &values)?
                .ok_or_else(|| not_found("label", id))?;
            names::save_label_names(tx, &label, input.alternate_names.as_deref())?;
            Ok(label)
        })
    }

    /// Alternate names by label, in order.
    pub fn label_names_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<AlternateName>>> {
        named::alternate_names_of(&self.store.conn(), Table::Labels, ids)
    }

    /// Sets only a label's logo, registering a path.
    pub fn set_label_picture(&self, id: i64, picture: &FileRef) -> Result<Label> {
        self.set_image(Table::Labels, "label", id, picture)
    }

    /// What keeps a label from being deleted, in words.
    pub fn label_blockers(&self, id: i64) -> Result<Vec<String>> {
        let releases = labels::release_count(&self.store.conn(), id)?;
        Ok(if releases == 0 {
            Vec::new()
        } else {
            vec![format!("On {}", count(releases, "release"))]
        })
    }

    /// Fails while the label is on a release.
    pub fn delete_label(&self, id: i64) -> Result<()> {
        let conn = self.store.conn();
        let releases = labels::release_count(&conn, id)?;
        if releases > 0 {
            return Err(Error::Invalid(format!(
                "The label is on {}",
                count(releases, "release")
            )));
        }
        named::delete(&conn, Table::Labels, id)
    }

    /// Language tags in use on names and titles, the most used first, for the
    /// language picker.
    pub fn languages(&self) -> Result<Vec<String>> {
        stats::languages(&self.store.conn())
    }

    /// Reads the files that refs give by path and aren't registered yet. It
    /// writes nothing, and reads them without holding the connection.
    fn prepare_refs<'a>(
        &self,
        refs: impl IntoIterator<Item = &'a FileRef>,
    ) -> Result<PreparedFiles> {
        let paths: Vec<String> = refs
            .into_iter()
            .filter(|file| file.file_id.is_none())
            .filter_map(|file| file.path.clone())
            .filter(|path| !path.is_empty())
            .collect();
        self.media.prepare_files(&paths)
    }

    /// A ref's file id, once its file is registered.
    fn ref_id(&self, file: &FileRef, registered: &HashMap<String, FileRecord>) -> Result<i64> {
        if let Some(id) = file.file_id {
            return Ok(id);
        }
        let Some(path) = file.path.as_deref().filter(|path| !path.is_empty()) else {
            return Err(Error::Invalid("A file needs an id or a path".into()));
        };
        registered
            .get(&self.media.normalize_path(path))
            .map(|file| file.id)
            .ok_or_else(|| Error::Invalid(format!("There's no file at {path}")))
    }

    fn set_file_links(
        &self,
        tx: &Connection,
        owner: LinkOwner,
        id: i64,
        links: &[FileLinkInput],
        registered: &HashMap<String, FileRecord>,
    ) -> Result<()> {
        let links = links
            .iter()
            .map(|link| {
                Ok(FileLink {
                    file_id: self.ref_id(&link.file, registered)?,
                    note: clean_or_none(link.note.as_deref()),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        file_links::set(tx, owner, id, &links)
    }

    /// Sets a picture or logo from a ref, registering a path in the same
    /// transaction: a file that can't be used isn't left registered.
    fn set_image(&self, table: Table, what: &str, id: i64, file: &FileRef) -> Result<Named> {
        let prepared = self.prepare_refs([file])?;
        self.store.transaction(|tx| {
            let registered = Media::insert_prepared(tx, prepared)?;
            let file_id = self.ref_id(file, &registered)?;
            named::set_picture(tx, table, id, file_id)?.ok_or_else(|| not_found(what, id))
        })
    }
}

fn artist_blockers(conn: &Connection, id: i64) -> Result<Vec<String>> {
    let usage = artists::usage(conn, id)?;
    Ok([
        (usage.releases > 0)
            .then(|| format!("Release artist on {}", count(usage.releases, "release"))),
        (usage.tracks > 0).then(|| format!("Performs on {}", count(usage.tracks, "track"))),
        (usage.credits > 0).then(|| format!("Credited {}", count(usage.credits, "time"))),
    ]
    .into_iter()
    .flatten()
    .collect())
}

/// A list's page, sorted by `name` or `id`, with its search.
fn name_query(args: &ListArgs, what: &str) -> Result<NameQuery> {
    Ok(NameQuery {
        paging: paging_of(args)?,
        sort: sort_of(args.sort.as_deref(), &[("name", NameSort::Name)], what)?,
        search: search_of(args.q.as_deref()),
    })
}

fn not_found(what: &str, id: i64) -> Error {
    Error::NotFound(format!("There's no {what} {id}"))
}

/// "1 release", "2 releases".
fn count(n: i64, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}
