# Design docs

Each design doc plans one topic, numbered in the order it's written: `N_topic.md`. [1_introduction.md](1_introduction.md) is the base design, and later docs build on it.

**A design doc is archival unless it's being worked on.** Only the doc whose topic is being designed or built right now is edited, and only as its status allows (below). Every other design doc is read-only: read it for context, but never edit it, not even to fix a typo, a stale name or a broken link. What's actually built is documented in the rest of `docs/` ([docs/README.md](../README.md)), which is kept up to date with the code.

## Index

| Document | Covers | Status |
|---|---|---|
| [1_introduction.md](1_introduction.md) | The base design: what liquidslunk keeps from slunk, the Rust backend and its commands, choosing the library, TanStack Query, and the phases of the port | Being built |

Planned, not written yet: **external libraries**, exporting music to other libraries. It takes the next free number when it's written.

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
