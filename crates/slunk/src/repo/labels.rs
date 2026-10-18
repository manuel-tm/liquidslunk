//! What only labels have: the releases they're on. slunk's labels
//! repository; the rest is `named.rs`.

use rusqlite::Connection;

use crate::Result;

/// How many releases the label is on.
pub fn release_count(conn: &Connection, id: i64) -> Result<i64> {
    Ok(conn.query_row(
        r#"select count(*) from "labels_releases" where "label_id" = ?"#,
        [id],
        |row| row.get(0),
    )?)
}
