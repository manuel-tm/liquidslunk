use std::io;
use std::path::{Path, PathBuf};

/// What can go wrong in the domain and the repository layer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A message for the user, like slunk's plain `Error`s ("Not a number of
    /// migrations: 0").
    #[error("{0}")]
    Invalid(String),

    /// What was asked for isn't there ("There's no file 12").
    #[error("{0}")]
    NotFound(String),

    /// The library has applied migrations this version doesn't know
    /// (backend.md, "Running migrations").
    #[error(
        "This library was upgraded by a newer version of liquidslunk. It has migrations this version doesn't know: {}",
        .0.join(", ")
    )]
    NewerLibrary(Vec<String>),

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error("{}: {source}", path.display())]
    Io { path: PathBuf, source: io::Error },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// An I/O error, with the path it happened on.
    pub fn io(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
        move |source| Error::Io {
            path: path.to_path_buf(),
            source,
        }
    }
}
