# AGENTS.md

Guidance for AI coding agents working in this repository. People should start with [README.md](README.md).

## Project

liquidslunk is a local music library manager, built as a desktop app on Tauri 2. Its SQLite database is the source of truth for all music metadata, and it only *reads* the user's music files.

It's a port of slunk, an earlier version of the same app, with a Node backend and a web frontend. slunk is frozen, and this repository stands on its own: see [slunk is a reference, never a dependency](#slunk-is-a-reference-never-a-dependency).

## Where things are

- **[`docs/design/`](docs/design/): design docs.** These are the plans: what was decided, and why, when it was decided. They're for planning and prompting, and a record of decisions once they're built, never the last word on what the code does. [`docs/design/README.md`](docs/design/README.md) lists them with their status.
- **The rest of [`docs/`](docs/README.md): what's actually built,** kept up to date with the code. Start at `docs/README.md`. Before changing the backend's structure, the commands, how the library is chosen, the migrations, or anything that touches files, read what [`backend.md`](docs/development/backend.md) says of it. A design doc can tell you why something was decided, not what the code does now.
- **`src/`: the frontend,** React in the app's webview. How it's built: [`docs/development/frontend.md`](docs/development/frontend.md).
- **`src-tauri/`: the app,** Tauri, with its commands, the library and the file protocol. **`crates/slunk/`: the domain and its database,** with no Tauri dependency: the contexts, the repositories, and the migrations with their parity tests. **`crates/slunk-cli/`: the database commands.** How they're built: [`docs/development/backend.md`](docs/development/backend.md). The schema as built: [`docs/reference/database.md`](docs/reference/database.md).
- **`temp/`: local assets for prompting, never committed.** See [Never reference files in `temp/`](#never-reference-files-in-temp).
- **`dev-library/`: scratch libraries for development, never committed.** See [Never commit or depend on `dev-library/`](#never-commit-or-depend-on-dev-library).

How to set up and run the app: [`docs/development/getting-started.md`](docs/development/getting-started.md).

Work happens in phases. Each design doc plans its own, numbered from 0, so a phase is named with its doc. Stay within the current phase unless asked otherwise.

Code comments and docs never cite a design doc: see [Cite `docs/`, never a design doc](#cite-docs-never-a-design-doc). Design docs cite each other: "design §5.4" and "phase 2" with no document named mean `1_introduction.md`, and a later design doc is named: "2_topic.md §3", "phase 0 of 2_topic.md".

## Workflow: design → code → docs

The full process is in [docs/README.md](docs/README.md#how-we-work).

- **Build from the design docs.** A design is where a change starts. Once the code lands, the code and `docs/` say what it does, and that, not the design, is what later work reads and cites.
- **Keep `docs/` up to date.**
  - Any change that affects something documented updates the docs in the same change: requirements and setup, commands and scripts, project structure, conventions, and behavior.
  - If you find a doc that no longer matches the code, fix it, even if your change didn't cause it.
  - Document only what exists, and never copy plans from `docs/design/` into the rest of `docs/`. What's built from a design is documented with the reasons a reader needs, so the docs stand without it.
- **Design docs are archival unless they're your task.** Edit a design doc only when your task is to design its topic or build its phases, and only as its status allows (below). Every other design doc is read-only: read it for context, but never edit it, not even to fix a typo, a stale name or a broken link. If an archived doc conflicts with your work, leave it as it is and say so in your report.
- **Check a design doc's status line before editing it.** It's the line right under the title, and it decides what you may change:
  - `> **Status: in design.**`: edit it freely;
  - `> **Status: being built.**`: only appendices (below);
  - `> **Status: complete.**`: don't touch it, not even to add an appendix. It's a record of what was planned. A later change to what it describes goes in the code and the rest of `docs/`, and a change that needs planning gets a new design doc.
- **Mark a design complete** when all its phases are built: a status line under its title, and its row in `docs/design/README.md`. That's the last edit the doc gets.
- **When the code shows a design has to change,** while its phases are being built:
  - Don't rewrite the design text. Add an appendix at the end of that design doc, plus a one-line note at the affected section.
  - If it changes a design decision (not just a detail), ask before going ahead.
- **New design topics** get a new doc with the next free number, `docs/design/N_topic.md`, and a row in `docs/design/README.md`.
- **`docs/design/README.md` isn't a design doc.** It's the index, updated whenever a design doc is added or changes status.

## Rules

### Cite `docs/`, never a design doc

A design doc records what was decided and why, when it was decided. Once it's built, the code and the rest of `docs/` say what liquidslunk does, and they keep changing while a finished design doesn't. The full rule is in [docs/README.md](docs/README.md#design-docs-and-these-docs).
- Code comments, the rest of `docs/`, `README.md` and this file never cite a design doc: no "design §5.4" or "4_external_libraries.md §8.3", no phase or appendix of one, and no link to one for what the code does. Only design docs, and their index, cite design docs. Saying where the designs are, as this file does, isn't citing one.
- A comment that needs more than it says points to the section of `docs/` that describes it, by the doc's file name and the section's heading: `(backend.md, "Running migrations")`. A module's or a component's comment points once, and what's inside it points again only to another section. Docs link to each other by section.
- What a reader needs from a design, a rule or the reason for it, is written into `docs/` in the same change as the code. If the section a comment needs doesn't exist, write it.
- slunk's docs aren't in this repository, so they aren't cited either: what a comment needs from them goes in the comment, or in `docs/`.

### slunk is a reference, never a dependency

liquidslunk ports slunk's schema, behavior and routes. slunk's own repository will be deleted once the port is done, so this one has to stand on its own:
- Read slunk's source for how something worked, when it's available to you. Never point to it: no local path, relative path or link to slunk's repository in docs, design docs, code, comments, commit messages or PR descriptions. For anyone else, and later for everyone, it would point to nothing.
- Never import, symlink or copy from it at build or run time. What liquidslunk needed from slunk was copied into this repository during the port, and is its own code and docs now. Anything more is written into the docs.
- Naming slunk is fine ("a port of slunk's import"). Pointing at its files isn't.

### Never write to the user's music files

- liquidslunk never moves, copies, renames, deletes or retags a user's file. Code that touches the user's files may only read them.
- The only things it writes are next to its database: `slunk.db`, SQLite's `-wal`/`-shm` files, and `slunk-backups/`. Apart from those, it writes its settings in the app's config folder ([backend.md](docs/development/backend.md#choosing-the-library)).
- **External libraries** ([What liquidslunk writes](docs/README.md#what-liquidslunk-writes)) add these places, and only these:
  - inside an external library's destination: the files a sync converts or copies, `cover.jpg`s, the manifest and its journal in `.liquidslunk/`, and its temporary files. It replaces or deletes only what the manifest lists, and the `._` files macOS makes for its own files on FAT32 and exFAT. It never makes the destination's folder, or writes outside it;
  - on an iPod, the same in `iPod_Control/Music`, with the manifest, its journal and temporary files in `iPod_Control/iTunes`, and the iPod's databases: the iTunesDB, a copy of the one it replaces, the ArtworkDB and its `.ithmb` files. It also deletes `Play Counts` once it's counted, and, once the user agrees, the music files of tracks it didn't write ([reference/ipod.md](docs/reference/ipod.md#the-files));
  - a sync's own folder in the system's temporary folder, while it converts.

  A destination is never inside the library's folder, so the user's files are still only read.
- For development, use a scratch library in `dev-library/`. Never open a real music library from a development build ([getting-started.md](docs/development/getting-started.md#running-the-app)).

### Never reference files in `temp/`

- `temp/` holds assets the user shares while prompting. Only its placeholder, `temp/.gitkeep`, is committed; the root `.gitignore` ignores everything else, so those files exist only on the user's machine.
- Never reference a file in `temp/` in anything that gets committed or published: docs, design docs, code, comments, commit messages or PR descriptions. For anyone else, the reference would point to nothing.
- If something from a `temp/` file belongs in the docs, write that content into the doc itself.

### Never commit or depend on `dev-library/`

- `dev-library/` holds scratch libraries for development. `dev-library/music/` is a copy of part of the user's real library: music files, artwork, rip logs, and backups of slunk's database for them.
- Only its placeholder, `dev-library/.gitkeep`, is committed; the root `.gitignore` ignores everything else. Never commit a file from it, or force-add one: they're copies of the user's own music.
- Nothing committed may depend on what's in it, since a fresh clone has an empty `dev-library/`. Tests make their own fixtures.
- Treat its files like the user's: read them, never write them. A development build writes only next to the library's database, as a release build does.

### The database is the part built to last

The schema is slunk's, carried over unchanged. Everything else can change faster. For the database:

- Change the schema only through a new migration. Never edit a migration that has already been committed.
- Enforce integrity in the database (constraints, triggers) wherever it can be expressed, not only in app code.
- A new migration is a one-way door: slunk refuses a library with a migration it doesn't have, so it can't open that library again, apart from its backups ([backend.md](docs/development/backend.md#the-migrations)). Plan one before writing it.

### External libraries grow through a design

External libraries, export and sync, sync a folder or an iPod with video, as [backend.md](docs/development/backend.md#the-external-libraries-context) describes. Anything more, like phones, other iPods, playlists or reading back from a destination, needs a design of its own first.

## Before you finish

- **Code:** these pass, from the repository's root:
  - **Frontend:** `npm run typecheck`, `npm run lint` and `npm test` pass, and `npm run format` has been run.
  - **Rust:** `cargo fmt --check`, `cargo clippy --workspace --all-targets` with no warnings, and `cargo test --workspace`.
  - **Bindings:** after changing a command or its types, a debug build has been started (`npm run tauri dev`), so `src/bindings.ts` is regenerated, and the change is in it.
  - **Migrations** apply and roll back cleanly through the CLI, on a scratch database: `npm run db:migrate`, `npm run db:rollback` and `npm run db:migrate` again, with `--db`.
  - **Screenshots:** a change that shows in the UI has been looked at in a screenshot of the running app, taken through WebDriver: see [Screenshots](docs/development/frontend.md#screenshots).
- **Docs:** everything in `docs/` outside `docs/design/` matches the code after your change. A design change made while building that design's phases is recorded as an appendix, and no other design doc was touched.
- **References:** nothing you wrote points to a file in slunk's repository or in `temp/`, and nothing you wrote outside `docs/design/` cites a design doc.
- **Report:** say what you verified and what you didn't.
