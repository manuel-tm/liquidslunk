# AGENTS.md

Guidance for AI coding agents working in this repository. People should start with [README.md](README.md).

## Project

liquidslunk is a local music library manager, built as a desktop app on Tauri 2. Its SQLite database is the source of truth for all music metadata, and it only *reads* the user's music files.

It's a port of slunk, an earlier version of the same app, with a Node backend and a web frontend. slunk is frozen, and this repository stands on its own: see [slunk is a reference, never a dependency](#slunk-is-a-reference-never-a-dependency).

## Where things are

- **[`docs/design/`](docs/design/): design docs.** These are the plans, used for planning and prompting, and an archive once they're built. [`docs/design/README.md`](docs/design/README.md) lists them with their status. [`1_introduction.md`](docs/design/1_introduction.md) is the base design: the port to Tauri 2, its architecture and its phases. Read its relevant sections before changing the backend's structure, the commands, how the library is chosen, the migrations, or anything that touches files.
- **The rest of [`docs/`](docs/README.md): what's actually built,** kept up to date with the code. Start at `docs/README.md`.
- **`temp/`: local assets for prompting, never committed.** See [Never reference files in `temp/`](#never-reference-files-in-temp).

There's no code yet. Phase 0 of `1_introduction.md` brings it in, and adds its folders here.

Work happens in phases. Each design doc plans its own, numbered from 0, so a phase is named with its doc. Stay within the current phase unless asked otherwise.

When citing a design doc in code comments or docs, "design §5.4" and "phase 2" with no document named mean `1_introduction.md`. Name any later design doc: "2_topic.md §3", "phase 0 of 2_topic.md".

## Workflow: design → code → docs

The full process is in [docs/README.md](docs/README.md#how-we-work).

- **Build from the design docs.**
- **Keep `docs/` up to date.**
  - Any change that affects something documented updates the docs in the same change: requirements and setup, commands and scripts, project structure, conventions, and behavior.
  - If you find a doc that no longer matches the code, fix it, even if your change didn't cause it.
  - Document only what exists, and never copy plans from `docs/design/` into the rest of `docs/`.
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

### slunk is a reference, never a dependency

liquidslunk ports slunk's schema, behavior and routes ([design §2.1](docs/design/1_introduction.md#21-what-the-port-keeps)). slunk's own repository will be deleted once the port is done, so this one has to stand on its own:
- Read slunk's source for how something worked, when it's available to you. Never point to it: no local path, relative path or link to slunk's repository in docs, design docs, code, comments, commit messages or PR descriptions. For anyone else, and later for everyone, it would point to nothing.
- Never import, symlink or copy from it at build or run time. What liquidslunk needs from slunk is copied into this repository, as phase 0 copies its frontend and backend, or written into the docs.
- Naming slunk is fine ("a port of slunk's import"). Pointing at its files isn't.

### Never write to the user's music files

- liquidslunk never moves, copies, renames, deletes or retags a user's file. Code that touches the user's files may only read them.
- The only things it writes are next to its database: `slunk.db`, SQLite's `-wal`/`-shm` files, and `slunk-backups/`. Apart from those, it writes its settings in the app's config folder ([design §6.2](docs/design/1_introduction.md#62-the-settings-file)).
- For development and tests, use a scratch library with sample files. Never open a real music library from a development build ([design §6.5](docs/design/1_introduction.md#65-in-development)).

### Never reference files in `temp/`

- `temp/` holds assets the user shares while prompting. Only its placeholder, `temp/.gitkeep`, is committed; the root `.gitignore` ignores everything else, so those files exist only on the user's machine.
- Never reference a file in `temp/` in anything that gets committed or published: docs, design docs, code, comments, commit messages or PR descriptions. For anyone else, the reference would point to nothing.
- If something from a `temp/` file belongs in the docs, write that content into the doc itself.

### The database is the part built to last

The schema is slunk's, carried over unchanged. Everything else can change faster. For the database:

- Change the schema only through a new migration. Never edit a migration that has already been committed.
- Enforce integrity in the database (constraints, triggers) wherever it can be expressed, not only in app code.
- No new migrations until the cut-over: the schema stays frozen while the port runs ([design §8](docs/design/1_introduction.md#8-the-transition)).

### Not designed yet: external libraries

Don't implement export or sync, and don't add tables for it. It needs its own design doc in `docs/design/` written and agreed first.

## Before you finish

- **Code:** there's none yet. Phase 0 of `1_introduction.md` adds the checks for the frontend and the backend here.
- **Docs:** everything in `docs/` outside `docs/design/` matches the code after your change. A design change made while building that design's phases is recorded as an appendix, and no other design doc was touched.
- **References:** nothing you wrote points to a file in slunk's repository or in `temp/`.
- **Report:** say what you verified and what you didn't.
