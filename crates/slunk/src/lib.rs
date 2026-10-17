//! liquidslunk's domain and its database (design §5.2), with no Tauri
//! dependency, so its tests run with a plain `cargo test` and the CLI uses it
//! directly.
//!
//! - `repo`: the repository layer: the connection, the migration runner, its
//!   backups, and the migrations.
//! - `media`: the media context, so far only where a registered file is.

pub mod error;
pub mod media;
pub mod repo;

pub use error::{Error, Result};
