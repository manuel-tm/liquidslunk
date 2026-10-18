//! The tagging context: tag definitions, and their values on releases and
//! tracks (slunk's design §4.3 Tags, §6).
//!
//! - `values`: values as they're typed, and as they're shown.
//! - `repository`: the records `repo::tags` stores.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rusqlite::Connection;

use crate::dates::{last_day, parse_date};
use crate::paging::{ListArgs, Page, paging_of, search_of, sort_of};
use crate::repo::store::Store;
use crate::repo::tags;
use crate::text::{clean_text, trim};
use crate::{Error, Result};

pub mod repository;
mod values;

pub use repository::*;
pub use values::{format_number, format_value};
use values::{parse_number, parse_value, same_value_key};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagInput {
    pub name: String,
    pub multiple: bool,
}

/// A tag's values on a release or track, as the user typed them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagValuesInput {
    pub kind: TagType,
    pub tag_id: i64,
    pub values: Vec<String>,
}

/// A list filter as the list's URL has it. Text takes `equals` or
/// `contains`. Numbers and dates take `equals`, or a range from `min` to
/// `max`. A date is its whole year, month or day: `max: 2006` includes Dec
/// 31, 2006.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagFilterInput {
    pub kind: TagType,
    pub tag_id: i64,
    pub equals: Option<String>,
    pub contains: Option<String>,
    pub min: Option<String>,
    pub max: Option<String>,
}

/// An owner's values of one tag, in the order they were set.
#[derive(Debug, Clone, PartialEq)]
pub struct OwnerTag {
    pub kind: TagType,
    pub tag_id: i64,
    pub values: Vec<TagValue>,
}

pub struct Tagging {
    store: Arc<Store>,
}

impl Tagging {
    pub fn new(store: Arc<Store>) -> Tagging {
        Tagging { store }
    }

    /// Every type's tags when `kind` is `None`.
    pub fn list_tags(&self, args: &ListArgs, kind: Option<TagType>) -> Result<Page<Tag>> {
        let query = TagQuery {
            paging: paging_of(args)?,
            sort: sort_of(args.sort.as_deref(), &[("name", TagSort::Name)], "Tags")?,
            search: search_of(args.q.as_deref()),
            kind,
        };
        let (items, total) = tags::list(&self.store.conn(), &query)?;
        Ok(Page { items, total })
    }

    pub fn get_tag(&self, kind: TagType, id: i64) -> Result<Option<Tag>> {
        tags::get(&self.store.conn(), kind, id)
    }

    /// The tags of these ids that exist, in no particular order.
    pub fn tags_by_ids(&self, kind: TagType, ids: &[i64]) -> Result<Vec<Tag>> {
        tags::get_many(&self.store.conn(), kind, ids)
    }

    pub fn create_tag(&self, kind: TagType, input: &TagInput) -> Result<Tag> {
        let name = clean_text(&input.name);
        self.store.transaction(|tx| {
            check_name(tx, kind, &name)?;
            tags::insert(tx, kind, &name, input.multiple)
        })
    }

    /// Renames a tag, or switches it between single and multiple values.
    pub fn update_tag(&self, kind: TagType, id: i64, input: &TagInput) -> Result<Tag> {
        let name = clean_text(&input.name);
        self.store.transaction(|tx| {
            let current = tags::get(tx, kind, id)?.ok_or_else(|| not_found(kind, id))?;
            check_name(tx, kind, &name)?;
            // The database refuses it too. This says how many owners are why.
            if current.multiple && !input.multiple {
                let first = paging_of(&ListArgs {
                    per_page: Some(1),
                    ..ListArgs::default()
                })?;
                let several = TagUsage {
                    releases: tags::multi_value_owners(tx, TagOwner::Release, kind, id, first)?.1,
                    tracks: tags::multi_value_owners(tx, TagOwner::Track, kind, id, first)?.1,
                };
                let total = several.releases + several.tracks;
                if total > 0 {
                    return Err(Error::Invalid(format!(
                        "{} {} several values for this tag. Leave one value on each before making it single-value.",
                        owners(several),
                        if total == 1 { "has" } else { "have" }
                    )));
                }
            }
            tags::update(tx, kind, id, &name, input.multiple)?.ok_or_else(|| not_found(kind, id))
        })
    }

    /// How many releases and tracks have the tag.
    pub fn tag_usage(&self, kind: TagType, id: i64) -> Result<TagUsage> {
        tags::usage(&self.store.conn(), kind, id)
    }

    /// What keeps a tag from being deleted, in words.
    pub fn tag_blockers(&self, kind: TagType, id: i64) -> Result<Vec<String>> {
        let usage = tags::usage(&self.store.conn(), kind, id)?;
        Ok(if usage.releases > 0 || usage.tracks > 0 {
            vec![format!("Set on {}", owners(usage))]
        } else {
            Vec::new()
        })
    }

    /// Fails while any release or track has a value. An unknown id deletes
    /// nothing.
    pub fn delete_tag(&self, kind: TagType, id: i64) -> Result<()> {
        let conn = self.store.conn();
        let usage = tags::usage(&conn, kind, id)?;
        if usage.releases > 0 || usage.tracks > 0 {
            return Err(Error::Invalid(format!(
                "The tag is set on {}",
                owners(usage)
            )));
        }
        tags::delete(&conn, kind, id)
    }

    /// A text tag's values, with how many releases and tracks have each.
    pub fn tag_values(&self, id: i64, args: &ListArgs) -> Result<Page<ValueCount>> {
        let (items, total) = tags::values(
            &self.store.conn(),
            id,
            paging_of(args)?,
            search_of(args.q.as_deref()).as_deref(),
        )?;
        Ok(Page { items, total })
    }

    /// Owners with several values, which keep a tag from being single-value.
    pub fn multi_value_owners(
        &self,
        owner: TagOwner,
        kind: TagType,
        id: i64,
        args: &ListArgs,
    ) -> Result<Page<i64>> {
        let (items, total) =
            tags::multi_value_owners(&self.store.conn(), owner, kind, id, paging_of(args)?)?;
        Ok(Page { items, total })
    }

    /// Whether the release or track exists.
    pub fn owner_exists(&self, owner: TagOwner, id: i64) -> Result<bool> {
        tags::owner_exists(&self.store.conn(), owner, id)
    }

    /// Releases' or tracks' titles, by id, to name them.
    pub fn owner_titles(&self, owner: TagOwner, ids: &[i64]) -> Result<HashMap<i64, String>> {
        tags::owner_titles(&self.store.conn(), owner, ids)
    }

    /// Each owner's tags, ordered by name, with their values in order. An
    /// owner with none is left out.
    pub fn tags_of(&self, owner: TagOwner, ids: &[i64]) -> Result<HashMap<i64, Vec<OwnerTag>>> {
        let by_owner = tags::values_of(&self.store.conn(), owner, ids)?;
        Ok(by_owner
            .into_iter()
            .map(|(id, rows)| {
                let mut owned: Vec<OwnerTag> = Vec::new();
                for row in rows {
                    match owned.last_mut() {
                        Some(last) if last.kind == row.kind && last.tag_id == row.tag_id => {
                            last.values.push(row.value)
                        }
                        _ => owned.push(OwnerTag {
                            kind: row.kind,
                            tag_id: row.tag_id,
                            values: vec![row.value],
                        }),
                    }
                }
                (id, owned)
            })
            .collect())
    }

    /// Replaces every tag value on a release or track. One that isn't there
    /// is `NotFound`: "There's no release 12".
    pub fn set_tags(&self, owner: TagOwner, id: i64, input: &[TagValuesInput]) -> Result<()> {
        self.store.transaction(|tx| {
            if !tags::owner_exists(tx, owner, id)? {
                return Err(Error::NotFound(format!(
                    "There's no {} {id}",
                    owner.as_str()
                )));
            }
            Tagging::set_tags_with(tx, owner, id, input)
        })
    }

    /// Replaces an owner's tag values, in a transaction that's already
    /// running. Blank values are left out, so a tag with none is removed.
    /// Values that repeat, ignoring case, are kept once.
    pub fn set_tags_with(
        tx: &Connection,
        owner: TagOwner,
        id: i64,
        input: &[TagValuesInput],
    ) -> Result<()> {
        let mut known: HashMap<(TagType, i64), Tag> = HashMap::new();
        for kind in TAG_TYPES {
            let ids: Vec<i64> = input
                .iter()
                .filter(|given| given.kind == kind)
                .map(|given| given.tag_id)
                .collect();
            for tag in tags::get_many(tx, kind, &ids)? {
                known.insert((kind, tag.id), tag);
            }
        }

        // Values by tag, in the order the tags first came, each kept once
        let mut by_tag: Vec<(&Tag, Vec<TagValue>, HashSet<String>)> = Vec::new();
        for given in input {
            let tag = known
                .get(&(given.kind, given.tag_id))
                .ok_or_else(|| not_found(given.kind, given.tag_id))?;
            let index = match by_tag
                .iter()
                .position(|(other, ..)| other.kind == tag.kind && other.id == tag.id)
            {
                Some(index) => index,
                None => {
                    by_tag.push((tag, Vec::new(), HashSet::new()));
                    by_tag.len() - 1
                }
            };
            let (_, values, keys) = &mut by_tag[index];
            for text in &given.values {
                if trim(text).is_empty() {
                    continue;
                }
                let value = parse_value(tag, text)?;
                if keys.insert(same_value_key(&value)) {
                    values.push(value);
                }
            }
        }

        let mut rows = Vec::new();
        for (tag, values, _) in by_tag {
            if !tag.multiple && values.len() > 1 {
                return Err(Error::Invalid(format!("{} takes one value", tag.name)));
            }
            rows.extend(values.into_iter().map(|value| TagValueRow {
                kind: tag.kind,
                tag_id: tag.id,
                value,
            }));
        }
        tags::set_values(tx, owner, id, &rows)
    }

    /// Sets a tag on many releases at once (a bulk edit): a single-value
    /// tag's value is replaced, and a multi-value tag gets the value added.
    /// Returns how many releases changed.
    pub fn set_tag_on_releases(
        &self,
        release_ids: &[i64],
        kind: TagType,
        tag_id: i64,
        value: &str,
    ) -> Result<i64> {
        self.store.transaction(|tx| {
            let (tag, value) = tag_and_value(tx, kind, tag_id, Some(value))?;
            let value = value.ok_or_else(|| Error::Invalid("Type a value to set".into()))?;
            tags::add_value(
                tx,
                TagOwner::Release,
                release_ids,
                &TagValueRow {
                    kind,
                    tag_id,
                    value,
                },
                !tag.multiple,
            )
        })
    }

    /// Removes a tag from many releases: one of its values, or with no value,
    /// all of them. Returns how many releases changed.
    pub fn remove_tag_from_releases(
        &self,
        release_ids: &[i64],
        kind: TagType,
        tag_id: i64,
        value: Option<&str>,
    ) -> Result<i64> {
        self.store.transaction(|tx| {
            let (_, value) = tag_and_value(tx, kind, tag_id, value)?;
            tags::remove_values(
                tx,
                TagOwner::Release,
                release_ids,
                kind,
                tag_id,
                value.as_ref(),
            )
        })
    }
}

/// List filters, checked and turned into ranges.
pub fn tag_filters(inputs: &[TagFilterInput]) -> Result<Vec<TagFilter>> {
    inputs.iter().map(filter_of).collect()
}

/// Each type's names are unique in the database. Across types, only the app
/// can check it (slunk's design §4.3 Tags).
fn check_name(tx: &Connection, kind: TagType, name: &str) -> Result<()> {
    match tags::find_by_name(tx, name)?
        .into_iter()
        .find(|other| other.kind != kind)
    {
        Some(other) => Err(Error::Invalid(format!(
            "There's already a {} tag named {}",
            other.kind.as_str(),
            other.name
        ))),
        None => Ok(()),
    }
}

/// A tag and one value for it, parsed, for a bulk edit.
fn tag_and_value(
    tx: &Connection,
    kind: TagType,
    tag_id: i64,
    text: Option<&str>,
) -> Result<(Tag, Option<TagValue>)> {
    let tag = tags::get(tx, kind, tag_id)?.ok_or_else(|| not_found(kind, tag_id))?;
    let value = match text {
        Some(text) if !trim(text).is_empty() => Some(parse_value(&tag, text)?),
        _ => None,
    };
    Ok((tag, value))
}

fn filter_of(input: &TagFilterInput) -> Result<TagFilter> {
    let given = |text: &Option<String>| {
        text.as_deref()
            .filter(|text| !trim(text).is_empty())
            .map(clean_text)
    };
    let (equals, contains) = (given(&input.equals), given(&input.contains));
    let (min, max) = (given(&input.min), given(&input.max));
    let filter = |min: Option<TagValue>, max: Option<TagValue>| TagFilter {
        kind: input.kind,
        tag_id: input.tag_id,
        equals: None,
        contains: None,
        min,
        max,
    };

    if input.kind == TagType::Text {
        if min.is_some() || max.is_some() || equals.is_some() == contains.is_some() {
            return Err(Error::Invalid(
                "A text tag filter takes a value, or part of one".into(),
            ));
        }
        return Ok(TagFilter {
            equals,
            contains,
            ..filter(None, None)
        });
    }
    if contains.is_some() || equals.is_some() == (min.is_some() || max.is_some()) {
        return Err(Error::Invalid(format!(
            "A {} tag filter takes a value, or a range",
            input.kind.as_str()
        )));
    }
    if input.kind == TagType::Number {
        let number = |text: Option<String>| {
            text.map(|text| parse_number(&text).map(TagValue::Number))
                .transpose()
        };
        if let Some(equals) = equals {
            let number = parse_number(&equals)?;
            return Ok(filter(
                Some(TagValue::Number(number)),
                Some(TagValue::Number(number)),
            ));
        }
        return Ok(filter(number(min)?, number(max)?));
    }
    // A date covers its whole year, month or day
    if let Some(equals) = equals {
        let date = parse_date(&equals)?;
        return Ok(filter(
            Some(TagValue::Date(date.epoch)),
            Some(TagValue::Date(last_day(&date))),
        ));
    }
    Ok(filter(
        min.map(|text| parse_date(&text).map(|date| TagValue::Date(date.epoch)))
            .transpose()?,
        max.map(|text| parse_date(&text).map(|date| TagValue::Date(last_day(&date))))
            .transpose()?,
    ))
}

/// "2 releases and 1 track", leaving out a count of 0.
fn owners(usage: TagUsage) -> String {
    [
        (usage.releases > 0).then(|| count(usage.releases, "release")),
        (usage.tracks > 0).then(|| count(usage.tracks, "track")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" and ")
}

fn not_found(kind: TagType, id: i64) -> Error {
    Error::NotFound(format!("There's no {} tag {id}", kind.as_str()))
}

fn count(n: i64, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}
