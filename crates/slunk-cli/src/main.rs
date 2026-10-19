//! The database commands (design §5.6), Ecto-style: `migrate`, `rollback [n]`,
//! `status`, `gen <name>`, `seed` and `reset`. The root `package.json` wraps
//! them as `npm run db:*`, which run from the repository's root.

use std::fs;
use std::path::{self, Path, PathBuf};
use std::process::ExitCode;
use std::time::SystemTime;

use clap::{Args, Parser, Subcommand};
use slunk::repo::migrations::MIGRATIONS;
use slunk::repo::{backups, connection, migrator, seeds};
use slunk::{Error, Result};

/// The repository's root, where the migrations are.
fn root() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    root.canonicalize().unwrap_or(root)
}

const TEMPLATE: &str = "//! What this migration changes, and why.

use rusqlite::Transaction;

use super::run;
use crate::Result;

pub fn up(tx: &Transaction) -> Result<()> {
    run(tx, &[])
}

pub fn down(tx: &Transaction) -> Result<()> {
    run(tx, &[])
}
";

#[derive(Parser)]
#[command(name = "slunk-cli", about = "liquidslunk's database commands")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Backs up the database, then applies pending migrations
    Migrate(Database),
    /// Backs up the database, then reverts the last n migrations
    Rollback {
        #[command(flatten)]
        database: Database,
        /// How many migrations to revert
        #[arg(default_value_t = 1)]
        n: usize,
    },
    /// Lists migrations as applied, pending, or MISSING (applied, but unknown here)
    Status(Database),
    /// Creates crates/slunk/src/repo/migrations/m<timestamp>_<name>.rs, and lists it
    Gen {
        /// The migration's name, in snake_case
        name: String,
    },
    /// Loads sample data, unless the database already has releases
    Seed(Database),
    /// Backs up and deletes the database, then migrates and seeds a new one
    Reset(Database),
}

#[derive(Args)]
struct Database {
    /// The library's database, like dev-library/music/slunk.db
    #[arg(long)]
    db: PathBuf,
    /// How many backups slunk-backups/ keeps
    #[arg(long, default_value_t = backups::DEFAULT_KEEP, value_parser = clap::value_parser!(u32).range(1..))]
    keep_backups: u32,
}

fn main() -> ExitCode {
    match run(Cli::parse().command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Migrate(database) => {
            let db = database.path()?;
            report(
                "Applied",
                &migrator::migrate(&db, database.keep_backups)?,
                &database,
            );
        }
        Command::Rollback { database, n } => {
            let db = database.path()?;
            report(
                "Rolled back",
                &migrator::rollback(&db, n, database.keep_backups)?,
                &database,
            );
        }
        Command::Status(database) => {
            let db = database.path()?;
            println!("Database: {}", db.display());
            for row in migrator::status(&db)? {
                let state = match (row.missing, row.applied_at) {
                    (true, _) => "MISSING",
                    (false, None) => "pending",
                    (false, Some(_)) => "applied",
                };
                let at = row.applied_at.map(backups::iso_8601).unwrap_or_default();
                println!("{state:<8} {}  {at}", row.name);
            }
        }
        Command::Gen { name } => generate(&name)?,
        Command::Seed(database) => seed(&database.path()?)?,
        Command::Reset(database) => {
            let db = database.path()?;
            if db.exists() {
                let backup = backups::backup(&db, database.keep_backups)?;
                println!("Backed up to {}", shown(&backup.path));
                report_removed(&backup.removed, &database);
                for suffix in ["", "-wal", "-shm"] {
                    let file = PathBuf::from(format!("{}{suffix}", db.display()));
                    if file.exists() {
                        fs::remove_file(&file).map_err(Error::io(&file))?;
                    }
                }
                println!("Deleted {}", db.display());
            }
            report(
                "Applied",
                &migrator::migrate(&db, database.keep_backups)?,
                &database,
            );
            seed(&db)?;
        }
    }
    Ok(())
}

impl Database {
    /// The database's absolute path. Its folder must exist.
    fn path(&self) -> Result<PathBuf> {
        let db = path::absolute(&self.db).map_err(Error::io(&self.db))?;
        match db.parent() {
            Some(folder) if folder.is_dir() => Ok(db),
            _ => Err(Error::Invalid(format!(
                "There's no folder for {}. Create it first.",
                db.display()
            ))),
        }
    }
}

/// Creates `m<timestamp>_<name>.rs` from the template, and adds it at the end
/// of `migrations![…]` in `mod.rs`.
fn generate(name: &str) -> Result<()> {
    let valid = name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !valid {
        return Err(Error::Invalid(
            "Usage: npm run db:gen -- <snake_case_name>".into(),
        ));
    }
    if MIGRATIONS
        .iter()
        .any(|migration| &migration.name[15..] == name)
    {
        return Err(Error::Invalid(format!(
            "A migration named {name} already exists."
        )));
    }
    let folder = root().join("crates/slunk/src/repo/migrations");
    let module = format!("m{}_{name}", backups::timestamp(SystemTime::now()));
    let file = folder.join(format!("{module}.rs"));
    if file.exists() {
        return Err(Error::Invalid(format!(
            "{} already exists.",
            file.display()
        )));
    }

    let list = folder.join("mod.rs");
    let source = fs::read_to_string(&list).map_err(Error::io(&list))?;
    let start = source
        .find("migrations![")
        .ok_or_else(|| Error::Invalid(format!("No migrations![…] in {}", list.display())))?;
    let end = start
        + source[start..].find("\n];").ok_or_else(|| {
            Error::Invalid(format!("No end to migrations![…] in {}", list.display()))
        })?;
    let updated = format!("{}\n    {module},{}", &source[..end], &source[end..]);

    fs::write(&file, TEMPLATE).map_err(Error::io(&file))?;
    fs::write(&list, updated).map_err(Error::io(&list))?;
    println!("Created {}", shown(&file));
    Ok(())
}

/// Loads the sample data (`repo/seeds.rs`) into a migrated database.
fn seed(db: &Path) -> Result<()> {
    let mut conn = connection::open(db)?;
    match seeds::seed(&mut conn)? {
        Some(seeded) => println!(
            "Seeded {} releases, {} artists, {} labels and {} tags.",
            seeded.releases, seeded.artists, seeded.labels, seeded.tags
        ),
        None => println!("Skipped seeding: the database already has releases."),
    }
    Ok(())
}

/// Prints what ran.
fn report(verb: &str, outcome: &migrator::Outcome, database: &Database) {
    if let Some(backup) = &outcome.backup {
        println!("Backed up to {}", shown(backup));
    }
    report_removed(&outcome.removed, database);
    if outcome.names.is_empty() {
        println!("Nothing to do.");
    }
    for name in &outcome.names {
        println!("{verb} {name}");
    }
}

fn report_removed(removed: &[PathBuf], database: &Database) {
    if !removed.is_empty() {
        let plural = if removed.len() == 1 { "" } else { "s" };
        println!(
            "Removed {} old backup{plural}, keeping the last {}",
            removed.len(),
            database.keep_backups
        );
    }
}

/// A path relative to the current folder when it's inside it, else absolute.
fn shown(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok().map(Path::to_path_buf))
        .unwrap_or_else(|| path.to_path_buf())
        .display()
        .to_string()
}
