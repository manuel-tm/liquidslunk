//! The tagging context's commands: tag definitions, their values, and a
//! release's tags. Each tag type has its own ids, so a tag is named by its
//! type and its id.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use slunk::paging::ListArgs;
use slunk::tagging::{self as domain, OwnerTag, TAG_TYPES, TagOwner, format_value};
use specta::Type;
use tauri::AppHandle;

use super::catalog::{IdArgs, OPTIONS, OptionsArgs, PageArgs};
use super::on_library;
use crate::error::CommandError;
use crate::library::Library;

/// Owners with several values, as the tag's form lists them.
const BLOCKING: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum TagType {
    Text,
    /// A real number
    Number,
    /// A day, without a time
    Date,
}

impl From<TagType> for domain::TagType {
    fn from(kind: TagType) -> Self {
        match kind {
            TagType::Text => domain::TagType::Text,
            TagType::Number => domain::TagType::Number,
            TagType::Date => domain::TagType::Date,
        }
    }
}

impl From<domain::TagType> for TagType {
    fn from(kind: domain::TagType) -> Self {
        match kind {
            domain::TagType::Text => TagType::Text,
            domain::TagType::Number => TagType::Number,
            domain::TagType::Date => TagType::Date,
        }
    }
}

/// A tag definition, like Genre or Score.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Tag {
    pub id: i64,
    #[serde(rename = "type")]
    pub kind: TagType,
    pub name: String,
    /// Several values per release or track, or at most one
    pub multiple: bool,
}

impl From<domain::Tag> for Tag {
    fn from(tag: domain::Tag) -> Self {
        Tag {
            id: tag.id,
            kind: tag.kind.into(),
            name: tag.name,
            multiple: tag.multiple,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagPage {
    pub items: Vec<Tag>,
    pub total: i64,
}

/// A release or track, by its title.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Titled {
    pub id: i64,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TitledPage {
    pub items: Vec<Titled>,
    pub total: i64,
}

/// A tag, how many releases and tracks have it, and what keeps it from being
/// deleted or made single-value.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagDetails {
    #[serde(flatten)]
    pub tag: Tag,
    /// How many releases have a value
    pub release_count: i64,
    /// How many tracks have a value
    pub track_count: i64,
    pub blockers: Vec<String>,
    /// The first 20 releases with several values, which keep the tag from
    /// being single-value
    pub multi_value_releases: TitledPage,
    /// The first 20 tracks with several values
    pub multi_value_tracks: TitledPage,
}

/// A text tag's value, and how many releases and tracks have it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagValueCount {
    pub value: String,
    pub releases: i64,
    pub tracks: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagValuePage {
    pub items: Vec<TagValueCount>,
    pub total: i64,
}

/// A tag set on a release or track, with its values: one for a single-value
/// tag.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagValues {
    pub tag: Tag,
    /// As text: `Rock`, `8.5`, or a date as `2006-01-10`. In the order they
    /// were set.
    pub values: Vec<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagsArgs {
    /// Every type's tags when it's left out
    #[serde(default, rename = "type")]
    #[specta(optional)]
    pub kind: Option<TagType>,
    #[serde(flatten)]
    pub page: PageArgs,
}

#[derive(Debug, Clone, Copy, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagArgs {
    #[serde(rename = "type")]
    pub kind: TagType,
    pub id: i64,
}

/// A page of a text tag's values.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagValueListArgs {
    /// A text tag
    pub id: i64,
    #[serde(default)]
    #[specta(optional)]
    pub page: Option<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// Part of the value, ignoring case
    #[serde(default)]
    #[specta(optional)]
    pub q: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagValueOptionsArgs {
    /// A text tag
    pub id: i64,
    #[serde(default)]
    #[specta(optional)]
    pub q: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagsByRefArgs {
    pub refs: Vec<TagArgs>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagInput {
    /// Unique across types, ignoring case
    pub name: String,
    #[serde(default)]
    #[specta(optional)]
    pub multiple: bool,
}

impl From<TagInput> for domain::TagInput {
    fn from(input: TagInput) -> Self {
        domain::TagInput {
            name: input.name,
            multiple: input.multiple,
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateTagArgs {
    #[serde(rename = "type")]
    pub kind: TagType,
    #[serde(flatten)]
    pub input: TagInput,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTagArgs {
    /// The type can't change
    #[serde(rename = "type")]
    pub kind: TagType,
    pub id: i64,
    #[serde(flatten)]
    pub input: TagInput,
}

/// A tag's values on a release or track, as typed: a number like `8.5`, a
/// date as `2006`, `2006-01` or `2006-01-10`. Blank values are left out.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagValuesInput {
    #[serde(rename = "type")]
    pub kind: TagType,
    pub tag_id: i64,
    pub values: Vec<String>,
}

impl From<TagValuesInput> for domain::TagValuesInput {
    fn from(input: TagValuesInput) -> Self {
        domain::TagValuesInput {
            kind: input.kind.into(),
            tag_id: input.tag_id,
            values: input.values,
        }
    }
}

/// A release or track list filter on one tag. Text takes `equals` or
/// `contains`; numbers and dates take `equals`, or `min` and `max`.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagFilterInput {
    #[serde(rename = "type")]
    pub kind: TagType,
    pub tag_id: i64,
    /// The value, ignoring case for text. A date covers its whole year,
    /// month or day (`2006`, `2006-01`, `2006-01-10`).
    #[serde(default)]
    #[specta(optional)]
    pub equals: Option<String>,
    /// Text only: part of the value, ignoring case
    #[serde(default)]
    #[specta(optional)]
    pub contains: Option<String>,
    /// Number or date, included. A date starts on its first day.
    #[serde(default)]
    #[specta(optional)]
    pub min: Option<String>,
    /// Number or date, included. A date runs to its last day: `2006`
    /// includes Dec 31, 2006.
    #[serde(default)]
    #[specta(optional)]
    pub max: Option<String>,
}

impl From<TagFilterInput> for domain::TagFilterInput {
    fn from(input: TagFilterInput) -> Self {
        domain::TagFilterInput {
            kind: input.kind.into(),
            tag_id: input.tag_id,
            equals: input.equals,
            contains: input.contains,
            min: input.min,
            max: input.max,
        }
    }
}

/// A tag and a value, to set on many releases.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetTagOnReleasesArgs {
    /// Releases that aren't there are skipped.
    pub ids: Vec<i64>,
    #[serde(rename = "type")]
    pub kind: TagType,
    pub tag_id: i64,
    pub value: String,
}

/// A tag, and a value to remove from many releases, or none to remove them
/// all.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoveTagFromReleasesArgs {
    pub ids: Vec<i64>,
    #[serde(rename = "type")]
    pub kind: TagType,
    pub tag_id: i64,
    #[serde(default)]
    #[specta(optional)]
    pub value: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetReleaseTagsArgs {
    pub id: i64,
    /// Every tag the release has. A tag left out, or with only blank values,
    /// is removed.
    pub tags: Vec<TagValuesInput>,
}

/// Tag definitions of one type, or of every type when `type` is left out.
#[tauri::command]
#[specta::specta]
pub async fn tags(app: AppHandle, args: TagsArgs) -> Result<TagPage, CommandError> {
    on_library(app, move |library| list(library, args)).await
}

pub fn list(library: &Library, args: TagsArgs) -> Result<TagPage, CommandError> {
    let page = library
        .tagging
        .list_tags(&args.page.into(), args.kind.map(Into::into))?;
    Ok(TagPage {
        items: page.items.into_iter().map(Tag::from).collect(),
        total: page.total,
    })
}

/// A tag, with what its page and its form show, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn tag(app: AppHandle, args: TagArgs) -> Result<Option<TagDetails>, CommandError> {
    on_library(app, move |library| details(library, args)).await
}

pub fn details(library: &Library, args: TagArgs) -> Result<Option<TagDetails>, CommandError> {
    let tagging = &library.tagging;
    let kind = args.kind.into();
    let Some(tag) = tagging.get_tag(kind, args.id)? else {
        return Ok(None);
    };
    let usage = tagging.tag_usage(kind, tag.id)?;
    let blocking = |owner: TagOwner| -> Result<TitledPage, CommandError> {
        let first = ListArgs {
            per_page: Some(BLOCKING),
            ..ListArgs::default()
        };
        let page = tagging.multi_value_owners(owner, kind, tag.id, &first)?;
        let mut titles = tagging.owner_titles(owner, &page.items)?;
        Ok(TitledPage {
            items: page
                .items
                .into_iter()
                .filter_map(|id| titles.remove(&id).map(|title| Titled { id, title }))
                .collect(),
            total: page.total,
        })
    };
    Ok(Some(TagDetails {
        release_count: usage.releases,
        track_count: usage.tracks,
        blockers: tagging.tag_blockers(kind, tag.id)?,
        multi_value_releases: blocking(TagOwner::Release)?,
        multi_value_tracks: blocking(TagOwner::Track)?,
        tag: tag.into(),
    }))
}

/// The first tags of any type with `q` in their name, by name, for a picker.
#[tauri::command]
#[specta::specta]
pub async fn tag_options(app: AppHandle, args: OptionsArgs) -> Result<Vec<Tag>, CommandError> {
    on_library(app, move |library| options(library, args)).await
}

pub fn options(library: &Library, args: OptionsArgs) -> Result<Vec<Tag>, CommandError> {
    let page = library.tagging.list_tags(&args.list(), None)?;
    Ok(page.items.into_iter().map(Tag::from).collect())
}

/// These tags, in the same order, leaving out any that don't exist. A list
/// names its tag filters with it.
#[tauri::command]
#[specta::specta]
pub async fn tags_by_ref(app: AppHandle, args: TagsByRefArgs) -> Result<Vec<Tag>, CommandError> {
    on_library(app, move |library| by_ref(library, args)).await
}

pub fn by_ref(library: &Library, args: TagsByRefArgs) -> Result<Vec<Tag>, CommandError> {
    let refs: Vec<(domain::TagType, i64)> = args
        .refs
        .iter()
        .map(|tag| (tag.kind.into(), tag.id))
        .collect();
    let tags = tags_by_id(library, &refs)?;
    Ok(refs
        .iter()
        .filter_map(|tag| tags.get(tag).cloned())
        .collect())
}

/// A text tag's values, with how many releases and tracks have each, the
/// most used first. Values that differ only in case count as one.
#[tauri::command]
#[specta::specta]
pub async fn tag_values(
    app: AppHandle,
    args: TagValueListArgs,
) -> Result<TagValuePage, CommandError> {
    on_library(app, move |library| values(library, args)).await
}

pub fn values(library: &Library, args: TagValueListArgs) -> Result<TagValuePage, CommandError> {
    let page = library.tagging.tag_values(
        args.id,
        &ListArgs {
            page: args.page,
            per_page: args.per_page,
            q: args.q,
            ..ListArgs::default()
        },
    )?;
    Ok(TagValuePage {
        items: page
            .items
            .into_iter()
            .map(|count| TagValueCount {
                value: count.value,
                releases: count.releases,
                tracks: count.tracks,
            })
            .collect(),
        total: page.total,
    })
}

/// A text tag's first values with `q` in them, the most used first, for a
/// picker.
#[tauri::command]
#[specta::specta]
pub async fn tag_value_options(
    app: AppHandle,
    args: TagValueOptionsArgs,
) -> Result<Vec<String>, CommandError> {
    on_library(app, move |library| value_options(library, args)).await
}

pub fn value_options(
    library: &Library,
    args: TagValueOptionsArgs,
) -> Result<Vec<String>, CommandError> {
    let page = library.tagging.tag_values(
        args.id,
        &ListArgs {
            per_page: Some(OPTIONS),
            q: args.q,
            ..ListArgs::default()
        },
    )?;
    Ok(page.items.into_iter().map(|count| count.value).collect())
}

/// Tag names are unique across types, ignoring case.
#[tauri::command]
#[specta::specta]
pub async fn create_tag(app: AppHandle, args: CreateTagArgs) -> Result<Tag, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(library: &Library, args: CreateTagArgs) -> Result<Tag, CommandError> {
    Ok(library
        .tagging
        .create_tag(args.kind.into(), &args.input.into())?
        .into())
}

/// Switching to single-value fails while a release or track has several
/// values.
#[tauri::command]
#[specta::specta]
pub async fn update_tag(app: AppHandle, args: UpdateTagArgs) -> Result<Tag, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(library: &Library, args: UpdateTagArgs) -> Result<Tag, CommandError> {
    Ok(library
        .tagging
        .update_tag(args.kind.into(), args.id, &args.input.into())?
        .into())
}

/// Deletes a tag, and returns its id. It fails while any release or track
/// has a value.
#[tauri::command]
#[specta::specta]
pub async fn delete_tag(app: AppHandle, args: TagArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: TagArgs) -> Result<i64, CommandError> {
    library.tagging.delete_tag(args.kind.into(), args.id)?;
    Ok(args.id)
}

/// A release's tags, by tag name, or null for an unknown release.
#[tauri::command]
#[specta::specta]
pub async fn release_tags(
    app: AppHandle,
    args: IdArgs,
) -> Result<Option<Vec<TagValues>>, CommandError> {
    on_library(app, move |library| of_release(library, args)).await
}

pub fn of_release(library: &Library, args: IdArgs) -> Result<Option<Vec<TagValues>>, CommandError> {
    if !library.tagging.owner_exists(TagOwner::Release, args.id)? {
        return Ok(None);
    }
    owned(library, TagOwner::Release, args.id).map(Some)
}

/// Replaces every tag value on the release, and returns its tags.
#[tauri::command]
#[specta::specta]
pub async fn set_release_tags(
    app: AppHandle,
    args: SetReleaseTagsArgs,
) -> Result<Vec<TagValues>, CommandError> {
    on_library(app, move |library| set_on_release(library, args)).await
}

pub fn set_on_release(
    library: &Library,
    args: SetReleaseTagsArgs,
) -> Result<Vec<TagValues>, CommandError> {
    let input: Vec<domain::TagValuesInput> = args.tags.into_iter().map(Into::into).collect();
    library
        .tagging
        .set_tags(TagOwner::Release, args.id, &input)?;
    owned(library, TagOwner::Release, args.id)
}

/// Sets a tag on many releases at once: a single-value tag's value is
/// replaced, and a multi-value tag gets the value added. Returns how many
/// releases changed.
#[tauri::command]
#[specta::specta]
pub async fn set_tag_on_releases(
    app: AppHandle,
    args: SetTagOnReleasesArgs,
) -> Result<i64, CommandError> {
    on_library(app, move |library| set_on_releases(library, args)).await
}

pub fn set_on_releases(library: &Library, args: SetTagOnReleasesArgs) -> Result<i64, CommandError> {
    Ok(library.tagging.set_tag_on_releases(
        &args.ids,
        args.kind.into(),
        args.tag_id,
        &args.value,
    )?)
}

/// Removes a tag from many releases at once: one of its values, or all of
/// them when `value` is left out. Returns how many releases changed.
#[tauri::command]
#[specta::specta]
pub async fn remove_tag_from_releases(
    app: AppHandle,
    args: RemoveTagFromReleasesArgs,
) -> Result<i64, CommandError> {
    on_library(app, move |library| remove_from_releases(library, args)).await
}

pub fn remove_from_releases(
    library: &Library,
    args: RemoveTagFromReleasesArgs,
) -> Result<i64, CommandError> {
    Ok(library.tagging.remove_tag_from_releases(
        &args.ids,
        args.kind.into(),
        args.tag_id,
        args.value.as_deref(),
    )?)
}

/// An owner's tags, with their definitions, and their values as text.
pub(crate) fn owned(
    library: &Library,
    owner: TagOwner,
    id: i64,
) -> Result<Vec<TagValues>, CommandError> {
    let owned: Vec<OwnerTag> = library
        .tagging
        .tags_of(owner, &[id])?
        .remove(&id)
        .unwrap_or_default();
    let refs: Vec<(domain::TagType, i64)> =
        owned.iter().map(|tag| (tag.kind, tag.tag_id)).collect();
    let mut tags = tags_by_id(library, &refs)?;
    Ok(owned
        .into_iter()
        .filter_map(|tag| {
            Some(TagValues {
                tag: tags.remove(&(tag.kind, tag.tag_id))?,
                values: tag.values.iter().map(format_value).collect(),
            })
        })
        .collect())
}

/// Tags by type and id, loaded with one query per type.
pub(crate) fn tags_by_id(
    library: &Library,
    refs: &[(domain::TagType, i64)],
) -> Result<HashMap<(domain::TagType, i64), Tag>, CommandError> {
    let mut tags = HashMap::new();
    for kind in TAG_TYPES {
        let ids: Vec<i64> = refs
            .iter()
            .filter(|(other, _)| *other == kind)
            .map(|&(_, id)| id)
            .collect();
        if ids.is_empty() {
            continue;
        }
        for tag in library.tagging.tags_by_ids(kind, &ids)? {
            tags.insert((kind, tag.id), Tag::from(tag));
        }
    }
    Ok(tags)
}
