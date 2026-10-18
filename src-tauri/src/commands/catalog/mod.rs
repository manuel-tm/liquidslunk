//! The catalog context's commands (slunk's `catalog.graphql`). Ported so far
//! (phase 2): artists, labels, the lists the user manages, and the languages
//! in use.
//!
//! - `artists`, `labels` and `lists` each hold their commands and DTOs, and
//!   the plain functions behind them, which the write parity script calls
//!   too, like `artists::create(library, args)`.
//! - This module holds what they share.

use serde::{Deserialize, Serialize};
use slunk::catalog as domain;
use slunk::media::FileRecord;
use slunk::paging::ListArgs;
use specta::Type;
use tauri::AppHandle;

use super::media::File;
use super::on_library;
use crate::error::CommandError;
use crate::library::Library;

pub mod artists;
pub mod labels;
pub mod lists;

/// Pickers offer the first 20 matches (slunk's design §7.3).
pub(crate) const OPTIONS: i64 = 20;

/// A page of a list: artists, labels, formats or media.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct PageArgs {
    /// From 1. Default 1.
    #[specta(optional)]
    pub page: Option<i64>,
    /// Default 50, at most 200
    #[specta(optional)]
    pub per_page: Option<i64>,
    /// `name` or `id`, with `-` in front for descending. Default `-id`,
    /// newest first.
    #[specta(optional)]
    pub sort: Option<String>,
    /// Part of the name, ignoring case. For artists and labels, part of an
    /// alternate name finds them too.
    #[specta(optional)]
    pub q: Option<String>,
}

impl From<PageArgs> for ListArgs {
    fn from(args: PageArgs) -> Self {
        ListArgs {
            page: args.page,
            per_page: args.per_page,
            sort: args.sort,
            q: args.q,
        }
    }
}

/// What a picker searches: the first matches by name.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct OptionsArgs {
    #[specta(optional)]
    pub q: Option<String>,
}

impl OptionsArgs {
    pub(crate) fn list(self) -> ListArgs {
        ListArgs {
            per_page: Some(OPTIONS),
            sort: Some("name".into()),
            q: self.q,
            ..ListArgs::default()
        }
    }
}

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IdArgs {
    pub id: i64,
}

/// A file by id, or a path to register first.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FileRefInput {
    #[specta(optional)]
    pub file_id: Option<i64>,
    /// Relative to the database's folder, or absolute
    #[specta(optional)]
    pub path: Option<String>,
}

impl From<FileRefInput> for domain::FileRef {
    fn from(file: FileRefInput) -> Self {
        domain::FileRef {
            file_id: file.file_id,
            path: file.path,
        }
    }
}

/// A file to link, by id or by a path to register first, with a note.
#[derive(Debug, Default, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct FileLinkInput {
    #[specta(optional)]
    pub file_id: Option<i64>,
    #[specta(optional)]
    pub path: Option<String>,
    /// Trimmed. Blank means none.
    #[specta(optional)]
    pub note: Option<String>,
}

impl From<FileLinkInput> for domain::FileLinkInput {
    fn from(link: FileLinkInput) -> Self {
        domain::FileLinkInput {
            file: domain::FileRef {
                file_id: link.file_id,
                path: link.path,
            },
            note: link.note,
        }
    }
}

/// Sets only a picture or logo.
#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SetPictureArgs {
    pub id: i64,
    /// A file given by path is registered first, in the same transaction.
    pub picture: FileRefInput,
}

/// A file linked to an artist, with its note.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileLink {
    pub file: File,
    pub note: Option<String>,
}

/// Language tags in use on names and titles, the most used first, for the
/// language picker.
#[tauri::command]
#[specta::specta]
pub async fn languages(app: AppHandle) -> Result<Vec<String>, CommandError> {
    on_library(app, in_use).await
}

pub fn in_use(library: &Library) -> Result<Vec<String>, CommandError> {
    Ok(library.catalog.languages()?)
}

/// Registered files by id, as DTOs.
fn files_by_id(
    library: &Library,
    ids: &[i64],
) -> Result<std::collections::HashMap<i64, File>, CommandError> {
    let files: Vec<FileRecord> = library.media.files_by_ids(ids)?;
    Ok(files
        .iter()
        .map(|file| (file.id, File::from(file)))
        .collect())
}
