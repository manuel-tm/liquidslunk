//! Rebuilds a table with a new definition, for the schema changes SQLite's
//! `ALTER TABLE` can't make: a column's type, CHECK, default or foreign key
//! (backend.md, "Changing an existing table"). It's part of the migration
//! system, ported from slunk's `rebuildTable()`. Committed migrations call it,
//! so its behavior must never change: extend it with new options instead.

use rusqlite::{Connection, OptionalExtension};

use super::quote;
use crate::{Error, Result};

/// How to rebuild a table.
#[derive(Debug, Clone, Copy)]
pub struct Rebuild<'a> {
    /// The new definition, complete: what follows `create table "new_<table>"`,
    /// its columns and constraints in parentheses, then `strict`. It's built
    /// under that temporary name, and renamed once the old table is gone.
    pub define: &'a str,
    /// SQL expressions over the old table, for new or changed columns, by
    /// column. Columns both definitions share are copied as they are, and any
    /// other new column gets its default.
    pub copy: &'a [(&'a str, &'a str)],
}

/// Rebuilds `table` with SQLite's recipe, keeping its rows, its indexes, its
/// triggers and its `AUTOINCREMENT` counter. Indexes and triggers come back
/// as they were: to change one, drop it before the rebuild and create the new
/// one after.
///
/// Runs only in a migration, where the runner has turned foreign keys off and
/// checks them before committing.
pub fn rebuild_table(conn: &Connection, table: &str, rebuild: Rebuild) -> Result<()> {
    // With foreign keys on, DROP TABLE would first delete every row, and
    // ON DELETE CASCADE would wipe the rows that point at them
    let foreign_keys: i64 = conn.query_row("pragma foreign_keys", [], |row| row.get(0))?;
    if foreign_keys != 0 {
        return Err(Error::Invalid(format!(
            "rebuild_table({table}) needs foreign keys off"
        )));
    }

    let temp = format!("new_{table}");
    let kept: Vec<String> = {
        let mut statement = conn.prepare(
            "select sql from sqlite_schema \
             where tbl_name = ? and type in ('index', 'trigger') and sql is not null \
             order by rowid",
        )?;
        let rows = statement.query_map([table], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let counter = sequence(conn, table)?;

    // Copy the rows into the new definition
    conn.execute(
        &format!("create table {} {}", quote(&temp), rebuild.define),
        [],
    )?;
    let old_columns = columns(conn, table)?;
    let new_columns = columns(conn, &temp)?;
    let unknown: Vec<&str> = rebuild
        .copy
        .iter()
        .map(|(column, _)| *column)
        .filter(|column| !new_columns.iter().any(|new| new == column))
        .collect();
    if !unknown.is_empty() {
        return Err(Error::Invalid(format!(
            "rebuild_table({table}): no column {}",
            unknown.join(", ")
        )));
    }
    let copied = |column: &String| rebuild.copy.iter().find(|(name, _)| name == column);
    let targets: Vec<&String> = new_columns
        .iter()
        .filter(|column| copied(column).is_some() || old_columns.contains(column))
        .collect();
    let names: Vec<String> = targets.iter().map(|column| quote(column)).collect();
    let values: Vec<String> = targets
        .iter()
        .map(|column| match copied(column) {
            Some((_, expression)) => expression.to_string(),
            None => quote(column),
        })
        .collect();
    conn.execute(
        &format!(
            "insert into {} ({}) select {} from {}",
            quote(&temp),
            names.join(", "),
            values.join(", "),
            quote(table)
        ),
        [],
    )?;

    // Swap the tables. Other tables' triggers still name the dropped table,
    // which would make the rename fail its schema check: legacy_alter_table
    // skips that check. Their foreign keys keep pointing at the name.
    conn.execute(&format!("drop table {}", quote(table)), [])?;
    conn.execute("pragma legacy_alter_table = on", [])?;
    let renamed = conn.execute(
        &format!("alter table {} rename to {}", quote(&temp), quote(table)),
        [],
    );
    conn.execute("pragma legacy_alter_table = off", [])?;
    renamed?;

    for statement in &kept {
        conn.execute(statement, [])?;
    }

    // Copying rows set the counter to the highest id left. Put back the old
    // one, so ids of deleted rows are never reused.
    if let Some(counter) = counter {
        let now = sequence(conn, table)?.unwrap_or(0);
        conn.execute("delete from sqlite_sequence where name = ?", [table])?;
        conn.execute(
            "insert into sqlite_sequence (name, seq) values (?, ?)",
            (table, counter.max(now)),
        )?;
    }
    Ok(())
}

fn columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut statement = conn.prepare("select name from pragma_table_info(?)")?;
    let rows = statement.query_map([table], |row| row.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The table's `AUTOINCREMENT` counter, or `None` if it has none.
fn sequence(conn: &Connection, table: &str) -> Result<Option<i64>> {
    let exists = conn
        .query_row(
            "select 1 from sqlite_schema where name = 'sqlite_sequence'",
            [],
            |_| Ok(()),
        )
        .optional()?;
    if exists.is_none() {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            "select seq from sqlite_sequence where name = ?",
            [table],
            |row| row.get(0),
        )
        .optional()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A database as a migration sees it: foreign keys are off, which
    /// rusqlite's bundled SQLite doesn't default to.
    fn database() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            pragma foreign_keys = off;
            create table "artists" ("id" integer not null primary key autoincrement, "name" text not null) strict;
            create index "artists_name_index" on "artists" ("name");
            create table "links" ("id" integer not null primary key, "artist_id" integer not null references "artists" ("id")) strict;
            create trigger "links_artist_is_known" before insert on "links"
              when (select 1 from "artists" where "id" = NEW."artist_id") is null
              begin select raise(abort, 'unknown artist'); end;
            create trigger "artists_named" before insert on "artists"
              when NEW."name" = '' begin select raise(abort, 'no name'); end;
            insert into "artists" ("name") values ('A'), ('B'), ('C');
            delete from "artists" where "id" = 3;
            insert into "links" ("artist_id") values (2);
            "#,
        )
        .unwrap();
        conn
    }

    fn schema(conn: &Connection, table: &str) -> Vec<String> {
        let mut statement = conn
            .prepare(
                "select type || ' ' || name from sqlite_schema where tbl_name = ? order by name",
            )
            .unwrap();
        statement
            .query_map([table], |row| row.get(0))
            .unwrap()
            .map(|row| row.unwrap())
            .collect()
    }

    #[test]
    fn keeps_rows_indexes_triggers_and_the_counter() {
        let conn = database();
        rebuild_table(
            &conn,
            "artists",
            Rebuild {
                define: r#"("id" integer not null primary key autoincrement, "name" text not null, "sort_name" text not null) strict"#,
                copy: &[("sort_name", "lower(name)")],
            },
        )
        .unwrap();

        let rows: Vec<(i64, String, String)> = conn
            .prepare(r#"select "id", "name", "sort_name" from "artists" order by "id""#)
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect();
        assert_eq!(
            rows,
            [(1, "A".into(), "a".into()), (2, "B".into(), "b".into())]
        );
        assert_eq!(
            schema(&conn, "artists"),
            [
                "table artists",
                "index artists_name_index",
                "trigger artists_named"
            ]
        );
        // The other table's trigger and key still work
        assert!(
            conn.execute(r#"insert into "links" ("artist_id") values (9)"#, [])
                .is_err()
        );
        // A deleted id isn't reused
        conn.execute(
            r#"insert into "artists" ("name", "sort_name") values ('D', 'd')"#,
            [],
        )
        .unwrap();
        let id: i64 = conn
            .query_row(r#"select max("id") from "artists""#, [], |row| row.get(0))
            .unwrap();
        assert_eq!(id, 4);
    }

    #[test]
    fn refuses_unknown_columns_and_foreign_keys_on() {
        let conn = database();
        let rebuild = Rebuild {
            define: r#"("id" integer not null primary key autoincrement, "name" text not null) strict"#,
            copy: &[("nickname", "name")],
        };
        let error = rebuild_table(&conn, "artists", rebuild).unwrap_err();
        assert_eq!(
            error.to_string(),
            "rebuild_table(artists): no column nickname"
        );

        let conn = database();
        conn.execute("pragma foreign_keys = on", []).unwrap();
        let error = rebuild_table(
            &conn,
            "artists",
            Rebuild {
                copy: &[],
                ..rebuild
            },
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "rebuild_table(artists) needs foreign keys off"
        );
    }
}
