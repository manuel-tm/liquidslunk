# liquidslunk

A local music library manager, as a desktop app. liquidslunk keeps its own database as the source of truth for your music's metadata, and only reads your music files.

liquidslunk is the next version of slunk, ported to Tauri 2 with its backend in Rust, so the whole app is one executable. It keeps slunk's database, so a library slunk made opens as it is. slunk is no longer developed.

> **Status:** the port is done: the app chooses and remembers its library, migrates it, and runs every page on Rust commands, from the dashboard to the import and the library check. `npm run tauri build` makes the app and a disk image, signed ad hoc for this Mac ([Release builds](docs/development/getting-started.md#release-builds)).
>
> Then came a music player ([The player](docs/development/frontend.md#the-player)): the player in the sidebar, with its shortcuts and macOS's Now Playing; its queue, with shuffle, repeat, play next and add to queue, from the release and track pages, the track list and the release table; and a Controls menu in the menu bar.
>
> Then came gapless playback ([Nearly gapless](docs/development/frontend.md#nearly-gapless)): the next track starts in a second element just before the current one ends, so tracks follow each other with nearly no gap.
>
> Then came external libraries ([The external libraries context](docs/development/backend.md#the-external-libraries-context)): a selection of the library, converted, tagged and given covers from the catalog, and synced to a folder or to an iPod with video. A track's page shows its tags as an external library writes them; external libraries are made and filled, from their own pages and from the catalog's; a folder library syncs to a folder or a card, converting with macOS's own encoders, with a plan to look at first, its progress, cancelling, and ejecting the card; and an iPod library syncs an iPod with video, writing its database and artwork as iTunes did.

## Documentation

- [**Documentation**](docs/README.md): what's built, how the docs are organized, and how we work.
- [**Getting started**](docs/development/getting-started.md): setting up and running the app.
- [**Design**](docs/design/): the plans, and why things were decided as they were.

The docs in `docs/` are kept up to date with the code: a change that affects something documented updates the docs in the same change. The exception is `docs/design/`, which is a history of the project's decisions, not a description of the code. A design doc is only edited while that design is being developed, and nothing outside `docs/design/` cites one.

AI coding agents: see [AGENTS.md](AGENTS.md).

## License

[MIT](LICENSE)
