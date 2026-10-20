use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::Result;

/// The database file's name. A library is a folder holding one.
pub const DATABASE_FILE: &str = "slunk.db";

/// How to open SQLite.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// The migration runner turns foreign keys off (backend.md, "Running
    /// migrations").
    pub foreign_keys: bool,
    /// Fail instead of creating a new database.
    pub must_exist: bool,
}

/// Opens SQLite with the app's connection settings (backend.md,
/// "Connection"): foreign keys as asked, WAL, and a 5 second busy timeout.
pub fn open_sqlite(path: &Path, options: Options) -> Result<Connection> {
    let mut flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    if !options.must_exist {
        flags |= OpenFlags::SQLITE_OPEN_CREATE;
    }
    let conn = Connection::open_with_flags(path, flags)?;
    conn.pragma_update(None, "foreign_keys", options.foreign_keys)?;
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get::<_, String>(0))?;
    conn.busy_timeout(Duration::from_secs(5))?;
    Ok(conn)
}

/// The app's connection, with foreign keys on. The database must exist:
/// `migrate` creates it. Dropping it, when it's the last connection to the
/// database, checkpoints the WAL and removes the `-wal` and `-shm` files.
pub fn open(path: &Path) -> Result<Connection> {
    open_sqlite(
        path,
        Options {
            foreign_keys: true,
            must_exist: true,
        },
    )
}

/// A read-only connection, which never creates the database or writes to it.
pub fn open_read_only(path: &Path) -> Result<Connection> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    Ok(Connection::open_with_flags(path, flags)?)
}
