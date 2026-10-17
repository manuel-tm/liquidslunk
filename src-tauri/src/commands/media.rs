//! The media context's commands. Phase 0 has only "Open the file" and "Show
//! in Finder" (design §5.5); phase 1 ports the rest.

use std::path::PathBuf;

use serde::Deserialize;
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::blocking;
use crate::error::CommandError;
use crate::library::Libraries;

#[derive(Debug, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileArgs {
    pub id: i64,
}

/// Opens a registered file in its default app.
#[tauri::command]
#[specta::specta]
pub async fn open_file(app: AppHandle, args: FileArgs) -> Result<(), CommandError> {
    blocking(move || {
        let path = on_disk(&app, args.id)?;
        app.opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}

/// Shows a registered file in Finder.
#[tauri::command]
#[specta::specta]
pub async fn reveal_file(app: AppHandle, args: FileArgs) -> Result<(), CommandError> {
    blocking(move || {
        let path = on_disk(&app, args.id)?;
        app.opener()
            .reveal_item_in_dir(path)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}

/// A registered file's absolute path, if it's there.
fn on_disk(app: &AppHandle, id: i64) -> Result<PathBuf, CommandError> {
    let library = app.state::<Libraries>().library()?;
    let path = {
        let conn = library.conn();
        slunk::media::file_path(&conn, &library.folder, id)?
    };
    match path {
        Some(path) if path.exists() => Ok(path),
        Some(path) => Err(CommandError::not_found(format!(
            "There's no file at {}.",
            path.display()
        ))),
        None => Err(CommandError::not_found("There's no such file.")),
    }
}
