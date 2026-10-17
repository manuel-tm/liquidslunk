//! A list's arguments, as every list command takes them (slunk's design
//! §7.2): a page, a sort column, and a search.

use crate::text::clean_text;
use crate::{Error, Result};

/// What a list command takes, before it's checked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListArgs {
    /// From 1. Default 1.
    pub page: Option<i64>,
    /// Default 50, and at most 200.
    pub per_page: Option<i64>,
    /// A column, with `-` in front for descending. Default `-id`, newest first.
    pub sort: Option<String>,
    /// Part of the name, title or path, ignoring case.
    pub q: Option<String>,
}

/// The rows a page covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Paging {
    pub offset: i64,
    pub limit: i64,
}

/// A page of rows, and how many there are in all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
}

/// A sort column and its direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sort<S> {
    /// `None` is `id`.
    pub column: Option<S>,
    pub descending: bool,
}

/// The rows a page covers. Pages hold at most 200 rows.
pub fn paging_of(args: &ListArgs) -> Result<Paging> {
    let page = args.page.unwrap_or(1);
    let per_page = args.per_page.unwrap_or(50);
    if page < 1 {
        return Err(Error::Invalid("No such page".into()));
    }
    let limit = per_page.clamp(1, 200);
    Ok(Paging {
        offset: (page - 1).saturating_mul(limit),
        limit,
    })
}

/// The sort column and direction, from the columns a list allows besides
/// `id`. `-id`, newest first, is the default. `what` names the list in the
/// error: "Files can't be sorted by size".
pub fn sort_of<S: Copy>(sort: Option<&str>, allowed: &[(&str, S)], what: &str) -> Result<Sort<S>> {
    let sort = sort.unwrap_or("-id");
    // Only one "-", as slunk's /^-/ removed
    let column = sort.strip_prefix('-').unwrap_or(sort);
    let descending = sort.starts_with('-');
    if column == "id" {
        return Ok(Sort {
            column: None,
            descending,
        });
    }
    match allowed.iter().find(|(name, _)| *name == column) {
        Some(&(_, column)) => Ok(Sort {
            column: Some(column),
            descending,
        }),
        None => Err(Error::Invalid(format!(
            "{what} can't be sorted by {column}"
        ))),
    }
}

/// The search text, or `None` when there's nothing to search for. It's in
/// NFC, like saved text, so either form of "é" finds the other.
pub fn search_of(q: Option<&str>) -> Option<String> {
    q.map(clean_text).filter(|q| !q.is_empty())
}

/// Escapes a LIKE pattern, to use with `escape '\'`.
pub fn like_pattern(search: &str) -> String {
    let mut pattern = String::with_capacity(search.len() + 2);
    pattern.push('%');
    for c in search.chars() {
        if matches!(c, '\\' | '%' | '_') {
            pattern.push('\\');
        }
        pattern.push(c);
    }
    pattern.push('%');
    pattern
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(page: Option<i64>, per_page: Option<i64>) -> ListArgs {
        ListArgs {
            page,
            per_page,
            ..ListArgs::default()
        }
    }

    #[test]
    fn pages_hold_50_rows_by_default_and_200_at_most() {
        assert_eq!(
            paging_of(&args(None, None)).unwrap(),
            Paging {
                offset: 0,
                limit: 50
            }
        );
        assert_eq!(
            paging_of(&args(Some(3), Some(20))).unwrap(),
            Paging {
                offset: 40,
                limit: 20
            }
        );
        assert_eq!(paging_of(&args(Some(1), Some(1000))).unwrap().limit, 200);
        assert_eq!(paging_of(&args(Some(1), Some(0))).unwrap().limit, 1);
        assert_eq!(
            paging_of(&args(Some(0), None)).unwrap_err().to_string(),
            "No such page"
        );
    }

    #[test]
    fn sorts_are_a_column_with_a_minus_for_descending() {
        let allowed = [("path", "path")];
        let sort = |sort| sort_of(sort, &allowed, "Files");
        assert_eq!(
            sort(None).unwrap(),
            Sort {
                column: None,
                descending: true
            }
        );
        assert_eq!(
            sort(Some("path")).unwrap(),
            Sort {
                column: Some("path"),
                descending: false
            }
        );
        assert_eq!(
            sort(Some("-path")).unwrap(),
            Sort {
                column: Some("path"),
                descending: true
            }
        );
        assert_eq!(
            sort(Some("id")).unwrap(),
            Sort {
                column: None,
                descending: false
            }
        );
        assert_eq!(
            sort(Some("-size")).unwrap_err().to_string(),
            "Files can't be sorted by size"
        );
        assert_eq!(
            sort(Some("--path")).unwrap_err().to_string(),
            "Files can't be sorted by -path"
        );
    }

    #[test]
    fn searches_are_nfc_and_trimmed_and_blank_is_none() {
        assert_eq!(
            search_of(Some(" Cafe\u{301} ")).as_deref(),
            Some("Caf\u{e9}")
        );
        assert_eq!(search_of(Some("  ")), None);
        assert_eq!(search_of(None), None);
    }

    #[test]
    fn like_patterns_escape_their_wildcards() {
        assert_eq!(like_pattern("50%_off\\"), "%50\\%\\_off\\\\%");
    }
}
