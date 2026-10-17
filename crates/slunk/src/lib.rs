//! liquidslunk's domain and its database (design §5.2), with no Tauri
//! dependency, so its tests run with a plain `cargo test` and the CLI uses it
//! directly.
//!
//! - `repo`: the repository layer: the connection, the migration runner, its
//!   backups, the migrations, and the repositories.
//! - `media`: the media context: registered files, and the library on disk.
//! - `paging`: a list's page, sort and search.
//! - `text`: text as it's saved.

pub mod error;
pub mod media;
pub mod paging;
pub mod repo;
pub mod text;

pub use error::{Error, Result};
