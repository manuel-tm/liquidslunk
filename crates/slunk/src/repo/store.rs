//! The open library's one connection, behind a mutex, as slunk had one
//! better-sqlite3 connection (backend.md, "Structure").

use std::sync::{Mutex, MutexGuard};

use rusqlite::{Connection, Transaction, TransactionBehavior};

use crate::Result;

pub struct Store {
    conn: Mutex<Connection>,
}

impl Store {
    pub fn new(conn: Connection) -> Store {
        Store {
            conn: Mutex::new(conn),
        }
    }

    /// The connection. Read files before taking it, never while holding it.
    pub fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Runs `work` in a transaction, which commits if it succeeds and rolls
    /// back if it fails. A context that writes inside another's transaction
    /// takes the `Transaction`, which derefs to a `Connection`, so there's
    /// no nesting. It starts with `BEGIN IMMEDIATE`, which takes the write
    /// lock at once: another process writing the library, like the database
    /// commands, makes it wait for the lock rather than fail midway.
    pub fn transaction<T>(&self, work: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let value = work(&tx)?;
        tx.commit()?;
        Ok(value)
    }

    /// Runs `work` in a read transaction, so what it reads is one
    /// consistent picture of the library, whatever another process writes
    /// meanwhile. It starts with a plain `BEGIN`, which takes no lock until
    /// the first read.
    pub fn read<T>(&self, work: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Deferred)?;
        let value = work(&tx)?;
        tx.commit()?;
        Ok(value)
    }
}
