//! The migration runner, ported from slunk's (design §5.6). It applies every
//! pending migration in a single `BEGIN IMMEDIATE … COMMIT`, after backing the
//! database up, and checks foreign keys before committing.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

use super::backups::{self, Backup};
use super::connection;
use super::migrations::MIGRATIONS;
use crate::{Error, Result};

/// A migration: its name, `<timestamp>_<snake_case>`, and its two directions.
/// Each runs inside the runner's transaction, so it never starts its own.
#[derive(Debug, Clone, Copy)]
pub struct Migration {
    pub name: &'static str,
    pub up: fn(&Transaction) -> Result<()>,
    pub down: fn(&Transaction) -> Result<()>,
}

/// What a run did: the migrations it ran, the backup it made first, and the
/// old backups that removed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub names: Vec<String>,
    pub backup: Option<PathBuf>,
    pub removed: Vec<PathBuf>,
}

/// A migration known here or applied to the database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub name: String,
    /// When it was applied, in epoch seconds, or `None` while it's pending.
    pub applied_at: Option<i64>,
    /// Applied, but unknown here: a newer version applied it.
    pub missing: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

/// The `migrations` table, as slunk's runner creates it the first time.
const CREATE_MIGRATIONS: &str = r#"create table if not exists "migrations" ("id" integer not null primary key, "name" text not null unique, "applied_at" integer default (unixepoch()) not null) strict"#;

/// Backs up, then applies every pending migration. A database that doesn't
/// exist yet is created, and isn't backed up.
pub fn migrate(db_path: &Path, keep_backups: u32) -> Result<Outcome> {
    migrate_with(db_path, MIGRATIONS, keep_backups)
}

/// [`migrate`], knowing only `migrations`, in order.
pub fn migrate_with(
    db_path: &Path,
    migrations: &[Migration],
    keep_backups: u32,
) -> Result<Outcome> {
    run(
        db_path,
        migrations,
        keep_backups,
        Direction::Up,
        |applied, known| {
            known
                .iter()
                .filter(|name| !applied.iter().any(|done| done == *name))
                .map(|name| name.to_string())
                .collect()
        },
    )
}

/// Backs up, then reverts the last `n` applied migrations, newest first.
pub fn rollback(db_path: &Path, n: usize, keep_backups: u32) -> Result<Outcome> {
    rollback_with(db_path, MIGRATIONS, n, keep_backups)
}

/// [`rollback`], knowing only `migrations`, in order.
pub fn rollback_with(
    db_path: &Path,
    migrations: &[Migration],
    n: usize,
    keep_backups: u32,
) -> Result<Outcome> {
    if n < 1 {
        return Err(Error::Invalid(format!("Not a number of migrations: {n}")));
    }
    run(
        db_path,
        migrations,
        keep_backups,
        Direction::Down,
        |applied, _| applied.iter().rev().take(n).cloned().collect(),
    )
}

/// The migrations `migrate` would apply, without changing anything. A
/// database that doesn't exist has them all pending.
pub fn pending(db_path: &Path) -> Result<Vec<String>> {
    let names = known(MIGRATIONS);
    if !db_path.exists() {
        return Ok(names.iter().map(|name| name.to_string()).collect());
    }
    let conn = connection::open_read_only(db_path)?;
    let applied = applied(&conn, &names)?;
    Ok(names
        .iter()
        .filter(|name| !applied.iter().any(|done| done == *name))
        .map(|name| name.to_string())
        .collect())
}

/// Every migration with when it was applied, plus applied ones unknown here.
pub fn status(db_path: &Path) -> Result<Vec<Status>> {
    let names = known(MIGRATIONS);
    let applied = if db_path.exists() {
        applied_rows(&connection::open_read_only(db_path)?)?
    } else {
        Vec::new()
    };
    let mut all: Vec<String> = names.iter().map(|name| name.to_string()).collect();
    for (name, _) in &applied {
        if !all.contains(name) {
            all.push(name.clone());
        }
    }
    all.sort();
    Ok(all
        .into_iter()
        .map(|name| Status {
            applied_at: applied
                .iter()
                .find(|(done, _)| *done == name)
                .map(|(_, at)| *at),
            missing: !names.contains(&name.as_str()),
            name,
        })
        .collect())
}

/// The latest applied migration, or `None` before the first one.
pub fn latest_migration(conn: &Connection) -> Result<Option<String>> {
    if !has_migrations_table(conn)? {
        return Ok(None);
    }
    Ok(conn
        .query_row(
            r#"select "name" from "migrations" order by "name" desc limit 1"#,
            [],
            |row| row.get(0),
        )
        .optional()?)
}

/// Whether the database has the runner's `migrations` table.
pub fn has_migrations_table(conn: &Connection) -> Result<bool> {
    Ok(conn
        .query_row(
            "select 1 from sqlite_schema where type = 'table' and name = 'migrations'",
            [],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn run(
    db_path: &Path,
    migrations: &[Migration],
    keep_backups: u32,
    direction: Direction,
    pick: impl Fn(&[String], &[&str]) -> Vec<String>,
) -> Result<Outcome> {
    let is_new = !db_path.exists();
    // Foreign keys stay off while the schema changes (design §5.6). The pragma
    // can't change inside a transaction, so it's set when the connection opens.
    let mut conn = connection::open_sqlite(
        db_path,
        connection::Options {
            foreign_keys: false,
            must_exist: false,
        },
    )?;
    let names = known(migrations);
    if pick(&applied(&conn, &names)?, &names).is_empty() {
        return Ok(Outcome::default());
    }
    let backup = if is_new {
        None
    } else {
        Some(backups::backup_into(&conn, db_path, keep_backups)?)
    };

    // Takes the write lock up front, so the app and the CLI can't interleave.
    // What's pending is read again under the lock.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute(CREATE_MIGRATIONS, [])?;
    let picked = pick(&applied(&tx, &names)?, &names);
    for name in &picked {
        let migration = migrations
            .iter()
            .find(|migration| migration.name == name)
            .expect("applied migrations are known, so every pick is");
        match direction {
            Direction::Up => {
                (migration.up)(&tx)?;
                tx.execute(r#"insert into "migrations" ("name") values (?)"#, [name])?;
            }
            Direction::Down => {
                (migration.down)(&tx)?;
                tx.execute(r#"delete from "migrations" where "name" = ?"#, [name])?;
            }
        }
    }
    foreign_key_check(&tx)?;
    tx.commit()?;

    let (backup, removed) = match backup {
        Some(Backup { path, removed }) => (Some(path), removed),
        None => (None, Vec::new()),
    };
    Ok(Outcome {
        names: picked,
        backup,
        removed,
    })
}

fn known(migrations: &[Migration]) -> Vec<&'static str> {
    migrations.iter().map(|migration| migration.name).collect()
}

/// Applied migration names, in order, after checking them against the known
/// ones.
fn applied(conn: &Connection, known: &[&str]) -> Result<Vec<String>> {
    let names: Vec<String> = applied_rows(conn)?
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let unknown: Vec<String> = names
        .iter()
        .filter(|name| !known.contains(&name.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(Error::NewerLibrary(unknown));
    }
    if let Some(last) = names.last() {
        let early: Vec<&str> = known
            .iter()
            .filter(|name| **name < last.as_str() && !names.iter().any(|done| done == *name))
            .copied()
            .collect();
        if !early.is_empty() {
            return Err(Error::Invalid(format!(
                "Pending migrations sort before the last applied one ({last}): {}. Give them a later timestamp.",
                early.join(", ")
            )));
        }
    }
    Ok(names)
}

fn applied_rows(conn: &Connection) -> Result<Vec<(String, i64)>> {
    if !has_migrations_table(conn)? {
        return Ok(Vec::new());
    }
    let mut statement =
        conn.prepare(r#"select "name", "applied_at" from "migrations" order by "name""#)?;
    let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fails, so the transaction rolls back, if any row breaks a foreign key.
fn foreign_key_check(conn: &Connection) -> Result<()> {
    let mut statement = conn.prepare("pragma foreign_key_check")?;
    let rows = statement.query_map([], |row| {
        let table: String = row.get("table")?;
        let rowid: Option<i64> = row.get("rowid")?;
        let parent: String = row.get("parent")?;
        let fkid: i64 = row.get("fkid")?;
        let rowid = rowid.map_or("null".to_string(), |id| id.to_string());
        Ok(format!(
            r#"{{"table":"{table}","rowid":{rowid},"parent":"{parent}","fkid":{fkid}}}"#
        ))
    })?;
    let broken = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    if broken.is_empty() {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "Foreign key check failed, rolled back:\n{}",
            broken.join("\n")
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::repo::migrations::run;

    fn create_a(tx: &Transaction) -> Result<()> {
        run(
            tx,
            &[r#"create table "a" ("id" integer not null primary key) strict"#],
        )
    }
    fn drop_a(tx: &Transaction) -> Result<()> {
        run(tx, &[r#"drop table "a""#])
    }
    fn create_b(tx: &Transaction) -> Result<()> {
        run(
            tx,
            &[
                r#"create table "b" ("id" integer not null primary key, "a_id" integer not null references "a" ("id")) strict"#,
            ],
        )
    }
    fn drop_b(tx: &Transaction) -> Result<()> {
        run(tx, &[r#"drop table "b""#])
    }
    fn orphan(tx: &Transaction) -> Result<()> {
        run(tx, &[r#"insert into "b" ("id", "a_id") values (1, 99)"#])
    }
    fn nothing(_: &Transaction) -> Result<()> {
        Ok(())
    }

    const A: Migration = Migration {
        name: "20260101000000_create_a",
        up: create_a,
        down: drop_a,
    };
    const B: Migration = Migration {
        name: "20260102000000_create_b",
        up: create_b,
        down: drop_b,
    };

    fn tables(db: &Path) -> Vec<String> {
        let conn = Connection::open(db).unwrap();
        let mut statement = conn
            .prepare("select name from sqlite_schema where type = 'table' order by name")
            .unwrap();
        statement
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|name| name.unwrap())
            .collect()
    }

    #[test]
    fn a_new_database_is_migrated_without_a_backup() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        let outcome = migrate_with(&db, &[A, B], 10).unwrap();
        assert_eq!(outcome.names, [A.name, B.name]);
        assert_eq!(outcome.backup, None);
        assert!(!dir.path().join(backups::FOLDER).exists());
        assert_eq!(tables(&db), ["a", "b", "migrations"]);

        // Nothing pending: no backup either
        assert_eq!(migrate_with(&db, &[A, B], 10).unwrap(), Outcome::default());
        assert!(!dir.path().join(backups::FOLDER).exists());
    }

    #[test]
    fn an_existing_database_is_backed_up_first() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrate_with(&db, &[A], 10).unwrap();
        let outcome = migrate_with(&db, &[A, B], 10).unwrap();
        assert_eq!(outcome.names, [B.name]);
        let backup = outcome.backup.unwrap();
        assert_eq!(backup.parent().unwrap(), dir.path().join(backups::FOLDER));
        assert_eq!(tables(&backup), ["a", "migrations"]);
    }

    #[test]
    fn rollback_reverts_the_last_ones_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrate_with(&db, &[A, B], 10).unwrap();
        let outcome = rollback_with(&db, &[A, B], 2, 10).unwrap();
        assert_eq!(outcome.names, [B.name, A.name]);
        assert!(outcome.backup.is_some());
        assert_eq!(tables(&db), ["migrations"]);
        assert!(matches!(
            rollback_with(&db, &[A, B], 0, 10),
            Err(Error::Invalid(message)) if message == "Not a number of migrations: 0"
        ));
    }

    #[test]
    fn a_library_from_a_newer_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrate_with(&db, &[A, B], 10).unwrap();
        let error = migrate_with(&db, &[A], 10).unwrap_err();
        assert!(matches!(&error, Error::NewerLibrary(names) if names == &[B.name]));
        assert!(
            error
                .to_string()
                .starts_with("This library was upgraded by a newer version of liquidslunk.")
        );
    }

    #[test]
    fn a_pending_migration_before_the_last_applied_one_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        let late_a = Migration {
            name: "20260103000000_create_a",
            ..A
        };
        let early = Migration {
            name: "20260101000000_early",
            up: nothing,
            down: nothing,
        };
        migrate_with(&db, &[late_a], 10).unwrap();
        let error = migrate_with(&db, &[early, late_a], 10).unwrap_err();
        assert_eq!(
            error.to_string(),
            "Pending migrations sort before the last applied one (20260103000000_create_a): 20260101000000_early. Give them a later timestamp."
        );
    }

    #[test]
    fn a_broken_foreign_key_rolls_everything_back() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        migrate_with(&db, &[A], 10).unwrap();
        let bad = Migration {
            name: "20260103000000_orphan",
            up: orphan,
            down: nothing,
        };
        let error = migrate_with(&db, &[A, B, bad], 10).unwrap_err();
        assert!(
            error
                .to_string()
                .starts_with("Foreign key check failed, rolled back:\n"),
            "{error}"
        );
        assert_eq!(tables(&db), ["a", "migrations"]);
        // The backup is still there
        assert_eq!(
            fs::read_dir(dir.path().join(backups::FOLDER))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    fn status_lists_pending_applied_and_unknown_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        let all = status(&db).unwrap();
        assert!(
            all.iter()
                .all(|row| row.applied_at.is_none() && !row.missing)
        );
        assert_eq!(pending(&db).unwrap().len(), MIGRATIONS.len());

        migrate_with(&db, &[A], 10).unwrap();
        let rows = status(&db).unwrap();
        let a = rows.iter().find(|row| row.name == A.name).unwrap();
        assert!(a.missing && a.applied_at.is_some());
        assert!(matches!(pending(&db), Err(Error::NewerLibrary(_))));
    }
}
