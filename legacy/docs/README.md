> **Legacy:** this is slunk's documentation, copied for the port. It describes `legacy/slunk-be/` and slunk's frontend before the port, not liquidslunk. Its citations ("design §4.3") mean slunk's design docs, in `legacy/docs/design/`. It's read-only, and phase 6 deletes it.

# Slunk documentation

This folder documents Slunk as it's actually built, and it's kept up to date with the code (see [Keeping docs up to date](#keeping-docs-up-to-date)). Plans live separately, in [`design/`](design/).

## Contents

| Path | What's in it |
|---|---|
| [`design/`](design/) | Design docs: numbered plans used for planning and prompting, and an archive once they're built. [design/README.md](design/README.md) lists every design doc and its status. Start with [1_introduction.md](design/1_introduction.md), the base design. |
| [`development/getting-started.md`](development/getting-started.md) | Setting up and running the project |
| [`development/backend.md`](development/backend.md) | How the backend is built: the database, migrations, GraphQL, and its conventions |
| [`development/frontend.md`](development/frontend.md) | How the frontend is built, and its conventions |
| [`reference/database.md`](reference/database.md) | The database schema as built, and the rules it enforces |

New docs are added as the code they describe lands:
- `development/` holds how to set up, run and work on each project.
- `reference/` describes what's built: the database. The GraphQL API is documented by its SDL, in `slunk-be/src/slunk_web/schema/`, and summed up in [development/backend.md](development/backend.md#graphql).

## Repository layout

```
docs/            documentation (this folder)
  design/        design docs (plans)
  development/   setup and conventions for the code
  reference/     the database schema
slunk-be/        backend
slunk-fe/        frontend
temp/            local assets for prompting, never committed
AGENTS.md        instructions for AI coding agents (CLAUDE.md points to it)
```

`temp/` holds files you share with an agent while prompting (screenshots, notes, sample data). Only its placeholder, `temp/.gitkeep`, is committed; the root `.gitignore` ignores everything else. Because those files exist only on your machine, nothing that gets committed (docs, design docs, code, commit messages) may reference them. If something in one of them belongs in the docs, write it into the doc itself.

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
## Appendix A: Releases store their disc count

- **Date:** 2026-11-02
- **Affects:** §4.3 Releases
- **Was:** the number of discs is computed from the tracks.
- **Now:** releases store `total_cds`.
- **Why:** box sets with missing discs showed the wrong count.
```

And at the affected section:

```markdown
> Changed during implementation, see [Appendix A](#appendix-a-releases-store-their-disc-count).
```

5. **Mark a design complete** when all its phases are built: a status line under its title, and its row in [design/README.md](design/README.md). For example:

   ```markdown
   > **Status: complete.** All of this design's phases (§8, phases 0 to 4) were built by 2026-10-15.
   ```

6. **Leave it alone.** A finished design doc is never edited again, not even to add an appendix: it's a record of what was planned and built. A later change to what it describes is made in the code and documented here, like any other change. A change that needs planning gets a new design doc.

A design doc is only ever edited by the work on its own topic. Every other design doc is read-only, whatever its status: it's read for context, but never edited, not even to fix a typo, a stale name or a broken link.

### Citing design docs

Each design doc numbers its own sections and its own phases from 0, so a citation names its document:
- **The base design** is cited without a name: "design §4.3", "phase 2" or "Appendix A" mean [1_introduction.md](design/1_introduction.md). The code and these docs cite it that way.
- **Any later design doc** is cited by its file name: "2_credits.md §3", "phase 0 of 2_credits.md".

### Keeping docs up to date

The docs outside `design/` must always match the code.
- **Update them in the same change as the code.** Any change that affects something documented updates the docs with it: requirements and setup, commands and scripts, project structure, conventions, and behavior.
- **A doc that no longer matches the code is a bug.** Fix it when you find it, even if your change didn't cause it.

`design/` is the exception. A design doc is only edited while its design is in progress: freely while it's being designed, and through appendices while its phases are being built. Once it's complete, it isn't edited again. Its index, [design/README.md](design/README.md), isn't a design doc: it's updated whenever a design doc is added or changes status.
