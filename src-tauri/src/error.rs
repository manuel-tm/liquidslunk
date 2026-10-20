//! The one error every command returns (backend.md, "Commands"):
//! `{ kind, message }`.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use specta::Type;

/// What went wrong, for the frontend to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// No library is open: the frontend goes to Welcome.
    NoLibrary,
    NotFound,
    /// A message for the user, which a form shows as it is.
    Invalid,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Type, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    pub kind: ErrorKind,
    pub message: String,
}

impl CommandError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Invalid, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Internal, message)
    }

    pub fn no_library() -> Self {
        Self::new(ErrorKind::NoLibrary, "No library is open.")
    }
}

impl From<slunk::Error> for CommandError {
    fn from(error: slunk::Error) -> Self {
        match error {
            slunk::Error::Invalid(message) => Self::invalid(message),
            slunk::Error::NotFound(message) => Self::not_found(message),
            slunk::Error::NewerLibrary(_) => {
                Self::invalid("This library was upgraded by a newer version of liquidslunk.")
            }
            slunk::Error::Sqlite(error) => error.into(),
            error @ slunk::Error::Io { .. } => Self::internal(error.to_string()),
        }
    }
}

impl From<rusqlite::Error> for CommandError {
    /// A constraint the database enforces is a message for the form, in words
    /// it can show. Anything else is internal.
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::ConstraintViolation) => {
                Self::invalid(readable(&error.to_string()))
            }
            _ => Self::internal(error.to_string()),
        }
    }
}

/// SQLite's constraint errors, in words a form can show, as slunk's backend
/// wrote them. The triggers' own messages are already readable ("a track's
/// file must be music"), so they pass through, as does anything else.
static MESSAGES: LazyLock<Vec<(Regex, &str)>> = LazyLock::new(|| {
    [
        (
            r"UNIQUE constraint failed: tracks\.file_id",
            "That file is already a track's audio",
        ),
        (
            r"UNIQUE constraint failed: tracks\.release_id, tracks\.cd_number, tracks\.track_number",
            "That disc already has a track with that number",
        ),
        (
            r"UNIQUE constraint failed: files\.path",
            "A file is already registered at that path",
        ),
        (
            r"UNIQUE constraint failed: (release_formats|release_media|\w+_tags|external_libraries)\.name",
            "There's already one with that name, ignoring case",
        ),
        (
            r"UNIQUE constraint failed: \w+_credits\.\w+_id, \w+_credits\.role",
            "Two credits have the same role, ignoring case",
        ),
        (
            r"UNIQUE constraint failed: (artist|label)_alternate_names\.\w+, \w+\.name",
            "It already has that alternate name",
        ),
        (
            r"UNIQUE constraint failed: external_libraries\.folder",
            "Another external library uses that folder",
        ),
        (
            r"UNIQUE constraint failed: (index 'external_library_rules_\w+'|external_library_rules\..+)",
            "Each rule can only be listed once",
        ),
        (
            r"UNIQUE constraint failed: \w+\.\w+_id, \w+\.\w+_id",
            "Each one can only be listed once",
        ),
        (
            r#"CHECK constraint failed: \("(name_)?language"[\s\S]*"#,
            "That isn't a language tag, like en, ja or ja-Latn",
        ),
        (r"CHECK constraint failed: (name|title|role)$", "The $1 can't be empty"),
        (
            r"CHECK constraint failed: (track_number|cd_number) > 0",
            "Disc and track numbers start at 1",
        ),
        (
            r"FOREIGN KEY constraint failed",
            "It refers to something that doesn't exist, or something still uses it",
        ),
    ]
    .into_iter()
    .map(|(pattern, message)| (Regex::new(pattern).expect("a valid pattern"), message))
    .collect()
});

/// The first message whose pattern matches replaces the match, as JavaScript's
/// `String.replace()` does.
fn readable(message: &str) -> String {
    for (pattern, readable) in MESSAGES.iter() {
        if pattern.is_match(message) {
            return pattern.replace(message, *readable).into_owned();
        }
    }
    message.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constraint_errors_read_as_slunks_did() {
        let cases = [
            (
                "UNIQUE constraint failed: tracks.file_id",
                "That file is already a track's audio",
            ),
            (
                "UNIQUE constraint failed: text_tags.name",
                "There's already one with that name, ignoring case",
            ),
            (
                "UNIQUE constraint failed: release_credits.release_id, release_credits.role",
                "Two credits have the same role, ignoring case",
            ),
            (
                "UNIQUE constraint failed: artists_releases.release_id, artists_releases.artist_id",
                "Each one can only be listed once",
            ),
            (
                "UNIQUE constraint failed: external_libraries.name",
                "There's already one with that name, ignoring case",
            ),
            (
                "UNIQUE constraint failed: external_libraries.folder",
                "Another external library uses that folder",
            ),
            (
                "UNIQUE constraint failed: external_library_rules.external_library_id, external_library_rules.text_tag_id, external_library_rules.tag_equals",
                "Each rule can only be listed once",
            ),
            (
                "UNIQUE constraint failed: index 'external_library_rules_external_library_id_real_number_tag_id_tag_equals_tag_min_tag_max_index'",
                "Each rule can only be listed once",
            ),
            (
                "CHECK constraint failed: (\"name_language\" GLOB '[a-z][a-z]'",
                "That isn't a language tag, like en, ja or ja-Latn",
            ),
            ("CHECK constraint failed: title", "The title can't be empty"),
            (
                "CHECK constraint failed: cd_number > 0",
                "Disc and track numbers start at 1",
            ),
            (
                "a track's file must be music",
                "a track's file must be music",
            ),
        ];
        for (sqlite, shown) in cases {
            assert_eq!(readable(sqlite), shown, "{sqlite}");
        }
    }

    #[test]
    fn a_failed_constraint_is_invalid_and_anything_else_internal() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "create table t (name text not null check (name <> ''));
             create trigger t_named before insert on t when NEW.name = 'x'
             begin select raise(abort, 'no x'); end;",
        )
        .unwrap();
        let error: CommandError = conn
            .execute("insert into t values ('')", [])
            .unwrap_err()
            .into();
        assert_eq!(
            error,
            CommandError::invalid("CHECK constraint failed: name <> ''")
        );
        let error: CommandError = conn
            .execute("insert into t values ('x')", [])
            .unwrap_err()
            .into();
        assert_eq!(error, CommandError::invalid("no x"));
        let error: CommandError = conn.execute("select nope", []).unwrap_err().into();
        assert_eq!(error.kind, ErrorKind::Internal);
    }
}
