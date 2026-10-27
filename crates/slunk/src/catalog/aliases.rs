//! Artist aliases, saved with their artist: the artist and the aliases given
//! become one group (backend.md, "The catalog context").

use std::collections::HashSet;

use rusqlite::Connection;

use super::{Artist, not_found};
use crate::repo::aliases;
use crate::repo::named::{self, Table};
use crate::{Error, Result};

/// The artist's aliases, by name ignoring case, then by id.
pub(super) fn aliases_of(conn: &Connection, artist_id: i64) -> Result<Vec<Artist>> {
    aliases::of(conn, artist_id)
}

/// Makes the artist and `alias_ids` one group, under a new number. Everyone
/// else keeps their group: the artist's old aliases that are left out stay
/// aliases of each other, and so do the other aliases of an artist given
/// without them. An empty list takes the artist out of its group, and one
/// left out (`None`) leaves it as it is.
pub(super) fn save_aliases(
    tx: &Connection,
    artist_id: i64,
    alias_ids: Option<&[i64]>,
) -> Result<()> {
    let Some(alias_ids) = alias_ids else {
        return Ok(());
    };
    let mut seen = HashSet::new();
    let ids: Vec<i64> = alias_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    if ids.contains(&artist_id) {
        return Err(Error::Invalid("An artist can't be its own alias".into()));
    }
    let found: HashSet<i64> = named::get_many(tx, Table::Artists, &ids)?
        .into_iter()
        .map(|artist| artist.id)
        .collect();
    if let Some(&missing) = ids.iter().find(|id| !found.contains(id)) {
        return Err(not_found("artist", missing));
    }
    let group: Vec<i64> = std::iter::once(artist_id).chain(ids).collect();
    aliases::remove(tx, &group)?;
    if group.len() > 1 {
        aliases::insert_group(tx, &group)?;
    }
    Ok(())
}
