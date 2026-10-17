//! Registered files. Phase 1 ports the rest of slunk's files repository.

use rusqlite::{Connection, OptionalExtension};

use crate::Result;

/// A registered file's path, relative to the database's folder, or `None` for
/// an unknown id.
pub fn path(conn: &Connection, id: i64) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            r#"select "path" from "files" where "id" = ?"#,
            [id],
            |row| row.get(0),
        )
        .optional()?)
}
