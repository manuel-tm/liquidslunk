//! Choosing the library (backend.md, "Choosing the library"): its status,
//! opening, creating, closing, and the recent ones.

use std::path::PathBuf;

use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::blocking;
use crate::error::CommandError;
use crate::library::{Created, Libraries, LibraryStatus, OpenProgress, Opened, RecentLibrary};

/// The open library, and the one the settings open at launch. It replaces
/// slunk's `health` query.
#[tauri::command]
#[specta::specta]
pub async fn library_status(app: AppHandle) -> Result<LibraryStatus, CommandError> {
    blocking(move || app.state::<Libraries>().status()).await
}

/// Opens the library at `path`, a `slunk.db`, migrating it first, and makes
/// it the one to open at launch. `progress` hears when it's being upgraded.
#[tauri::command]
#[specta::specta]
pub async fn open_library(
    app: AppHandle,
    path: String,
    progress: Channel<OpenProgress>,
) -> Result<Opened, CommandError> {
    blocking(move || {
        app.state::<Libraries>()
            .open(&PathBuf::from(path), true, |event| {
                let _ = progress.send(event);
            })
    })
    .await
}

/// Creates a library in `folder` and opens it, unless the folder has one.
#[tauri::command]
#[specta::specta]
pub async fn create_library(
    app: AppHandle,
    folder: String,
    progress: Channel<OpenProgress>,
) -> Result<Created, CommandError> {
    blocking(move || {
        app.state::<Libraries>()
            .create(&PathBuf::from(folder), |event| {
                let _ = progress.send(event);
            })
    })
    .await
}

/// Closes the open library. The next launch still opens it.
#[tauri::command]
#[specta::specta]
pub async fn close_library(app: AppHandle) -> Result<(), CommandError> {
    blocking(move || {
        app.state::<Libraries>().close();
        Ok(())
    })
    .await
}

/// The last five libraries opened, newest first.
#[tauri::command]
#[specta::specta]
pub async fn recent_libraries(app: AppHandle) -> Result<Vec<RecentLibrary>, CommandError> {
    blocking(move || Ok(app.state::<Libraries>().recent())).await
}

/// Shows the open library's database in Finder.
#[tauri::command]
#[specta::specta]
pub async fn reveal_library(app: AppHandle) -> Result<(), CommandError> {
    blocking(move || {
        let library = app.state::<Libraries>().library()?;
        app.opener()
            .reveal_item_in_dir(&library.db)
            .map_err(|error| CommandError::internal(error.to_string()))
    })
    .await
}
