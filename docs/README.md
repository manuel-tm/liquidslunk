# liquidslunk documentation

This folder documents liquidslunk as it's actually built, and it's kept up to date with the code (see [Keeping docs up to date](#keeping-docs-up-to-date)). Plans live separately, in [`design/`](design/): they record what was decided, and why. What liquidslunk does is what the code and these docs say ([Design docs and these docs](#design-docs-and-these-docs)).

liquidslunk is a port of slunk, an earlier version of the same app. slunk's own docs aren't in this repository: what liquidslunk kept from them was rewritten into these docs, for the new code, as each part was ported. These docs describe only what's built here.

## Contents

| Path | What's in it |
|---|---|
| [`design/`](design/) | Design docs: numbered plans, used for planning and prompting, and a record of what was decided, and why, once they're built. [design/README.md](design/README.md) lists every design doc and its status. |
| [`development/getting-started.md`](development/getting-started.md) | Setting up and running the app |
| [`development/frontend.md`](development/frontend.md) | How the frontend is built, and its conventions |
| [`development/backend.md`](development/backend.md) | How the Rust backend is built: the contexts, the database and its migrations, the commands, and the library |
| [`reference/database.md`](reference/database.md) | The database schema as built, and the rules it enforces |
| [`reference/ipod.md`](reference/ipod.md) | What liquidslunk reads and writes on an iPod: its iTunesDB, its artwork and Play Counts, byte by byte |

New docs are added as the code they describe lands:
- `development/` holds how to set up, run and work on the app.
- `reference/` describes what's built: the database, and the iPod's formats. The commands are documented by their Rust doc comments, which tauri-specta copies into `src/bindings.ts`.

## Repository layout

```
docs/            documentation (this folder)
  design/        design docs (plans)
  development/   setup and conventions for the code
  reference/     the database schema, and the iPod's formats
src/             the frontend: React, in the app's webview
src-tauri/       the app: Tauri, its commands, the library, the file protocol
crates/slunk/    the domain and its database: contexts, repositories,
                 migrations, with no Tauri dependency
crates/slunk-cli/  the database commands (npm run db:*)
scripts/         scripts for working on the app: screenshots
dev-library/     scratch libraries for development, never committed
temp/            local assets for prompting, never committed
AGENTS.md        instructions for AI coding agents (CLAUDE.md points to it)
mise.toml        pinned Node and Rust
Cargo.toml       the Cargo workspace
package.json     the frontend, the Tauri CLI, and every npm script
README.md
LICENSE
```

`temp/` holds files you share with an agent while prompting (screenshots, notes, sample data). Only its placeholder, `temp/.gitkeep`, is committed; the root `.gitignore` ignores everything else. Because those files exist only on your machine, nothing that gets committed (docs, design docs, code, commit messages) may reference them. If something in one of them belongs in the docs, write it into the doc itself.

`dev-library/` holds scratch libraries for development. `dev-library/music/` is a copy of part of a real library, so, as with `temp/`, only its placeholder (`dev-library/.gitkeep`) is committed, and nothing committed depends on what's in it.

The same goes for slunk: its repository won't stay around, so nothing here points to its files. What liquidslunk needs from it is copied into this repository, or written into these docs.

## What liquidslunk writes

liquidslunk only reads your music files: it never moves, copies, renames, deletes or retags them. It writes in three places:
- **Next to the library's database:** `slunk.db`, SQLite's `-wal` and `-shm` files, and `slunk-backups/`.
- **Its settings,** in the app's config folder ([backend.md](development/backend.md#choosing-the-library)).
- **An external library's destination,** when it syncs: the files it converts or copies there, `cover.jpg`s, and its manifest and temporary files in `.liquidslunk/`. It replaces or deletes only what its manifest lists, and never writes outside the destination's folder. While it converts, it uses a folder of its own in the system's temporary folder ([backend.md](development/backend.md#syncing-a-folder-library)).
- **An iPod,** when it syncs: the files it converts or copies into `iPod_Control/Music`, the iTunesDB, the ArtworkDB and its thumbnails, a copy of the old iTunesDB, and its manifest and temporary files beside the iTunesDB. It deletes only what its manifest lists, `Play Counts` once it's counted, and, once you agree, the music files of tracks it didn't write ([reference/ipod.md](reference/ipod.md#the-files)).

## How we work

Work flows from design to code to docs:

1. **Design.** A topic is planned in `design/N_topic.md`, with the next free number, and gets a row in the index, [design/README.md](design/README.md). Its status line, right under its title, reads `> **Status: in design.**`. While it's being designed, it's edited freely.
2. **Code.** The code is built from the design, phase by phase, and its status line changes to `> **Status: being built.**`. While the phases are being built, the design's text stays as it was, and only appendices are added (step 4).
3. **Document.** When code lands, what it does is documented here, in the same change, with what a reader needs from the design: its rules, and the reasons for them that aren't plain. These docs describe only what exists, and they stand on their own: nothing here needs a design doc to be understood. Plans that weren't built stay in the design.
4. **Record design changes as appendices.** When the code shows that a design has to change, while its phases are being built, the design text isn't rewritten:
   - an appendix is added at the end of that design doc;
   - a one-line note is added at the affected section, so nobody follows the old text by mistake.

Appendices are lettered in order (A, B, …). For example (this one is made up):

```markdown
## Appendix A: The sidecar picks its own port

- **Date:** 2026-11-02
- **Affects:** §8 The transition
- **Was:** the sidecar listens on port 4000, which the Vite proxy expects.
- **Now:** Rust picks a free port and passes it to the sidecar and the proxy.
- **Why:** slunk, still running on port 4000, kept the sidecar from starting.
```

And at the affected section:

```markdown
> Changed during implementation, see [Appendix A](#appendix-a-the-sidecar-picks-its-own-port).
```

5. **Mark a design complete** when all its phases are built: a status line under its title, and its row in [design/README.md](design/README.md). For example (also made up):

   ```markdown
   > **Status: complete.** All of this design's phases (§9, phases 0 to 6) were built by 2027-01-15.
   ```

6. **Leave it alone.** A finished design doc is never edited again, not even to add an appendix: it's a record of what was planned and built. A later change to what it describes is made in the code and documented here, like any other change. A change that needs planning gets a new design doc.

A design doc is only ever edited by the work on its own topic. Every other design doc is read-only, whatever its status: it's read for context, but never edited, not even to fix a typo, a stale name or a broken link.

### Design docs and these docs

A design doc is how a decision was reached: what was planned, what else was considered, and why. It's read when planning a change, or to learn why something was decided as it was. It isn't the last word on what the code does: once a design is built, the code and these docs are. They keep changing, while a finished design is never edited again, so what it says drifts from what's built.

So nothing outside `design/` cites a design doc: not the code's comments, not these docs, the README or AGENTS.md. That's no "design §5.4" or "2_topic.md §3", no phase or appendix of one, and no link to one for what the code does. Saying where the designs are, as this page does, isn't citing one.
- **Code comments** that need more than they say point to the section of these docs that describes it, by the doc's file name and the section's heading: `(backend.md, "Running migrations")`. File names are unique in `docs/`. A module's or a component's comment points once, and what's inside it points again only to another section.
- **These docs** link to each other, by section.
- **What a design says that a reader needs,** a rule or the reason for one, is written here, so nobody has to read a design to know what the code does, or why.
- **slunk's docs** aren't in this repository, so they aren't cited either. What the code needs from them is in its comments, or in these docs.

Design docs cite each other as they're written. Each numbers its own sections and its own phases from 0, so a citation names its document: "design §5.4", "phase 2" or "Appendix A" with no name mean the base design, [1_introduction.md](design/1_introduction.md), and a later one is cited by its file name, "2_topic.md §3", "phase 0 of 2_topic.md".

### Keeping docs up to date

The docs outside `design/` must always match the code.
- **Update them in the same change as the code.** Any change that affects something documented updates the docs with it: requirements and setup, commands and scripts, project structure, conventions, and behavior.
- **A doc that no longer matches the code is a bug.** Fix it when you find it, even if your change didn't cause it.

`design/` is the exception. A design doc is only edited while its design is in progress: freely while it's being designed, and through appendices while its phases are being built. Once it's complete, it isn't edited again. Its index, [design/README.md](design/README.md), isn't a design doc: it's updated whenever a design doc is added or changes status.
