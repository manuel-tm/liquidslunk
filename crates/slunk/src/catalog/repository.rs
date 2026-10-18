//! The records the catalog reads and writes, which `repo::named`,
//! `repo::artists`, `repo::labels`, `repo::lists` and `repo::stats` store.

use crate::paging::{Paging, Sort};

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
