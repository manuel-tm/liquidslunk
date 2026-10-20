//! The repository layer (backend.md, "The database"): the connection, the
//! migration runner with its backups, the migrations, the repositories, and
//! the sample data (`seeds`). Repositories are plain functions over a
//! `&Connection`, and a `Transaction` is one (backend.md, "Structure").

pub mod artists;
pub mod backups;
pub mod connection;
pub mod credits;
pub mod external_libraries;
pub mod file_links;
pub mod files;
pub mod helpers;
pub mod labels;
pub mod lists;
pub mod migrations;
pub mod migrator;
pub mod named;
pub mod rebuild_table;
pub mod releases;
pub mod seeds;
pub mod stats;
pub mod store;
pub mod tags;
pub mod titles;
pub mod tracks;

/// An SQL identifier, quoted, as Kysely quotes table and column names.
pub(crate) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
