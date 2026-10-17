# legacy/

This folder is slunk, the app liquidslunk ports, as of its last commit. It's here for the port ([design §8](../docs/design/1_introduction.md#8-the-transition)), and phase 6 deletes it.

- **`slunk-be/`** is slunk's backend. In debug builds, the app runs it as its sidecar, which serves the routes not ported yet ([docs/development/backend.md](../docs/development/backend.md#the-sidecar)). It's also the reference for porting the backend's behavior. Its one change from slunk: `SLUNK_SKIP_MIGRATE=1` skips the migration on start, since Rust owns migrations.
- **`docs/`** is slunk's documentation, copied whole, with a line at the top of each file saying so. It describes `slunk-be/` and slunk's frontend before the port, which is now in `src/`. Its citations ("design §4.3") mean slunk's design docs, in `docs/design/` here.

**Everything here is read-only.** Don't edit it, not even to fix a typo or a broken link: it's a record of what's being ported.

**slunk's rules aren't liquidslunk's.** The rules for working in this repository are in the root [AGENTS.md](../AGENTS.md). slunk's own `AGENTS.md` wasn't copied, and anything the legacy docs say about how to work doesn't apply here.
