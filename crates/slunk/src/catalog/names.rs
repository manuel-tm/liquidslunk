//! Alternate names, saved after their artist's or label's main name, and the
//! links credited with them.

use std::collections::HashSet;

use rusqlite::Connection;

use super::{
    AlternateName, AlternateNameInput, AlternateNameValues, Artist, Label, NamedKind, NamedValues,
};
use crate::languages::language_tag;
use crate::repo::artists::{self, NewAlternateName};
use crate::repo::named::{self, Table};
use crate::text::{clean_or_none, clean_text};
use crate::{Error, Result};

/// An artist's or label's own columns, as they're saved.
pub(super) fn named_values(
    name: &str,
    name_language: Option<&str>,
    picture: Option<i64>,
) -> Result<NamedValues> {
    Ok(NamedValues {
        name: clean_text(name),
        name_language: language_tag(name_language)?,
        picture,
    })
}

/// Saves an artist's alternate names, after its main name. Links keep showing
/// what they showed:
/// - with `keep_old_name`, a rename credits the links that showed the old
///   main name with it, as an alternate name;
/// - a name that goes leaves its links showing the main name, and so does a
///   real name that's now the main name.
///
/// Left out, the names stay as they are, except one that's now the main
/// name: it's promoted, and goes unless it's a real name.
pub(super) fn save_artist_names(
    tx: &Connection,
    before: Option<&Artist>,
    artist: &Artist,
    input: Option<&[AlternateNameInput]>,
    keep_old_name: bool,
) -> Result<()> {
    let id = artist.id;
    let names_now = || -> Result<Vec<AlternateName>> {
        Ok(named::alternate_names_of(tx, Table::Artists, &[id])?
            .remove(&id)
            .unwrap_or_default())
    };
    let mut existing = names_now()?;

    // Before anything else, so links whose name goes below don't get it
    let mut kept: Option<AlternateName> = None;
    if let Some(before) = before
        && keep_old_name
        && before.name != artist.name
        && artists::main_name_uses(tx, id)? > 0
    {
        let name = match existing.iter().find(|name| name.name == before.name) {
            Some(name) => name.clone(),
            None => artists::add_alternate_name(
                tx,
                id,
                &NewAlternateName {
                    name: before.name.clone(),
                    language: before.name_language.clone(),
                    real_name: false,
                    note: None,
                },
            )?,
        };
        artists::credit_main_name_links(tx, id, name.id)?;
        kept = Some(name);
        existing = names_now()?;
    }

    let mut list: Vec<AlternateNameValues> = match input {
        Some(input) => alternate_name_rows(NamedKind::Artist, &artist.name, input)?,
        None => existing
            .iter()
            .filter(|name| name.name != artist.name || name.real_name)
            .map(AlternateNameValues::from)
            .collect(),
    };
    let mut claimed: HashSet<i64> = list.iter().filter_map(|name| name.id).collect();
    // A new name the artist already has keeps its row, and so its links
    for name in &mut list {
        if name.id.is_some() {
            continue;
        }
        if let Some(same) = existing
            .iter()
            .find(|other| other.name == name.name && !claimed.contains(&other.id))
        {
            claimed.insert(same.id);
            name.id = Some(same.id);
        }
    }
    if let Some(kept) = kept
        && !claimed.contains(&kept.id)
    {
        claimed.insert(kept.id);
        list.push(AlternateNameValues::from(&kept));
    }

    let going = existing
        .iter()
        .map(|name| name.id)
        .filter(|name_id| !claimed.contains(name_id));
    let now_main = list
        .iter()
        .filter(|name| name.name == artist.name)
        .filter_map(|name| name.id);
    let uncredited: Vec<i64> = going.chain(now_main).collect();
    artists::uncredit(tx, id, &uncredited)?;
    named::set_alternate_names(tx, Table::Artists, id, &list)
}

/// Saves a label's alternate names, after its main name. Left out, they stay
/// as they are, except one that's now the main name.
pub(super) fn save_label_names(
    tx: &Connection,
    label: &Label,
    input: Option<&[AlternateNameInput]>,
) -> Result<()> {
    let list = match input {
        Some(input) => alternate_name_rows(NamedKind::Label, &label.name, input)?,
        None => named::alternate_names_of(tx, Table::Labels, &[label.id])?
            .remove(&label.id)
            .unwrap_or_default()
            .iter()
            .filter(|name| name.name != label.name)
            .map(AlternateNameValues::from)
            .collect(),
    };
    named::set_alternate_names(tx, Table::Labels, label.id, &list)
}

/// Alternate names as they're saved: cleaned, a blank one left out, and
/// languages in canonical form. Refuses a name given twice, and one that's the
/// main name, unless it's a real name: an artist who performs under their real
/// name can still record it. Labels have no real names. Names compare exactly,
/// case included, so "ALIKA" isn't "Alika".
fn alternate_name_rows(
    what: NamedKind,
    main: &str,
    names: &[AlternateNameInput],
) -> Result<Vec<AlternateNameValues>> {
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for input in names {
        let name = clean_text(&input.name);
        if name.is_empty() {
            continue;
        }
        let real_name = what == NamedKind::Artist && input.real_name;
        if !seen.insert(name.clone()) {
            return Err(Error::Invalid(format!("\"{name}\" is listed twice")));
        }
        if name == main && !real_name {
            return Err(Error::Invalid(format!(
                "\"{name}\" is already the {}'s main name",
                what.as_str()
            )));
        }
        rows.push(AlternateNameValues {
            id: input.id,
            language: language_tag(input.language.as_deref())?,
            name,
            real_name,
            note: clean_or_none(input.note.as_deref()),
        });
    }
    Ok(rows)
}
