# Design docs

Each design doc plans one topic, numbered in the order it's written: `N_topic.md`. [1_introduction.md](1_introduction.md) is the base design, and later docs build on it.

**A design doc is archival unless it's being worked on.** Only the doc whose topic is being designed or built right now is edited, and only as its status allows (below). Every other design doc is read-only: read it for context, but never edit it, not even to fix a typo, a stale name or a broken link. What's actually built is documented in the rest of `docs/` ([docs/README.md](../README.md)), which is kept up to date with the code.

## Index

| Document | Covers | Status |
|---|---|---|
| [1_introduction.md](1_introduction.md) | The base design: what liquidslunk keeps from slunk, the Rust backend and its commands, choosing the library, TanStack Query, and the phases of the port | Complete |
| [2_player.md](2_player.md) | The music player: playback in the sidebar, between the library menu and Import, with a queue and shortcuts; macOS's Now Playing and media keys; and a Controls menu in the menu bar | Complete |
| [3_gapless.md](3_gapless.md) | Gapless playback: the next track loaded ahead in a second element, and started just before the current one ends | Complete |
| [4_external_libraries.md](4_external_libraries.md) | External libraries: a selection of the library, converted, tagged and given covers from the catalog, and synced to a folder or to an iPod with video (5th and 5.5th generations) | Being built |

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
