//! Release formats and release media, which share their commands, with a
//! `kind`.

use serde::{Deserialize, Serialize};
use slunk::catalog as domain;
use specta::Type;
use tauri::AppHandle;

use super::{OptionsArgs, PageArgs};
use crate::commands::on_library;
use crate::error::CommandError;
use crate::library::Library;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ListKind {
    ReleaseFormats,
    ReleaseMedia,
}

impl From<ListKind> for domain::ListKind {
    fn from(kind: ListKind) -> Self {
        match kind {
            ListKind::ReleaseFormats => domain::ListKind::ReleaseFormats,
            ListKind::ReleaseMedia => domain::ListKind::ReleaseMedia,
        }
    }
}

impl From<domain::ListKind> for ListKind {
    fn from(kind: domain::ListKind) -> Self {
        match kind {
            domain::ListKind::ReleaseFormats => ListKind::ReleaseFormats,
            domain::ListKind::ReleaseMedia => ListKind::ReleaseMedia,
        }
    }
}

/// A row of a list the user manages: a release format or release medium.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListItem {
    pub id: i64,
    pub kind: ListKind,
    pub name: String,
}

impl From<domain::ListItem> for ListItem {
    fn from(item: domain::ListItem) -> Self {
        ListItem {
            id: item.id,
            kind: item.kind.into(),
            name: item.name,
        }
    }
}

/// A row in the list, with how many releases have it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UsedListItem {
    #[serde(flatten)]
    pub item: ListItem,
    pub uses: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListItemPage {
    pub items: Vec<UsedListItem>,
    pub total: i64,
}

/// A row, with how many releases have it, and what keeps it from being
/// deleted, in words.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListItemDetails {
    #[serde(flatten)]
    pub item: ListItem,
    pub uses: i64,
    pub blockers: Vec<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListItemsArgs {
    pub kind: ListKind,
    #[serde(flatten)]
    pub page: PageArgs,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListItemArgs {
    pub kind: ListKind,
    pub id: i64,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListItemOptionsArgs {
    pub kind: ListKind,
    #[serde(flatten)]
    pub options: OptionsArgs,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateListItemArgs {
    pub kind: ListKind,
    /// Unique in its list, ignoring case
    pub name: String,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RenameListItemArgs {
    pub kind: ListKind,
    pub id: i64,
    pub name: String,
}

/// Release formats or release media.
#[tauri::command]
#[specta::specta]
pub async fn list_items(app: AppHandle, args: ListItemsArgs) -> Result<ListItemPage, CommandError> {
    on_library(app, move |library| list(library, args)).await
}

pub fn list(library: &Library, args: ListItemsArgs) -> Result<ListItemPage, CommandError> {
    let kind = args.kind.into();
    let page = library.catalog.list_items(kind, &args.page.into())?;
    let ids: Vec<i64> = page.items.iter().map(|item| item.id).collect();
    let uses = library.catalog.list_item_uses(kind, &ids)?;
    Ok(ListItemPage {
        items: page
            .items
            .into_iter()
            .map(|item| UsedListItem {
                uses: uses.get(&item.id).copied().unwrap_or(0),
                item: item.into(),
            })
            .collect(),
        total: page.total,
    })
}

/// A format or medium, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn list_item(
    app: AppHandle,
    args: ListItemArgs,
) -> Result<Option<ListItemDetails>, CommandError> {
    on_library(app, move |library| details(library, args)).await
}

pub fn details(
    library: &Library,
    args: ListItemArgs,
) -> Result<Option<ListItemDetails>, CommandError> {
    let kind = args.kind.into();
    let Some(item) = library.catalog.get_list_item(kind, args.id)? else {
        return Ok(None);
    };
    let uses = library.catalog.list_item_uses(kind, &[item.id])?;
    Ok(Some(ListItemDetails {
        uses: uses.get(&item.id).copied().unwrap_or(0),
        blockers: library.catalog.list_item_blockers(kind, item.id)?,
        item: item.into(),
    }))
}

/// The first formats or media whose name has `q` in it, by name, for a
/// picker.
#[tauri::command]
#[specta::specta]
pub async fn list_item_options(
    app: AppHandle,
    args: ListItemOptionsArgs,
) -> Result<Vec<ListItem>, CommandError> {
    on_library(app, move |library| options(library, args)).await
}

pub fn options(
    library: &Library,
    args: ListItemOptionsArgs,
) -> Result<Vec<ListItem>, CommandError> {
    let page = library
        .catalog
        .list_items(args.kind.into(), &args.options.list())?;
    Ok(page.items.into_iter().map(ListItem::from).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn create_list_item(
    app: AppHandle,
    args: CreateListItemArgs,
) -> Result<ListItem, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(library: &Library, args: CreateListItemArgs) -> Result<ListItem, CommandError> {
    Ok(library
        .catalog
        .create_list_item(args.kind.into(), &args.name)?
        .into())
}

#[tauri::command]
#[specta::specta]
pub async fn rename_list_item(
    app: AppHandle,
    args: RenameListItemArgs,
) -> Result<ListItem, CommandError> {
    on_library(app, move |library| rename(library, args)).await
}

pub fn rename(library: &Library, args: RenameListItemArgs) -> Result<ListItem, CommandError> {
    Ok(library
        .catalog
        .rename_list_item(args.kind.into(), args.id, &args.name)?
        .into())
}

/// Deletes a format or medium, and returns its id. It fails while anything
/// uses it.
#[tauri::command]
#[specta::specta]
pub async fn delete_list_item(app: AppHandle, args: ListItemArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: ListItemArgs) -> Result<i64, CommandError> {
    library
        .catalog
        .delete_list_item(args.kind.into(), args.id)?;
    Ok(args.id)
}
