# Getting started

> Keep this page up to date: any change to requirements, setup or scripts updates it in the same change.

## Requirements

- macOS, with Xcode's command-line tools (`xcode-select --install`), which Tauri builds with.
- [mise](https://mise.jdx.dev/), which installs the pinned versions of Node and Rust: Node 26.10.0 and Rust 1.99.1, pinned in `mise.toml`.
- npm, which comes with Node.
- The `sqlite3` command-line tool, which macOS has, to make the migrations' parity fixtures again. Nothing else needs it.

## Setup

From the repository's root:

```sh
mise install
npm install
npm install --prefix legacy/slunk-be
```

The second install is slunk's backend, which the app runs as its sidecar until phase 6 ([backend.md](backend.md#the-sidecar)). The first build downloads and compiles the Rust dependencies, which takes a few minutes.

## Running the app

```sh
npm run tauri dev
```

This starts Vite, then builds and opens the app. A change to the frontend shows at once. A change to the Rust code rebuilds and restarts the app.

The first launch asks for a library: Welcome. In development, use a scratch library in `dev-library/`, never a real music library:
- **Open a library…** and choose `dev-library/music/slunk.db`, a copy of part of a real library, if you have it. If it's missing, copy one of the backups in `dev-library/music/slunk-backups/` to `dev-library/music/slunk.db`: the app backs it up and upgrades it as it opens it.
- **New library…** and choose a new folder in `dev-library/`, for an empty one.

The next launch opens the same library. A debug build keeps its settings in `settings.dev.json`, apart from the installed app's, in `~/Library/Application Support/com.liquidslunk.desktop/`. To open a library without changing them, for scripts:

```sh
npm run tauri dev -- -- -- --library /absolute/path/to/slunk.db
```

When a library opens, the app starts slunk's backend on port 4000, and its output shows in the same terminal. Stop anything else listening on port 4000 first, like slunk itself.

There are no release builds before phase 6, since they don't start slunk's backend, so most pages couldn't load. `npm run tauri build -- --no-bundle` builds one, in `target/release/liquidslunk`, to check something only a release build does.

## Scripts

Run from the repository's root:

| Script | What it does |
|---|---|
| `npm run tauri dev` | Builds and runs the app, with Vite |
| `npm run tauri build` | Builds a release version of the app |
| `npm run dev` | Starts Vite alone, which `tauri dev` does itself |
| `npm run build` | Type-checks and builds the frontend into `dist/`, which `tauri build` does itself |
| `npm run typecheck` | Type-checks the app and config files (`tsc -b`) |
| `npm run lint` | Runs ESLint |
| `npm run format` | Formats `.ts`/`.tsx` files with Prettier |
| `npm run codegen` | Regenerates the GraphQL types in `src/gql/` from slunk's backend's schema and the app's queries |
| `npm run screenshot -- [route] [file.png]` | Takes a screenshot of the running app: see [frontend.md](frontend.md#screenshots) |
| `npm run db:migrate -- --db <path>` | Backs up a library's database, then applies pending migrations |
| `npm run db:rollback -- --db <path> [n]` | Backs up the database, then reverts the last *n* migrations |
| `npm run db:status -- --db <path>` | Lists migrations as applied, pending, or `MISSING` |
| `npm run db:gen -- <name>` | Creates a migration |
| `npm run db:seed -- --db <path>` | Loads sample data, unless the database already has releases |
| `npm run db:reset -- --db <path>` | Backs up and deletes the database, then migrates and seeds a new one |

The `db:*` scripts are described in [backend.md](backend.md#the-database-commands).

For Rust, run from the repository's root:

| Command | What it does |
|---|---|
| `cargo test --workspace` | Runs the Rust tests, including the migrations' parity with slunk |
| `cargo clippy --workspace --all-targets` | Lints the Rust code |
| `cargo fmt` | Formats the Rust code |

How the frontend is built is described in [frontend.md](frontend.md), and the backend in [backend.md](backend.md).
