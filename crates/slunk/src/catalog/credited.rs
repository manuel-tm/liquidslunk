//! Artists as credited, on releases and credit lines, and credit lines as
//! they're saved.

use std::collections::{HashMap, HashSet};

use rusqlite::Connection;

use super::{
    AlternateName, Artist, ArtistLink, Credit, CreditInput, CreditedArtistInput, not_found,
};
use crate::languages::script_language;
use crate::repo::artists::{self, NewAlternateName};
use crate::repo::named::{self, Table};
use crate::text::{clean_or_none, clean_text};
use crate::{Error, Result};

/// Resolves the names artists are credited with, in one transaction: blank,
/// or the main name, is the main name; one of the artist's alternate names
/// is that name; anything else is added to them, in the language its script
/// gives away. Names compare exactly, case included. A name it adds is
/// remembered, so two links can be credited with it.
#[derive(Default)]
pub(super) struct CreditedNames {
    known: HashMap<i64, (Artist, Vec<AlternateName>)>,
}

impl CreditedNames {
    /// Loads the artists it doesn't know yet. One that isn't there is
    /// `NotFound`: "There's no artist 12".
    fn load(&mut self, tx: &Connection, ids: &[i64]) -> Result<()> {
        let mut missing: Vec<i64> = Vec::new();
        for &id in ids {
            if !self.known.contains_key(&id) && !missing.contains(&id) {
                missing.push(id);
            }
        }
        if missing.is_empty() {
            return Ok(());
        }
        let mut names = named::alternate_names_of(tx, Table::Artists, &missing)?;
        for artist in named::get_many(tx, Table::Artists, &missing)? {
            let alternate = names.remove(&artist.id).unwrap_or_default();
            self.known.insert(artist.id, (artist, alternate));
        }
        match missing.iter().find(|id| !self.known.contains_key(id)) {
            Some(&absent) => Err(not_found("artist", absent)),
            None => Ok(()),
        }
    }

    /// The links, each with the id of the name it's credited with.
    pub(super) fn resolve(
        &mut self,
        tx: &Connection,
        links: &[CreditedArtistInput],
    ) -> Result<Vec<ArtistLink>> {
        let ids: Vec<i64> = links.iter().map(|link| link.artist_id).collect();
        self.load(tx, &ids)?;
        let mut resolved = Vec::new();
        for link in links {
            let (artist, names) = self
                .known
                .get_mut(&link.artist_id)
                .expect("a loaded artist");
            let printed = clean_or_none(link.credited_as.as_deref());
            let mut credited_name_id = None;
            if let Some(printed) = printed.filter(|printed| *printed != artist.name) {
                let id = match names.iter().find(|name| name.name == printed) {
                    Some(name) => name.id,
                    None => {
                        let name = artists::add_alternate_name(
                            tx,
                            artist.id,
                            &NewAlternateName {
                                language: script_language(&printed).map(String::from),
                                name: printed,
                                real_name: false,
                                note: None,
                            },
                        )?;
                        let id = name.id;
                        names.push(name);
                        id
                    }
                };
                credited_name_id = Some(id);
            }
            resolved.push(ArtistLink {
                artist_id: link.artist_id,
                credited_name_id,
            });
        }
        Ok(resolved)
    }

    /// Credit lines as they're saved, with their artists' names resolved.
    pub(super) fn lines(
        &mut self,
        tx: &Connection,
        credits: &[CreditInput],
    ) -> Result<Vec<Credit>> {
        credit_lines(credits)?
            .into_iter()
            .map(|(role, artists)| {
                Ok(Credit {
                    role,
                    artists: self.resolve(tx, &artists)?,
                })
            })
            .collect()
    }
}

/// Each artist once, as first given: an artist appears once per list.
pub(super) fn first_of_each(links: &[CreditedArtistInput]) -> Vec<CreditedArtistInput> {
    let mut seen = HashSet::new();
    links
        .iter()
        .filter(|link| seen.insert(link.artist_id))
        .cloned()
        .collect()
}

/// Credit lines as they're saved: roles cleaned, and an artist given twice on a
/// line kept once, with the first name it's credited with. Refuses an empty
/// role, a line with no artists, and two lines with the same role, ignoring
/// case.
fn credit_lines(credits: &[CreditInput]) -> Result<Vec<(String, Vec<CreditedArtistInput>)>> {
    let mut roles = HashSet::new();
    credits
        .iter()
        .map(|credit| {
            let role = clean_text(&credit.role);
            if role.is_empty() {
                return Err(Error::Invalid("A credit needs a role".into()));
            }
            if credit.artists.is_empty() {
                return Err(Error::Invalid(format!(
                    "The {role} credit needs an artist"
                )));
            }
            if !roles.insert(role.to_ascii_lowercase()) {
                return Err(Error::Invalid(format!(
                    "Two credits have the role {role}, ignoring case. Put their artists on one line."
                )));
            }
            Ok((role, first_of_each(&credit.artists)))
        })
        .collect()
}
