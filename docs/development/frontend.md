# Frontend

> Keep this page up to date: any change to the frontend's stack, structure or conventions updates it in the same change.

The frontend is slunk's, moved into the app when the port began, in `src/`. It does everything slunk's did. It has:
- Welcome, which chooses the library ([Routes and pages](#routes-and-pages));
- the app shell (sidebar with the library menu, the player and the Import button, page header with back and forward) and a "not found" page;
- the dashboard: the library's status, the catalog's totals, and the newest releases;
- the files pages: list, register, read and update;
- the catalog's pages: releases (with the release page and its edit tabs), artists (with their tabs), tracks, labels, release formats and release media;
- alternate names of artists and labels, and the language of each name;
- artists as credited on releases, tracks and credit lines, with the name printed there;
- alternate titles of releases, tracks and discs, under their main titles, and the language of each title;
- tags: their definitions' pages, a release's Tags tab, its tags on the release page, a track's tags, and tag filters on the release and track lists;
- bulk edits on the release list: setting or removing a tag on many releases;
- the import: a release built from a folder, step by step, with titles split at " = " and artists matched by any of their names;
- the library check: files missing, moved or changed on disk, unregistered files, and gaps in the catalog;
- the music player, in the sidebar: a queue of tracks, played or added from the release and track pages, the track list and the release table, with shuffle, repeat, shortcuts, macOS's Now Playing and a Controls menu in the menu bar, and nearly no gap between tracks ([The player](#the-player));
- tags as exported, on the track page: the track's tags as an external library will write them, and where a folder library will put it ([Tags as exported](#tags-as-exported));
- external libraries: their list and their section in the sidebar, each one's page with its Overview, Contents and Settings, "Add to external library" on the release, artist and track pages and in the bulk edits, where each release, artist and track is in one, and syncing a folder library or an iPod, with its plan, its progress and Eject ([External libraries](#external-libraries)).

Its data comes from Tauri commands, through TanStack Query ([Data](#data)). slunk's frontend read it through GraphQL, and the port moved each page to commands as the context behind it was ported.

## Stack

| Piece | Choice |
|---|---|
| Build | Vite 8 |
| UI | React 19, TypeScript 6 |
| Routing | React Router 8, in data mode (`createBrowserRouter`) |
| Desktop | Tauri 2: `@tauri-apps/api`, and `@tauri-apps/plugin-dialog` for choosing a library |
| Data | TanStack Query 5 over Tauri commands, typed by tauri-specta |
| Components | shadcn/ui, style `base-lyra`, built on Base UI primitives (`@base-ui/react`) |
| Styling | Tailwind CSS 4 |
| Icons | lucide-react |
| Fonts | Nunito Sans (body) and Outfit (headings), self-hosted through Fontsource |
| Toasts | sonner |
| Dates | react-day-picker and date-fns, used by `calendar` |
| Drag and drop | dnd-kit (`@dnd-kit/core`, `sortable`, `modifiers`, `utilities`), for the queue sheet's rows |
| Tests | Vitest, for plain functions ([Tests](#tests)) |
| Screenshots | WebDriver, embedded in the app by its `webdriver` feature |

## Structure

The frontend is at the repository's root, next to the Rust backend ([backend.md](backend.md#structure)):

```
package.json               the frontend's dependencies, and every npm script
components.json            shadcn configuration
vite.config.ts             Vite configuration, for Tauri
index.html
public/favicon.svg         the favicon, a copy of the app's icon
scripts/screenshot.ts      a screenshot of the running app
src/
  main.tsx                 entry point: mounts the router and the <Toaster />
                           inside ThemeProvider, QueryClientProvider and
                           TooltipProvider
  router.tsx               the routes
  bindings.ts              the commands, events and their types (generated
                           by tauri-specta)
  index.css                Tailwind setup, theme tokens (light and dark), fonts
  routes/                  one component per page
    library.tsx            /library, Welcome
    dashboard.tsx          /
    library-check.tsx      /library-check
    not-found.tsx          any unknown route or id
    files/                 /files: list, create (register), read, update
    releases/
      list.tsx             /releases
      bulk-edit.tsx        the release list's bulk edits
      page.tsx             /releases/:id, the release page
      files.tsx            /releases/:id/files
      edit.tsx             /releases/create, and the edit tabs' frame
      forms/               details, tracks, files, credits and tags: the edit
                           tabs' forms, which the import's steps reuse
    artists/
      list.tsx, form.tsx   /artists, create and update
      page.tsx             /artists/:id and its other tabs
    tracks/                /tracks: list, read, form
    labels/                /labels: list, read, form
    lists/                 /release-formats and /release-media: one set of
                           pages, by kind
    tags/                  /tags: list, read, form
    import/steps.tsx       /import and its steps
    external-libraries/
      list.tsx             /external-libraries
      form.tsx             /external-libraries/create, and the profile form,
                           which the Settings tab is too
      page.tsx             /external-libraries/:id: Overview, Contents and
                           Settings
      sync-sheet.tsx       Sync: the plan, the progress and the result
  components/
    app-layout.tsx         the app shell: the player, the sidebar, and the
                           page beside it, or Welcome when no library is
                           open
    app-sidebar.tsx        the sidebar: the library menu, the player,
                           Import, the sections, the lists
    player/
      provider.tsx         the player: its queue, its two <audio> elements,
                           its shortcuts and Now Playing; usePlayer() and
                           usePlayerTime()
      elements.ts          the two elements: the current one, the one that
                           loads what's up next, and the handoff between them
      ticks.worker.ts      a worker that ticks every few milliseconds, to
                           time the handoff in a hidden window
      sidebar-player.tsx   the player's controls, in the sidebar's header
      queue-sheet.tsx      the queue, in a sheet: play, drag to move, remove,
                           clear
      play-buttons.tsx     a track's play button in a list, the queue menu
                           (Play next, Add to queue), and a release's play
                           button for the release table
    library-menu.tsx       the open library, with a menu to switch libraries
    page-header.tsx        a page's back and forward, breadcrumb title,
                           buttons and tabs
    history-buttons.tsx    back and forward, and ⌘[ and ⌘]
    tab-links.tsx          tabs that are routes, for the header
    button-link.tsx        a link that looks like a button
    query-error.tsx        a failed request or save, with the backend's reason
    delete-button.tsx      Delete and its confirmation, or what blocks it
    list/                  the list page frame, pager, search, sortable header
    picker.tsx             a combobox that searches the backend
    pickers.tsx            a picker per kind of row: artist, label, file, …
    ordered-list.tsx       picked rows in order, with up, down and remove
    chip-list.tsx          a compact picked list, for table rows
    credits-editor.tsx     credit lines: a role, then its artists
    file-links-editor.tsx  file links, each with a note
    alternate-names-editor.tsx  an artist's or label's alternate names
    alternate-names.tsx    alternate names under a name, and the one a
                           search matched
    language-picker.tsx    a language tag, by name or tag
    titles.tsx             a title with its language, the lines of alternate
                           titles under it, and a list row's one
    alternate-titles-editor.tsx  alternate titles, and a "Titles" popover
                           for a table row
    title-split.tsx        the import's split of a title at " = ", and Swap
    other-names-offer.tsx  the import's offer of names from artist tags
    credited-as.tsx        the name an artist is credited with: a field,
                           and a chip label with a popover
    file-uses.tsx          what uses a file, each linking to it
    tag-fields.tsx         a tag definition's fields, and the new-tag dialog
    tag-value-input.tsx    a tag value, suggesting a text tag's values
    tags-editor.tsx        a release's or track's tags, as rows to edit
    tag-values.tsx         a tag's values, each linking to the filtered list
    tag-filters.tsx        a list's tag filters: chips, and "Tag filter"
    external-libraries.tsx a kind's icon, "Add to external library", and
                           where something is in one, with Remove
    exported-tags.tsx      a track's tags as an external library writes them,
                           for the track page
    release-table.tsx      the release table, with a column set per page
    names.tsx              names that link to their pages: "A, B, C"
    credit-lines.tsx       credit lines, as "Engineer – A, B"
    cover.tsx              a cover or picture, or a placeholder
    cover-chooser.tsx      a cover, picture or logo that opens full size,
                           or, while empty, a dialog to choose one from the
                           disk
    file-gallery.tsx       a primary image, then linked files
    folder-browser.tsx     browses the disk from the database's folder, by
                           links or in place, and can pick a file
    file-type.tsx          a file type's label and icon
    theme-provider.tsx     light / dark / system theme
    ui/                    shadcn components (see "Installed components" below)
  hooks/
    use-list-params.ts     a list's state in the query string
    use-debounced.ts       a value once it stops changing, for searches
    use-mobile.ts          useIsMobile(), used by the sidebar
  lib/
    query.ts               the QueryClient, useCommand(), call(), idParam(),
                           fileUrl()
    library.ts             opening, creating and closing libraries, and the
                           launch
    files.ts               opening a file, or showing it in Finder
    format.ts              sizes, durations, dates, release dates, roles
    languages.ts           language names, tags, lang/dir for names, and
                           the language a script gives away
    names.ts               the alternate name a search matched
    credited.ts            artists as credited: as options, and as inputs
    titles.ts              which alternate title a list row shows, the
                           artists' names in a title's language, and a
                           title's parts around " = "
    import.ts              the artists the import's tags pick, the tags left
                           to pick, and the other names to offer
    queue.ts               the player's queue, as a reducer, and what plays
                           next; queue.test.ts tests them
    tags.ts                tag types, values, and the list's tag filters
    external.ts            external libraries' labels, rules in words and as
                           inputs, memberships, plans and syncs in words;
                           external.test.ts tests them
    sync.ts                the sync running, outside the pages; Eject and
                           Locate
    utils.ts               cn() helper
```

## Conventions

### Routes and pages

- **Welcome** (`/library`, `routes/library.tsx`) is outside `AppLayout`, so it has no sidebar. It opens a library with a file dialog, filtered to `.db` files, creates one in a folder with a folder dialog, or opens a recent one. One that isn't there right now is dimmed. A folder that has a library already offers to open it instead. While a library opens, it shows "Opening …", or "Backing up and upgrading the library…" when it has migrations pending, and a toast says what the upgrade did. In debug builds, it says to keep libraries in `dev-library/`, where the dialogs start.
- **The launch:** `AppLayout` sends the user to Welcome while no library is open. The first time Welcome sees the status, it opens the library the settings remember, without asking (`launchLibrary()` in `lib/library.ts`). If that library isn't there, Welcome says so, with "Try again". Choosing or closing a library ends the launch, so it happens once per run.
- **Routes** are defined in `src/router.tsx`, as children of the `AppLayout` route, which renders the sidebar beside the current page. They're slunk's routes, as its frontend had them. The last one, `*`, shows `NotFound`.
- **Pages** live in `src/routes/`, one component per route. Each one starts with a `PageHeader`: its breadcrumb title (`crumbs`), then its buttons and tabs as `children`, which go on the right. Every crumb but the last links to its `to`.
- **The sidebar's header** is the library menu (`LibraryMenu`): the open library's folder name, with "Switch library…", which closes it and goes to Welcome, "Show in Finder", and the recent libraries, which open in its place. Under it are the player's controls ([The player](#the-player)), then Import.
- **Switching libraries** clears TanStack Query's cache, so nothing from the old library shows. `AppLayout` is keyed by the library's path, so every page mounts again.
- **Back and forward** (`HistoryButtons`) are in every page's header, before the title, and ⌘[ and ⌘] do the same: a desktop window has no browser buttons. They use the index React Router keeps in `history.state`.
- **The sidebar** lists the sections, in `sections` in `app-sidebar.tsx`, then the external libraries in their own group, each with a dot that's filled while it's connected, or a spinner while it syncs, and the lists the user manages in theirs, `lists`. The external libraries' label links to their list. A section's link stays active on its pages below it (`/artists/12` keeps Artists active).
- **Tabs** that are routes, like an artist's or a release's edit tabs, use `TabLinks`, so each tab is a route.
- **Wide tables** scroll inside the page: `SidebarInset` has `min-w-0`, so a table can't widen the page past the window.
- **Unknown ids:** a read or update page whose command returns null renders `NotFound`. A page reads its id with `idParam()` from `@/lib/query`, which parses it with `Number()`, and renders `NotFound` without asking when it isn't an id (`/files/abc`).
- **Names** of artists and labels, wherever they're listed, go through `Names`, which joins them with commas: "U2, Green Day". An "&" is only ever part of a name, as in "Simon & Garfunkel".
- **Artists as credited:** a release's and a track's artists, features and performers, and a credit line's artists, are `CreditedArtist`s: `{ artist: { id, name, nameLanguage }, creditedAs: { name, language } | null }`. `Names` takes them as they come, and shows the name printed, linking to the artist, with the main name in its tooltip: "ALIKA" links to Alika. There's no "*". So an artist page's Releases tab reads "ALIKA – virmalised", and its Appearances and Credits tabs add "as A. Ronnell" after a row's title (`CreditedAsNote`), since they don't show the name otherwise.
- **Titles** go through `Title`, which sets `lang` to the title's language: a release's `titleLanguage`, and a track's or disc's own, or else the release's. Commands that return a title return its `titleLanguage` too.
  - `AlternateTitleLines` shows a line under the main one for each alternate title, smaller and muted, with no "=". An unprinted one is in italics, with a tooltip saying it was added in liquidslunk.
  - Given the artists the main line names, a line repeats them in its title's language (`artistsIn()` in `lib/titles.ts`): each artist's first name in that language, else the name it's credited with unless that's in another language, else the main name. When they'd all read the same as on the main line, or the title has no language, the line shows only the title. So the commands return those artists' `alternateNames`.
  - The release page's heading, the track page's heading and the tracklist use it. The tracklist repeats the artists only on a various-artists release, where its rows name them. Disc headings show their alternate titles too. "Hide alternate titles" hides them in the tracklist, and the browser remembers it (`slunk.release.hideAlternateTitles`), like "Hide credits".
  - Lists show one alternate title under each main title, cut to one line (`ListedAlternateTitle`): the one the search matched, or else the first. `ReleaseTable` takes the list's `q` for it.
- **Languages of names:** a shown name gets `textProps(language)` from `lib/languages.ts`, which sets `lang` to its language and `dir="auto"`. `lang` picks the glyphs (Japanese and Chinese share code points but not shapes), and `dir="auto"` keeps a right-to-left name from moving the dashes and commas around it. So a command that returns an artist's or label's name returns its `nameLanguage` too. `Names` does it for each name, a crumb takes a `lang`, and a picker option one too. `languageName()` names a tag with the browser's `Intl.DisplayNames`: `ja-Latn` is "Japanese (Latin)".
- **Alternate names** show under an artist's or label's name on its page (`AlternateNames`): "Real name", then "Other names", each with its language. In the artist and label lists, a row that a search found by an alternate name shows that name under the main one (`MatchedName`), and the pickers show it after the option's name, muted.
- **Links:** use React Router's `Link`.
  - shadcn components built on Base UI take it through their `render` prop: `<SidebarMenuButton render={<Link to="/" />}>`.
  - For a link that looks like a button, use `ButtonLink`. Don't render a `Link` inside `Button`: Base UI then gives the link `role="button"`, so assistive tech announces a button.
  - A plain `<a>` styled with `buttonVariants()` needs `cn()` around it, which drops the base classes the variant overrides. Otherwise `border-transparent` hides the outline's border.

### Data

Data comes from Tauri commands, through TanStack Query.

#### Commands

- **The commands** are in `src/bindings.ts`, which tauri-specta generates from the Rust commands whenever a debug build starts ([backend.md](backend.md#commands)), with the events the backend sends, like `events.playerControl`. It's committed, and lint and Prettier skip it. Each command returns `{ status: "ok", data } | { status: "error", error }`, and the error is `{ kind, message }`.
- **Reading:** `useCommand(name, args, options)` from `@/lib/query` is the one way to read. It calls `commands[name](args)` through TanStack Query, with `[name, args]` as its key, and returns the data or throws a `BackendError`, whose `message` `QueryError` shows. A command with no arguments leaves them out: `useCommand("libraryStatus")`. `{ fresh: true }` keeps no cached copy, so a form never starts from out-of-date data. `{ keepPrevious: true }` keeps the last arguments' data while new ones load, with `isPlaceholderData` set, so what shows doesn't flicker: the player reads with it.
- **Disabled reads** (`{ enabled: false }`) still give what's cached for their key, so a component that disables one ignores its data too. A tag's page reads `tagValues` only for a text tag, and drops what it gives for a number or date tag, whose id can be a text tag's.
- **Writing:** mutations use `useMutation`, with a `mutationFn` that calls `call(name, …args)`, which unwraps a result the same way `useCommand()` does. Callers `await mutateAsync()` inside `try`, and show the mutation's `error` with `QueryError`. After every mutation that goes through `useMutation`, the `MutationCache` invalidates every query, so the mounted ones fetch again. `call()` on its own invalidates nothing: it's for commands that change no data, like opening a file, or that reset the caches themselves, like opening a library (`lib/library.ts`).
  - **Work in steps** is one mutation whose `mutationFn` makes the calls, so the caches are invalidated once, at the end: registering files calls `registerFiles` for a few files at a time, for its progress bar, and the list doesn't change under it. Since the steps before a failure are saved, it invalidates on error too.
  - **Deletes** go to the list, then `queryClient.removeQueries()` for the deleted row's keys, like `["file", { id }]`. `DeleteButton` does that in `afterDelete`, which runs once it has gone to the list.
- **Forms** read with `{ fresh: true }`, so they start from what's saved. The form component is keyed by the id it edits (`key={artist?.id ?? "new"}`): `/artists/create` and `/artists/:id/update` render the same route component, so without a key, going from one to the other, as back and forward can, would keep the other's values.
- **Ids** are numbers. A picker's options have string ids, as `Picker`'s `Option` has, and so `Cover`, `CoverChooser`, `Names` and `FileGallery` take either: `FilePicker` maps each file to `{ id: String(file.id), name: file.path }`, and the others do the same. The helpers that turn options into inputs turn them back with `Number()`: `creditedInput()`, `fileLinksInput()`, `tagValuesInput()` and `tagFilterInput()`, as the release and track forms do with labels, formats, media, a track's release and its audio file.
- **Defaults:** `staleTime: 0`, so a page shows what's cached and fetches again; `retry: false`, since a local call that fails will fail the same way again; and `refetchOnWindowFocus: false`.
- **No library:** a command that needs a library says `noLibrary` when none is open, and the app goes to `/library`.

#### Files

- **A file's bytes** come from the app's file protocol: `fileUrl(id)` from `@/lib/query` is `slunk://localhost/files/:id`, for `<img>` and `<audio>`. Audio can seek.
- **"Open the file"** opens it in its default app, and "Show in Finder" shows it, with `openFile(id)` and `revealFile(id)` from `@/lib/files`, which call commands. A failure shows a toast.
- **The files pages** (`routes/files/`) and the folder browser call the media commands: `files`, `file`, `folder`, `folderFiles`, `registerFiles`, `updateFile` and `deleteFile` ([backend.md](backend.md#the-media-context)). Their types come from `@/bindings`: `FileType`, `FileUseKind`, `FolderFile`, `FileDetails`.

#### The catalog

- **Artists** (`routes/artists/list.tsx`, `form.tsx`) call `artists`, `artist`, `createArtist` and `updateArtist`; **labels** (`routes/labels/list.tsx`, `form.tsx`) `labels`, `label`, `createLabel` and `updateLabel`; and **release formats and release media** (`routes/lists/`) `listItems`, `listItem`, `createListItem`, `renameListItem` and `deleteListItem`, with a `kind` ([backend.md](backend.md#the-catalog-context)). `ListKind` comes from `@/bindings`.
- **The pickers** call their own small commands, which return the first 20 by name: `artistOptions`, `labelOptions` and `listItemOptions`. `CreditedAsInput` asks `artistNames`, and `LanguagePicker` `languages`.

#### Releases

- **The release list** (`routes/releases/list.tsx`) calls `releases`, with its page, sort, search and filters; the page also names the filters' format, medium and label for their pickers (`filterFormat`, `filterMedium`, `filterLabel`). "Select all N that match" calls `releaseIds`, and the bulk edits `setTagOnReleases` and `removeTagFromReleases`.
- **The release page** calls `releasePage`, which has everything it shows; Delete calls `deleteRelease`, and choosing a cover `setReleaseCover`. Its files page calls `releaseFiles`.
- **The edit tabs:** their header calls `releaseHeader`, so it names the release on any tab. Details calls `release`, then `createRelease` or `updateRelease`; Tracks `releaseTracks`, then `setReleaseTracks`; Files `releaseFiles`, then `setReleaseFiles`; Credits `releaseCredits`, then `setReleaseCredits`.
- **The artist page** (`routes/artists/page.tsx`): its frame calls `artistHeader`, with the counts on the tabs, which every tab shares from the cache; the tabs call `artistReleases`, `artistAppearances`, `artistCredits`, and `artist` for the Files tab. **The label's page** calls `labelPage`, with its releases.
- **The dashboard** calls `libraryStatus`, `catalogTotals`, `fileCounts`, and `releases` for the newest twelve.
- **Credit roles** in the credits editor come from `creditRoles`.

#### Tracks and the import

- **The track list** (`routes/tracks/list.tsx`) calls `tracks`, with its page, sort, search, release and tag filters; the page also names the release it filters on, for its picker (`filterRelease`).
- **The track page** calls `trackPage`, and Delete `deleteTrack`. Its Tags as exported calls `exportedTags`, once it's opened.
- **The track picker** calls `trackOptions`.
- **The track form** calls `trackForm`: with `id`, the track it edits, and for a new track, the release and audio file that `?release=` and `?file=` prefill it with. It saves with `createTrack` or `updateTrack`, tags included.
- **The import** reads with `releaseSuggestion` (the release step), `releaseTracks` with the folder, whose `trackSuggestions` are the folder's rows (the tracks step), and `importFiles`, the release's cover and links with the folder's files (the files step). Its steps save with the edit tabs' commands, and `OtherNamesOffer` with `addArtistName`.

#### External libraries

- **The list and the sidebar** call `externalLibraries`. A page calls `externalLibrary`, with what's in it; Contents `externalLibraryRules`, saved with `setExternalLibraryRules`, and `externalLibraryTracks`; the form `createExternalLibrary` or `updateExternalLibrary`, and `exportedTags` with the template for its example; and Delete `deleteExternalLibrary`, with `removeFiles` when its checkbox is on. A new iPod library's form calls `mountedIpods`, fresh. Sync calls `syncPlan`, fresh each time the sheet opens, then `sync` with a `Channel` for its progress, and `erase` when the plan has tracks from somewhere else, and `cancelSync`; Eject `ejectExternalLibrary`; and Locate `locateExternalLibrary`. These three go through `call()`, in `lib/sync.ts`, and invalidate every query themselves when they end.
- **The catalog's pages** call `externalMemberships` for where a release, artist or track is, `addToExternalLibrary` and `removeFromExternalLibrary`.

#### Tags

- **The tag pages** (`routes/tags/`) call `tags`, once per type, `tag`, which has the counts, the blockers and the first 20 releases and tracks with several values, and `tagValues` for a text tag's values; their writes are `createTag`, `updateTag` and `deleteTag` ([backend.md](backend.md#the-tagging-context)). `TagType`, `Tag` and `TagDetails` come from `@/bindings`.
- **The Tags tab** (`forms/tags.tsx`), which the import's last step reuses, reads `releaseTags` and saves with `setReleaseTags`.
- **The pickers:** `TagPicker` calls `tagOptions`, and `NewTagDialog` `createTag`. `TagValuePicker` and `TagValueInput` ask `tagValueOptions` for a text tag's values. `TagFilters` names its chips with `tagsByRef`.

### Lists

Lists share their frame:
- **State:** `useListParams()` keeps a list's state in the query string: `page`, `sort`, `q` and filters such as `?type=` and `?unused=1`. Changing anything but the page goes back to page 1. Typing in the search box replaces the history entry instead of adding one. Filters that can repeat, like tag filters, are added with `append` and removed one value at a time with `remove`.
- **Controls** (`components/list/`):
  - `Pager`: "1–50 of 172", with previous and next.
  - `SearchBox`: searches after a pause in typing.
  - `SortableHead`: a header that cycles ascending (`?sort=path`), descending (`?sort=-path`), then the default.
- **Size:** lists ask for 50 rows a page.
- **Frame:** `ListLayout` is a list page's frame: the header and its New button, the search box, filters, the pager, then the table, or what's there while it loads, fails or is empty.
- **The release table** (`ReleaseTable`) is shared by the release list and the label and artist pages, each with its own columns. With `selection`, each row gets a checkbox, and the header's selects the page.
- **Pictures:** rows of releases and artists start with their cover or picture (`Cover`, 64px: `size-16`), linking to its page: the release table, an artist's appearances, and the artist list, whose columns are id, picture, name, added, updated. They're the original images, loaded lazily.
  - `Cover` takes exactly the size its `className` gives it. It drops the `max-width: 100%` Tailwind gives every image, which lets a table too wide for the page, like the release list, squeeze an image's column to nothing. So a `Cover` always gets a width, like `size-10` or `w-full`.
- **The tags list** (`/tags`) is the exception to the frame: it shows a table per type, each with every tag of that type, up to 200, and a note to search when there are more. The search box and the name sort apply to all three.

### Forms

- **One form** serves both create and update, built from shadcn `field`s. It loads first, then renders a form component that keeps the values in state, starting from what loaded.
- **Labels:** every control has one. A `Select` gets an `id` on its `SelectTrigger`, which the `FieldLabel`'s `htmlFor` points at.
- **Saving** goes to the read page. When a save fails, the form stays and shows the backend's reason, which is often the database's own message (a CHECK or a trigger).
- **Pickers:** artists, labels, formats, media, releases, files and tags are chosen with the pickers in `pickers.tsx`, each a `Picker` that searches the backend as the user types and offers the first 20 matches. `FilePicker` searches with the `files` command, by path; the artist, label, format, medium and tag pickers with their own commands (`artistOptions`, `labelOptions`, `listItemOptions`, `tagOptions`), by name; `ReleasePicker` with `releaseOptions`, by title; and `TrackPicker` with `trackOptions`, by title, with its release after it. Except for files, releases and tracks, a picker also offers to create what was typed ("Create artist 'Robbie Nevil'"); `ArtistPicker` doesn't with `creatable={false}`. Each picker owns its command, so its results stay typed.
  - `Picker` is generic over its option type, so an option can carry more than an id and a name. A `TagOption` also has the tag's type, its id within the type (`tagId`, a string like the other pickers' ids), and whether it's multiple. Its `id` is `type:id`, since ids repeat across types.
  - An option with an `image` shows it before its name, or a placeholder when it's null. The artist picker sets it to the artist's `picture`, and the release picker aliases the field as it aliases the name: `image: cover { id }`.
  - An option's `lang` is its name's language, and its `detail` shows after the name, muted: the label picker sets it to the alternate name the search matched. Typing an alternate name exactly counts as an exact match, so the picker doesn't offer to create a second artist with it.
  - The artist picker offers the artist, then the artist credited with each alternate name the search matches: "Alika · as ALIKA", and "(real name)" after a real one. That option has the artist's id, its own `key`, and `creditedAs`. Only it credits the name, so a search hint like "Bjork" is never printed by mistake.
  - `TagPicker` with `creatable` asks for the rest of a new tag in a dialog (`NewTagDialog`): its type, and single or multiple. Cancelling it creates nothing, since `onCreate` can return null.
  - `TagValuePicker` adds a value to a multi-value text tag: one that other releases already have, the most used first, or what was typed ("Add 'Funk'"). A single text value (`TagValueInput`) suggests the same values through a `<datalist>`.
- **Ordered lists** (release artists, labels, formats, media, a track's artists and features) use `OrderedList`, whose order sets `position`. A track row uses the compact `ChipList`. File links use `FileLinksEditor`.
- **Credited names** are edited where artists are linked. An artist option carries `creditedAs`, the name printed: `creditedOption()` makes one from a `CreditedArtist`, and `creditedInput()` turns options into `CreditedArtistInput`s.
  - `OrderedList` and `ChipList` take `credited` for artists. An ordered list's row then has a `CreditedAsInput`, and a chip shows the name printed, which opens a popover with one (`CreditedChipLabel`). The credits editor's chips do the same.
  - `CreditedAsInput` is a text field that suggests the artist's alternate names through a `<datalist>`, asked for with `artistNames` only while it has focus. Whatever is typed is the name: blank is the main name, which the placeholder shows ("as Drake"), and a name the artist doesn't have yet is added to its alternate names on save.
- **Alternate names** use `AlternateNamesEditor`, in the artist and label forms: a row per name, with its language, a note, and for an artist whether it's a real name. A name listed twice, or the same as the main name (unless it's a real name), is marked, and `alternateNamesValid()` keeps the form from saving until it's fixed. Names compare exactly, case included. `artistNamesInput()` and `labelNamesInput()` turn the rows into the commands' `ArtistNameInput`s and `LabelNameInput`s, with each saved artist name's id.
- **Alternate titles** use `AlternateTitlesEditor`, in the release details form and the track form, beside the title's language: a row per title, with its language and whether it's printed. A title listed twice, or the same as the main title, is marked, and `alternateTitlesValid()` keeps the form from saving. In the tracks editor, each row and each disc title has a "Titles" button (`TitlesPopover`) that opens the language and the editor in a popover, and shows how many alternate titles there are. A disc's is disabled until it has a title.
- **The artist form** also shows how many links each name is credited on, and how many show the main name.
  - "Make main" swaps a name with the main name, language included: the old main name takes its row as a new name, and a real name stays too. The save then sends `keepOldName`, so every link keeps showing what it showed.
  - Before saving, `SaveDialog` asks when the save changes what links show: removing a name that's in use credits its links with the main name, and a rename while links show the old name asks whether to keep it on them ("Keep 'Alika'" or "Show 'Alika Milova'").
- **Languages** are picked with `LanguagePicker`, a `Picker` with no backend search of its own: it offers the tags in use first (the `languages` command), then every ISO 639-1 language and a few script variants (`LANGUAGES`), matched by name or tag. Any other valid tag can be typed in ("Use the tag ja-Latn-hepburn"), and the backend stores it in canonical form.
- **Credits** use `CreditsEditor`, a row per line:
  - The role is a text field, typed as printed. It suggests every spelling in use, the most used first, through a `<datalist>` (`useCreditRoles`). It only asks while it has focus, so a release with many lines doesn't send a query per line.
  - The line's artists are chips, each with a button to move it earlier and one to remove it, then a picker that adds one.
  - "Add credit" takes a role and an artist. When a line already has the role, ignoring case, the artist joins that line instead.
  - A line with no role, with no artist, or with a role another line has is marked. `creditsValid()` keeps the form from saving until it's fixed, and `creditsInput()` turns the lines into `CreditInput`s.
  - Credit lines show the role as stored, and the artist page's Credits tab groups them by role ignoring case, with `roleKey()` from `lib/format.ts`.
- **The release forms** in `routes/releases/forms/` each take a release id and an `onSaved`, so the edit tabs and the import's steps use the same ones.
- **A dialog with its own form,** like `NewTagDialog`, calls `stopPropagation()` on submit. The dialog is portaled out of the page's form in the DOM, but React still bubbles the submit event to it, which would save the page's form too.

### Choosing an image

The release and artist pages show the cover or picture at 256px (`size-64`), and the label page its logo at 96px (`size-24`). Clicking one shows it in a dialog at its full size, up to the window's.

On the release, artist and label pages, an empty cover, picture or logo is a button (`CoverChooser`). It opens `ChooseImageDialog`, so the image is set without leaving the page:
- **Browsing** is the `FolderBrowser`, in place: with `onOpen` instead of `linkTo`, a folder opens inside the dialog instead of changing the URL. Images can be picked, and other files are faded.
- **Picking** an image saves it at once, with `setReleaseCover`, `setArtistPicture` or `setLabelPicture`. An image that isn't registered yet is sent by path, and the backend registers it in the same transaction. A failure stays in the dialog, with the backend's reason.
- **Where it starts:** a release's cover starts in the folder holding all its tracks (`CD1/` and `CD2/` start in the folder above them). Otherwise the dialog starts in the folder of the last image chosen, which the browser remembers per library (`slunk.imageFolder:<the library's path>` in `localStorage`, since the folder is relative to the library), or else the database's folder. A folder that can't be opened offers to go back to the database's folder.

Once an image is set, it's changed in the edit forms, as before.

### Deleting

Delete is a button on the read page that opens an `alert-dialog`. `DeleteButton` does it for every entity:
- While something uses the row, the dialog lists what, instead of offering to delete. The reasons are the entity's `blockers`, from the backend.
- After a delete, the app goes to the list, with a toast, then runs `afterDelete`, which removes the deleted entity's queries.
- Its children are options for the delete, shown under its description, like an external library's "Also remove the files liquidslunk wrote in …", or an iPod library's "Also remove the tracks liquidslunk put on IPOD".

### Import

`routes/import/steps.tsx` is the import, a route per step, with a stepper at the top whose steps are links once the release exists:
1. **Folder** (`/import?path=…`): the `FolderBrowser`.
2. **Release** (`/import/release?folder=…`): the details form, prefilled from `releaseSuggestion`. Saving replaces the history entry with `/import/:id/release?folder=…`, where going back edits the release.
3. **Tracks:** the tracks form with `folder`. There's a row for each audio file in the folder that isn't a track yet, prefilled from the folder's `trackSuggestions`. A file that's another release's track has a row, unchecked and locked. On a regular release, tag artists who are release artists aren't added to the tracks. A disc without a title yet gets its files' disc title tag.
4. **Cover & files:** the folder's images and other files. The cover is chosen among the images, defaulting to one named cover, folder or front. Each other file gets a checkbox and a note. The cover isn't also a file link.
5. **Credits:** the credits form.
6. **Tags:** the tags form. Finish opens the release page.

Files are registered only when a step saves them, together with what uses them.

**Names and titles from the tags:**
- **Artists** the backend matched are picked already, each credited as the backend says (`lib/import.ts`): ARTIST=ALIKA picks Alika, credited as "ALIKA". An artist tagged twice is picked once.
- **Tags that picked no artist,** because none matched or several did, are named above the form, or under the row's artists ("Tags: J. Smith (2 artists)"). The artists picker has each one typed in, in turn: `OrderedList` and `ChipList` take them as `typed`, and pass the next one to the picker as `defaultQuery` after each pick. Clicking the picker lists the matches, or offers to create the artist.
- **Other names** from tags like "Ryo Fukui = 福居良" are offered by `OtherNamesOffer`, above the form or the tracks: "Add to Ryo Fukui's names" adds one at once, with `addArtistName`, in the language its script gives away (`scriptLanguage()`: kana are `ja`, Hangul `ko`).
- **Titles with " = "** ("Scenery = シーナリィ") are kept whole, and offered a split (`components/title-split.tsx`). Nothing is split unless the user says so, and an "=" without spaces around it ("E=MC²") isn't offered one.
  - The release step offers it under the title (`offerSplit` on the details form, and on `DetailsTab` once the release exists). Split makes the first part the main title, and the others printed alternate titles, each in the language its script gives away. Then "Swap" trades the main title with the first alternate one, languages included, for a user who wants the original language first.
  - The tracks step has a split button by each title that has one, and "Split all" above the table. A split row's button then swaps it, and "Swap all" swaps every split row.

### Disc titles

A disc can have its own title, as printed: "Live At Madison Square Garden - February 5, 2009".
- **The Tracks tab** (`forms/tracks.tsx`) has a title field above the table for each disc its rows are on, in disc order. They're saved with the tracks. A blank one means no title, and a disc left without rows loses its title.
- **The release page** heads each disc with its number and title: "CD 2 · Live At Madison Square Garden - February 5, 2009". Discs are headed when there's more than one, or when the disc has a title.

### Tags

- **Routes** include the type, `/tags/:type/:id`, where the type is `text`, `number` or `date`. An unknown type shows `NotFound`.
- **The release page** shows a row per tag after Released, as "Genre: Pop, Rock". Each value links to the release list filtered by it (`TagValues`). Date values show as dates ("Mar 1, 2024").
- **The Tags tab** (`forms/tags.tsx`) edits the release's tags with `TagsEditor`. A single-value tag is one input, and a multi-value tag is a `ChipList`. "Add a tag" picks one, or creates it. Saving sends every tag, and an emptied one is removed.
- **Tracks** have tags too. The track form has a Tags section, the same `TagsEditor`, saved with the rest of the track. The track page shows them, each value linking to the track list filtered by it.
- **The tag pages** count releases and tracks. A text tag's values table has both counts, each linking to its filtered list.
- **List filters,** on `/releases` and `/tracks`, are query string parameters, one per filter, which all have to match (`lib/tags.ts`):

  | Parameter | Matches |
  |---|---|
  | `text.3=Rock` | Text tag 3 is Rock, ignoring case |
  | `text.3.contains=ro` | Text tag 3 contains "ro" |
  | `number.5=8` | Number tag 5 is 8 |
  | `number.5.min=8`, `number.5.max=10` | Number tag 5 is at least 8, at most 10 |
  | `date.2=2024` | Date tag 2 is in 2024: a date can be a year, a month or a day |
  | `date.2.min=2024-03`, `date.2.max=2024` | Date tag 2 is on or after March 1, 2024, on or before Dec 31, 2024 |

  "Genre is Rock and Score ≥ 8" is `?text.3=Rock&number.5.min=8`. Each filter is a chip ("Score ≥ 8") that removes it, and "Tag filter" adds one: "is" or "contains" for text, a range for numbers and dates (`TagFilters`). `TagFilters` names the chips with the `tagsByRef` command; the list's own query only sends the filters.

### Tags as exported

The track page ends with "Tags as exported" (`ExportedTags`), closed by default. It's a table of the track's tags as every external library will write them, whatever its format, and the path a folder library that keeps the original files will give it, by the default template ([backend.md](backend.md#the-external-libraries-context)). It's how the mapping is checked against real releases.
- It reads `exportedTags` only once it's opened.
- A field with no value isn't written, so it has no row. A field written as several values, like Artists or Label, shows each one as a badge.
- The title and the album have their languages' `lang`, and every value has `dir="auto"`.

### External libraries

An external library is a selection of the library, for a folder or an iPod ([backend.md](backend.md#the-external-libraries-context)). Both kinds sync.
- **`/external-libraries`** lists them, with their kind, destination, whether it's connected and when it was last synced, and has New folder library… and New iPod library….
- **Creating one** (`/external-libraries/create?kind=folder` or `?kind=ipod`) asks for its name and profile, and a folder library's folder, chosen with a folder dialog. A new iPod library lists the iPods connected (`MountedIpods`), each with its model, what's free on it, and the external library it belongs to, or why liquidslunk won't sync it, with Look again; its first sync goes to the one connected then. It then goes to its Contents. The template field shows the path it gives the catalog's newest track, or why it can't be used. A folder library keeps the original files by default, and an iPod library makes AAC.
- **Its page** (`/external-libraries/:id`) has three tabs, each a route, under a header with Delete. While its destination is connected, Delete's dialog has "Also remove the files liquidslunk wrote in …", or for an iPod, "Also remove the tracks liquidslunk put on IPOD"; while it syncs, it can't be deleted.
  - **Overview:** its kind and destination, a folder or an iPod's model, whether it's connected, with what's free there, or why not ("No: the folder isn't there", "No: the folder belongs to another external library, Car", "No: no iPod is connected. …", "No: several iPods are connected, and none is this one's yet. …"), what's in it (releases, tracks and about how much they take, and how many a sync will leave out), its format, and when it was last synced. Under them: **Sync…**, while it's connected and no sync runs, which says how far it's got while it syncs, and opens the sheet again; **Eject**, while it's on a disk that ejects; a folder library's **Locate…**, while it isn't connected, which asks for the folder where it is now, with a folder dialog; and an iPod library's **Look again**. An iPod library's page also looks for its iPod again when the window comes to the front.
  - **Contents:** its rules, in two lists, "In it" and "Left out", each naming what it names, linking to it, with a button that removes it. A release, an artist or a track is added with its picker, and a tag value with "Add a tag value": a text tag's value, or a number or date tag's range, either end left empty. Releases and tracks are left out with theirs. Each change saves the whole list, and a picker starts afresh after each pick. Below them, the tracks the rules come to, a page of 50 at a time, by release.
  - **Settings:** the same form as creating one, with its kind fixed, and the template's example for its first track.
- **Sync** (`SyncSheet`), in a sheet ([backend.md](backend.md#syncing-a-folder-library), [Syncing an iPod](backend.md#syncing-an-ipod)), under the folder, or the iPod's name and model:
  - **The plan,** worked out when the sheet opens, and again on Look again: what it does, in short ("Add 42 tracks (1.8 GB), update 7, remove 3."), whether it fits ("It needs 1.8 GB, and 12 GB is free.", or in red, "It needs 3.2 GB more than the folder has free."), then a section for each action, Add, Convert again, Restore, Retag, Move and Remove, and Left out, each listing its first 200: a track by its title, artist and album, or a release's cover, with a folder library's paths, a move's old path, a retag's changes ("Title: Shoka → 初夏"), or why it's left out. Sync is off while there's nothing to do, it doesn't fit, or another sync runs.
  - **On an iPod with tracks from somewhere else,** the plan asks first, in red, "This iPod has 1,234 tracks from somewhere else. Erase them, then sync?", lists them in an Erase section, and its button is "Erase them, then sync", in red, which syncs with `erase`.
  - **Its progress:** the stage, the steps done of all of them, as a bar, the track in hand, what's written, and Cancel sync, which stops after the file in hand. The sheet can be closed: the sync goes on, its state is kept outside the pages (`useSyncRun()`, `lib/sync.ts`), and the Overview's Syncing button opens it again.
  - **The result:** what it did ("Erased 12 tracks from somewhere else. Added 42, retagged 7 and removed 3."), what it wrote, the files it couldn't write and why, or why it stopped, with Eject when it ejects, and Close.
- **"Add to external library"** (`AddToExternalLibrary`) is a menu of them, on the release, artist and track pages and in the release list's bulk edits, with New external library…. A toast says where it went: "Added Scenery to Car".
- **Where it is** (`ExternalMemberships`): the release, artist and track pages list the external libraries it's in, and why: "On Car", "On Car, through Pink Floyd", "On Car, through Pink Floyd (11 of 12 tracks)", or for a release with only some tracks in, "3 of 12 tracks on Car". Each links to the library's Contents, and its button takes it out: "Remove from Car" for its own rule, or "Leave out of Car" when a broader rule brings it in.

### Bulk edits

The release list's rows have checkboxes (`bulk-edit.tsx`). The selection is kept by id, across pages, until it's cleared.
- **Selecting:** a row's checkbox, or the header's for the page. "Select all N that match" asks `releaseIds` for every release the filters match.
- **Set a tag:** a tag and a value. A single-value tag gets the value instead of its old one, and a multi-value tag gets it added. The tag picker can create a tag.
- **Remove a tag:** a tag, and a value to remove only that one.
- **Add to external library:** each selected release, whole ([External libraries](#external-libraries)).
- After each, a toast says how many releases changed, and the list is fetched again, as every query is after a mutation. The selection stays, for another edit.

### Library check

`/library-check` runs the check when it opens (`libraryCheck`), and again after "Check again" or a fix, since every mutation invalidates every query. It reads with `{ fresh: true }`, since a cached result would be out of date. Each kind of problem is a section with its count, and "None." when there's nothing:
- **Missing files:** what uses each one (`FileUses`), and where it may have moved. "Relink" points the record at a path, and "Relink all N found" does it for every file whose likely new path is clear (`relinkFiles`). A file found nowhere links to its edit page.
- **Changed files:** the size change and modification time. "Rehash" reads one again, and "Rehash all" all of them (`rehashFiles`).
- **Unregistered files,** by folder, with "Register…", which opens `/files/create?folder=…`.
- **Releases with no artist** and **tracks nobody performs,** each linking to the edit tab that fixes it.

### The player

The player plays a queue of tracks from the sidebar.
- **Where it is.** `PlayerProvider` (`components/player/provider.tsx`) is inside `AppLayout`'s `SidebarProvider`, around the sidebar and the page. It renders the app's two `<audio>` elements, hidden, one ahead of the other ([Nearly gapless](#nearly-gapless)). So the music keeps playing from page to page, and since `SidebarProvider` is keyed by the library, it stops when the library switches or closes. The sidebar's header shows the controls (`SidebarPlayer`), between the library menu and Import. When the sidebar is hidden, collapsed or in its sheet below 768 px, so is the player, and the music plays on.
- **The controls:** the release's cover, the track's title, who performs it and the release, each linking to its page; a bar of how far the track has played, with the time played and the track's length; shuffle, previous, play/pause, next, and repeat; mute, the volume, and the queue's button. Shuffle and repeat are in the accent color while they're on. With nothing queued, the player is one line: "Nothing playing".
- **Playing** replaces the queue. Pages call `usePlayer()`, and never touch the element.
  - On the release page, Play, by the tracklist's heading, plays the release from its first track, or from a random one when shuffled. A track's play button (`TrackPlayButton`) plays the release from that track. The current track's number becomes a speaker icon, and its button pauses.
  - The track page's Play plays the track alone.
  - On the track list, a row's play button plays the page's rows, in the list's order, from that row, so a filtered list plays like a playlist, a page at a time.
  - In the release table, on the release list, a label's page and an artist's Releases tab, each cover shows a play button on hover or keyboard focus (`ReleasePlayButton`). It asks `releaseTrackIds` for the release's tracks, then plays them like the release page's Play. A release with no tracks says so in a toast.
- **Adding** keeps the queue. The queue menu (`QueueMenu`), by the release page's Play, on each tracklist row, by the track page's Play, and on each track list row, has Play next, which puts the tracks right after the current one, and Add to queue, which puts them at the end. A toast says what was added: "Added 12 tracks to the queue". With nothing queued, either one starts a queue, paused at its first track.
- **The queue** is a reducer, `queueReducer` in `lib/queue.ts`, with no React and no element. The provider keeps it with `useReducer`, and the current element follows it. An entry is a track id with a key of its own, so a track can be queued twice. The current element's source changes only when the current entry does, never on a fetch, so a save never interrupts what plays.
- **Its rules:**
  - Previous goes back to the track's start after its first 3 seconds, and to the previous track before that. On the first track, it goes back to its start, or with repeat all, to the last track.
  - Next, and a track that ends, go to the next track. After the last, the player stops, paused at the last track's start, or with repeat all, goes on to the first.
  - Repeat goes from off, to all, to one. With repeat one, a track that ends plays again, while Next and Previous still move as with repeat off.
  - Shuffle plays the entries after the current one in a random order, and turning it off plays them in their order before again. While it's on, Play next and Add to queue still put tracks right after the current one and at the end, and they keep those places when it's turned off. A new queue started with shuffle on starts at the track that was clicked, or a random one, and shuffles the rest. Shuffling takes its seed from the action, so the reducer stays pure.
  - A track that can't play, missing or in a format WebKit can't decode, shows a toast with its path, and the next one plays. Once every entry has failed in a row, the player stops, even with repeat on.
- **The queue sheet** (`QueueSheet`), from the queue's button, lists the whole queue in the order it plays, with each track's cover, title, artists and length. The entries that have played are muted, and the current one is marked. It opens scrolled to the current entry. Clicking a row, or its cover, plays it. A row's button removes it, and Clear empties the queue.
  - **Moving a row:** it's dragged from anywhere on it, its title, cover and buttons included, with dnd-kit. The drag starts once the pointer has moved a few pixels, so a click still plays a row or follows a link, and the list scrolls while a row is dragged near its edges. dnd-kit keeps the click that ends a drag from reaching the row, but not its default, which on a link is the browser loading the link's page itself, so the sheet prevents that default for a moment after a drag. With its grip focused, the up and down arrow keys move it a place, the focus stays on it, and a live region says where it went ("Survive moved to place 2 of 11"). The keys don't go through dnd-kit's keyboard sensor: it listens on the document, and Base UI's dialog popup, which the sheet is, stops arrow keys from leaving it.
- **Remembering.** The queue is remembered per library, in `localStorage` under `slunk.player.queue:<the library's path>`, since its ids belong to the library: its tracks in the order they play, the current one, and while shuffled, their order before. It's written as it changes, and `PlayerProvider`, which `AppLayout` gives the library's path, starts from it, paused at the current track's start (`restoredQueue()`). Shuffle and repeat are remembered for every library (`slunk.player.shuffle`, `slunk.player.repeat`). What isn't a queue, like something an older version wrote, is ignored.
- **What it shows** comes from `playerTracks`, with the queue's ids. It's mounted, so every save fetches it again, and it reads with `{ keepPrevious: true }`, so it doesn't flicker when the queue changes. A track that's deleted leaves the queue. When it's the current one, the next one plays.
- **The time** comes from `usePlayerTime()`, a store the current element's events update, which only the progress bar reads, so nothing else renders again on each `timeupdate`. The track's length is the element's once it has read the file, and the catalog's `durationMs` until then.
- **No seeking.** WebKit seeks a FLAC file by estimate, not with its seek table, so a seek can play from several seconds away from the time it shows: on a live track 4:28 long, a seek to 4:18 played from about 4:08. So nothing in the player seeks: the bar only shows how far the track has played, and Now Playing has no seek action, so it shows no scrubber or skips. Previous still goes back to a track's start, which is exact. The file page's preview is the browser's own player, so it can still seek, by estimate.
- **One sound at a time:** when the file page's preview starts, the player pauses, and when the player starts, the preview pauses. Both of the player's elements are the player's. The provider listens for `play` on the document, in the capture phase, since media events don't bubble.
- **Shortcuts,** while something is queued: Space plays or pauses, ⌘→ and ⌘← go to the next and the previous track, and ⌘↑ and ⌘↓ change the volume by 10%. Space is ignored while a field or another control has focus, since Space presses that. The ⌘ arrows are ignored in a field, where they move the caret.
  - The Controls menu shows the ⌘ arrows as its items' key equivalents, but the provider still handles them. WebKit gives the page a key first, and the menu gets only what the page leaves: without the provider's handler, ⌘↓ on a long page scrolls it to its end, and the menu never sees it. In a field, WebKit moves the caret, so the menu doesn't get them there either.
- **The volume,** and whether it's muted, apply to both elements, and are remembered in `localStorage` (`slunk.player.volume`), for every library, as the theme is. Changing the volume unmutes. It's kept in hundredths, so steps of 10% don't drift: 0.8 - 0.1 would be 0.7000000000000001.
- **Now Playing.** The provider sets `navigator.mediaSession`'s metadata (the title, who performs it, the release, and the cover as `fileUrl()`), its state, and handlers for play, pause, previous and next. So Control Center, the media keys and headphones' controls work the player. Now Playing offers exactly the actions with handlers: previous and next, and no seeking. WebKit drops a handler set before a media element has registered with its Now Playing, and offers its own actions instead (a scrubber and 15-second skips), so the provider sets the handlers again whenever either element starts playing.
- **The Controls menu** in the menu bar ([backend.md](backend.md#the-menu-bar)) does what the player's buttons do, with the sidebar hidden too. Whenever the player plays or pauses, the queue empties or fills, or shuffle or repeat changes, the provider calls `setControlsMenu({ playing, queued, shuffle, repeat })`, and the menu shows Play or Pause, the checks, and every item off with nothing queued. As the provider unmounts, at Welcome, it calls it with nothing queued. It listens for `events.playerControl`, a click on an item, and calls its own actions, through `useEffectEvent`, so it listens once.
- **No click needed.** wry turns autoplay on, so WKWebView plays without a click, and a track can start once `playerTracks` has answered.

#### Nearly gapless

WebKit takes about a quarter of a second to start an element's clock, loaded or not, so a track started at the last one's `ended` follows a silence of a quarter to half a second. So the player starts the next track in a second element, just before the current one ends. `playerElements()` (`components/player/elements.ts`) runs the two elements, and the provider passes it their events.
- **The current element** plays the current entry. Only its events count: play and pause, `waiting`, an error, and the time.
- **The other one loads what's up next,** once the current one has started: `upNext()` in `lib/queue.ts`, the entry a track that ends goes on to, by the queue's rules. That's the same entry again with repeat one, and nothing after the last track without repeat all. Whenever the queue changes what's up next, by Play next, Add to queue, a move, a removal, Clear, shuffle or repeat, the other element loads the new one, or empties. After each swap, the provider says what's up next again.
- **The handoff comes in two steps.** When the current track has `LEAD` left, 240 ms, by its `currentTime` and `duration`, the other element plays, and nothing else happens: work on the main thread right after `play()`, like a render, delays the start by as long as it lasts. When the current track ends, the elements swap, and the queue goes on with the `handoff` action: `ended`, without a restart, since what's up next already plays from its start. So the player shows the next track when it's heard.
- **Timing it.** `timeupdate` comes four times a second, which is too coarse, and WebKit runs a hidden page's timers about once a second. So for a track's last second, a new worker (`ticks.worker.ts`) ticks every few milliseconds, and the time is checked at each tick: while the page plays audio, WebKit doesn't throttle a new worker's timers. It does slow down one that has sat idle for minutes, so each track's watch gets a new one, ended when it stops. From the last tick before the lead, the rest of the wait, up to 12 ms, is spent on the main thread, before `play()`, where blocking doesn't delay the start. A `MessageChannel` loop would tick more often, but WebKit runs it as fast as it can, which takes most of a core.
- **How close it gets.** WebKit's start-up varies by file, from about 200 to 310 ms, so a transition leaves a silence of up to about 50 ms, or overlaps by a few tens of milliseconds.
- **Without a handoff,** because the next file hasn't loaded yet, or failed to load, the track that ends goes on at `ended`, and the next one starts only then. Next, a jump, or a track that ends uses the other element when it holds that entry, so it starts sooner. A file that failed to load ahead is skipped when it would play, with its toast.
- **During a handoff,** Pause, Previous, a jump, or a change to what's up next stops the next track, back at its start, and Next keeps it playing.
- **Guards.** A track that plays on more than 250 ms past its `duration`, which was wrong, is cut off. The other element plays only for a handoff: WebKit sometimes starts the element loaded ahead by itself as the window comes into view, and it's paused and put back at its start before it's heard.
- **Muted, in a hidden window,** macOS can suspend the page for seconds at a time, since nothing audible plays, and the queue waits at a track's end until it runs again.

### Components

- **Adding shadcn components:** use `npx shadcn@latest add <component>`. They're copied into `src/components/ui/` and become our code, so they get edited there rather than wrapped.
- **Building screens:** compose those components. Don't hand-build primitives that shadcn provides.
- **Icons:** they come from `lucide-react`.

**Installed components**, in `src/components/ui/`:

| Purpose | Components |
|---|---|
| Layout and navigation | `sidebar`, `breadcrumb`, `separator`, `scroll-area`, `tabs`, `collapsible` |
| Showing data | `table`, `card`, `item`, `badge`, `avatar`, `skeleton`, `empty`, `tooltip` |
| Forms | `field`, `label`, `input`, `input-group`, `textarea`, `select`, `combobox`, `checkbox`, `switch`, `toggle`, `toggle-group`, `calendar`, `slider` |
| Overlays | `dialog`, `alert-dialog`, `sheet`, `popover`, `dropdown-menu` |
| Feedback | `sonner` (toasts), `alert`, `progress`, `spinner` |
| Actions | `button` |

**Local changes to shadcn files.** Re-adding a component with `--overwrite` replaces it with the registry version, so reapply these afterwards:
- `sonner.tsx` reads the theme from our `ThemeProvider` (`@/components/theme-provider`) instead of `next-themes`, which isn't installed.
- `hooks/use-mobile.ts` subscribes to the media query with `useSyncExternalStore`. The registry version sets state inside an effect, which the react-hooks lint rule rejects.
- `scroll-area.tsx` has its unused `React` import removed, which `tsc -b` rejects.
- `slider.tsx` takes `thumb`, with `getAriaLabel` and `getAriaValueText` for each thumb's input, which Base UI reads only from the thumb. (Unchanged from the registry: it takes its value as an array, and a single number renders two thumbs.)

### Styling

- **Configuration:** Tailwind 4 is configured in CSS (`src/index.css`); there's no `tailwind.config`. Theme tokens are CSS variables (`--background`, `--primary`, …), with a `.dark` block overriding them.
- **Colors:** use the tokens (`bg-background`, `text-muted-foreground`, …) rather than raw colors, so both light and dark themes work.
- **Classes:** merge them with `cn()`, exported from `@/lib/utils`.
- **Fonts:** `font-sans` (Nunito Sans) is the default; use `font-heading` (Outfit) for headings.

### Theme

`ThemeProvider` supports light, dark and system (the default). The choice is saved in `localStorage` under `theme`, and `useTheme()` reads and changes it.

Pressing `d` toggles between light and dark. It's ignored while typing in a field or when a modifier key is held.

### TypeScript

- **`strict`.**
- **`erasableSyntaxOnly`:** no `enum`, `namespace` or parameter properties.
- **`verbatimModuleSyntax`:** use `import type` for types.
- **Imports:** `@/` resolves to `src/`.

### Comments

- **Comments** never cite a design doc. One that needs more than it says points to the section of `docs/` that describes it, by the doc's file name and the section's heading: `(frontend.md, "The player")`. A component's comment points once, and what's in it points again only to another section ([Design docs and these docs](../README.md#design-docs-and-these-docs)).

### Formatting and linting

- **Prettier:** no semicolons, double quotes, 2-space indent, ES5 trailing commas, 80 columns. `prettier-plugin-tailwindcss` sorts classes, including inside `cn()` and `cva()`. Run `npm run format` instead of formatting by hand.
- **ESLint:** the recommended JavaScript and typescript-eslint rules, plus react-hooks and react-refresh. Run `npm run lint`.

### Tests

`npm test` runs Vitest (`vitest run`), configured in `vite.config.ts`'s `test`, so `@/` resolves as it does in the app.
- **Where:** next to what they test, as `src/**/*.test.ts`. They import `describe`, `it` and `expect` from `vitest`, with no globals, so `tsc -b` checks them like the rest of `src/`.
- **What:** plain functions, in Node, with no DOM. So far that's the player's queue (`lib/queue.test.ts`), and external libraries' rules, memberships, plans and syncs in words (`lib/external.test.ts`). Components are checked in screenshots instead.

### Screenshots

A change that shows in the UI is looked at in a screenshot of the running app. A browser tab can't show it, since a page there has no IPC, so screenshots go through WebDriver: with the `webdriver` feature, `tauri-plugin-wdio-webdriver` embeds a W3C WebDriver server in the app, on `127.0.0.1:4445`. Never ship a build with it.

```sh
npm run tauri dev -- --features webdriver
# then, in another terminal
npm run screenshot -- /releases releases.png
```

- The route is optional, and the app navigates to it in place, so it keeps its state. The file defaults to `screenshot.png`.
- `THEME=dark` or `THEME=light` takes it in that theme. Otherwise the theme follows the system.
- It waits 1.5 seconds for the page to settle, or `WAIT=<ms>`.
- The picture is the window's webview at its pixel density: 2560×1600 for the 1280×800 window on a Retina display.
- To click or type first, any W3C WebDriver client works against `127.0.0.1:4445`. `scripts/screenshot.ts` shows the protocol with `fetch`: a session, `execute/async`, `screenshot`. The element click endpoint sends real input, which Base UI's menus need. Element Send Keys types at the end of what a field holds, so clear it first with Element Clear. Native dialogs, like choosing a library, are outside the webview, so WebDriver can't use them.
- Open menus and popovers may not show in the picture, though WebDriver finds and clicks what's in them: check what they say through the DOM, with `execute/async`. A sheet, like Sync's, does show.
- A release build can be driven the same way: `npm run tauri build -- --no-bundle --features webdriver`, then run `target/release/liquidslunk`.
