//! liquidslunk's domain and its database (design §5.2), with no Tauri
//! dependency, so its tests run with a plain `cargo test` and the CLI uses it
//! directly.
//!
//! - `repo`: the repository layer: the connection, the migration runner, its
//!   backups, the migrations, and the repositories.
//! - `media`: the media context: registered files, and the library on disk.
//! - `catalog`: the catalog context: artists, labels, the lists the user
//!   manages, releases and tracks, and what a folder suggests for importing
//!   it.
//! - `tagging`: the tagging context: tag definitions, and their values on
//!   releases and tracks.
//! - `library_check`: the library check, over the media context and the
//!   catalog.
//! - `dates`: dates as they're typed, stored and shown.
//! - `languages`: language tags, as they're saved.
//! - `paging`: a list's page, sort and search.
//! - `text`: text as it's saved.

pub mod catalog;
pub mod dates;
pub mod error;
pub mod languages;
pub mod library_check;
pub mod media;
pub mod paging;
pub mod repo;
pub mod tagging;
pub mod text;

pub use error::{Error, Result};
