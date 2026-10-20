//! The migrations, one module each, `m<timestamp>_<name>.rs` (design §5.6).
//! Their SQL is the statements slunk's Kysely migrations ran, so they build
//! the same schema slunk did. The schema they build is described in
//! `docs/reference/database.md`.
//!
//! - Never edit a migration once it has been committed. Write a new one, with
//!   `slunk-cli gen <name>`, which adds it at the end of [`MIGRATIONS`].
//! - Every `down` undoes its `up`, or fails with an explicit "irreversible"
//!   error.
//! - A migration runs inside the runner's transaction, so it never starts its
//!   own.
//! - Migrations don't use app code, which changes over time while a migration
//!   must not. The exception is `rebuild_table`, which is part of the
//!   migration system.

use rusqlite::Transaction;

use super::migrator::Migration;
use crate::Result;

/// Declares each migration's module and lists it in [`MIGRATIONS`], in order.
macro_rules! migrations {
    ($($module:ident),* $(,)?) => {
        $(mod $module;)*

        /// Every migration, in the order they apply. A test checks that it's
        /// sorted and lists every module in this folder.
        pub const MIGRATIONS: &[Migration] = &[$(Migration {
            name: stringify!($module).split_at(1).1,
            up: $module::up,
            down: $module::down,
        }),*];
    };
}

migrations![
    m20261011141955_create_baseline,
    m20261013060918_add_tag_value_rules,
    m20261014052413_add_track_tags,
    m20261015192802_free_text_credit_roles,
    m20261015195853_add_disc_titles,
    m20261016144030_add_alternate_names,
    m20261016151144_add_credited_names,
    m20261016180957_add_alternate_titles,
    m20261020045756_add_external_libraries,
];

/// Runs statements in order, each prepared on its own, as Kysely ran them.
pub(crate) fn run(tx: &Transaction, statements: &[&str]) -> Result<()> {
    for statement in statements {
        tx.execute(statement, [])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn the_list_is_sorted_and_complete() {
        let names: Vec<&str> = MIGRATIONS.iter().map(|migration| migration.name).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted, "MIGRATIONS isn't sorted, or lists one twice");

        let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/repo/migrations");
        let mut modules: Vec<String> = fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|file| file != "mod.rs")
            .collect();
        modules.sort();
        let listed: Vec<String> = names.iter().map(|name| format!("m{name}.rs")).collect();
        assert_eq!(
            modules, listed,
            "a module in migrations/ isn't in MIGRATIONS"
        );
        for name in names {
            let (stamp, rest) = name.split_at(14);
            assert!(stamp.bytes().all(|b| b.is_ascii_digit()), "{name}");
            assert!(rest.starts_with('_') && rest.len() > 1, "{name}");
        }
    }
}
