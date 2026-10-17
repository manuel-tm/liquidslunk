//! Tag values the baseline didn't check: a date tag's value is a day at
//! midnight UTC, and a number tag's value is finite. Rows already there are
//! checked first.

use rusqlite::Transaction;

use super::run;
use crate::{Error, Result};

/// A rule on a value table: the rows that break it, and its triggers.
struct Rule {
    table: &'static str,
    broken: &'static str,
    message: &'static str,
    triggers: [&'static str; 2],
}

const RULES: [Rule; 2] = [
    Rule {
        table: "release_date_tags",
        // Days only, like release dates
        broken: "value % 86400 <> 0",
        message: "a date tag's value must be a day, at midnight UTC",
        triggers: [
            r#"CREATE TRIGGER release_date_tags_value_is_day_insert
          BEFORE INSERT ON release_date_tags
          WHEN NEW.value % 86400 <> 0
          BEGIN
            SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
          END"#,
            r#"CREATE TRIGGER release_date_tags_value_is_day_update
          BEFORE UPDATE OF value ON release_date_tags
          WHEN NEW.value % 86400 <> 0
          BEGIN
            SELECT RAISE(ABORT, 'a date tag''s value must be a day, at midnight UTC');
          END"#,
        ],
    },
    Rule {
        table: "release_real_number_tags",
        // SQLite stores infinity, which JSON can't carry. NaN is already NULL.
        broken: "abs(value) = 9e999",
        message: "a number tag's value must be finite",
        triggers: [
            r#"CREATE TRIGGER release_real_number_tags_value_is_finite_insert
          BEFORE INSERT ON release_real_number_tags
          WHEN abs(NEW.value) = 9e999
          BEGIN
            SELECT RAISE(ABORT, 'a number tag''s value must be finite');
          END"#,
            r#"CREATE TRIGGER release_real_number_tags_value_is_finite_update
          BEFORE UPDATE OF value ON release_real_number_tags
          WHEN abs(NEW.value) = 9e999
          BEGIN
            SELECT RAISE(ABORT, 'a number tag''s value must be finite');
          END"#,
        ],
    },
];

pub fn up(tx: &Transaction) -> Result<()> {
    for rule in &RULES {
        // The triggers only check new writes, so rows already there are
        // checked now
        let count: i64 = tx.query_row(
            &format!(
                "SELECT count(*) AS count FROM {}\n        WHERE {}",
                rule.table, rule.broken
            ),
            [],
            |row| row.get(0),
        )?;
        if count > 0 {
            return Err(Error::Invalid(format!(
                "{} has {count} row(s) that break this: {}",
                rule.table, rule.message
            )));
        }
        run(tx, &rule.triggers)?;
    }
    Ok(())
}

pub fn down(tx: &Transaction) -> Result<()> {
    run(tx, DOWN)
}

const DOWN: &[&str] = &[
    r#"DROP TRIGGER release_date_tags_value_is_day_insert"#,
    r#"DROP TRIGGER release_date_tags_value_is_day_update"#,
    r#"DROP TRIGGER release_real_number_tags_value_is_finite_insert"#,
    r#"DROP TRIGGER release_real_number_tags_value_is_finite_update"#,
];
