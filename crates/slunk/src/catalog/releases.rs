//! Releases: their details, alternate titles, artists as credited, labels,
//! formats and media; their cover and file links; their credit lines and their
//! tracks'; and what the release pages read.

use std::collections::HashMap;

use rusqlite::Connection;

use super::credited::{CreditedNames, first_of_each};
use super::titles::save_alternate_titles;
use super::{
    AlternateTitle, ArtistLink, Catalog, Credit, CreditInput, CreditOwner, DiscTitle,
    FileLinkInput, FileRef, Release, ReleaseFilterArgs, ReleaseFilters, ReleaseInput, ReleaseLink,
    ReleaseQuery, ReleaseSort, ReleaseValues, Totals, Track, TrackArtist, TrackCreditsInput, count,
    not_found,
};
use crate::dates::{format_date, parse_date};
use crate::languages::language_tag;
use crate::media::{LinkOwner, Media};
use crate::paging::{ListArgs, Page, paging_of, search_of, sort_of};
use crate::repo::titles::{self, TitleOwner};
use crate::repo::{credits, releases, stats, tracks};
use crate::text::{clean_or_none, clean_text, trim};
use crate::{Error, Result};

// How many roles the credits editor suggests
const ROLE_SUGGESTIONS: i64 = 20;

impl Catalog {
    /// A page of releases. The filters all have to match: "EPs on vinyl",
    /// "Genre is Rock and Score is 8 or more".
    pub fn list_releases(
        &self,
        args: &ListArgs,
        filters: &ReleaseFilterArgs,
    ) -> Result<Page<Release>> {
        let query = ReleaseQuery {
            paging: paging_of(args)?,
            sort: sort_of(
                args.sort.as_deref(),
                &[
                    ("title", ReleaseSort::Title),
                    ("released", ReleaseSort::Released),
                ],
                "Releases",
            )?,
            filters: release_filters(args.q.as_deref(), filters),
        };
        let (items, total) = releases::list(&self.store.conn(), &query)?;
        Ok(Page { items, total })
    }

    /// Every matching release's id, newest first, to select them all for a
    /// bulk edit.
    pub fn release_ids(&self, q: Option<&str>, filters: &ReleaseFilterArgs) -> Result<Vec<i64>> {
        releases::ids(&self.store.conn(), &release_filters(q, filters))
    }

    pub fn get_release(&self, id: i64) -> Result<Option<Release>> {
        releases::get(&self.store.conn(), id)
    }

    /// The releases of these ids that exist, in no particular order.
    pub fn releases_by_ids(&self, ids: &[i64]) -> Result<Vec<Release>> {
        releases::get_many(&self.store.conn(), ids)
    }

    /// Linked ids by release, in display order.
    pub fn release_links_of(
        &self,
        link: ReleaseLink,
        ids: &[i64],
    ) -> Result<HashMap<i64, Vec<i64>>> {
        releases::links_of(&self.store.conn(), link, ids)
    }

    /// Release artists, as credited, by release.
    pub fn release_artists_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<ArtistLink>>> {
        releases::artists_of(&self.store.conn(), ids)
    }

    /// Alternate titles by release, in order.
    pub fn release_titles_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<AlternateTitle>>> {
        titles::alternate_titles_of(&self.store.conn(), TitleOwner::Release, ids)
    }

    /// Disc titles by release, by disc number.
    pub fn disc_titles_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<DiscTitle>>> {
        releases::disc_titles_of(&self.store.conn(), ids)
    }

    /// Creates a release with its alternate titles, artists, labels,
    /// formats and media, in one transaction.
    pub fn create_release(&self, input: &ReleaseInput) -> Result<Release> {
        let values = release_values(input)?;
        self.store.transaction(|tx| {
            let release = releases::insert(tx, &values)?;
            save_alternate_titles(
                tx,
                TitleOwner::Release,
                release.id,
                &release.title,
                input.alternate_titles.as_deref(),
                "the release",
            )?;
            set_release_links(tx, release.id, input)?;
            Ok(release)
        })
    }

    /// Saves a release's details. Its title's language, left out, stays as
    /// it is.
    pub fn update_release(&self, id: i64, input: &ReleaseInput) -> Result<Release> {
        let values = release_values(input)?;
        self.store.transaction(|tx| {
            let release = releases::update(tx, id, &values, input.title_language.is_none())?
                .ok_or_else(|| not_found("release", id))?;
            save_alternate_titles(
                tx,
                TitleOwner::Release,
                id,
                &release.title,
                input.alternate_titles.as_deref(),
                "the release",
            )?;
            set_release_links(tx, id, input)?;
            Ok(release)
        })
    }

    /// What keeps a release from being deleted, in words.
    pub fn release_blockers(&self, id: i64) -> Result<Vec<String>> {
        let tracks = releases::track_count(&self.store.conn(), id)?;
        Ok(if tracks == 0 {
            Vec::new()
        } else {
            vec![format!("Has {}", count(tracks, "track"))]
        })
    }

    /// Fails while the release has tracks. Its links, titles, credits, tags
    /// and file links go with it, and its files stay registered. An unknown
    /// id deletes nothing.
    pub fn delete_release(&self, id: i64) -> Result<()> {
        let conn = self.store.conn();
        let tracks = releases::track_count(&conn, id)?;
        if tracks > 0 {
            return Err(Error::Invalid(format!(
                "The release still has {}. Delete them first.",
                count(tracks, "track")
            )));
        }
        releases::delete(&conn, id)
    }

    /// Replaces a release's credit lines, and its tracks'.
    pub fn set_release_credits(
        &self,
        id: i64,
        credits: &[CreditInput],
        track_credits: &[TrackCreditsInput],
    ) -> Result<Release> {
        self.store.transaction(|tx| {
            let release = releases::get(tx, id)?.ok_or_else(|| not_found("release", id))?;
            let on_release: Vec<i64> = tracks::of_releases(tx, &[id])?
                .remove(&id)
                .unwrap_or_default()
                .iter()
                .map(|track| track.id)
                .collect();
            let mut credited = CreditedNames::default();
            credits::set(tx, CreditOwner::Release, id, &credited.lines(tx, credits)?)?;
            for track in track_credits {
                if !on_release.contains(&track.track_id) {
                    return Err(Error::Invalid(format!(
                        "Track {} isn't on this release",
                        track.track_id
                    )));
                }
                credits::set(
                    tx,
                    CreditOwner::Track,
                    track.track_id,
                    &credited.lines(tx, &track.credits)?,
                )?;
            }
            Ok(release)
        })
    }

    /// Sets a release's cover and replaces its file links, registering files
    /// given by path in the same transaction.
    pub fn set_release_files(
        &self,
        id: i64,
        cover: Option<&FileRef>,
        files: &[FileLinkInput],
    ) -> Result<Release> {
        let refs = cover.into_iter().chain(files.iter().map(|link| &link.file));
        let prepared = self.prepare_refs(refs)?;
        self.store.transaction(|tx| {
            let registered = Media::insert_prepared(tx, prepared)?;
            let cover = cover
                .map(|cover| self.ref_id(cover, &registered))
                .transpose()?;
            let release =
                releases::set_cover(tx, id, cover)?.ok_or_else(|| not_found("release", id))?;
            self.set_file_links(tx, LinkOwner::Release, id, files, &registered)?;
            Ok(release)
        })
    }

    /// Sets only a release's cover, keeping its file links. A file given by
    /// path is registered in the same transaction, so a file the database
    /// refuses isn't left registered.
    pub fn set_release_cover(&self, id: i64, cover: &FileRef) -> Result<Release> {
        let prepared = self.prepare_refs([cover])?;
        self.store.transaction(|tx| {
            let registered = Media::insert_prepared(tx, prepared)?;
            let file_id = self.ref_id(cover, &registered)?;
            releases::set_cover(tx, id, Some(file_id))?.ok_or_else(|| not_found("release", id))
        })
    }

    /// Tracks by release, in order: disc, then number.
    pub fn tracks_of_releases(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<Track>>> {
        tracks::of_releases(&self.store.conn(), ids)
    }

    /// The tracks of these ids that exist, in no particular order.
    pub fn tracks_by_ids(&self, ids: &[i64]) -> Result<Vec<Track>> {
        tracks::get_many(&self.store.conn(), ids)
    }

    /// Each track's own artists and features, as credited, in order.
    pub fn track_artists_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<TrackArtist>>> {
        tracks::artists_of(&self.store.conn(), ids)
    }

    /// Alternate titles by track, in order.
    pub fn track_titles_of(&self, ids: &[i64]) -> Result<HashMap<i64, Vec<AlternateTitle>>> {
        titles::alternate_titles_of(&self.store.conn(), TitleOwner::Track, ids)
    }

    /// Credit lines by release or track, in order.
    pub fn credits_of(&self, owner: CreditOwner, ids: &[i64]) -> Result<HashMap<i64, Vec<Credit>>> {
        credits::of(&self.store.conn(), owner, ids)
    }

    /// Roles in use, for the credits editor's suggestions: the first 20 with
    /// `q` in them, ignoring case, the most used first. Each spelling is its
    /// own suggestion.
    pub fn credit_roles(&self, q: Option<&str>) -> Result<Vec<String>> {
        credits::roles(
            &self.store.conn(),
            search_of(q).as_deref(),
            ROLE_SUGGESTIONS,
        )
    }

    /// Releases with no artist, by title: the library check's.
    pub fn releases_without_artists(&self) -> Result<Vec<Release>> {
        releases::without_artists(&self.store.conn())
    }

    /// The catalog's totals, for the dashboard.
    pub fn totals(&self) -> Result<Totals> {
        stats::totals(&self.store.conn())
    }
}

/// A release's date with only its known parts: `2006`, `2006-01` or
/// `2006-01-10`.
pub fn released(release: &Release) -> Option<String> {
    Some(format_date(
        release.release_date?,
        release.release_date_precision?,
    ))
}

fn release_filters(q: Option<&str>, filters: &ReleaseFilterArgs) -> ReleaseFilters {
    ReleaseFilters {
        search: search_of(q),
        format: filters.format,
        medium: filters.medium,
        label: filters.label,
        artist: filters.artist,
        tags: filters.tags.clone(),
    }
}

/// A release's own columns, as they're saved: the title cleaned, the
/// language canonical, and the date as typed, read.
fn release_values(input: &ReleaseInput) -> Result<ReleaseValues> {
    let title = clean_text(&input.title);
    let title_language = language_tag(input.title_language.as_ref().and_then(Option::as_deref))?;
    let released = input
        .released
        .as_deref()
        .map(trim)
        .filter(|text| !text.is_empty());
    let date = released.map(parse_date).transpose()?;
    Ok(ReleaseValues {
        title,
        title_language,
        release_date: date.map(|date| date.epoch),
        release_date_precision: date.map(|date| date.precision),
        catalog_number: clean_or_none(input.catalog_number.as_deref()),
        various_artists: input.various_artists,
    })
}

/// A release's artists, as credited, and its labels, formats and media.
fn set_release_links(tx: &Connection, id: i64, input: &ReleaseInput) -> Result<()> {
    let artists = CreditedNames::default().resolve(tx, &first_of_each(&input.artists))?;
    releases::set_artists(tx, id, &artists)?;
    releases::set_links(tx, ReleaseLink::Labels, id, &unique(&input.label_ids))?;
    releases::set_links(tx, ReleaseLink::Formats, id, &unique(&input.format_ids))?;
    releases::set_links(tx, ReleaseLink::Media, id, &unique(&input.medium_ids))
}

/// Each id once, in the order it first came.
fn unique(ids: &[i64]) -> Vec<i64> {
    let mut seen = std::collections::HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}
