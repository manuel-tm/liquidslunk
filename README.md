# liquidslunk

A local music library manager, as a desktop app. liquidslunk keeps its own database as the source of truth for your music's metadata, and only reads your music files.

liquidslunk is the next version of slunk, ported to Tauri 2 with its backend in Rust, so the whole app is one executable. It keeps slunk's database, so a library slunk made opens as it is. slunk is no longer developed.

> **Status:** being built. The base design, [Introduction to liquidslunk](docs/design/1_introduction.md), plans the port in phases 0 to 6. Phases 0 and 1 are built: the app runs, chooses and remembers its library, and migrates it in Rust, and its files pages run on Rust commands, while slunk's backend still serves every page that isn't ported yet, in development only. There are no release builds until the port is done.

## Documentation

- [**Documentation**](docs/README.md): what's built, how the docs are organized, and how we work.
- [**Getting started**](docs/development/getting-started.md): setting up and running the app.
- [**Design**](docs/design/): the plans, starting with [Introduction to liquidslunk](docs/design/1_introduction.md).

The docs in `docs/` are kept up to date with the code: a change that affects something documented updates the docs in the same change. The exception is `docs/design/`, which is mostly a history of the project. A design doc is only edited while that design is being developed.

AI coding agents: see [AGENTS.md](AGENTS.md).

## License

[MIT](LICENSE)
