//! Runs commands on a library without the app, for the write parity script
//! (`scripts/write-parity.ts`, design §10): the same functions the commands
//! call, with the same JSON arguments.
//!
//!   cargo run -p liquidslunk --example run_commands -- <slunk.db> <steps.json>
//!
//! `steps.json` is a list of `{ "command": "registerFiles", "args": {…} }`,
//! with the bindings' names. It prints a JSON list of results, each
//! `{ "ok": … }` or `{ "error": { "kind", "message" } }`. The library must be
//! migrated already.

use std::path::Path;

use liquidslunk_lib::commands::media;
use liquidslunk_lib::error::CommandError;
use liquidslunk_lib::library::Library;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use slunk::repo::connection;

#[derive(Deserialize)]
struct Step {
    command: String,
    #[serde(default)]
    args: Value,
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, db, steps] = &arguments[..] else {
        eprintln!("Usage: run_commands <slunk.db> <steps.json>");
        std::process::exit(2);
    };
    let db = std::path::absolute(Path::new(db)).expect("an absolute path");
    let steps: Vec<Step> =
        serde_json::from_str(&std::fs::read_to_string(steps).expect("the steps"))
            .expect("a list of steps");
    let library = Library::new(&db, connection::open(&db).expect("a migrated library"));
    let results: Vec<Value> = steps
        .into_iter()
        .map(|step| match run(&library, &step.command, step.args) {
            Ok(value) => json!({ "ok": value }),
            Err(error) => json!({ "error": error }),
        })
        .collect();
    println!("{}", Value::Array(results));
}

/// A command, by its name in the bindings.
fn run(library: &Library, command: &str, args: Value) -> Result<Value, CommandError> {
    let value = match command {
        "files" => json(media::list_files(library, parse(args)?)?),
        "file" => json(media::file_details(library, parse(args)?)?),
        "fileCounts" => json(media::counts(library)?),
        "folder" => json(media::browse_folder(library, parse(args)?)?),
        "folderFiles" => json(media::scan_folder(library, parse(args)?)?),
        "registerFiles" => json(media::register(library, parse(args)?)?),
        "updateFile" => json(media::update(library, parse(args)?)?),
        "deleteFile" => json(media::delete(library, parse(args)?)?),
        _ => return Err(CommandError::invalid(format!("No command {command}"))),
    };
    Ok(value)
}

/// Arguments as Tauri deserializes them.
fn parse<T: DeserializeOwned>(args: Value) -> Result<T, CommandError> {
    serde_json::from_value(args).map_err(|error| CommandError::invalid(error.to_string()))
}

fn json(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("a DTO serializes")
}
