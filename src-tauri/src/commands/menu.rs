//! The Controls menu's command (backend.md, "The menu bar"). The menu is in
//! `crate::menu`.

use tauri::{AppHandle, Manager};

use crate::error::CommandError;
use crate::menu::{Controls, ControlsMenu};

/// Shows the player in the Controls menu: Play or Pause, Shuffle's and
/// Repeat's checks, and every item off while nothing is queued. It isn't
/// `async`, so it runs on the main thread, where the menu is, and the
/// player's calls change it in the order they're made.
#[tauri::command]
#[specta::specta]
pub fn set_controls_menu(app: AppHandle, args: Controls) -> Result<(), CommandError> {
    let Some(menu) = app.try_state::<ControlsMenu>() else {
        return Ok(());
    };
    menu.show(args)
        .map_err(|error| CommandError::internal(error.to_string()))
}
