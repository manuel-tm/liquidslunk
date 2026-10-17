> **Legacy:** this is slunk's documentation, copied for the port. It describes `legacy/slunk-be/` and slunk's frontend before the port, not liquidslunk. Its citations ("design §4.3") mean slunk's design docs, in `legacy/docs/design/`. It's read-only, and phase 6 deletes it.

# Getting started

> Keep this page up to date: any change to requirements, setup or scripts updates it in the same change.

## Requirements

- [mise](https://mise.jdx.dev/), which installs the pinned Node version
- Node 26.10.0, pinned in `slunk-be/.tool-versions` and `slunk-fe/mise.toml`
- npm, which comes with Node

## Backend

```sh
cd slunk-be
mise install
npm install
cp .env.example .env
npm run db:migrate
npm run dev
```

- `.env` sets `SLUNK_DB`, the database. The example points it at `dev-library/slunk.db`, a scratch folder for development. Put sample music files and images in `dev-library/` to register them. Never point it at a real music library.
- `npm run db:migrate` creates the database. The server also migrates on start, so this step is optional.
- `npm run db:seed` adds sample data, if you want some.
- The server listens on `http://127.0.0.1:4000/graphql`.

Scripts, run inside `slunk-be/`:

| Script | What it does |
|---|---|
| `npm run dev` | Starts the server, restarting it when a file changes |
| `npm start` | Starts the server |
| `npm run db:gen -- <name>` | Creates `priv/repo/migrations/<timestamp>_<name>.ts` |
| `npm run db:migrate` | Backs up the database, applies pending migrations, and regenerates the DB types |
| `npm run db:rollback -- [n]` | Backs up the database, reverts the last *n* migrations (default 1), and regenerates the DB types |
| `npm run db:status` | Lists migrations as applied, pending, or `MISSING` (applied, but the file is gone) |
| `npm run db:seed` | Loads sample data, unless the database already has releases |
| `npm run db:reset` | Backs up and deletes the database, then migrates and seeds |
| `npm run codegen` | Regenerates the resolver types from the GraphQL schema |
| `npm run typecheck` | Type-checks with `tsc` |
| `npm run lint` | Runs ESLint |
| `npm run format` | Formats `.ts` files with Prettier |

The database commands back the database up first, and each backup removes the oldest ones beyond the last 10 in `slunk-backups/`. `SLUNK_KEEP_BACKUPS` in `.env` changes how many are kept.

How the backend is built is described in [backend.md](backend.md).

## Frontend

```sh
cd slunk-fe
mise install
npm install
npm run dev
```

`npm install` also downloads Playwright's headless Chromium (about 200 MB, once) into Playwright's cache, `~/Library/Caches/ms-playwright` on macOS. It's for taking screenshots of the app: see [frontend.md](frontend.md#screenshots).

Vite prints the local URL when it starts. It proxies `/graphql` and `/raw` (the files' bytes) to the backend at `127.0.0.1:4000`, so start the backend first. The dashboard shows whether it can reach it.

Scripts, run inside `slunk-fe/`:

| Script | What it does |
|---|---|
| `npm run dev` | Starts the Vite dev server |
| `npm run build` | Type-checks and builds for production |
| `npm run codegen` | Regenerates the GraphQL types in `src/gql/` from the backend's schema and the app's queries |
| `npm run typecheck` | Type-checks the app and config files (`tsc -b`, following the project references) |
| `npm run lint` | Runs ESLint |
| `npm run format` | Formats `.ts`/`.tsx` files with Prettier |
| `npm run preview` | Serves the production build, with the same proxy |

How the frontend is built is described in [frontend.md](frontend.md).
