//! The records the catalog reads and writes, which `repo::named`,
//! `repo::artists`, `repo::labels`, `repo::lists`, `repo::releases`,
//! `repo::titles`, `repo::tracks`, `repo::credits` and `repo::stats` store.

use crate::dates::DatePrecision;
use crate::paging::{Paging, Sort};
use crate::tagging::TagFilter;

/// An artist or a label: they have the same columns. A label's picture is
/// its logo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub id: i64,
    /// The main name (slunk's 3_alternate_names.md §4.2)
    pub name: String,
    /// A BCP 47 tag, or `None` when it isn't known
    pub name_language: Option<String>,
    /// A file id
    pub picture: Option<i64>,
    pub inserted_at: i64,
    pub updated_at: i64,
}

pub type Artist = Named;
pub type Label = Named;

/// Artists and labels both have alternate names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NamedKind {
    Artist,
    Label,
}

impl NamedKind {
    /// "artist" or "label", as messages name it.
    pub fn as_str(self) -> &'static str {
        match self {
            NamedKind::Artist => "artist",
            NamedKind::Label => "label",
        }
    }
}

/// An artist's or label's own columns, as they're saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedValues {
    pub name: String,
    pub name_language: Option<String>,
    pub picture: Option<i64>,
}

/// A name an artist or label goes by, besides its main one (slunk's
/// 3_alternate_names.md §4.2). A label's is never a real name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateName {
    pub id: i64,
    pub name: String,
    /// A BCP 47 tag, or `None` when it isn't known
    pub language: Option<String>,
    /// A legal or birth name
    pub real_name: bool,
    pub note: Option<String>,
}

/// An alternate name to save. With an id, it's one the owner has.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateNameValues {
    pub id: Option<i64>,
    pub name: String,
    pub language: Option<String>,
    pub real_name: bool,
    pub note: Option<String>,
}

impl From<&AlternateName> for AlternateNameValues {
    fn from(name: &AlternateName) -> Self {
        AlternateNameValues {
            id: Some(name.id),
            name: name.name.clone(),
            language: name.language.clone(),
            real_name: name.real_name,
            note: name.note.clone(),
        }
    }
}

/// Where an artist is used, which keeps it from being deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtistUsage {
    /// Releases it's a release artist on
    pub releases: i64,
    /// Tracks it performs or is featured on
    pub tracks: i64,
    /// Credit lines it's on
    pub credits: i64,
}

/// The lists the user manages, which are just names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListKind {
    ReleaseFormats,
    ReleaseMedia,
}

impl ListKind {
    /// "format" or "medium", as messages name a row.
    pub fn noun(self) -> &'static str {
        match self {
            ListKind::ReleaseFormats => "format",
            ListKind::ReleaseMedia => "medium",
        }
    }
}

/// A release format or release medium.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListItem {
    pub kind: ListKind,
    pub id: i64,
    pub name: String,
}

/// The columns artists, labels and lists can be sorted by, besides `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameSort {
    Name,
}

/// A page of artists, labels or list rows, once the arguments are checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameQuery {
    pub paging: Paging,
    pub sort: Sort<NameSort>,
    /// Part of the name, ignoring case. For artists and labels, part of an
    /// alternate name finds them too.
    pub search: Option<String>,
}

/// A release (slunk's design §4.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub id: i64,
    /// The main title (slunk's 3_alternate_names.md §4.2)
    pub title: String,
    /// Its language, and its discs' and tracks' unless they have their own
    pub title_language: Option<String>,
    /// Epoch seconds, midnight UTC
    pub release_date: Option<i64>,
    pub release_date_precision: Option<DatePrecision>,
    pub catalog_number: Option<String>,
    /// A file id
    pub cover_picture: Option<i64>,
    /// Each track names its own performers: compilations, soundtracks
    pub various_artists: bool,
    pub inserted_at: i64,
    pub updated_at: i64,
}

/// A release's own columns, as saving its details sets them: all but the
/// cover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseValues {
    pub title: String,
    pub title_language: Option<String>,
    pub release_date: Option<i64>,
    pub release_date_precision: Option<DatePrecision>,
    pub catalog_number: Option<String>,
    pub various_artists: bool,
}

/// A track. Only read so far: tracks are saved through slunk's backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub id: i64,
    pub title: String,
    /// `None`: the release's title language
    pub title_language: Option<String>,
    pub duration_ms: Option<i64>,
    pub track_number: i64,
    pub cd_number: i64,
    pub release_id: i64,
    pub file_id: i64,
    pub inserted_at: i64,
    pub updated_at: i64,
}

/// An artist named on a release, a track or a credit line, and the
/// alternate name printed there: `None` for the main name (slunk's
/// 3_alternate_names.md §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtistLink {
    pub artist_id: i64,
    pub credited_name_id: Option<i64>,
}

/// A track's artist, or a featured one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackArtist {
    pub artist_id: i64,
    pub credited_name_id: Option<i64>,
    pub featured: bool,
}

/// A title besides the main one, of a release, a track or a disc (slunk's
/// 3_alternate_names.md §4.2). `printed` says it's on the release as
/// published, rather than added, like a translation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlternateTitle {
    pub title: String,
    pub language: Option<String>,
    pub printed: bool,
}

/// A disc's own title, as printed. Only discs with a title have one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscTitle {
    pub cd_number: i64,
    pub title: String,
    /// `None`: the release's title language
    pub language: Option<String>,
    pub alternate_titles: Vec<AlternateTitle>,
}

/// A credit line: a role as printed, and the artists who hold it, in order
/// (slunk's 2_credits.md §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    pub role: String,
    pub artists: Vec<ArtistLink>,
}

/// What credit lines belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CreditOwner {
    Release,
    Track,
}

/// The rows that link a release to artists, labels, formats or media. A
/// release's artists also have credited names: see `releases::artists_of`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReleaseLink {
    Artists,
    Labels,
    Formats,
    Media,
}

/// A release the artist is on as a track artist only, with those tracks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Appearance {
    pub release_id: i64,
    pub track_ids: Vec<i64>,
}

/// A credit line the artist is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtistCredit {
    pub role: String,
    pub release_id: i64,
    /// `None` for a line on the release itself
    pub track_id: Option<i64>,
    /// The alternate name the artist is credited with on the line
    pub credited_name_id: Option<i64>,
}

/// How many of an artist's releases have a format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatCount {
    pub format_id: i64,
    pub count: i64,
}

/// The catalog's totals, for the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totals {
    pub releases: i64,
    pub artists: i64,
    pub tracks: i64,
    pub labels: i64,
    /// The tracks' playing time
    pub duration_ms: i64,
}

/// The columns releases can be sorted by, besides `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseSort {
    Title,
    /// The release date, undated releases last either way
    Released,
}

/// A release list's search and filters. They all have to match.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ReleaseFilters {
    /// Part of the title or of an alternate title, ignoring case
    pub search: Option<String>,
    pub format: Option<i64>,
    pub medium: Option<i64>,
    pub label: Option<i64>,
    /// Releases the artist is a release artist on
    pub artist: Option<i64>,
    pub tags: Vec<TagFilter>,
}

/// A page of releases, once the arguments are checked.
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseQuery {
    pub paging: Paging,
    pub sort: Sort<ReleaseSort>,
    pub filters: ReleaseFilters,
}
