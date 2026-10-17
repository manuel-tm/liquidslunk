# liquidslunk documentation

This folder documents liquidslunk as it's actually built, and it's kept up to date with the code (see [Keeping docs up to date](#keeping-docs-up-to-date)). Plans live separately, in [`design/`](design/).

liquidslunk is a port of slunk, an earlier version of the same app. slunk's own docs aren't these docs:
- They're in `legacy/docs/`, each marked as slunk's, for reference while the port runs. Phase 6 deletes them.
- These docs describe only what's built here, as it's built. Each phase carries over what it ports, rewritten for the new code.

## Contents

| Path | What's in it |
|---|---|
| [`design/`](design/) | Design docs: numbered plans used for planning and prompting, and an archive once they're built. [design/README.md](design/README.md) lists every design doc and its status. Start with [1_introduction.md](design/1_introduction.md), the base design. |
| [`development/getting-started.md`](development/getting-started.md) | Setting up and running the app |
| [`development/frontend.md`](development/frontend.md) | How the frontend is built, and its conventions |
| [`development/backend.md`](development/backend.md) | How the Rust backend is built: the database, its migrations, the commands, the library, and slunk's backend as the sidecar |
| [`reference/database.md`](reference/database.md) | The database schema as built, and the rules it enforces |

New docs are added as the code they describe lands:
- `development/` holds how to set up, run and work on the app.
- `reference/` describes what's built: the database. The commands are documented by their Rust doc comments, which tauri-specta copies into `src/bindings.ts`.

## Repository layout

```
docs/            documentation (this folder)
  design/        design docs (plans)
  development/   setup and conventions for the code
  reference/     the database schema
src/             the frontend: React, in the app's webview
src-tauri/       the app: Tauri, its commands, the library, the file protocol
crates/slunk/    the domain and its database: migrations, with no Tauri
                 dependency
crates/slunk-cli/  the database commands (npm run db:*)
scripts/         scripts for working on the app, like screenshots
legacy/          slunk as of its last commit, read-only, until phase 6
dev-library/     scratch libraries for development, never committed
temp/            local assets for prompting, never committed
AGENTS.md        instructions for AI coding agents (CLAUDE.md points to it)
mise.toml        pinned Node and Rust
Cargo.toml       the Cargo workspace
package.json     the frontend, the Tauri CLI, and every npm script
README.md
LICENSE
```

`legacy/` holds slunk's backend, which the app runs as its sidecar until the port is done, and slunk's docs. Everything in it is read-only: see [legacy/README.md](../legacy/README.md).

`temp/` holds files you share with an agent while prompting (screenshots, notes, sample data). Only its placeholder, `temp/.gitkeep`, is committed; the root `.gitignore` ignores everything else. Because those files exist only on your machine, nothing that gets committed (docs, design docs, code, commit messages) may reference them. If something in one of them belongs in the docs, write it into the doc itself.

`dev-library/` holds scratch libraries for development. `dev-library/music/` is a copy of part of a real library, so, as with `temp/`, only its placeholder (`dev-library/.gitkeep`) is committed, and nothing committed depends on what's in it.

The same goes for slunk: its repository won't stay around, so nothing here points to its files. What liquidslunk needs from it is copied into this repository, or written into these docs.

## How we work

Work flows from design to code to docs:

1. **Design.** A topic is planned in `design/N_topic.md`, with the next free number, and gets a row in the index, [design/README.md](design/README.md). Its status line, right under its title, reads `> **Status: in design.**`. While it's being designed, it's edited freely.
2. **Code.** The code is built from the design, phase by phase, and its status line changes to `> **Status: being built.**`. While the phases are being built, the design's text stays as it was, and only appendices are added (step 4).
3. **Document.** When code lands, what it does is documented here, in the same change. Docs describe only what exists. Plans stay in the design docs, so nothing is written twice.
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

### Citing design docs

Each design doc numbers its own sections and its own phases from 0, so a citation names its document:
- **The base design** is cited without a name: "design §5.4", "phase 2" or "Appendix A" mean [1_introduction.md](design/1_introduction.md). The code and these docs cite it that way.
- **Any later design doc** is cited by its file name: "2_topic.md §3", "phase 0 of 2_topic.md".
- **slunk's design docs,** in `legacy/docs/design/` until phase 6, are cited with slunk's name: "slunk's design §7.4", "slunk's 3_alternate_names.md §7.1". The frontend's comments carried over from slunk cite them that way.

### Keeping docs up to date

The docs outside `design/` must always match the code.
- **Update them in the same change as the code.** Any change that affects something documented updates the docs with it: requirements and setup, commands and scripts, project structure, conventions, and behavior.
- **A doc that no longer matches the code is a bug.** Fix it when you find it, even if your change didn't cause it.

`design/` is the exception. A design doc is only edited while its design is in progress: freely while it's being designed, and through appendices while its phases are being built. Once it's complete, it isn't edited again. Its index, [design/README.md](design/README.md), isn't a design doc: it's updated whenever a design doc is added or changes status.
