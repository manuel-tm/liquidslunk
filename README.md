# liquidslunk

A local music library manager, as a desktop app. liquidslunk keeps its own database as the source of truth for your music's metadata, and only reads your music files.

liquidslunk is the next version of slunk, ported to Tauri 2 with its backend in Rust, so the whole app is one executable. It keeps slunk's database, so a library slunk made opens as it is. slunk is no longer developed.

> **Status:** the port is done. The base design, [Introduction to liquidslunk](docs/design/1_introduction.md), planned it in phases 0 to 6, which are all built: the app chooses and remembers its library, migrates it, and runs every page on Rust commands, from the dashboard to the import and the library check. `npm run tauri build` makes the app and a disk image, signed ad hoc for this Mac.
>
> Next is a music player, planned in [2_player.md](docs/design/2_player.md). Its phase 0 is built: the player in the sidebar, playing a release from the release page, with its shortcuts and macOS's Now Playing.

## Documentation

- [**Documentation**](docs/README.md): what's built, how the docs are organized, and how we work.
- [**Getting started**](docs/development/getting-started.md): setting up and running the app.
- [**Design**](docs/design/): the plans, starting with [Introduction to liquidslunk](docs/design/1_introduction.md).

The docs in `docs/` are kept up to date with the code: a change that affects something documented updates the docs in the same change. The exception is `docs/design/`, which is mostly a history of the project. A design doc is only edited while that design is being developed.

AI coding agents: see [AGENTS.md](AGENTS.md).

## License

[MIT](LICENSE)
