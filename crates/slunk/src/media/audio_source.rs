//! What the audio route serves of a file (backend.md, "Serving FLAC as
//! MP4"): a FLAC file as an MP4 that holds its frames, and any other file,
//! or a FLAC file whose frames can't be found, as it is.
//!
//! Each MP4's header is made once, from one pass over the file, and kept in
//! memory for the last files served, never on disk.

use std::collections::VecDeque;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::SystemTime;

use super::audio::flac::FlacError;
use super::audio::mp4::FlacMp4;

/// What's served for a file.
#[derive(Debug, Clone)]
pub enum AudioSource {
    /// A FLAC file, as an MP4
    Mp4(Arc<FlacMp4>),
    /// The file as it is. It may be missing.
    File(PathBuf),
}

/// How many files' MP4s are kept: the one playing, the one loaded ahead, and
/// those played or previewed before them.
const KEPT: usize = 16;

/// A file's MP4, once it's made, or `None` when the file isn't served as
/// one. Whoever asks first makes it, and anyone who asks meanwhile waits.
type Made = Arc<OnceLock<Option<Arc<FlacMp4>>>>;

/// The MP4s of the files served last, most recent first.
#[derive(Default)]
pub(crate) struct Mp4s {
    kept: Mutex<VecDeque<Kept>>,
}

/// A file's MP4, as the file was when it was made.
struct Kept {
    id: i64,
    path: PathBuf,
    length: u64,
    modified: Option<SystemTime>,
    made: Made,
}

impl Mp4s {
    /// What's served for file `id`, at `path`. A FLAC file's MP4 is made the
    /// first time, and when the file has changed since. When its frames
    /// can't be found, `failed` hears why, once, and it's served as it is.
    pub fn source(
        &self,
        id: i64,
        path: PathBuf,
        failed: impl FnOnce(&Path, &FlacError),
    ) -> AudioSource {
        let Ok(metadata) = path.metadata() else {
            return AudioSource::File(path);
        };
        let made = self.made(id, &path, &metadata);
        let mp4 = made.get_or_init(|| match FlacMp4::new(&path) {
            Ok(mp4) => Some(Arc::new(mp4)),
            Err(FlacError::NotFlac) => None,
            Err(error) => {
                failed(&path, &error);
                None
            }
        });
        match mp4 {
            Some(mp4) => AudioSource::Mp4(mp4.clone()),
            None => AudioSource::File(path),
        }
    }

    /// The file's MP4 as it's kept, or a new one to make when it isn't, or
    /// the file has changed since.
    fn made(&self, id: i64, path: &Path, metadata: &Metadata) -> Made {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        let length = metadata.len();
        let modified = metadata.modified().ok();
        if let Some(index) = kept.iter().position(|kept| kept.id == id)
            && let Some(found) = kept.remove(index)
            && found.path == path
            && found.length == length
            && found.modified == modified
        {
            let made = found.made.clone();
            kept.push_front(found);
            return made;
        }
        let made = Made::default();
        kept.push_front(Kept {
            id,
            path: path.to_path_buf(),
            length,
            modified,
            made: made.clone(),
        });
        kept.truncate(KEPT);
        made
    }
}
