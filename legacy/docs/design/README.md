> **Legacy:** this is slunk's documentation, copied for the port. It describes `legacy/slunk-be/` and slunk's frontend before the port, not liquidslunk. Its citations ("design §4.3") mean slunk's design docs, in `legacy/docs/design/`. It's read-only, and phase 6 deletes it.

# Design docs

Each design doc plans one topic, numbered in the order it's written: `N_topic.md`. [1_introduction.md](1_introduction.md) is the base design, and later docs build on it.

**A design doc is archival unless it's being worked on.** Only the doc whose topic is being designed or built right now is edited, and only as its status allows (below). Every other design doc is read-only: read it for context, but never edit it, not even to fix a typo, a stale name or a broken link. What's actually built is documented in the rest of `docs/` ([docs/README.md](../README.md)), which is kept up to date with the code.

## Index

| Document | Covers | Status |
|---|---|---|
| [1_introduction.md](1_introduction.md) | The base design: requirements, data model, migrations, backend, frontend routes | Complete: phases 0 to 4 built |
| [2_credits.md](2_credits.md) | Credits with free-text roles, each shared by several artists, and artist names joined with commas | Complete: phase 0 built |
| [3_alternate_names.md](3_alternate_names.md) | Alternate names for artists and labels, including real names; the name each artist is credited with; alternate titles for releases, discs and tracks; a language on every name and title | Complete: phases 0 to 3 built |

Planned, not written yet: **external libraries**, exporting music to other libraries. `1_introduction.md` calls it `2_external_libraries.md`, but that name was only a placeholder. It takes the next free number when it's written.

This README isn't a design doc. It's updated whenever a design doc is added or changes status.

## Statuses

A doc's status is its status line, right under its title. The index above mirrors it.

| Status line | What may be edited in the doc |
|---|---|
| `> **Status: in design.**` | Anything |
| `> **Status: being built.**` | Only appendices, each with a one-line note at the section it affects |
| `> **Status: complete.**` | Nothing. It's an archive. |

When a doc moves to the next status, its status line and its row above are updated. Marking it complete is the last edit it gets.

The full process, with examples of an appendix and a status line, is in [docs/README.md](../README.md#how-we-work).
