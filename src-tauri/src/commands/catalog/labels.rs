//! Labels, with their alternate names and logos, and the label's page.

use serde::{Deserialize, Serialize};
use slunk::catalog as domain;
use specta::Type;
use tauri::AppHandle;

use super::releases::{self, ReleaseFiltersArgs, ReleasePage, ReleasesArgs};
use super::{IdArgs, OptionsArgs, PageArgs, SetPictureArgs, files_by_id};
use crate::commands::media::{File, FileRef};
use crate::commands::on_library;
use crate::error::CommandError;
use crate::library::Library;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Label {
    pub id: i64,
    /// The main name
    pub name: String,
    /// The main name's language: a BCP 47 tag. Null when it isn't known.
    pub name_language: Option<String>,
    /// The logo
    pub picture: Option<FileRef>,
    /// Epoch seconds
    pub inserted_at: i64,
    /// Epoch seconds
    pub updated_at: i64,
}

impl From<domain::Label> for Label {
    fn from(label: domain::Label) -> Self {
        Label {
            id: label.id,
            name: label.name,
            name_language: label.name_language,
            picture: label.picture.map(|id| FileRef { id }),
            inserted_at: label.inserted_at,
            updated_at: label.updated_at,
        }
    }
}

/// A name a label goes by, besides its main one. It has no id: nothing
/// refers to it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelName {
    pub name: String,
    pub language: Option<String>,
    pub note: Option<String>,
}

impl From<domain::AlternateName> for LabelName {
    fn from(name: domain::AlternateName) -> Self {
        LabelName {
            name: name.name,
            language: name.language,
            note: name.note,
        }
    }
}

/// A label in a list or a picker, with its alternate names, in order.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelListItem {
    #[serde(flatten)]
    pub label: Label,
    pub alternate_names: Vec<LabelName>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelPage {
    pub items: Vec<LabelListItem>,
    pub total: i64,
}

/// A label, as its form edits it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelDetails {
    pub id: i64,
    pub name: String,
    pub name_language: Option<String>,
    pub alternate_names: Vec<LabelName>,
    /// The logo
    pub picture: Option<File>,
}

/// A label to save.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelInput {
    pub name: String,
    #[serde(default)]
    #[specta(optional)]
    pub name_language: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub picture_id: Option<i64>,
    /// Replaces the alternate names, in order. Left out, they stay as they
    /// are.
    #[serde(default)]
    #[specta(optional)]
    pub alternate_names: Option<Vec<LabelNameInput>>,
}

impl From<LabelInput> for domain::LabelInput {
    fn from(input: LabelInput) -> Self {
        domain::LabelInput {
            name: input.name,
            name_language: input.name_language,
            picture_id: input.picture_id,
            alternate_names: input.alternate_names.map(|names| {
                names
                    .into_iter()
                    .map(|name| domain::AlternateNameInput {
                        id: None,
                        name: name.name,
                        language: name.language,
                        real_name: false,
                        note: name.note,
                    })
                    .collect()
            }),
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelNameInput {
    pub name: String,
    #[serde(default)]
    #[specta(optional)]
    pub language: Option<String>,
    #[serde(default)]
    #[specta(optional)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateLabelArgs {
    pub id: i64,
    #[serde(flatten)]
    pub input: LabelInput,
}

/// Labels, with their alternate names. `q` matches part of the main name or
/// of an alternate name.
#[tauri::command]
#[specta::specta]
pub async fn labels(app: AppHandle, args: PageArgs) -> Result<LabelPage, CommandError> {
    on_library(app, move |library| list(library, args)).await
}

pub fn list(library: &Library, args: PageArgs) -> Result<LabelPage, CommandError> {
    let page = library.catalog.list_labels(&args.into())?;
    Ok(LabelPage {
        items: with_names(library, page.items)?,
        total: page.total,
    })
}

/// A label, as its form edits it, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn label(app: AppHandle, args: IdArgs) -> Result<Option<LabelDetails>, CommandError> {
    on_library(app, move |library| details(library, args)).await
}

pub fn details(library: &Library, args: IdArgs) -> Result<Option<LabelDetails>, CommandError> {
    let catalog = &library.catalog;
    let Some(label) = catalog.get_label(args.id)? else {
        return Ok(None);
    };
    let names = catalog.label_names_of(&[label.id])?.remove(&label.id);
    let pictures: Vec<i64> = label.picture.into_iter().collect();
    let mut files = files_by_id(library, &pictures)?;
    Ok(Some(LabelDetails {
        id: label.id,
        name: label.name,
        name_language: label.name_language,
        alternate_names: names
            .unwrap_or_default()
            .into_iter()
            .map(LabelName::from)
            .collect(),
        picture: label.picture.and_then(|id| files.remove(&id)),
    }))
}

/// The first labels with `q` in a name, by name, for a picker.
#[tauri::command]
#[specta::specta]
pub async fn label_options(
    app: AppHandle,
    args: OptionsArgs,
) -> Result<Vec<LabelListItem>, CommandError> {
    on_library(app, move |library| options(library, args)).await
}

pub fn options(library: &Library, args: OptionsArgs) -> Result<Vec<LabelListItem>, CommandError> {
    let page = library.catalog.list_labels(&args.list())?;
    with_names(library, page.items)
}

#[tauri::command]
#[specta::specta]
pub async fn create_label(app: AppHandle, args: LabelInput) -> Result<Label, CommandError> {
    on_library(app, move |library| create(library, args)).await
}

pub fn create(library: &Library, args: LabelInput) -> Result<Label, CommandError> {
    Ok(library.catalog.create_label(&args.into())?.into())
}

#[tauri::command]
#[specta::specta]
pub async fn update_label(app: AppHandle, args: UpdateLabelArgs) -> Result<Label, CommandError> {
    on_library(app, move |library| update(library, args)).await
}

pub fn update(library: &Library, args: UpdateLabelArgs) -> Result<Label, CommandError> {
    Ok(library
        .catalog
        .update_label(args.id, &args.input.into())?
        .into())
}

/// Sets only the logo. A file given by path is registered first, in the same
/// transaction.
#[tauri::command]
#[specta::specta]
pub async fn set_label_picture(
    app: AppHandle,
    args: SetPictureArgs,
) -> Result<Label, CommandError> {
    on_library(app, move |library| set_picture(library, args)).await
}

pub fn set_picture(library: &Library, args: SetPictureArgs) -> Result<Label, CommandError> {
    Ok(library
        .catalog
        .set_label_picture(args.id, &args.picture.into())?
        .into())
}

/// Deletes a label, with its alternate names, and returns its id. It fails
/// while the label is on a release.
#[tauri::command]
#[specta::specta]
pub async fn delete_label(app: AppHandle, args: IdArgs) -> Result<i64, CommandError> {
    on_library(app, move |library| delete(library, args)).await
}

pub fn delete(library: &Library, args: IdArgs) -> Result<i64, CommandError> {
    library.catalog.delete_label(args.id)?;
    Ok(args.id)
}

/// Labels, with their alternate names, loaded in one query.
fn with_names(
    library: &Library,
    labels: Vec<domain::Label>,
) -> Result<Vec<LabelListItem>, CommandError> {
    let ids: Vec<i64> = labels.iter().map(|label| label.id).collect();
    let mut names = library.catalog.label_names_of(&ids)?;
    Ok(labels
        .into_iter()
        .map(|label| LabelListItem {
            alternate_names: names
                .remove(&label.id)
                .unwrap_or_default()
                .into_iter()
                .map(LabelName::from)
                .collect(),
            label: label.into(),
        })
        .collect())
}

/// The label's page: its logo and names, and its releases.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelView {
    pub id: i64,
    pub name: String,
    pub name_language: Option<String>,
    pub alternate_names: Vec<LabelName>,
    /// The logo
    pub picture: Option<FileRef>,
    /// What keeps it from being deleted, in words
    pub blockers: Vec<String>,
    pub releases: ReleasePage,
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LabelPageArgs {
    pub id: i64,
    #[serde(default)]
    #[specta(optional)]
    pub page: Option<i64>,
    #[serde(default)]
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// `title`, `released` or `id`, with `-` in front for descending
    #[serde(default)]
    #[specta(optional)]
    pub sort: Option<String>,
}

/// The label's page, or null for an unknown id.
#[tauri::command]
#[specta::specta]
pub async fn label_page(
    app: AppHandle,
    args: LabelPageArgs,
) -> Result<Option<LabelView>, CommandError> {
    on_library(app, move |library| page(library, args)).await
}

pub fn page(library: &Library, args: LabelPageArgs) -> Result<Option<LabelView>, CommandError> {
    let catalog = &library.catalog;
    let Some(label) = catalog.get_label(args.id)? else {
        return Ok(None);
    };
    let releases = releases::list(
        library,
        ReleasesArgs {
            page: args.page,
            per_page: args.per_page,
            sort: args.sort,
            filters: ReleaseFiltersArgs {
                label: Some(label.id),
                ..ReleaseFiltersArgs::default()
            },
        },
    )?;
    Ok(Some(LabelView {
        alternate_names: catalog
            .label_names_of(&[label.id])?
            .remove(&label.id)
            .unwrap_or_default()
            .into_iter()
            .map(LabelName::from)
            .collect(),
        blockers: catalog.label_blockers(label.id)?,
        picture: label.picture.map(|id| FileRef { id }),
        id: label.id,
        name: label.name,
        name_language: label.name_language,
        releases,
    }))
}
