# liquidslunk

A local music library manager, as a desktop app. liquidslunk keeps its own database as the source of truth for your music's metadata, and only reads your music files.

liquidslunk is the next version of slunk, ported to Tauri 2 with its backend in Rust, so the whole app is one executable. It keeps slunk's database, so a library slunk made opens as it is. slunk is no longer developed.

> **Status:** the port is done. The base design, [Introduction to liquidslunk](docs/design/1_introduction.md), planned it in phases 0 to 6, which are all built: the app chooses and remembers its library, migrates it, and runs every page on Rust commands, from the dashboard to the import and the library check. `npm run tauri build` makes the app and a disk image, signed ad hoc for this Mac.
>
> Then came a music player, planned in [2_player.md](docs/design/2_player.md), which is built: the player in the sidebar, with its shortcuts and macOS's Now Playing; its queue, with shuffle, repeat, play next and add to queue, from the release and track pages, the track list and the release table; and a Controls menu in the menu bar.
>
> Then came gapless playback, planned in [3_gapless.md](docs/design/3_gapless.md), which is built: the next track starts in a second element just before the current one ends, so tracks follow each other with nearly no gap.
>
> Now come external libraries, planned in [4_external_libraries.md](docs/design/4_external_libraries.md): a selection of the library, converted, tagged and given covers from the catalog, and synced to a folder or to an iPod with video. Its first phase is built: a track's page shows its tags as an external library will write them, and where a folder library will put it. Nothing is written yet.

## Documentation

- [**Documentation**](docs/README.md): what's built, how the docs are organized, and how we work.
- [**Getting started**](docs/development/getting-started.md): setting up and running the app.
- [**Design**](docs/design/): the plans, starting with [Introduction to liquidslunk](docs/design/1_introduction.md).

The docs in `docs/` are kept up to date with the code: a change that affects something documented updates the docs in the same change. The exception is `docs/design/`, which is mostly a history of the project. A design doc is only edited while that design is being developed.

AI coding agents: see [AGENTS.md](AGENTS.md).

## License

[MIT](LICENSE)
