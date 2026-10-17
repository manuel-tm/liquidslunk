//! The open library's one connection, behind a mutex (design §5.4), as slunk
//! had one better-sqlite3 connection.

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
    /// lock at once, since slunk's backend writes the same database until
    /// phase 6.
    pub fn transaction<T>(&self, work: impl FnOnce(&Transaction<'_>) -> Result<T>) -> Result<T> {
        let mut conn = self.conn();
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let value = work(&tx)?;
        tx.commit()?;
        Ok(value)
    }
}
