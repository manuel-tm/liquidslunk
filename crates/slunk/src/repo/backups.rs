//! Backups of a library, in `slunk-backups/` next to its database
//! (design §5.6). The migration runner backs up before it changes anything,
//! then removes the oldest backups beyond the number to keep.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::Connection;

use super::connection;
use crate::{Error, Result};

/// The backups' folder, next to the database.
pub const FOLDER: &str = "slunk-backups";

/// How many backups to keep when the settings don't say (`keepBackups`).
pub const DEFAULT_KEEP: u32 = 10;

/// A backup that was made, and the old ones that removed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub path: PathBuf,
    pub removed: Vec<PathBuf>,
}

/// Copies the database into `slunk-backups/`, then removes all but the newest
/// `keep` backups. The database must exist.
pub fn backup(db_path: &Path, keep: u32) -> Result<Backup> {
    let conn = connection::open_sqlite(
        db_path,
        connection::Options {
            foreign_keys: true,
            must_exist: true,
        },
    )?;
    backup_into(&conn, db_path, keep)
}

/// [`backup`], over a connection the caller already has open.
pub(crate) fn backup_into(conn: &Connection, db_path: &Path, keep: u32) -> Result<Backup> {
    backup_at(conn, db_path, keep, SystemTime::now())
}

fn backup_at(conn: &Connection, db_path: &Path, keep: u32, now: SystemTime) -> Result<Backup> {
    let dir = db_path.parent().unwrap_or(Path::new(".")).join(FOLDER);
    fs::create_dir_all(&dir).map_err(Error::io(&dir))?;
    let stamp = timestamp(now);
    let mut path = dir.join(format!("slunk-{stamp}.db"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("slunk-{stamp}-{n}.db"));
        n += 1;
    }
    let target = path
        .to_str()
        .ok_or_else(|| Error::Invalid(format!("Can't back up to {}", path.display())))?;
    conn.execute("vacuum into ?", [target])?;
    let removed = prune(&dir, keep)?;
    Ok(Backup { path, removed })
}

/// Removes all but the newest `keep` backups. Only files named like this
/// runner's backups are touched; anything else in the folder stays.
fn prune(dir: &Path, keep: u32) -> Result<Vec<PathBuf>> {
    let mut backups = Vec::new();
    for entry in fs::read_dir(dir).map_err(Error::io(dir))? {
        let entry = entry.map_err(Error::io(dir))?;
        if let Some(order) = entry.file_name().to_str().and_then(backup_order) {
            backups.push((order, entry.path()));
        }
    }
    // Oldest first. `-2` comes after the plain name of the same second.
    backups.sort();
    let old = backups.len().saturating_sub(keep as usize);
    let mut removed = Vec::new();
    for (_, path) in backups.into_iter().take(old) {
        fs::remove_file(&path).map_err(Error::io(&path))?;
        removed.push(path);
    }
    Ok(removed)
}

/// Where a backup's name sorts, `slunk-<timestamp>[-<n>].db`, or `None` for
/// any other name.
fn backup_order(name: &str) -> Option<(String, u64)> {
    let rest = name.strip_prefix("slunk-")?.strip_suffix(".db")?;
    let (stamp, suffix) = rest.split_at_checked(14)?;
    if !stamp.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = match suffix.strip_prefix('-') {
        None if suffix.is_empty() => 1,
        Some(n) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => n.parse().ok()?,
        _ => return None,
    };
    Some((stamp.to_string(), n))
}

/// `YYYYMMDDHHMMSS` in UTC, the timestamp in backup and migration names.
pub fn timestamp(time: SystemTime) -> String {
    let (year, month, day, hour, minute, second) = utc(time);
    format!("{year:04}{month:02}{day:02}{hour:02}{minute:02}{second:02}")
}

/// An instant as Unix epoch seconds, in ISO 8601 UTC, as JavaScript's
/// `toISOString()` writes it: `2026-10-11T14:19:55.000Z`.
pub fn iso_8601(epoch_seconds: i64) -> String {
    let (year, month, day, hour, minute, second) = civil(epoch_seconds);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.000Z")
}

fn utc(time: SystemTime) -> (i64, u32, u32, u32, u32, u32) {
    let seconds = match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(since) => since.as_secs() as i64,
        Err(before) => -(before.duration().as_secs() as i64),
    };
    civil(seconds)
}

/// The UTC date and time of an epoch second, by Howard Hinnant's
/// `civil_from_days`.
fn civil(seconds: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = seconds.div_euclid(86_400);
    let time = seconds.rem_euclid(86_400) as u32;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day, time / 3600, time / 60 % 60, time % 60)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    #[test]
    fn timestamps_are_utc() {
        assert_eq!(timestamp(at(0)), "19700101000000");
        assert_eq!(timestamp(at(1_792_234_179)), "20261017104939");
        // A leap day
        assert_eq!(timestamp(at(951_782_400)), "20000229000000");
        assert_eq!(iso_8601(1_792_234_179), "2026-10-17T10:49:39.000Z");
    }

    #[test]
    fn only_backup_names_have_an_order() {
        assert_eq!(
            backup_order("slunk-20261013063235.db"),
            Some(("20261013063235".into(), 1))
        );
        assert_eq!(
            backup_order("slunk-20261013063235-12.db"),
            Some(("20261013063235".into(), 12))
        );
        for name in [
            "slunk.db",
            "slunk-2026101306323.db",
            "slunk-20261013063235-.db",
            "slunk-20261013063235-x.db",
            "slunk-20261013063235.db.bak",
            "notes.txt",
        ] {
            assert_eq!(backup_order(name), None, "{name}");
        }
    }

    #[test]
    fn backups_keep_the_newest_and_nothing_else_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("slunk.db");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("create table t (x); insert into t values (1);")
            .unwrap();
        let folder = dir.path().join(FOLDER);
        fs::create_dir_all(&folder).unwrap();
        for name in [
            "slunk-20200101000000.db",
            "slunk-20200101000000-2.db",
            "notes.txt",
        ] {
            fs::write(folder.join(name), "old").unwrap();
        }

        let first = backup_at(&conn, &db, 2, at(1_792_234_179)).unwrap();
        assert_eq!(first.path, folder.join("slunk-20261017104939.db"));
        assert_eq!(first.removed, vec![folder.join("slunk-20200101000000.db")]);
        let copy = Connection::open(&first.path).unwrap();
        let x: i64 = copy
            .query_row("select x from t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(x, 1);

        // Two backups in the same second get a suffix
        let second = backup_at(&conn, &db, 2, at(1_792_234_179)).unwrap();
        assert_eq!(second.path, folder.join("slunk-20261017104939-2.db"));
        assert_eq!(
            second.removed,
            vec![folder.join("slunk-20200101000000-2.db")]
        );
        let mut left: Vec<_> = fs::read_dir(&folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "notes.txt",
                "slunk-20261017104939-2.db",
                "slunk-20261017104939.db"
            ]
        );
    }
}
