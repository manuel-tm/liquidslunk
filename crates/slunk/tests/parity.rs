//! Migration parity with slunk (design §5.6). The fixtures in
//! `fixtures/parity/` are what slunk's own migration runner made, written while
//! its backend was in the repository: a database at each migration, with sample
//! rows, then where slunk took it, migrated to the end, and rolled back one. So
//! these tests check the Rust migrator against what slunk did. They cover
//! slunk's eight migrations, which never change, and only those: the
//! migrations after them are liquidslunk's, with their own tests.

use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde::Deserialize;
use slunk::repo::migrations::MIGRATIONS;
use slunk::repo::migrator::{migrate_with, rollback_with};

/// slunk's eight migrations, the first in `MIGRATIONS`
fn slunks() -> &'static [slunk::repo::migrator::Migration] {
    &MIGRATIONS[..8]
}

/// A row of `sqlite_schema`, without `rootpage`.
#[derive(Debug, PartialEq, Eq, Deserialize)]
struct SchemaRow {
    r#type: String,
    name: String,
    tbl_name: String,
    sql: Option<String>,
}

#[test]
fn a_new_database_has_slunks_schema() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("slunk.db");
    migrate_with(&db, slunks(), 10).unwrap();

    let theirs: Vec<SchemaRow> = serde_json::from_str(&fixture("fresh.schema.json")).unwrap();
    assert_eq!(schema(&db, "order by rowid"), theirs);
    assert_same(
        &snapshot(&db),
        &snapshot(&load(dir.path(), "slunk", &fixture("fresh.sql"))),
        "a new database",
    );
}

#[test]
fn upgrades_end_where_slunks_do() {
    for k in 0..slunks().len() {
        let dir = tempfile::tempdir().unwrap();
        let db = load(dir.path(), "rust", &fixture(&format!("state_{k}.sql")));
        migrate_with(&db, slunks(), 10).unwrap();
        let theirs = load(dir.path(), "slunk", &fixture(&format!("final_{k}.sql")));
        assert_same(
            &snapshot(&db),
            &snapshot(&theirs),
            &format!("from the seeded state after {k} migrations"),
        );
    }
}

#[test]
fn rollbacks_end_where_slunks_do() {
    for k in 1..=slunks().len() {
        let dir = tempfile::tempdir().unwrap();
        let start = if k < slunks().len() {
            format!("state_{k}.sql")
        } else {
            format!("final_{}.sql", k - 1)
        };
        let db = load(dir.path(), "rust", &fixture(&start));
        let outcome = rollback_with(&db, slunks(), 1, 10).unwrap();
        assert_eq!(outcome.names, [slunks()[k - 1].name]);
        let theirs = load(
            dir.path(),
            "slunk",
            &fixture(&format!("rolled_back_{k}.sql")),
        );
        assert_same(
            &snapshot(&db),
            &snapshot(&theirs),
            &format!("rolling back the seeded state after {k} migrations"),
        );
    }
}

#[test]
fn every_down_reverses_its_up() {
    // The runner's own table, and SQLite's counters, which stay once made
    let ours = |row: &SchemaRow| {
        ![
            "migrations",
            "sqlite_autoindex_migrations_1",
            "sqlite_sequence",
        ]
        .contains(&row.name.as_str())
    };
    // add_track_tags' down recreates the baseline's *_tags_multiple_off
    // triggers with the same logic, laid out differently, as slunk's did. So
    // the SQL is compared without whitespace and parentheses.
    let layout = |row: SchemaRow| SchemaRow {
        sql: row.sql.map(|sql| {
            sql.chars()
                .filter(|c| !c.is_whitespace() && *c != '(' && *c != ')')
                .collect()
        }),
        ..row
    };
    for (i, migration) in MIGRATIONS.iter().enumerate() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        let before = |db: &Path| -> Vec<SchemaRow> {
            schema(db, "order by name")
                .into_iter()
                .filter(ours)
                .map(layout)
                .collect()
        };
        migrate_with(&db, &MIGRATIONS[..i], 10).unwrap();
        let expected = before(&db);
        migrate_with(&db, &MIGRATIONS[..=i], 10).unwrap();
        let outcome = rollback_with(&db, &MIGRATIONS[..=i], 1, 10).unwrap();
        assert_eq!(outcome.names, [migration.name]);
        assert_eq!(
            before(&db),
            expected,
            "{}'s down doesn't undo its up",
            migration.name
        );
    }
}

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A database made from a dump, in a folder of its own.
fn load(dir: &Path, name: &str, dump: &str) -> PathBuf {
    let folder = dir.join(name);
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join("slunk.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch(dump)
        .unwrap();
    path
}

fn schema(db: &Path, order: &str) -> Vec<SchemaRow> {
    let conn = Connection::open(db).unwrap();
    let mut statement = conn
        .prepare(&format!(
            "select type, name, tbl_name, sql from sqlite_schema {order}"
        ))
        .unwrap();
    statement
        .query_map([], |row| {
            Ok(SchemaRow {
                r#type: row.get(0)?,
                name: row.get(1)?,
                tbl_name: row.get(2)?,
                sql: row.get(3)?,
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Everything in a database, as lines: its schema, by name, then every
/// table's rows, by rowid. `migrations.applied_at` is left out, since it's
/// when each run happened. So is an empty `sqlite_sequence`, which SQLite
/// keeps once it's made, but the sqlite3 shell's `.dump` doesn't write.
fn snapshot(db: &Path) -> Vec<String> {
    let conn = Connection::open(db).unwrap();
    let counters: i64 = conn
        .query_row(
            "select count(*) from sqlite_schema where name = 'sqlite_sequence'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let no_counters = counters == 0
        || conn
            .query_row("select count(*) from sqlite_sequence", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
            == 0;
    let mut lines: Vec<String> = schema(db, "order by type, name")
        .into_iter()
        .filter(|row| !(no_counters && row.name == "sqlite_sequence"))
        .map(|row| {
            let sql = row.sql.unwrap_or_default();
            format!("{} {} on {}: {sql}", row.r#type, row.name, row.tbl_name)
        })
        .collect();
    let tables: Vec<String> = conn
        .prepare("select name from sqlite_schema where type = 'table' order by name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    for table in tables {
        let order = if table == "sqlite_sequence" {
            "name"
        } else {
            "rowid"
        };
        let mut statement = conn
            .prepare(&format!(r#"select * from "{table}" order by {order}"#))
            .unwrap();
        let columns: Vec<String> = statement
            .column_names()
            .into_iter()
            .map(String::from)
            .collect();
        let mut rows = statement.query([]).unwrap();
        while let Some(row) = rows.next().unwrap() {
            let values: Vec<String> = columns
                .iter()
                .enumerate()
                .map(|(i, column)| {
                    if table == "migrations" && column == "applied_at" {
                        return format!("{column}=…");
                    }
                    format!("{column}={}", shown(row.get_ref(i).unwrap()))
                })
                .collect();
            lines.push(format!("{table}: {}", values.join(", ")));
        }
    }
    lines
}

fn shown(value: ValueRef) -> String {
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(n) => n.to_string(),
        ValueRef::Real(x) => format!("{x:?}"),
        ValueRef::Text(text) => format!("{:?}", String::from_utf8_lossy(text)),
        ValueRef::Blob(bytes) => format!("{bytes:02x?}"),
    }
}

fn assert_same(ours: &[String], theirs: &[String], context: &str) {
    if ours == theirs {
        return;
    }
    let only = |a: &[String], b: &[String]| -> String {
        a.iter()
            .filter(|line| !b.contains(line))
            .map(|line| format!("  {line}\n"))
            .collect()
    };
    panic!(
        "{context}, Rust's database isn't slunk's.\nOnly in Rust's:\n{}Only in slunk's:\n{}",
        only(ours, theirs),
        only(theirs, ours)
    );
}
