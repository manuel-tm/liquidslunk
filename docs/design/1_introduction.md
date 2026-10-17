# Introduction to liquidslunk

> **Status: being built.**

liquidslunk is a local music library manager, built as a desktop app on Tauri 2. It's a port of slunk, an earlier version of the same app: a Node backend and a web frontend that talked over GraphQL. liquidslunk keeps what slunk does and its database, rewrites the backend in Rust, and builds it into the app. slunk is frozen, and this repository stands on its own.

This document is the base design. It covers:
- what liquidslunk is, and what it keeps from slunk;
- why the backend moves into the app, in Rust, and the frontend from Apollo to TanStack Query;
- the shape of the Rust backend, its commands, and how it serves files;
- choosing the library in the app, and remembering the choice;
- how the frontend changes;
- how to get there one context at a time, with slunk's backend as a stopgap;
- the phases to build it in.

**Design docs.** liquidslunk's design lives in `docs/design/`, one numbered document per topic (`N_topic.md`), numbered in the order they're written. This one comes first, and later documents build on it. Design docs are plans while they're designed and built, and an archive once they're complete. What's actually built is documented in the rest of `docs/` ([docs/README.md](../README.md)). Every design doc and its status is listed in [README.md](README.md).

**Legacy file names.**
- Backend files named here by their file name, like `catalog.ts` or `migrator.ts`, are slunk's backend, which phase 0 copies into `legacy/slunk-be/` (§8).
- "slunk's docs" are the ones phase 0 copies into `legacy/docs/`.
- Frontend paths, like `src/lib/apollo.ts`, are this repository's, from phase 0 on.

## 1. Summary

- liquidslunk is one executable. The backend runs inside the Tauri app, in Rust, with SQLite compiled in. There's no Node runtime, no server and no port.
- It's a port of slunk. **The data model doesn't change.** The schema, the migrations and the files next to the database are slunk's, so a library slunk made opens as it is. Every page keeps what it showed.
- The backend keeps slunk's contexts (media, catalog, tagging, library check), its rules and its migration runner. The frontend calls it through **Tauri commands**, typed end to end by tauri-specta. GraphQL and Apollo go away.
- The frontend keeps React 19, React Router, shadcn and Tailwind. **TanStack Query** replaces Apollo Client, with one command per document the frontend sends today, returning the same shape.
- File bytes come from a custom protocol, `slunk://localhost/files/:id`.
- **The library is chosen in the app.** The first launch asks for one: open an existing `slunk.db`, or create one in a folder. The choice is saved in the app's settings, and later launches open it without asking. The sidebar switches libraries. There's no `.env`.
- **The port is incremental.** In development, slunk's backend runs as a sidecar behind the Vite proxy the frontend already has. Each context moves to Rust with its routes, until nothing sends GraphQL. Then the sidecar is deleted.
- Seven phases: **0** Shell and library → **1** Media → **2** Artists, labels and lists → **3** Tags → **4** Releases → **5** Tracks and import → **6** Library check and cut-over.

## 2. What liquidslunk is

**A record keeper.** liquidslunk keeps track of a music collection. Its SQLite database is the source of truth for the music's metadata. That lets it store more than file tags can, and query it fast.
- **It never writes to the user's files:** no moving, copying, renaming, deleting or retagging. It records where each file is, relative to the database's folder, so the database and the files can move together.
- **It writes only next to its database:** `slunk.db`, SQLite's `-wal` and `-shm` files, and `slunk-backups/`. Apart from those, it keeps its settings in the app's config folder (§6.2).
- **It's local:** one user, on their own machine, with nothing to deploy.

**What it catalogs**, as slunk did:
- **files** the user registers, each `music`, `image` or `other`;
- **artists** and **labels**, with alternate names, each name with a language. Those are other spellings, names in other scripts, and an artist's real names;
- **releases**, with:
  - formats (Album, EP, …) and media (CD, Vinyl, …);
  - labels and a catalog number;
  - a cover;
  - alternate titles;
- **tracks**, each one registered audio file, on a disc of a release, with its own performers and features;
- **credits**, as lines: a role as printed, then the artists who hold it;
- **the name each artist is credited with**, wherever it's linked;
- **tags**, typed (text, number, date), single- or multi-value, on releases and tracks;
- **file links**, connecting images and other files to releases, artists and tracks.

**The database is the part built to last.** It enforces integrity itself, with `STRICT` tables, foreign keys, CHECKs and triggers. Its schema changes only through migrations. Everything else can change faster.

**What it shows:**
- release and artist pages laid out like Discogs, without anything about selling;
- plain CRUD pages for the other entities;
- an import that builds a release from a folder, step by step;
- a library check that finds files that moved, changed or went missing on disk, and gaps in the catalog.

### 2.1 What the port keeps

slunk as of its last commit is the reference for:
- **the schema and its migrations,** ported statement by statement and checked against slunk's (§5.6);
- **the backend's behavior:** every rule of its contexts, and its error messages;
- **the routes,** and what each one shows.

**Its code and docs come along in phase 0,** all at once (§8):
- the frontend goes into `src/`, and becomes liquidslunk's;
- the backend goes into `legacy/slunk-be/`. It runs as the sidecar, and it's the reference for the port;
- the docs go into `legacy/docs/`, each marked as slunk's.

Everything in `legacy/` is read-only, and phase 6 deletes it.

**Its history doesn't come along.** This repository starts with a fresh history.

**Docs move with the code.** liquidslunk's own docs, in `docs/`, describe only what's built here. When a phase ports something, it carries the legacy docs' sections about it into `docs/`, rewritten for the new code and checked against it. So a rule is in `docs/` only once liquidslunk does it.

## 3. Why

- **One executable.** liquidslunk is local (§2), so it should install and run like any desktop app. slunk needed Node 26, two terminals and an `.env`.
- **No open port.** slunk's API listened on 127.0.0.1. It refused foreign `Host` headers, because it can list any folder on the disk. Any process on the machine could still call it. Commands go through the webview's IPC, so no other process can reach them.
- **The sidecar alone doesn't get there.** Shipping slunk's backend as a sidecar means a Node single-executable build of about 100 MB. better-sqlite3 is a native addon, so it ships beside that build, or the backend moves to `node:sqlite`. That's still a release candidate in Node 26, and it would need a Kysely dialect. The app would also manage a second process: start it, wait for its port, kill it, and recover when it dies.
- **GraphQL's strengths don't apply here.** It lets many clients pick their fields over a network. liquidslunk has one client, in the same process. Its costs do apply:
  - two codegen pipelines;
  - an `ID` scalar that turns ids into strings;
  - the rules Apollo's normalized cache imposes. About ten types in slunk's schema are shaped around it (`FileUse.entityId`, `Credit` without an id, `ReleaseFormat` and `ReleaseMedium` behind `ListItem`, …), and `src/lib/apollo.ts` needs `merge: false` policies.

  The frontend barely uses that cache anyway: every query is `cache-and-network`, and three places evict or refetch by hand.
- **No `.env`.** A user can't be asked to edit one. Choosing a library is part of the app.

## 4. Scope

In scope:
- the Tauri 2 shell, with slunk's frontend moved into it;
- a Rust port of the backend: the database connection, the migration runner and every migration, the four contexts, and the `db:*` commands;
- Tauri commands and TanStack Query in place of GraphQL and Apollo;
- choosing, creating, remembering and switching libraries;
- serving file bytes through a custom protocol;
- the docs for everything built;
- macOS builds.

Out of scope:
- **Schema changes.** None until the cut-over (§8). The port is easier to check when the database stays still.
- **New features.** Every route keeps what it showed in slunk, apart from the library picker.
- **Windows and Linux packages.** Nothing here rules them out: paths are stored with forward slashes, and §5.5 covers the protocol's URL on Windows.
- **Signing, notarization and auto-updates.** See §12.
- **External libraries:** exporting music to other libraries. slunk planned it but never designed it, and it needs its own design doc first.

## 5. Architecture

### 5.1 Choosing the backend

| | A. Node sidecar, GraphQL | B. Rust, GraphQL over IPC | C. Rust commands, TanStack Query |
|---|---|---|---|
| One executable | No: a Node build of ~100 MB, with better-sqlite3's native addon beside it | Yes | Yes |
| Listens on a port | Yes, 127.0.0.1 | No | No |
| Backend work | Packaging only | Full port, plus a GraphQL schema in Rust (async-graphql) | Full port |
| Frontend work | The Apollo link's URL | A custom Apollo link | Every route's data layer: 84 documents in 40 files |
| Types | kysely-codegen, graphql-codegen twice | Rust → SDL → graphql-codegen | Rust → TypeScript, one step (tauri-specta) |
| Cache rules | Apollo's, with its id rules | Apollo's, with its id rules | A cache per query, no id rules |
| Progress for long work | Several requests | Several requests | A `Channel` per command |

**Decision: C.** B is the fallback if converting the frontend ever turns out to be the bottleneck: its frontend changes are one link. A is used only as a temporary bridge, in development (§8).

### 5.2 Shape

```
┌──────────────────────── liquidslunk: one process ────────────────────────┐
│ WebView: React, React Router, TanStack Query, shadcn                     │
│    │ commands.releasePage({ id })             <img src="slunk://…/12">   │
│    ▼                                                  ▼                  │
│ src-tauri: commands, DTOs, the open library     the file protocol        │
│    │                                                  │                  │
│    ▼                                                  ▼                  │
│ crates/slunk: contexts (media, catalog, tagging, library check)          │
│    │                                                                     │
│    ▼                                                                     │
│ crates/slunk/repo: rusqlite, migrator  ──────►  slunk.db (the library)   │
└──────────────────────────────────────────────────────────────────────────┘
```

The repository keeps slunk's Phoenix-style split: the domain's contexts in one place, and the layer that serves them in another.

```
liquidslunk/
  AGENTS.md  CLAUDE.md  README.md  LICENSE
  docs/                      this repository's documentation
  mise.toml                  pinned Node and Rust
  Cargo.toml                 the Cargo workspace
  package.json               the frontend, the tauri CLI, the db:* scripts
  vite.config.ts  index.html  components.json  tsconfig*.json
  src/                       the React app (slunk's frontend)
    bindings.ts              commands and types (generated by tauri-specta)
    lib/query.ts             the QueryClient, useCommand(), fileUrl()
  src-tauri/                 (Phoenix: lib/slunk_web)
    tauri.conf.json  capabilities/  icons/
    src/lib.rs               the builder: plugins, state, protocol, commands
    src/library.rs           opening, creating and switching libraries (§6)
    src/settings.rs          the settings file (§6.2)
    src/protocol.rs          slunk://localhost/files/:id (§5.5)
    src/commands/            one module per context: DTOs and commands
    src/sidecar.rs           slunk's backend, in debug builds until phase 6 (§8)
  crates/slunk/              (Phoenix: lib/slunk), with no Tauri dependency
    src/media/  src/catalog/  src/tagging/  src/library_check.rs
    src/text.rs  dates.rs  languages.rs  paging.rs
    src/repo/                connection, repositories, migrator, rebuild_table
    src/repo/migrations/     one module per migration
  crates/slunk-cli/          the db:* commands (§5.6)
  legacy/                    slunk as of its last commit, read-only, until phase 6 (§8)
    README.md                what this folder is
    docs/                    slunk's docs, each marked as legacy
    slunk-be/                slunk's backend
  dev-library/               scratch libraries for development, never committed
  temp/                      local assets for prompting, never committed
```

`crates/slunk` doesn't depend on Tauri, so its tests run with a plain `cargo test`, and the CLI uses it directly.

### 5.3 Stack

| Piece | Choice | In slunk |
|---|---|---|
| Shell | Tauri 2 | Node 26 server, Vite in a browser tab |
| API | Tauri commands, typed by tauri-specta | Apollo Server 5 on Express 5 |
| SQLite | rusqlite, with the `bundled` feature | better-sqlite3 |
| Queries | Plain SQL, and sea-query for lists with dynamic sorts and filters | Kysely |
| Migrations | slunk's runner, ported (§5.6) | `migrator.ts` |
| Audio | lofty | music-metadata |
| Hashing, walking folders | sha2, walkdir | `node:crypto`, `node:fs` |
| Text | unicode-normalization (NFC) | `String.prototype.normalize` |
| Language tags | ICU4X's locale canonicalizer | `Intl.getCanonicalLocales` |
| Sorting file names | ICU4X's collator, with numeric ordering | `localeCompare(…, { numeric: true })` |
| Errors | thiserror, serialized with serde | `Error` and `formatError` |
| Frontend data | TanStack Query 5 | Apollo Client 4 |
| Native dialogs | tauri-plugin-dialog | none |
| Opening files | tauri-plugin-opener | `/raw/files/:id` in a browser tab |
| One window | tauri-plugin-single-instance | none |

`bundled` compiles SQLite into the app, so the version is the one we pin, not the system's. The schema needs SQLite 3.38 or later for `STRICT`, `unixepoch()` and generated columns. The error texts that `errors.ts` rewrites come from SQLite itself, so they don't change.

### 5.4 Commands

**What there is.**
- One command for each document the frontend sends today: page queries like `release_page` and `release_list`, and mutations like `update_release` and `set_release_tracks`. That's about 80 commands.
- Each page query returns what its route shows and nothing else. That's slunk's rule for its queries: one per route, asking only for what it shows.
- Pickers keep their own small search commands.
- `library_status` replaces slunk's `health` query.

**Their DTOs.**
- They live in `src-tauri/src/commands/` and use `#[serde(rename_all = "camelCase")]`.
- Each matches its GraphQL document's result, so components change little: `release.artists[].artist.name` and `creditedAs` stay where they are.
- Interfaces become tagged enums: `Tag` is `{ type: "text" | "number" | "date", … }`, so code that checked `__typename` checks `type`.
- Fragments like `ArtistHeader` become shared structs.

**Ids** are numbers. They're `i64` in Rust, exported to TypeScript as `number`. tauri-specta refuses 64-bit integers unless it's told how to export them, so set its bigint behavior to `Number`. SQLite ids stay far below 2^53. Ids that come from the URL are parsed with `Number()`.

**Batching.** A page's related names are loaded the way slunk's DataLoaders do: the page's rows first, then one query per relation over their ids, with the chunked `in (…)` helper from `helpers.ts`. A join would repeat each release once for every artist, label, format and medium it has. There's no DataLoader, since each command knows exactly what it needs.

**Threading.**
- Every command is `async`, and does its database and disk work on Tokio's blocking pool. In Tauri 2, a command that isn't `async` runs on the main thread and freezes the window while it works.
- There's one connection behind a mutex, as slunk had with better-sqlite3.
- slunk's rule stays: read files (hashing, durations) before taking the connection, never while holding it.

**Transactions.**
- `store.transaction(|tx| …)` runs a closure on a `rusqlite::Transaction`.
- Repositories are plain functions that take a `&Connection`, and a `Transaction` is one. A context that writes inside another's transaction passes the `tx` along.
- That removes a trap slunk had to document. There, a context's own store wasn't bound to the transaction, and starting a second transaction from it waited forever.

**Errors.**
- There's one serializable error, `{ kind, message }`, where `kind` is one of:
  - `noLibrary`: no library is open;
  - `notFound`;
  - `invalid`: a message for the form, like slunk's plain `Error`s ("The release still has 3 tracks. Delete them first.");
  - `internal`.
- SQLite's constraint errors go through the same table as `errors.ts`, ported as it is: `UNIQUE constraint failed: tracks.file_id` becomes "That file is already a track's audio". The triggers' own messages pass through.
- `QueryError` shows `message`, as it does today.

**Progress.** Long work can report through a `tauri::ipc::Channel`: registering files, the library check. Registering keeps the frontend's batching at first, so its progress bar works unchanged.

**Types.** tauri-specta writes `src/bindings.ts` from the commands and DTOs whenever a debug build starts. It wraps each result as `{ status: "ok", data } | { status: "error", error }`, and `useCommand()` unwraps it (§7.2). The generated file is committed, like `src/gql/` today.

### 5.5 Serving files

- **The URL** is `slunk://localhost/files/:id`, or `http://slunk.localhost/files/:id` on Windows. It's registered with `register_asynchronous_uri_scheme_protocol`, so reading a file never blocks the main thread. `fileUrl(id)` in `src/lib/query.ts` builds it, and replaces the six places that build `/raw/files/…` today.
- **What it serves** is what `files_controller.ts` serves: the bytes of a registered file, by id, read-only. An unknown id, or a file missing on disk, gets a 404. The content type comes from the extension.
- **Ranges.** It answers Range requests with 206 and `Content-Range`, so audio can seek. A protocol response is a whole body in memory, so a range with no end (`bytes=0-`) gets about 1 MB, and the media element asks for the rest as it plays.
- **CSP.** The app's CSP allows `slunk:` and `http://slunk.localhost` in `img-src` and `media-src`.
- **Opening a file.** "Open the file" opens it in its default app, with the opener plugin and the file's absolute path, from a command. Next to it, "Show in Finder" uses the same plugin.

### 5.6 Migrations

**The runner** is slunk's `migrator.ts`, ported as it is. `migrate` does this:
1. Opens its own connection with foreign keys off. A table rebuild drops the old table, and with foreign keys on, that would cascade deletes. The pragma can't change inside a transaction, so it's set first.
2. Compares the migrations with the `migrations` table. It fails if an applied migration is unknown, or if a pending one sorts before the last applied one.
3. If anything is pending, backs the library up with `VACUUM INTO` to `slunk-backups/slunk-<timestamp>.db`, next to the database:
   - a database that didn't exist yet isn't backed up;
   - two backups in the same second get a `-2`, `-3`, … suffix;
   - then it removes the oldest backups beyond `keepBackups` (§6.2). It only ever removes files named like its backups.
4. Starts `BEGIN IMMEDIATE`, which takes the write lock, and reads what's pending again. Then it runs each `up` and records it in `migrations`, which it creates the first time.
5. Runs `PRAGMA foreign_key_check`, and rolls back if any row breaks a foreign key.
6. Commits.

`rollback [n]` works the same way, running `down` for the last *n* migrations, newest first. After a failure, the database is as it was, and the backup is still there.

**The same names everywhere:**
- the `migrations` table and its rows (`20261011141955_create_baseline`);
- the backup files;
- and so, what retention may remove.

An existing library sees nothing to apply.

**Migrations are Rust modules.**
- Each one is `src/repo/migrations/m20261011141955_create_baseline.rs`, with `up` and `down` taking a `&Transaction`. They're listed in order in `migrations/mod.rs`, and a test checks the list is sorted and complete.
- The SQL of slunk's eight migrations is taken from what Kysely actually ran: run slunk's migrator on a fresh database with Kysely's query log on, and copy each migration's statements.
- `add_credited_names` rebuilds a table. So `rebuild_table`, slunk's helper for changes `ALTER TABLE` can't make, is ported, with the two steps slunk found it needs:
  - **`legacy_alter_table` is on for the rename.** Other tables' triggers name the dropped table, which would make the rename fail.
  - **The `AUTOINCREMENT` counter is put back afterwards,** so the id of a deleted row is never handed out again.

  It reads the old table's SQL and its `sqlite_sequence` value, so it can't be a fixed script.

**A library from a newer version.** When the library has an applied migration the app doesn't know, it's refused: "This library was upgraded by a newer version of liquidslunk."

**Parity tests**, part of phase 0's exit:
- **Fresh.** A database migrated by slunk and one migrated by Rust have the same `sqlite_schema`.
- **Upgrades.** From each of the eight intermediate states, seeded, finishing with Rust and finishing with slunk give the same `.dump`, apart from `applied_at`.
- **Rollback.** Every `down` reverses its `up`.

**Commands.** `crates/slunk-cli` keeps slunk's Ecto-style commands: `migrate`, `rollback [n]`, `status`, `gen <name>`, `seed` and `reset`, all taking `--db <path>`. The root `package.json` wraps them as `npm run db:*`.
- `gen` writes the module and adds it to `mod.rs`.
- `seed` runs through `legacy/slunk-be` until phase 6, since the seeds use the catalog.

**Rules:**
- Never edit a migration once it has been committed. Write a new one.
- Every `down` works, or fails with an explicit "irreversible" error.
- A migration runs inside the runner's transaction, so it never starts its own.
- Migrations don't use app code, which changes over time while a migration must not. The exception is `rebuild_table`, which is part of the migration system.
- Starter rows, like the first release formats, are inserted by migrations, and nothing may depend on them. Sample data goes in the seeds.
- No new migrations until the cut-over (§8).

## 6. Choosing the library

### 6.1 What happens at launch

```
launch
 └─ a library saved in the settings?
     ├─ no ─────────────────────────────────────────► Welcome
     └─ yes ─ is it there? ─ no ─────────────────────► Welcome: "Couldn't find …", Try again
                  └─ yes ─ open it (§6.3) ─ fails ───► Welcome, with the reason
                                  └─ opens ──────────► the app, at /
```

**Welcome** is the route `/library`, outside `AppLayout`, so it has no sidebar. It offers:
- **Open a library…**: a file dialog, filtered to `.db` files.
- **New library…**: a folder dialog. It creates `slunk.db` in that folder and migrates it. A folder that already has one offers to open it instead.
- **Recent libraries:** the last five opened, each with its folder. One that isn't there right now is dimmed and says so. That's usual for a library on an external drive.

Saving the choice is what makes it last. The next launch opens the same library without asking, until another one is chosen.

### 6.2 The settings file

- **Where:** `settings.json` in the app's config folder (`app_config_dir()`; on macOS, `~/Library/Application Support/com.liquidslunk.desktop/`).
- **Who reads it:** Rust owns it. The webview never reads it, only calls commands: `library_status`, `open_library`, `create_library`, `close_library` and `recent_libraries`. So it needs no capability or store plugin.
- **When it's written:** atomically (a temporary file, then a rename), and only once a library has opened, so a library that fails to open never becomes the saved one.

```json
{
  "version": 1,
  "library": "/Users/…/Music/slunk.db",
  "recent": ["/Users/…/Music/slunk.db", "/Volumes/Archive/slunk.db"],
  "keepBackups": 10
}
```

- `library` is the absolute path to the database. Paths in `files.path` stay relative to its folder, so the library still moves as a whole.
- `keepBackups` is how many backups `slunk-backups/` keeps: a whole number from 1, 10 by default, as slunk's `SLUNK_KEEP_BACKUPS` was. It has no UI yet.
- slunk's other settings go away: `SLUNK_DB` is the choice above, and there's no `PORT`.

### 6.3 Opening a library

`open_library(path)` and `create_library(folder)` do this, in order, on the blocking pool:
1. **New library:** the folder exists and can be written, and has no `slunk.db` yet.
2. **Existing library:** the file is an SQLite database, and either has a `migrations` table or is empty. Any other database is refused: "That isn't a Slunk library."
3. **Migrate**, with its backup (§5.6). While anything is pending, Welcome shows "Backing up and upgrading the library…". A library from a newer version is refused.
4. **Open** the app's connection, with `foreign_keys = ON`, `journal_mode = WAL` and `busy_timeout = 5000`. Wire the contexts as `server.ts` does, and keep them in Tauri's state as the open library.
5. **Save** the settings.
6. **In debug builds before phase 6, start the sidecar** (§8).

Each failure comes back as an `invalid` error, which Welcome shows in words. The migration's result (the backup made, the migrations applied) comes back too, for a toast.

Commands that need a library return `noLibrary` when none is open, and the frontend sends the user to `/library`.

### 6.4 Switching and closing

> Changed during implementation, see [Appendix A](#appendix-a-switching-opens-the-new-library-before-closing-the-old-one) and [Appendix B](#appendix-b-the-sidecar-stops-before-the-connection-closes).

- **The sidebar's header** shows the library's folder name, with a menu: Switch library…, Show in Finder, and the recent libraries.
- **Switching:**
  1. Close the connection, which checkpoints the WAL and removes `-wal` and `-shm`.
  2. Stop the sidecar.
  3. Clear TanStack Query's cache, and Apollo's while it's there, so nothing from the old library shows.
  4. Open the new library, then go to `/`.
- **Per-library browser state.** `slunk.imageFolder` holds a folder relative to the library, so its `localStorage` key includes the library's path. Theme, "Hide credits" and "Hide alternate titles" stay global.
- **Closing the app** closes the connection the same way, on `RunEvent::Exit`.
- **One instance.** tauri-plugin-single-instance focuses the open window instead of starting a second app on the same library.

### 6.5 In development

- **Separate settings.** Debug builds use `settings.dev.json`, so `npm run tauri dev` never opens the library the installed app uses. The first dev run asks: open the library in `dev-library/music/`, or create one in a new folder of `dev-library/`. Development never points at a real music library, and this makes that hard to get wrong.
- **`--library <path>`**, in debug builds only, opens a library without touching the settings, for scripts and screenshots.
- **Tests** open temporary databases. They never read `dev-library/`, which a fresh clone doesn't have. Audio tests use tiny fixtures committed with the tests: a second of silence in each format, with tags set.

## 7. Frontend

### 7.1 Running in Tauri

- **Vite:** `server.strictPort`, `clearScreen: false`, and `src-tauri/` and `crates/` left out of the watcher. The proxy keeps `/graphql` until phase 6 (§8), and drops `/raw` in phase 0.
- **The router** stays `createBrowserRouter`, and lists keep their state in the query string, as in slunk. Phase 0 checks that reloading a deep route works in a release build. If it doesn't, `createHashRouter` changes nothing else.
- **Back and forward.** A desktop window has no browser buttons, and the back button is how a list is found again with its page, sort and filters. So the header gets back and forward buttons, and ⌘[ and ⌘].
- **`localStorage`** works as it does in a browser, per app.
- **Fonts** are already bundled, through Fontsource.

### 7.2 TanStack Query

- **Defaults:**
  - `staleTime: 0`: a page shows what's cached and fetches again, which is what `cache-and-network` does today;
  - `retry: false`: a local call that fails will fail the same way again;
  - `refetchOnWindowFocus: false`.
- **`useCommand(name, args, options)`** is the one way to read:
  - It calls `commands[name](args)`, and throws the `{ kind, message }` error, or returns the data.
  - Its query key is `[name, args]`. The name is a string, never `fn.name`, which minification renames.
- **Forms start from fresh data.** `useCommand(…, { fresh: true })` sets `gcTime: 0`, so a form never seeds its state from a cached copy that's out of date. That's what `network-only` does today, on the forms, the import's steps and the library check.
- **Mutations** use `useMutation`. Callers `await mutateAsync()` inside `try` and show `QueryError`, as they do now.
- **Invalidation:** the `MutationCache`'s `onSuccess` invalidates every query. Only mounted queries fetch again, usually one or two, over local IPC. That replaces `refetchQueries` and the `merge: false` policies. Make it narrower only if it shows up as slow.
- **Deletes:** go to the list, then `removeQueries` for the deleted entity's keys. That replaces `cache.evict`.
- **The edit tabs' header** calls a small `release_header` command instead of reading the cache (`cache-only` today).
- **No other state library.** TanStack Query holds what comes from the backend. The URL and React state hold the rest, as now.

### 7.3 Converting a route

| Today (Apollo) | Then (TanStack Query) |
|---|---|
| `useQuery(Doc, { variables })` | `useCommand("releasePage", { id })` |
| `fetchPolicy: "network-only"` | `useCommand(…, { fresh: true })` |
| `fetchPolicy: "cache-only"` (edit tabs) | Its own small command |
| `@include(if: $withRelease)` | An optional argument, and a nullable field in the DTO |
| `useMutation(Doc)`, `try { await mutate() }` | `useMutation({ mutationFn })`, `try { await mutateAsync() }` |
| `refetchQueries: ["Releases"]` | Nothing: every mutation invalidates |
| `cache.evict(…)` | `queryClient.removeQueries(…)` |
| Fragments (`...ArtistHeader`) | A shared DTO struct |
| `__typename === "TextTag"` | `tag.type === "text"` |
| `id: string` | `id: number` |
| `` `/raw/files/${id}` `` | `fileUrl(id)` |

A converted route has no `graphql()` documents left, and its DTOs are the port's contract with it.

## 8. The transition

The backend moves one context at a time. To keep every route working in the meantime, slunk's backend runs beside the app, in development only.
- **`legacy/`** is slunk as of its last commit, laid out as slunk was, so the docs' links to each other still work.
  - **`legacy/README.md`** is the one new file. It says what the folder is, that it's read-only, and that liquidslunk's rules are the root `AGENTS.md`'s.
  - **`legacy/docs/`** is slunk's docs, copied whole. Each file starts with one added line, and nothing else in it changes:

    > **Legacy:** this is slunk's documentation, copied for the port. It describes `legacy/slunk-be/` and slunk's frontend before the port, not liquidslunk. Its citations ("design §4.3") mean slunk's design docs, in `legacy/docs/design/`. It's read-only, and phase 6 deletes it.

  - **`legacy/slunk-be/`** is slunk's backend, with one change: `SLUNK_SKIP_MIGRATE=1` skips the migration on start, since Rust owns migrations from phase 0.
  - **slunk's `AGENTS.md` and `CLAUDE.md` aren't copied.** Agents load instruction files from the folders they work in, and slunk's instructions aren't liquidslunk's.
  - **slunk's frontend isn't in `legacy/`.** It's in `src/`, where it becomes liquidslunk's. The legacy docs' paths into `slunk-fe/` mean what's now in `src/`, as it was before the port.
- **The sidecar.**
  - When a library opens in a debug build, `src-tauri/src/sidecar.rs` starts `node legacy/slunk-be/src/server.ts` on port 4000, with `SLUNK_DB` and `SLUNK_SKIP_MIGRATE=1`.
  - Vite proxies `/graphql` to it, as the frontend expects already, so Apollo needs no change and there's no CORS.
  - Rust stops it when the library closes or the app exits. It doesn't need the shell plugin, since the webview never starts it.
- **No release builds.** Release builds never start the sidecar, so they aren't made until phase 6. Until then, the real library is opened with slunk itself, or with `npm run tauri dev`. If a release build is needed sooner, see §12.
- **Both write the same database.** WAL and `busy_timeout` handle it, and each request reads the latest commit.
- **Keeping both caches fresh.** After an Apollo mutation, TanStack Query invalidates everything. After a command's mutation, Apollo refetches its active queries. That's a few lines in the providers.
- **The schema is frozen until phase 6.** The legacy backend wouldn't know a new table or column. Also, slunk refuses a library with a migration it doesn't have, and slunk is the safety net until the cut-over. After phase 6, the first new migration is a one-way door: slunk can't open that library again, apart from its backups.
- **What moves when.** A route moves when the context behind it is ported. Then all of its data comes from commands, and its documents are deleted. Shared components, like the pickers, move with the context they search.
- **Tracking progress.** `grep -rl 'graphql(' src` lists what's left. Phase 6 starts when it lists nothing.

## 9. Phases

Each phase is a vertical slice, as in slunk: it ports a context with its commands, converts the routes that use it, documents what it built, and ends with something usable. Its docs are the legacy docs' sections for what it ported, rewritten for the new code and checked against it (§2.1). At the end of a phase, its routes make no GraphQL requests, and every code check in AGENTS.md's "Before you finish" passes. Phase 0 adds the checks, and phase 6 leaves their final list.

**Phase 0: Shell and library**
- The repository:
  - all of slunk copied at once, from its last commit: its frontend at the root, and its backend and docs in `legacy/` (§8);
  - only the files git tracks, with `git archive`, so nothing local comes along: no `node_modules/`, builds, `.env`, databases, or `temp/` files;
  - a fresh history: slunk's files are copied in, not its commits;
  - `dev-library/` is there already, git-ignored except its `.gitkeep`. `dev-library/music/` holds copies of real music files, and in `slunk-backups/`, two backups of slunk's database for them, from before most of the migrations.
- The Tauri 2 scaffold, named liquidslunk, with the identifier `com.liquidslunk.desktop` (§12).
- The Cargo workspace, `crates/slunk` and `crates/slunk-cli`, with Node and Rust pinned in `mise.toml`.
- Rust:
  - the connection;
  - the migrator, `rebuild_table`, the eight migrations and their parity tests (§5.6);
  - backups and retention;
  - `migrate`, `rollback`, `status`, `gen` and `reset` in the CLI;
  - the error type, the settings, and opening, creating, switching and closing libraries (§6);
  - the file protocol (§5.5);
  - `library_status`;
  - tauri-specta.
- The sidecar (§8).
- The frontend:
  - the Tauri settings in Vite;
  - TanStack Query and `useCommand()`;
  - Welcome and the redirect to it;
  - the sidebar's library menu;
  - `fileUrl()`;
  - back and forward;
  - the dashboard's status from `library_status`.

  Every other route stays on Apollo.
- Choose how to take screenshots (§10).
- Docs, as `docs/README.md` lays them out:
  - `docs/development/getting-started.md`: setting up Node, Rust and Tauri, and running the app;
  - `docs/development/frontend.md`: the frontend as it's moved in, with its conventions, carried over from the legacy frontend doc;
  - `docs/development/backend.md`: the Rust backend so far, and the sidecar while it runs;
  - `docs/reference/database.md`: the schema the ported migrations build, and the rules it enforces, carried over from the legacy reference and checked against the parity tests.
- AGENTS.md:
  - "Where things are" lists the code's folders, and `legacy/`: it's read-only, its docs are slunk's, and their rules don't apply;
  - "Before you finish" gets the code checks:
    - `npm run typecheck`, `npm run lint` and `npm run format` for the frontend;
    - `cargo fmt --check`, `cargo clippy` and `cargo test` for Rust;
    - the bindings regenerated;
    - migrations apply and roll back cleanly through the CLI;
    - `npm run codegen` after changing a GraphQL document, until phase 6;
    - a screenshot of any change that shows in the UI, taken the way phase 0 settles on.
- *Exit:*
  - The first launch asks for a library. Opening one that needs migrating backs it up, migrates it and shows the whole app working, through the sidecar. That's one of the backups in `dev-library/music/slunk-backups/`, copied to `dev-library/music/slunk.db`.
  - Creating a new library in `dev-library/` works too.
  - A restart opens it without asking, and switching libraries works.
  - The migration parity tests pass.
  - Audio plays and seeks on the release page.
  - Reloading a deep route works in a release build.

**Phase 1: Media**
- Port:
  - `media.ts` and `media/` (`library.ts`, `file_types.ts`, `audio.ts`);
  - `files_repository.ts`, and the file links in `links_repositories.ts`;
  - `paging.ts` and `helpers.ts`.
- Commands: `files`, `file`, `file_counts`, `folder`, `folder_files`, `register_files`, `update_file` and `delete_file`.
- Routes: `/files` and its CRUD pages, `FolderBrowser`, `FileUses`, and the file picker.
- *Exit:*
  - Files are browsed, registered, relinked and deleted through commands.
  - Files registered by Rust have the same `path`, `size_bytes`, `mtime` and `sha256` that slunk gives the same files (§10).
  - Durations and tags match on the sample formats.

**Phase 2: Artists, labels and lists**
- Port the catalog's artists and labels, release formats and media, and languages:
  - alternate names, credited names, and `saveArtistNames` with its swaps;
  - pictures and logos;
  - an artist's file links.

  Also `text.ts` and `languages.ts`.
- Routes:
  - `/artists` and its form, `/labels` and its form, `/release-formats` and `/release-media`;
  - the artist, label, format, medium and language pickers.

  The artist page's tabs and the label's read page show releases, so they wait for phase 4.
- *Exit:* artists and labels, with their alternate names, are created, edited, renamed with "keep the old name", and deleted through commands, with the same results as through GraphQL (§10).

**Phase 3: Tags**
- Port `tagging.ts`, `tags_repository.ts` and `dates.ts`.
- Routes:
  - `/tags` and its pages;
  - the tag and tag value pickers, and `TagFilters`;
  - the release's Tags tab.
- *Exit:* tag definitions and values work through commands, including single-value rules and switching a tag to single.

**Phase 4: Releases**
- Port the catalog's releases:
  - details, alternate titles, artists as credited, labels, formats and media;
  - the cover and file links, and credit lines;
  - the release list's filters and `releaseIds`, and bulk edits;
  - the dashboard's totals.
- Routes:
  - `/releases`, the release page, `/releases/:id/files`, and the Details, Files and Credits tabs;
  - the bulk edits;
  - the artist page and its tabs, the label's read page;
  - the rest of the dashboard.
- *Exit:* "releases where Genre is Rock and Score ≥ 8" works, and so do the release page and every edit tab but Tracks, through commands.

**Phase 5: Tracks and import**
- Port tracks, `setReleaseTracks` with disc titles, track tags, the import's suggestions (`suggestRelease`, `suggestTracks`, `suggestArtists`), and `addArtistName`.
- Routes: `/tracks` and its pages, the Tracks tab, and the import's steps, from picking a folder to setting tags.
- *Exit:* a release is imported from a folder from start to finish through commands, with titles split at " = " and artists matched by any of their names.

**Phase 6: Library check and cut-over**
- Port `library_check.ts`, `relinkFiles` and `rehashFiles`, then convert `/library-check`.
- Port `seeds.ts` to `slunk-cli seed`.
- Delete:
  - `legacy/` (slunk's backend and docs), the sidecar, the `/graphql` proxy and `SLUNK_SKIP_MIGRATE`;
  - Apollo, `graphql`, graphql-codegen and `src/gql/`.
- Docs:
  - `backend.md` describes only the Rust backend;
  - the Data section of `frontend.md` describes commands and TanStack Query;
  - AGENTS.md drops GraphQL and `legacy/`. Its "Before you finish" then holds the final code checks, for the frontend and the Rust backend, with `npm run codegen` gone.
- A release build: icons, and the `.app` and `.dmg`, signed ad hoc (§12).
- Mark this design complete.
- *Exit:*
  - The installed app opens the real library and does everything slunk did.
  - `grep -rl 'graphql(' src` finds nothing, and nothing listens on a port.
  - Every code check in AGENTS.md's "Before you finish" passes on a fresh clone.

## 10. Parity and risks

**Checking behavior.**
- slunk's backend doc, `legacy/docs/development/backend.md`, describes the backend rule by rule. Each phase carries its context's rules into `docs/development/backend.md` as it ports them, and each rule gets a Rust test. So the legacy doc is the checklist, and a rule moves only once the code and a test do it.
- For writes, a script runs the same mutations on two copies of a seeded library, one through GraphQL and one through commands, then compares their `.dump`s, leaving out `inserted_at`, `updated_at` and `applied_at`. It's written in phase 1 and reused after that.

**Details that have to match exactly.** If one is off, the first library check after the switch flags every file, or an import orders its tracks differently.
- **mtime** is whole seconds, rounded down (`Math.floor(mtimeMs / 1000)`).
- **Paths:**
  - resolved and made relative lexically, without touching the disk, as Node's `path` does;
  - `../` for files above the library's folder;
  - forward slashes always;
  - symlinks followed when browsing, but not when scanning.
- **File names** sort with ICU collation and numeric ordering, so "2" comes before "10" in folders and in the import's track rows.
- **Text** is NFC, then trimmed like JavaScript's `trim()`. That also trims U+FEFF, which Rust's `trim()` doesn't.
- **Lowercasing** in `catalog.ts` and `tagging.ts` is ASCII only, so `to_ascii_lowercase()`, never `to_lowercase()`.
- **Language tags** are checked against a table of what `Intl.getCanonicalLocales` returns now: case, aliases, and what it refuses.
- **Hashes** are lowercase hex, which a CHECK enforces.

**Audio.**
- lofty doesn't read DSF, DFF or WMA, which music-metadata does. A file it can't read gets no duration and no tags, as an unreadable file does today, so these degrade without failing.
- Phase 1 counts the extensions of the real library's music files. If those formats are there, it adds a small reader for DSF's header and ID3 chunk, and decides what to do about WMA.
- lofty's tag keys are checked against music-metadata's for album artist, the disc's title (`DISCSUBTITLE`, `TSST`) and dates, on samples of every format.

**Media over the custom protocol.** WebKit asks for audio in ranges. Phase 0 checks seeking in FLAC, MP3 and M4A. `dev-library/music/` only has FLAC, so phase 0 adds an MP3 and an M4A there. If it doesn't work, the fallback is the asset protocol, with its scope widened at runtime to each registered file's folder.

**tauri-specta** is still a 2.0 release candidate. Pin its exact version. If it gets in the way, ts-rs for the types and a thin hand-written `invoke` wrapper per command do the same job.

**Screenshots.** Every change that shows in the UI gets looked at in a screenshot. slunk took them with Playwright on a plain browser tab, which no longer works, since there's no IPC there. Phase 0 tries, in order:
- WebDriver inside debug builds (`tauri-plugin-wdio-webdriver`, or WebdriverIO's Tauri service). These support macOS, unlike the official `tauri-driver`.
- `mockIPC` with fixtures, for checks that only need the layout.

> Changed during implementation, see [Appendix C](#appendix-c-screenshots-go-through-webdriver-behind-a-feature).

**macOS privacy prompts.** macOS asks before an app reads Desktop, Documents, Downloads, or external and network volumes. It asks once per app, and the dev and release builds count as two apps.

**Network drives.** SQLite's WAL doesn't work on network file systems. That was already true of slunk, but a picker makes it easy to choose a library on a NAS. Welcome says so when opening one fails.

## 11. Alternatives considered

- **The sidecar as the end state (§5.1 A).** It's the least work, but it isn't one executable:
  - a ~100 MB Node build, with better-sqlite3's native addon beside it;
  - or `node:sqlite`, still a release candidate, which would need a Kysely dialect.

  It also keeps an open port and a second process to manage. It stays useful as the bridge (§8), in development only.
- **Rust with GraphQL over IPC (§5.1 B).** async-graphql, with an Apollo link that calls one `graphql` command, would leave the frontend's 84 documents untouched. It keeps everything §3 lists against GraphQL, and puts a schema between Rust and TypeScript that one step of tauri-specta doesn't need. It's the fallback if converting the frontend stalls.
- **Porting everything, then switching, without a bridge.** It's simpler, since there's one data client at a time. But liquidslunk would be unusable until the end, and the library picker couldn't be tried with real pages. The bridge is cheap, since the Vite proxy already exists and nothing is packaged.
- **sqlx instead of rusqlite.**
  - It brings async pools and compile-time checked queries, but only for static SQL, and the hard queries here are dynamic: sorts, searches, tag filters.
  - Its migrator runs each migration in its own transaction, without the backup and foreign key check of §5.6.
  - rusqlite matches better-sqlite3: synchronous, one connection, SQLite compiled in.
- **tauri-plugin-sql.** It hands the webview raw SQL. Writes would go around the contexts, and the split of §5.2 would be gone.
- **The asset protocol for file bytes (`convertFileSrc`).** It serves any path in its scope, and files may sit anywhere (`../`), so the scope would be the whole disk. The custom protocol serves registered files only, by id, like `/raw/files/:id`. It's the fallback in §10.
- **tauri-plugin-store for the settings.** It works, but only Rust reads the settings, before any page loads. A file and serde do the job without a plugin or a capability.
- **TanStack Router.** React Router 8 is already in use, in data mode, and does what's needed.

## 12. Settled questions

1. **Platforms:** macOS only for now. Nothing in this design is macOS-only, and Windows and Linux get checked when they're wanted.
2. **Repositories are plain functions,** not traits.
   - slunk's design asked for repositories that let the database be swapped later, which in Rust means traits and generics throughout.
   - Here, repositories are functions over a `&Connection`, in the same modules: contexts call only repositories, and commands call only contexts.
   - That keeps the boundaries without the traits. They can come back if a second database ever does.
3. **Names:**
   - The app is called liquidslunk, and its bundle identifier is `com.liquidslunk.desktop`. The identifier names the config folder that holds the settings (§6.2), so it doesn't change.
   - The database keeps slunk's names, `slunk.db` and `slunk-backups/`, so existing libraries open as they are, and the backup naming and retention rules stay.
4. **No release builds before the cut-over.** If one is ever needed sooner, the sidecar gets packaged as a Node single executable with better-sqlite3's addon as a bundled resource, which is a phase of its own.
5. **Signing:** ad hoc, which is enough for one machine. Distributing the app needs a Developer ID and notarization, revisited when the app is shared.

Still open:

6. **Screenshot tooling:** WebDriver in debug builds, or `mockIPC` fixtures (§10). Phase 0 decides, trying WebDriver first.

   > Settled during implementation, see [Appendix C](#appendix-c-screenshots-go-through-webdriver-behind-a-feature).

## Appendix A: Switching opens the new library before closing the old one

- **Date:** 2026-10-17
- **Affects:** §6.4 Switching and closing
- **Was:** switching closes the open library and stops the sidecar first, then clears the caches and opens the new library.
- **Now:** opening another library checks it and migrates it first (§6.3, steps 1 to 3), while the open one stays open. Only then does the open one close, before the new connection opens. The frontend clears its caches once the new library is open. "Switch library…" in the sidebar still closes the open library, then shows Welcome.
- **Why:** a library that fails to open, like one that isn't there or isn't a Slunk library, leaves the open one as it was, instead of no library at all.

## Appendix B: The sidecar stops before the connection closes

- **Date:** 2026-10-17
- **Affects:** §6.4 Switching and closing
- **Was:** closing a library closes the connection, which checkpoints the WAL and removes `-wal` and `-shm`, then stops the sidecar.
- **Now:** closing a library, and the app's exit, stop the sidecar first, then close the connection.
- **Why:** SQLite only removes `-wal` and `-shm` when the app's connection is the last one to close. When slunk's backend closed last, they stayed next to the database. Both orders were checked, with the app's connection and slunk's backend on the same library.

## Appendix C: Screenshots go through WebDriver, behind a feature

- **Date:** 2026-10-17
- **Affects:** §10 Screenshots, §12 question 6
- **Was:** still open: WebDriver inside debug builds, or `mockIPC` fixtures. Phase 0 decides, trying WebDriver first.
- **Now:** WebDriver. tauri-plugin-wdio-webdriver embeds a W3C WebDriver server on `127.0.0.1:4445` in builds with the `webdriver` cargo feature, which is off by default: `npm run tauri dev -- --features webdriver`, then `npm run screenshot`. `mockIPC` isn't needed.
- **Why:** it works on macOS's WKWebView, with real input, which Base UI's menus need. It's an opt-in feature rather than part of every debug build, since it opens an automation server on localhost. That also lets a release build be checked the same way, as phase 0's check of reloading a deep route needed.
