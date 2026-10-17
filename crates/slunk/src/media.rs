//! The media context, as far as phase 0 needs it: where a registered file is
//! on disk, for serving its bytes and opening it. It never writes to the
//! user's files. Phase 1 ports the rest of slunk's media context.

use std::path::{Path, PathBuf};

use rusqlite::Connection;

use crate::Result;
use crate::repo::files;

/// A registered file's absolute path, or `None` for an unknown id. The file
/// may not be there.
pub fn file_path(conn: &Connection, library: &Path, id: i64) -> Result<Option<PathBuf>> {
    Ok(files::path(conn, id)?.map(|path| absolute(library, &path)))
}

/// A path relative to the library's folder, made absolute lexically, without
/// touching the disk, as Node's `path.resolve()` does: `..` goes up, even
/// above the library's folder.
pub fn absolute(library: &Path, path: &str) -> PathBuf {
    let mut absolute = library.to_path_buf();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                absolute.pop();
            }
            part => absolute.push(part),
        }
    }
    absolute
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_resolve_lexically() {
        let library = Path::new("/Users/me/Music");
        assert_eq!(
            absolute(library, "Ryo Fukui/01 Early Summer.flac"),
            Path::new("/Users/me/Music/Ryo Fukui/01 Early Summer.flac")
        );
        assert_eq!(
            absolute(library, "../Other/./a/../notes.txt"),
            Path::new("/Users/me/Other/notes.txt")
        );
        assert_eq!(absolute(library, ""), library);
    }
}
