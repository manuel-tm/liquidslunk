//! A track's tags, worked out from the catalog (4_external_libraries.md §4).
//! It's the one mapping every external library writes, whatever its format,
//! and path templates name files from it, so a file's name and its tags
//! always agree. It writes only what the catalog's own structure says: tag
//! values and credit lines are the user's own vocabulary, so nothing here
//! reads them (§4.3).

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use icu_properties::CodePointMapData;
use icu_properties::props::Script;
use rusqlite::Connection;

use crate::Result;
use crate::catalog::{
    AlternateName, AlternateTitle, Artist, ArtistLink, DiscTitle, ListKind, Release, ReleaseLink,
    Track, TrackArtist, performers, released,
};
use crate::dates::{DatePrecision, format_date};
use crate::repo::named::Table;
use crate::repo::titles::{self, TitleOwner};
use crate::repo::{lists, named, releases, tracks};

/// A track's tags, as every external library writes them. A field with no
/// value isn't written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tags {
    /// The track's title, with its features: "Bridges (feat. Mari, Sander)"
    pub title: String,
    /// Who performs the track, as credited, joined with ", "
    pub artist: Option<String>,
    /// Each performer, then each feature, as credited
    pub artists: Vec<String>,
    /// The release's title
    pub album: String,
    /// The release's artists, as credited, joined with ", "
    pub album_artist: Option<String>,
    /// A various-artists release
    pub compilation: bool,
    /// The release date, as precise as it's known: `2006`, `2006-01` or
    /// `2006-01-10`
    pub date: Option<String>,
    /// The release date's year
    pub year: Option<i64>,
    pub track_number: i64,
    /// The last track number on the track's disc
    pub track_total: i64,
    pub disc_number: i64,
    /// The release's last disc
    pub disc_total: i64,
    /// The disc's title
    pub disc_title: Option<String>,
    /// The release's labels, in order
    pub labels: Vec<String>,
    pub catalog_number: Option<String>,
    /// The release's media, as they're named: CD, Vinyl
    pub media: Vec<String>,
    /// The release's formats, as they're named: Album, EP, Live
    pub release_types: Vec<String>,
    pub sort: SortFields,
    /// The release's cover, a file id
    pub cover: Option<i64>,
}

/// What players sort by instead of a name or title that isn't in Latin
/// script: its Latin-script form (§4.2). `None`: they sort by the name
/// itself.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SortFields {
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub title: Option<String>,
}

/// Each track's tags, by id, read with one connection. Ids that aren't
/// tracks are left out.
pub(crate) fn read(conn: &Connection, track_ids: &[i64]) -> Result<HashMap<i64, Tags>> {
    let found = tracks::get_many(conn, track_ids)?;
    let ids: Vec<i64> = found.iter().map(|track| track.id).collect();
    let release_ids = unique(found.iter().map(|track| track.release_id));
    let release_artists = releases::artists_of(conn, &release_ids)?;
    let own = tracks::artists_of(conn, &ids)?;
    let artist_ids = unique(
        release_artists
            .values()
            .flatten()
            .map(|link| link.artist_id)
            .chain(own.values().flatten().map(|artist| artist.artist_id)),
    );
    let labels = releases::links_of(conn, ReleaseLink::Labels, &release_ids)?;
    let formats = releases::links_of(conn, ReleaseLink::Formats, &release_ids)?;
    let media = releases::links_of(conn, ReleaseLink::Media, &release_ids)?;
    let catalog = Snapshot {
        releases: by_id(releases::get_many(conn, &release_ids)?, |release| {
            release.id
        }),
        totals: totals(tracks::of_releases(conn, &release_ids)?),
        release_artists,
        own,
        artists: Artists {
            main: by_id(
                named::get_many(conn, Table::Artists, &artist_ids)?,
                |artist| artist.id,
            ),
            names: named::alternate_names_of(conn, Table::Artists, &artist_ids)?,
        },
        release_titles: titles::alternate_titles_of(conn, TitleOwner::Release, &release_ids)?,
        track_titles: titles::alternate_titles_of(conn, TitleOwner::Track, &ids)?,
        disc_titles: releases::disc_titles_of(conn, &release_ids)?,
        label_names: named::get_many(
            conn,
            Table::Labels,
            &unique(labels.values().flatten().copied()),
        )?
        .into_iter()
        .map(|label| (label.id, label.name))
        .collect(),
        format_names: list_names(conn, ListKind::ReleaseFormats, &formats)?,
        medium_names: list_names(conn, ListKind::ReleaseMedia, &media)?,
        labels,
        formats,
        media,
    };
    Ok(found
        .iter()
        .filter_map(|track| Some((track.id, catalog.tags(track)?)))
        .collect())
}

/// What the tags of some tracks are worked out from, read in one go.
struct Snapshot {
    releases: HashMap<i64, Release>,
    totals: HashMap<i64, Totals>,
    release_artists: HashMap<i64, Vec<ArtistLink>>,
    own: HashMap<i64, Vec<TrackArtist>>,
    artists: Artists,
    release_titles: HashMap<i64, Vec<AlternateTitle>>,
    track_titles: HashMap<i64, Vec<AlternateTitle>>,
    disc_titles: HashMap<i64, Vec<DiscTitle>>,
    labels: HashMap<i64, Vec<i64>>,
    formats: HashMap<i64, Vec<i64>>,
    media: HashMap<i64, Vec<i64>>,
    label_names: HashMap<i64, String>,
    format_names: HashMap<i64, String>,
    medium_names: HashMap<i64, String>,
}

impl Snapshot {
    fn tags(&self, track: &Track) -> Option<Tags> {
        let release = self.releases.get(&track.release_id)?;
        let own = slice(&self.own, track.id);
        let release_artists = slice(&self.release_artists, release.id);
        let performing = performers(release, own, release_artists);
        let features: Vec<ArtistLink> = own
            .iter()
            .filter(|artist| artist.featured)
            .map(|artist| ArtistLink {
                artist_id: artist.artist_id,
                credited_name_id: artist.credited_name_id,
            })
            .collect();
        let performer_names = self.artists.names(&performing);
        let feature_names = self.artists.names(&features);
        let title = if feature_names.is_empty() {
            track.title.clone()
        } else {
            format!("{} (feat. {})", track.title, feature_names.join(", "))
        };
        let release_names = self.artists.names(release_artists);
        let totals = self.totals.get(&release.id);
        let names = |ids: &HashMap<i64, Vec<i64>>, names: &HashMap<i64, String>| -> Vec<String> {
            slice(ids, release.id)
                .iter()
                .filter_map(|id| names.get(id).cloned())
                .collect()
        };
        Some(Tags {
            sort: SortFields {
                artist: self.artists.joined_sort(&performing),
                album_artist: self.artists.joined_sort(release_artists),
                album: title_sort(
                    &release.title,
                    release.title_language.as_deref(),
                    slice(&self.release_titles, release.id),
                ),
                title: title_sort(
                    &track.title,
                    track
                        .title_language
                        .as_deref()
                        .or(release.title_language.as_deref()),
                    slice(&self.track_titles, track.id),
                ),
            },
            title,
            artist: joined(&performer_names),
            artists: performer_names.into_iter().chain(feature_names).collect(),
            album: release.title.clone(),
            album_artist: joined(&release_names),
            compilation: release.various_artists,
            date: released(release),
            year: release
                .release_date
                .and_then(|epoch| format_date(epoch, DatePrecision::Year).parse().ok()),
            track_number: track.track_number,
            track_total: totals
                .and_then(|totals| totals.last_tracks.get(&track.cd_number).copied())
                .unwrap_or(track.track_number),
            disc_number: track.cd_number,
            disc_total: totals.map_or(track.cd_number, |totals| totals.last_disc),
            disc_title: slice(&self.disc_titles, release.id)
                .iter()
                .find(|disc| disc.cd_number == track.cd_number)
                .map(|disc| disc.title.clone()),
            labels: names(&self.labels, &self.label_names),
            catalog_number: release.catalog_number.clone(),
            media: names(&self.media, &self.medium_names),
            release_types: names(&self.formats, &self.format_names),
            cover: release.cover_picture,
        })
    }
}

/// A release's last disc, and each disc's last track number.
struct Totals {
    last_disc: i64,
    last_tracks: HashMap<i64, i64>,
}

fn totals(tracks: HashMap<i64, Vec<Track>>) -> HashMap<i64, Totals> {
    tracks
        .into_iter()
        .map(|(release, tracks)| {
            let mut totals = Totals {
                last_disc: 0,
                last_tracks: HashMap::new(),
            };
            for track in tracks {
                totals.last_disc = totals.last_disc.max(track.cd_number);
                let last = totals.last_tracks.entry(track.cd_number).or_default();
                *last = (*last).max(track.track_number);
            }
            (release, totals)
        })
        .collect()
}

/// Artists and their alternate names, which name them as credited.
struct Artists {
    main: HashMap<i64, Artist>,
    names: HashMap<i64, Vec<AlternateName>>,
}

impl Artists {
    /// The name printed on a link, and its language: one of the artist's
    /// alternate names, or its main name.
    fn credited(&self, link: &ArtistLink) -> Option<(&str, Option<&str>)> {
        let artist = self.main.get(&link.artist_id)?;
        let alternate = link
            .credited_name_id
            .and_then(|id| slice(&self.names, artist.id).iter().find(|n| n.id == id));
        Some(match alternate {
            Some(name) => (name.name.as_str(), name.language.as_deref()),
            None => (artist.name.as_str(), artist.name_language.as_deref()),
        })
    }

    /// The names printed on links, in order.
    fn names(&self, links: &[ArtistLink]) -> Vec<String> {
        links
            .iter()
            .filter_map(|link| Some(self.credited(link)?.0.to_string()))
            .collect()
    }

    /// What a link sorts by, when the name printed isn't in Latin script: the
    /// artist's main name, when that is, or else the first of its alternate
    /// names that's a Latin-script form of the name printed. A real name is
    /// another name, not a form of this one, so it doesn't count.
    fn sort(&self, link: &ArtistLink) -> Option<String> {
        let artist = self.main.get(&link.artist_id)?;
        let (name, language) = self.credited(link)?;
        if is_latin(name) {
            return None;
        }
        if is_latin(&artist.name) {
            return Some(artist.name.clone());
        }
        slice(&self.names, artist.id)
            .iter()
            .find(|form| {
                !form.real_name && is_latin_form(language, &form.name, form.language.as_deref())
            })
            .map(|form| form.name.clone())
    }

    /// What names joined with ", " sort by: each one's sort name, or else the
    /// name itself. `None` when none of them has a sort name.
    fn joined_sort(&self, links: &[ArtistLink]) -> Option<String> {
        let sorts: Vec<Option<String>> = links.iter().map(|link| self.sort(link)).collect();
        if sorts.iter().all(Option::is_none) {
            return None;
        }
        let names: Vec<String> = links
            .iter()
            .zip(sorts)
            .filter_map(|(link, sort)| sort.or_else(|| Some(self.credited(link)?.0.to_string())))
            .collect();
        joined(&names)
    }
}

/// What a title sorts by, when it isn't in Latin script: its first alternate
/// title that's a Latin-script form of it. A translation isn't one.
fn title_sort(
    title: &str,
    language: Option<&str>,
    alternates: &[AlternateTitle],
) -> Option<String> {
    if is_latin(title) {
        return None;
    }
    alternates
        .iter()
        .find(|form| is_latin_form(language, &form.title, form.language.as_deref()))
        .map(|form| form.title.clone())
}

/// Whether another name or title is a Latin-script form of one in
/// `language`: it's in Latin script, and its language is tagged with the
/// Latin script, in the same language, like `ja-Latn` for `ja`. When
/// `language` isn't known, any language tagged with the Latin script will
/// do, so "Atarashii Hikari" in `ja-Latn` is a form of "新しい光", while a
/// translation, like "Early Summer" in `en`, isn't.
fn is_latin_form(language: Option<&str>, form: &str, form_language: Option<&str>) -> bool {
    let Some(form_language) = form_language else {
        return false;
    };
    let mut subtags = form_language.split('-');
    let form_primary = subtags.next();
    // A canonical tag's script follows its language: ja-Latn
    if subtags.next() != Some("Latn") || !is_latin(form) {
        return false;
    }
    language.is_none_or(|language| language.split('-').next() == form_primary)
}

/// Whether a text is in Latin script, by ICU's script property: none of its
/// characters belong to another script. Digits, punctuation and spaces
/// belong to none, so "1999" is.
fn is_latin(text: &str) -> bool {
    let scripts = CodePointMapData::<Script>::new();
    let neutral = [
        Script::Latin,
        Script::Common,
        Script::Inherited,
        Script::Unknown,
    ];
    text.chars().all(|c| neutral.contains(&scripts.get(c)))
}

/// Names joined with ", ", as the app joins them, or `None` for none.
fn joined(names: &[String]) -> Option<String> {
    (!names.is_empty()).then(|| names.join(", "))
}

/// Formats' or media's names, by id.
fn list_names(
    conn: &Connection,
    kind: ListKind,
    links: &HashMap<i64, Vec<i64>>,
) -> Result<HashMap<i64, String>> {
    Ok(
        lists::get_many(conn, kind, &unique(links.values().flatten().copied()))?
            .into_iter()
            .map(|item| (item.id, item.name))
            .collect(),
    )
}

fn by_id<T>(rows: Vec<T>, id: impl Fn(&T) -> i64) -> HashMap<i64, T> {
    rows.into_iter().map(|row| (id(&row), row)).collect()
}

fn slice<K: Eq + Hash, T>(map: &HashMap<K, Vec<T>>, key: K) -> &[T] {
    map.get(&key).map(Vec::as_slice).unwrap_or_default()
}

/// Each id once, in the order it first came.
fn unique(ids: impl IntoIterator<Item = i64>) -> Vec<i64> {
    let mut seen = HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latin_script_is_told_apart_by_its_characters() {
        assert!(is_latin("Tõnu Naissoo"));
        assert!(is_latin("9mm Parabellum Bullet"));
        assert!(is_latin("1999"));
        assert!(is_latin("ＡＢＣ"));
        assert!(!is_latin("新しい光"));
        assert!(!is_latin("MIYAVI 雅"));
        assert!(!is_latin("Алика"));
    }

    #[test]
    fn a_latin_form_is_in_the_same_language_tagged_latin() {
        assert!(is_latin_form(Some("ja"), "Shoka", Some("ja-Latn")));
        assert!(is_latin_form(Some("ja"), "Shoka", Some("ja-Latn-hepburn")));
        // A language that isn't known takes any Latin-script tag
        assert!(is_latin_form(None, "Atarashii Hikari", Some("ja-Latn")));
        // A translation, another language, or text in another script isn't
        assert!(!is_latin_form(Some("ja"), "Early Summer", Some("en")));
        assert!(!is_latin_form(None, "Early Summer", Some("en")));
        assert!(!is_latin_form(Some("ja"), "Chuxia", Some("zh-Latn")));
        assert!(!is_latin_form(Some("ja"), "しょか", Some("ja-Latn")));
        assert!(!is_latin_form(None, "Shoka", None));
    }
}
