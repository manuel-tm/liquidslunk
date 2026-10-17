//! The commands the frontend calls, one module per context, with their DTOs
//! (design §5.4). tauri-specta writes `src/bindings.ts` from them whenever a
//! debug build starts.
//!
//! - Every command is `async`, and does its database and disk work on Tokio's
//!   blocking pool: a command that isn't `async` runs on the main thread.
//! - Every command returns `Result<_, CommandError>`, which the bindings wrap
//!   as `{ status: "ok", data } | { status: "error", error }`.
//! - A query command takes its arguments as one struct, `args`, so the
//!   frontend calls `useCommand("releasePage", { id })`.

use tauri::Wry;
use tauri_specta::{Builder, collect_commands};

use crate::error::CommandError;

pub mod library;
pub mod media;

/// Every command, for the invoke handler and the bindings.
pub fn builder() -> Builder<Wry> {
    Builder::<Wry>::new()
        .commands(collect_commands![
            library::library_status,
            library::open_library,
            library::create_library,
            library::close_library,
            library::recent_libraries,
            library::reveal_library,
            media::open_file,
            media::reveal_file,
        ])
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
