//! The records the tagging context reads and writes, which `repo::tags`
//! stores.

use crate::paging::{Paging, Sort};

/// Each type has its own tables, so the same id exists once per type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagType {
    Text,
    /// A real number
    Number,
    /// A day, without a time
    Date,
}

/// The types, in the order lists show them when names tie.
pub const TAG_TYPES: [TagType; 3] = [TagType::Text, TagType::Number, TagType::Date];

impl TagType {
    /// "text", "number" or "date", as messages name it.
    pub fn as_str(self) -> &'static str {
        match self {
            TagType::Text => "text",
            TagType::Number => "number",
            TagType::Date => "date",
        }
    }

    pub(crate) fn named(text: &str) -> Option<TagType> {
        TAG_TYPES.into_iter().find(|kind| kind.as_str() == text)
    }
}

/// A tag definition, like Genre or Score.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    pub kind: TagType,
    pub id: i64,
    pub name: String,
    /// Several values per release or track, or at most one
    pub multiple: bool,
}

/// What tags are set on: releases, and tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TagOwner {
    Release,
    Track,
}

impl TagOwner {
    /// "release" or "track", as messages name it.
    pub fn as_str(self) -> &'static str {
        match self {
            TagOwner::Release => "release",
            TagOwner::Track => "track",
        }
    }
}

/// Text, a number, or a date as epoch seconds at midnight UTC.
#[derive(Debug, Clone, PartialEq)]
pub enum TagValue {
    Text(String),
    Number(f64),
    Date(i64),
}

/// One value of a tag, on a release or track.
#[derive(Debug, Clone, PartialEq)]
pub struct TagValueRow {
    pub kind: TagType,
    pub tag_id: i64,
    pub value: TagValue,
}

/// How many releases and tracks have a tag.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TagUsage {
    pub releases: i64,
    pub tracks: i64,
}

/// A text tag's value, and how many releases and tracks have it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValueCount {
    pub value: String,
    pub releases: i64,
    pub tracks: i64,
}

/// A list filter on one tag. Text matches `equals` or `contains`, ignoring
/// case. Numbers and dates match the range, both ends included.
#[derive(Debug, Clone, PartialEq)]
pub struct TagFilter {
    pub kind: TagType,
    pub tag_id: i64,
    pub equals: Option<String>,
    pub contains: Option<String>,
    pub min: Option<TagValue>,
    pub max: Option<TagValue>,
}

/// What tag lists sort by, besides `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagSort {
    Name,
}

/// A page of tags: of one type, or of every type when `kind` is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagQuery {
    pub paging: Paging,
    pub sort: Sort<TagSort>,
    pub search: Option<String>,
    pub kind: Option<TagType>,
}
