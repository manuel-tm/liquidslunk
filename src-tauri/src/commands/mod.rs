//! The commands the frontend calls, one module per context, with their DTOs
//! (design §5.4). tauri-specta writes `src/bindings.ts` from them whenever a
//! debug build starts.
//!
//! - Every command is `async`, and does its database and disk work on Tokio's
//!   blocking pool: a command that isn't `async` runs on the main thread. The
//!   exception is `set_controls_menu`, which belongs there, with the menu.
//! - Every command returns `Result<_, CommandError>`, which the bindings wrap
//!   as `{ status: "ok", data } | { status: "error", error }`.
//! - A query command takes its arguments as one struct, `args`, so the
//!   frontend calls `useCommand("releasePage", { id })`.

use tauri::{AppHandle, Manager, Wry};
use tauri_specta::{Builder, collect_commands, collect_events};

use crate::error::CommandError;
use crate::library::{Libraries, Library};

pub mod catalog;
pub mod external;
pub mod library;
pub mod library_check;
pub mod media;
pub mod menu;
pub mod tagging;

/// Every command and event, for the invoke handler and the bindings.
pub fn builder() -> Builder<Wry> {
    Builder::<Wry>::new()
        .commands(collect_commands![
            library::library_status,
            library::open_library,
            library::create_library,
            library::close_library,
            library::recent_libraries,
            library::reveal_library,
            media::files,
            media::file,
            media::file_counts,
            media::folder,
            media::folder_files,
            media::register_files,
            media::update_file,
            media::delete_file,
            media::open_file,
            media::reveal_file,
            catalog::lists::list_items,
            catalog::lists::list_item,
            catalog::lists::list_item_options,
            catalog::lists::create_list_item,
            catalog::lists::rename_list_item,
            catalog::lists::delete_list_item,
            catalog::artists::artists,
            catalog::artists::artist,
            catalog::artists::artist_options,
            catalog::artists::artist_names,
            catalog::artists::create_artist,
            catalog::artists::update_artist,
            catalog::artists::set_artist_picture,
            catalog::artists::add_artist_name,
            catalog::artists::delete_artist,
            catalog::artists::artist_header,
            catalog::artists::artist_releases,
            catalog::artists::artist_appearances,
            catalog::artists::artist_credits,
            catalog::labels::labels,
            catalog::labels::label,
            catalog::labels::label_options,
            catalog::labels::create_label,
            catalog::labels::update_label,
            catalog::labels::set_label_picture,
            catalog::labels::delete_label,
            catalog::labels::label_page,
            catalog::releases::releases,
            catalog::releases::release_ids,
            catalog::releases::release_page,
            catalog::releases::release_header,
            catalog::releases::release,
            catalog::releases::release_files,
            catalog::releases::release_credits,
            catalog::releases::release_options,
            catalog::releases::create_release,
            catalog::releases::update_release,
            catalog::releases::delete_release,
            catalog::releases::set_release_cover,
            catalog::releases::set_release_files,
            catalog::releases::set_release_credits,
            catalog::releases::credit_roles,
            catalog::releases::catalog_totals,
            catalog::tracks::tracks,
            catalog::tracks::track_page,
            catalog::tracks::track_form,
            catalog::tracks::create_track,
            catalog::tracks::update_track,
            catalog::tracks::delete_track,
            catalog::tracks::release_tracks,
            catalog::tracks::set_release_tracks,
            catalog::tracks::player_tracks,
            catalog::tracks::release_track_ids,
            catalog::tracks::track_options,
            catalog::import::release_suggestion,
            catalog::import::import_files,
            catalog::languages,
            external::external_libraries,
            external::external_library,
            external::create_external_library,
            external::update_external_library,
            external::delete_external_library,
            external::external_library_rules,
            external::set_external_library_rules,
            external::add_to_external_library,
            external::remove_from_external_library,
            external::external_memberships,
            external::external_library_tracks,
            external::exported_tags,
            external::locate_external_library,
            external::eject_external_library,
            external::sync_plan,
            external::sync,
            external::cancel_sync,
            tagging::tags,
            tagging::tag,
            tagging::tag_options,
            tagging::tags_by_ref,
            tagging::tag_values,
            tagging::tag_value_options,
            tagging::create_tag,
            tagging::update_tag,
            tagging::delete_tag,
            tagging::release_tags,
            tagging::set_release_tags,
            tagging::set_tag_on_releases,
            tagging::remove_tag_from_releases,
            library_check::library_check,
            library_check::relink_files,
            library_check::rehash_files,
            menu::set_controls_menu,
        ])
        .events(collect_events![crate::menu::PlayerControl])
        // Ids are i64. SQLite's stay far below 2^53, so they're numbers.
        .dangerously_cast_bigints_to_number()
}

/// Runs blocking work on Tokio's blocking pool.
pub(crate) async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| CommandError::internal(error.to_string()))?
}

/// Runs work on the open library, on Tokio's blocking pool. Without one, it's
/// a `noLibrary` error.
pub(crate) async fn on_library<T: Send + 'static>(
    app: AppHandle,
    work: impl FnOnce(&Library) -> Result<T, CommandError> + Send + 'static,
) -> Result<T, CommandError> {
    blocking(move || {
        let library = app.state::<Libraries>().library()?;
        work(&library)
    })
    .await
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_bindings_export() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bindings.ts");
        super::builder()
            .export(specta_typescript::Typescript::default(), &path)
            .unwrap();
        let bindings = std::fs::read_to_string(&path).unwrap();
        assert!(
            bindings.contains("registerFiles: (args: RegisterFilesArgs)"),
            "{bindings}"
        );
        assert!(
            bindings.contains("updateArtist: (args: UpdateArtistArgs)"),
            "{bindings}"
        );
        assert!(
            bindings.contains("setReleaseTags: (args: SetReleaseTagsArgs)"),
            "{bindings}"
        );
        assert!(
            bindings.contains("updateRelease: (args: UpdateReleaseArgs)"),
            "{bindings}"
        );
        assert!(
            bindings.contains("setReleaseTracks: (args: SetReleaseTracksArgs)"),
            "{bindings}"
        );
        if let Ok(copy) = std::env::var("LIQUIDSLUNK_BINDINGS_COPY") {
            std::fs::write(copy, bindings).unwrap();
        }
    }
}
