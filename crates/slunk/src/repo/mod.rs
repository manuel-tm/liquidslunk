//! The repository layer: the connection, the migration runner with its
//! backups, the migrations (design §5.6), and the repositories. Repositories
//! are plain functions over a `&Connection`, and a `Transaction` is one
//! (design §12.2).

pub mod backups;
pub mod connection;
pub mod file_links;
pub mod files;
pub mod helpers;
pub mod migrations;
pub mod migrator;
pub mod rebuild_table;
pub mod store;

/// An SQL identifier, quoted, as Kysely quotes table and column names.
pub(crate) fn quote(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}
