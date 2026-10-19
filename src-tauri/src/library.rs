//! Opening, creating, switching and closing libraries (design §6). A library
//! is a folder holding a `slunk.db`. The open one is kept in Tauri's state,
//! with the app's one connection to it and the contexts wired to it.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::ErrorCode;
use serde::Serialize;
use slunk::catalog::Catalog;
use slunk::library_check::LibraryCheck;
use slunk::media::Media;
use slunk::repo::connection::{self, DATABASE_FILE};
use slunk::repo::migrator;
use slunk::repo::store::Store;
use slunk::tagging::Tagging;
use specta::Type;

use crate::error::CommandError;
use crate::settings::{self, Settings};

/// The open library.
pub struct Library {
    /// Its database, as an absolute path.
    pub db: PathBuf,
    /// The database's folder, which file paths are relative to.
    pub folder: PathBuf,
    /// The app's one connection (design §5.4), which the contexts share.
    pub store: Arc<Store>,
    pub media: Arc<Media>,
    pub catalog: Arc<Catalog>,
    pub tagging: Tagging,
    pub library_check: LibraryCheck,
}

impl Library {
    /// The library at `db`, an absolute path, over the app's connection to
    /// it, with its contexts wired as slunk's `server.ts` wired them.
    pub fn new(db: &Path, conn: rusqlite::Connection) -> Library {
        let store = Arc::new(Store::new(conn));
        let media = Arc::new(Media::new(store.clone(), slunk::media::Library::new(db)));
        let catalog = Arc::new(Catalog::new(store.clone(), media.clone()));
        Library {
            db: db.to_path_buf(),
            folder: folder_of(db),
            library_check: LibraryCheck::new(media.clone(), catalog.clone()),
            catalog,
            tagging: Tagging::new(store.clone()),
            media,
            store,
        }
    }

    fn info(&self) -> Result<LibraryInfo, CommandError> {
        let migration = migrator::latest_migration(&self.store.conn())?;
        Ok(LibraryInfo {
            path: shown(&self.db),
            folder: shown(&self.folder),
            name: name_of(&self.folder),
            migration,
        })
    }
}

/// A library, as the frontend shows it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryInfo {
    /// Its database, as an absolute path.
    pub path: String,
    /// The database's folder.
    pub folder: String,
    /// The folder's name, which names the library.
    pub name: String,
    /// The latest applied migration.
    pub migration: Option<String>,
}

/// What `library_status` says, which replaces slunk's `health` query.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    /// The open library, if one is.
    pub library: Option<LibraryInfo>,
    /// The library the settings open at launch, if any.
    pub saved: Option<SavedLibrary>,
    /// In debug builds, the scratch libraries' folder, `dev-library/`, where
    /// development libraries belong (design §6.5).
    pub dev_library: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedLibrary {
    pub path: String,
    /// Whether it's there right now. A library on an external drive often
    /// isn't.
    pub found: bool,
}

/// A library opened recently.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentLibrary {
    pub path: String,
    pub folder: String,
    pub name: String,
    pub found: bool,
}

/// What opening a library did, for a toast.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub library: LibraryInfo,
    pub migration: MigrationReport,
}

/// The migrations opening a library applied, and the backup made first.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MigrationReport {
    pub backup: Option<String>,
    pub applied: Vec<String>,
    /// Old backups removed, beyond the number to keep.
    pub removed: Vec<String>,
}

/// What creating a library did.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Created {
    Created {
        library: LibraryInfo,
        migration: MigrationReport,
    },
    /// The folder has a library already: the frontend offers to open it.
    Exists { path: String },
}

/// What opening a library reports while it works.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OpenProgress {
    /// It has migrations pending, so it's being backed up and upgraded.
    Migrating { pending: Vec<String> },
}

/// The libraries: the open one, and the settings that remember them.
pub struct Libraries {
    settings_path: PathBuf,
    settings: Mutex<Settings>,
    open: Mutex<Option<Arc<Library>>>,
    /// Held while a library opens or closes, so two can't interleave.
    switching: Mutex<()>,
}

impl Libraries {
    /// The libraries, with the settings in the app's config folder.
    pub fn new(config_folder: &Path) -> Self {
        let settings_path = config_folder.join(settings::FILE);
        Self {
            settings: Mutex::new(Settings::load(&settings_path)),
            settings_path,
            open: Mutex::new(None),
            switching: Mutex::new(()),
        }
    }

    /// The open library, for commands that need one.
    pub fn library(&self) -> Result<Arc<Library>, CommandError> {
        lock(&self.open)
            .clone()
            .ok_or_else(CommandError::no_library)
    }

    pub fn status(&self) -> Result<LibraryStatus, CommandError> {
        let library = lock(&self.open).clone();
        let saved = lock(&self.settings).library.clone();
        Ok(LibraryStatus {
            library: library.map(|library| library.info()).transpose()?,
            saved: saved.map(|path| SavedLibrary {
                found: path.is_file(),
                path: shown(&path),
            }),
            dev_library: dev_library().map(|folder| shown(&folder)),
        })
    }

    pub fn recent(&self) -> Vec<RecentLibrary> {
        lock(&self.settings)
            .recent
            .iter()
            .map(|db| {
                let folder = folder_of(db);
                RecentLibrary {
                    path: shown(db),
                    name: name_of(&folder),
                    folder: shown(&folder),
                    found: db.is_file(),
                }
            })
            .collect()
    }

    /// Opens an existing library (design §6.3), closing the open one once the
    /// new one is ready. With `remember`, the settings open it at launch.
    pub fn open(
        &self,
        db: &Path,
        remember: bool,
        progress: impl Fn(OpenProgress),
    ) -> Result<Opened, CommandError> {
        let _switching = lock(&self.switching);
        let db = absolute(db)?;
        check_library(&db)?;
        self.open_checked(&db, remember, progress)
    }

    /// Creates `slunk.db` in a folder, migrates it and opens it, unless the
    /// folder has one already.
    pub fn create(
        &self,
        folder: &Path,
        progress: impl Fn(OpenProgress),
    ) -> Result<Created, CommandError> {
        let _switching = lock(&self.switching);
        let folder = absolute(folder)?;
        check_folder(&folder)?;
        let db = folder.join(DATABASE_FILE);
        if db.exists() {
            return Ok(Created::Exists { path: shown(&db) });
        }
        let Opened { library, migration } = self.open_checked(&db, true, progress)?;
        Ok(Created::Created { library, migration })
    }

    /// Closes the open library: its connection closes, which checkpoints the
    /// WAL and removes the `-wal` and `-shm` files. The settings still open it
    /// at the next launch.
    pub fn close(&self) {
        let _switching = lock(&self.switching);
        self.close_open();
    }

    fn close_open(&self) {
        lock(&self.open).take();
    }

    fn open_checked(
        &self,
        db: &Path,
        remember: bool,
        progress: impl Fn(OpenProgress),
    ) -> Result<Opened, CommandError> {
        // Migrate, with its backup
        let keep_backups = lock(&self.settings).keep_backups;
        let pending = migrator::pending(db).map_err(opening_error)?;
        if !pending.is_empty() {
            progress(OpenProgress::Migrating { pending });
        }
        let outcome = migrator::migrate(db, keep_backups).map_err(opening_error)?;

        // Open the app's connection, in place of the open library's
        self.close_open();
        let conn = connection::open(db).map_err(opening_error)?;
        let library = Arc::new(Library::new(db, conn));
        *lock(&self.open) = Some(library.clone());

        // Save the choice, now that it opened
        if remember {
            let mut settings = lock(&self.settings);
            settings.remember(db);
            if let Err(error) = settings.save(&self.settings_path) {
                eprintln!("Can't save {}: {error}", self.settings_path.display());
            }
        }

        Ok(Opened {
            library: library.info()?,
            migration: MigrationReport {
                backup: outcome.backup.as_deref().map(shown),
                applied: outcome.names,
                removed: outcome.removed.iter().map(|path| shown(path)).collect(),
            },
        })
    }
}

/// An existing library: an SQLite database with the migration runner's
/// table, or an empty one (design §6.3, step 2).
fn check_library(db: &Path) -> Result<(), CommandError> {
    if !db.exists() {
        return Err(CommandError::invalid(format!(
            "Couldn't find {}.",
            db.display()
        )));
    }
    let not_a_library = || CommandError::invalid("That isn't a Slunk library.");
    if !db.is_file() {
        return Err(not_a_library());
    }
    let mut header = [0; 16];
    let mut file = File::open(db).map_err(|error| CommandError::invalid(error.to_string()))?;
    let length = file
        .metadata()
        .map_err(|error| CommandError::invalid(error.to_string()))?
        .len();
    if length == 0 {
        return Ok(());
    }
    if file.read_exact(&mut header).is_err() || &header != b"SQLite format 3\0" {
        return Err(not_a_library());
    }
    let conn = connection::open_read_only(db).map_err(opening_error)?;
    let tables: i64 = conn
        .query_row(
            "select count(*) from sqlite_schema where type = 'table'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| opening_error(error.into()))?;
    if tables > 0 && !migrator::has_migrations_table(&conn).map_err(opening_error)? {
        return Err(not_a_library());
    }
    Ok(())
}

/// A folder to create a library in: it exists, and can be written (design
/// §6.3, step 1).
fn check_folder(folder: &Path) -> Result<(), CommandError> {
    if !folder.is_dir() {
        return Err(CommandError::invalid(format!(
            "There's no folder at {}.",
            folder.display()
        )));
    }
    if !writable(folder) {
        return Err(CommandError::invalid(format!(
            "liquidslunk can't write to {}.",
            folder.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn writable(folder: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(path) = CString::new(folder.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: access() only reads the NUL-terminated path it's given.
    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
}

#[cfg(not(unix))]
fn writable(folder: &Path) -> bool {
    folder
        .metadata()
        .is_ok_and(|metadata| !metadata.permissions().readonly())
}

/// A failure while opening, in words Welcome shows. SQLite's WAL doesn't work
/// on network file systems, so a library there fails with an I/O or locking
/// error (design §10).
fn opening_error(error: slunk::Error) -> CommandError {
    let network = matches!(
        &error,
        slunk::Error::Sqlite(sqlite) if matches!(
            sqlite.sqlite_error_code(),
            Some(ErrorCode::SystemIoFailure | ErrorCode::FileLockingProtocolFailed | ErrorCode::CannotOpen)
        )
    );
    if let slunk::Error::Sqlite(sqlite) = &error
        && sqlite.sqlite_error_code() == Some(ErrorCode::NotADatabase)
    {
        return CommandError::invalid("That isn't a Slunk library.");
    }
    let error = CommandError::from(error);
    let message = if network {
        format!(
            "Couldn't open the library: {}. SQLite can't keep a library on a network drive, so if it's on one, move it to a disk of this computer.",
            error.message
        )
    } else {
        error.message
    };
    CommandError::invalid(message)
}

fn absolute(path: &Path) -> Result<PathBuf, CommandError> {
    std::path::absolute(path).map_err(|error| CommandError::invalid(error.to_string()))
}

fn folder_of(db: &Path) -> PathBuf {
    db.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn name_of(folder: &Path) -> String {
    folder
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| shown(folder))
}

fn shown(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// In debug builds, the repository's `dev-library/`.
fn dev_library() -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../dev-library")
            .canonicalize()
            .ok()
    } else {
        None
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn only_slunk_libraries_and_empty_databases_open() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.db");
        let error = check_library(&missing).unwrap_err();
        assert!(error.message.starts_with("Couldn't find "), "{error}");

        let text = dir.path().join("notes.db");
        fs::write(&text, "not a database, but long enough to have a header").unwrap();
        assert_eq!(
            check_library(&text).unwrap_err().message,
            "That isn't a Slunk library."
        );

        let other = dir.path().join("other.db");
        rusqlite::Connection::open(&other)
            .unwrap()
            .execute_batch("create table things (id integer primary key)")
            .unwrap();
        assert_eq!(
            check_library(&other).unwrap_err().message,
            "That isn't a Slunk library."
        );

        let empty = dir.path().join("empty.db");
        File::create(&empty).unwrap();
        check_library(&empty).unwrap();

        let library = dir.path().join(DATABASE_FILE);
        migrator::migrate(&library, 10).unwrap();
        check_library(&library).unwrap();
    }

    #[test]
    fn a_library_is_created_in_a_folder_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        check_folder(dir.path()).unwrap();
        let error = check_folder(&dir.path().join("nope")).unwrap_err();
        assert!(
            error.message.starts_with("There's no folder at "),
            "{error}"
        );
    }
}
