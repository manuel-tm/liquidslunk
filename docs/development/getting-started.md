# Getting started

> Keep this page up to date: any change to requirements, setup or scripts updates it in the same change.

## Requirements

- macOS, with Xcode's command-line tools (`xcode-select --install`), which Tauri builds with.
- [mise](https://mise.jdx.dev/), which installs the pinned versions of Node and Rust: Node 26.10.0 and Rust 1.99.1, pinned in `mise.toml`.
- npm, which comes with Node.
- `afconvert`, which macOS has, to make the audio fixtures again, and `qlmanage`, which it has too, to make the app's icon again. Nothing else needs them.

## Setup

From the repository's root:

```sh
mise install
npm install
```

The first build downloads and compiles the Rust dependencies, which takes a few minutes.

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

## Release builds

```sh
npm run tauri build
```

This builds the app in release mode, then bundles it: `target/release/bundle/macos/liquidslunk.app`, and a disk image of it in `target/release/bundle/dmg/`. Both are signed ad hoc, which is enough to run them on this Mac. The disk image's script asks Finder to lay out its window, which needs the terminal allowed to control Finder (System Settings › Privacy & Security › Automation). Without that, `CI=true npm run tauri build` makes the image without the layout. Sharing the app needs a Developer ID and notarization, which aren't set up. To install it, drag `liquidslunk.app` to Applications, from the disk image or from Finder.

A release build keeps its settings in `settings.json`, in `~/Library/Application Support/com.liquidslunk.desktop/`, and asks for a library at its first launch. It doesn't take `--library`: that's for debug builds. `npm run tauri build -- --no-bundle` builds only `target/release/liquidslunk`, to check something only a release build does.

## The app's icon

`src-tauri/icons/icon.svg` is the icon's source: the library menu's glyph on the app's blue. To change it, edit the SVG, then make every size from it:

```sh
qlmanage -t -s 1024 -o /tmp src-tauri/icons/icon.svg
npm run tauri icon /tmp/icon.svg.png
rm -r src-tauri/icons/android src-tauri/icons/ios src-tauri/icons/64x64.png
cp src-tauri/icons/icon.svg public/favicon.svg
```

`qlmanage`, Quick Look, draws the SVG as a 1024-pixel PNG, and `tauri icon` writes every size the bundle lists in `tauri.conf.json`, and some for Windows. The app is macOS only, so the Android and iOS sets go. The webview's favicon is a copy of the SVG.

## Scripts

Run from the repository's root:

| Script | What it does |
|---|---|
| `npm run tauri dev` | Builds and runs the app, with Vite |
| `npm run tauri build` | Builds a release version of the app, and its disk image: see [Release builds](#release-builds) |
| `npm run dev` | Starts Vite alone, which `tauri dev` does itself |
| `npm run build` | Type-checks and builds the frontend into `dist/`, which `tauri build` does itself |
| `npm run typecheck` | Type-checks the app and config files (`tsc -b`) |
| `npm run lint` | Runs ESLint |
| `npm test` | Runs the frontend's tests, with Vitest: see [frontend.md](frontend.md#tests) |
| `npm run format` | Formats `.ts`/`.tsx` files with Prettier |
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
| `cargo test --workspace` | Runs the Rust tests, including the migrations' parity with slunk, audio files read as slunk read them, and language tags canonical as slunk saved them |
| `cargo clippy --workspace --all-targets` | Lints the Rust code |
| `cargo fmt` | Formats the Rust code |

How the frontend is built is described in [frontend.md](frontend.md), and the backend in [backend.md](backend.md).
