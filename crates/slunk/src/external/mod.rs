//! The external libraries context (4_external_libraries.md): a selection of
//! the library, converted, tagged and given covers from the catalog, for a
//! folder or an iPod. So far it has the mapping (phase 0): a track's tags,
//! and its path in a folder library. Nothing writes them yet.
//!
//! - `tags`: a track's tags, worked out from the catalog (§4).
//! - `template`: path templates, and names on disk (§6).

use std::collections::HashMap;
use std::sync::Arc;

use crate::Result;
use crate::media::paths;
use crate::repo::store::Store;
use crate::repo::{files, tracks};

mod tags;
mod template;

pub use tags::{SortFields, Tags};
pub use template::{DEFAULT_TEMPLATE, Template, TrackPath, safe_name, unique_paths};

/// A track's tags, and its path in a folder library that keeps the original
/// files: what the track page shows of it (phase 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exported {
    pub tags: Tags,
    /// Relative to the folder library
    pub path: String,
}

pub struct External {
    store: Arc<Store>,
}

impl External {
    pub fn new(store: Arc<Store>) -> External {
        External { store }
    }

    /// Each track's tags, by id. Ids that aren't tracks are left out.
    pub fn tags_of(&self, ids: &[i64]) -> Result<HashMap<i64, Tags>> {
        tags::read(&self.store.conn(), ids)
    }

    /// A track's tags, and its path by a template in a folder library that
    /// keeps the original files, so its file keeps its extension, in
    /// lowercase. `None` for a track that isn't there.
    pub fn exported(&self, id: i64, template: &Template) -> Result<Option<Exported>> {
        let conn = self.store.conn();
        let Some(track) = tracks::get(&conn, id)? else {
            return Ok(None);
        };
        let Some(tags) = tags::read(&conn, &[id])?.remove(&id) else {
            return Ok(None);
        };
        let file = files::path(&conn, track.file_id)?.unwrap_or_default();
        let extension = paths::extname(&file).trim_start_matches('.').to_lowercase();
        Ok(Some(Exported {
            path: template.path(&tags, &extension).safe(),
            tags,
        }))
    }
}
