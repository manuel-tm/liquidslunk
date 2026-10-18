//! Runs commands on a library without the app, for the write parity script
//! (`scripts/write-parity/`, design §10): the same functions the commands
//! call, with the same JSON arguments.
//!
//!   cargo run -p liquidslunk --example run_commands -- <slunk.db> <steps.json>
//!
//! `steps.json` is a list of `{ "command": "registerFiles", "args": {…} }`,
//! with the bindings' names. It prints a JSON list of results, each
//! `{ "ok": … }` or `{ "error": { "kind", "message" } }`. The library must be
//! migrated already.

use std::path::Path;

use liquidslunk_lib::commands::catalog::{self, artists, labels, lists};
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
        "listItems" => json(lists::list(library, parse(args)?)?),
        "listItem" => json(lists::details(library, parse(args)?)?),
        "listItemOptions" => json(lists::options(library, parse(args)?)?),
        "createListItem" => json(lists::create(library, parse(args)?)?),
        "renameListItem" => json(lists::rename(library, parse(args)?)?),
        "deleteListItem" => json(lists::delete(library, parse(args)?)?),
        "artists" => json(artists::list(library, parse(args)?)?),
        "artist" => json(artists::details(library, parse(args)?)?),
        "artistOptions" => json(artists::options(library, parse(args)?)?),
        "artistNames" => json(artists::names(library, parse(args)?)?),
        "createArtist" => json(artists::create(library, parse(args)?)?),
        "updateArtist" => json(artists::update(library, parse(args)?)?),
        "setArtistPicture" => json(artists::set_picture(library, parse(args)?)?),
        "deleteArtist" => json(artists::delete(library, parse(args)?)?),
        "labels" => json(labels::list(library, parse(args)?)?),
        "label" => json(labels::details(library, parse(args)?)?),
        "labelOptions" => json(labels::options(library, parse(args)?)?),
        "createLabel" => json(labels::create(library, parse(args)?)?),
        "updateLabel" => json(labels::update(library, parse(args)?)?),
        "setLabelPicture" => json(labels::set_picture(library, parse(args)?)?),
        "deleteLabel" => json(labels::delete(library, parse(args)?)?),
        "languages" => json(catalog::in_use(library)?),
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
