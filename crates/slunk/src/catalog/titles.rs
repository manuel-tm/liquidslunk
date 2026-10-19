//! Alternate titles, saved after their release's, track's or disc's main title.

use std::collections::HashSet;

use rusqlite::Connection;

use super::{AlternateTitle, AlternateTitleInput};
use crate::languages::language_tag;
use crate::repo::titles::{self, TitleOwner};
use crate::text::clean_text;
use crate::{Error, Result};

/// Alternate titles as they're saved: cleaned, a blank one left out, and
/// languages in canonical form. Refuses a title given twice, and one that's
/// the main title. Titles compare exactly, case included. `what` names the
/// owner in the errors: "the release", "track 3".
pub(super) fn alternate_title_rows(
    main: &str,
    titles: &[AlternateTitleInput],
    what: &str,
) -> Result<Vec<AlternateTitle>> {
    let mut seen = HashSet::new();
    let mut rows = Vec::new();
    for input in titles {
        let title = clean_text(&input.title);
        if title.is_empty() {
            continue;
        }
        if !seen.insert(title.clone()) {
            return Err(Error::Invalid(format!(
                "\"{title}\" is listed twice on {what}"
            )));
        }
        if title == main {
            return Err(Error::Invalid(format!(
                "\"{title}\" is already the main title of {what}"
            )));
        }
        rows.push(AlternateTitle {
            title,
            language: language_tag(input.language.as_deref())?,
            printed: input.printed.unwrap_or(true),
        });
    }
    Ok(rows)
}

/// Saves a release's or track's alternate titles, after its main title.
/// Left out, they stay as they are, except one that's now the main title,
/// which goes.
pub(super) fn save_alternate_titles(
    tx: &Connection,
    owner: TitleOwner,
    id: i64,
    main: &str,
    input: Option<&[AlternateTitleInput]>,
    what: &str,
) -> Result<()> {
    if let Some(input) = input {
        return titles::set_alternate_titles(
            tx,
            owner,
            id,
            &alternate_title_rows(main, input, what)?,
        );
    }
    let existing = titles::alternate_titles_of(tx, owner, &[id])?
        .remove(&id)
        .unwrap_or_default();
    if existing.iter().any(|title| title.title == main) {
        let kept: Vec<AlternateTitle> = existing
            .into_iter()
            .filter(|title| title.title != main)
            .collect();
        titles::set_alternate_titles(tx, owner, id, &kept)?;
    }
    Ok(())
}
