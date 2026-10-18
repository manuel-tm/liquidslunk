// Phase 2's steps: the catalog's artists, labels and lists, and the
// languages in use. They run after phase 1's, on the seeded library: its
// artists, labels and five releases, the formats and media the baseline
// adds, and the files phase 1 left registered. File 2, music/Ryo
// Fukui/cover.jpg, stays registered: it's Drake's picture. The files under
// music/Drake/ are new to this phase.
import type { Phase, Step } from "./media.ts"

const FILE = "id path name type sizeBytes mtime sha256"
const ITEM = "id kind name"
const ARTIST = "id name nameLanguage picture { id }"
const ARTIST_NAME = "id name language realName note"
const LABEL = "id name nameLanguage picture { id }"
const LABEL_NAME = "name language note"

type Kind = "release_formats" | "release_media"
type Args = Record<string, unknown>

const listItems = (name: string, kind: Kind, args: Args = {}): Step => ({
  name: `listItems ${kind} ${name}`,
  graphql: `query ($kind: ListKind!, $page: Int, $perPage: Int, $sort: String, $q: String) {
    listItems(kind: $kind, page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total items { ${ITEM} uses }
    }
  }`,
  variables: () => ({ kind, ...args }),
  command: "listItems",
  args: () => ({ kind, ...args }),
})

const listItem = (kind: Kind, id: number): Step => ({
  name: `listItem ${kind} ${id}`,
  graphql: `query ($kind: ListKind!, $id: ID!) {
    listItem(kind: $kind, id: $id) { ${ITEM} uses blockers }
  }`,
  variables: () => ({ kind, id }),
  command: "listItem",
  args: () => ({ kind, id }),
})

const listItemOptions = (kind: Kind, q: string): Step => ({
  name: `listItemOptions ${kind} "${q}"`,
  graphql: `query ($kind: ListKind!, $q: String) {
    listItems(kind: $kind, q: $q, perPage: 20, sort: "name") { items { ${ITEM} } }
  }`,
  variables: () => ({ kind, q }),
  command: "listItemOptions",
  args: () => ({ kind, q }),
  fromGraphql: (page) => (page as { items: unknown }).items,
})

const createListItem = (kind: Kind, name: string): Step => ({
  name: `createListItem ${kind} "${name}"`,
  graphql: `mutation ($kind: ListKind!, $name: String!) {
    createListItem(kind: $kind, name: $name) { ${ITEM} }
  }`,
  variables: () => ({ kind, name }),
  command: "createListItem",
  args: () => ({ kind, name }),
})

const renameListItem = (kind: Kind, id: number, name: string): Step => ({
  name: `renameListItem ${kind} ${id} "${name}"`,
  graphql: `mutation ($kind: ListKind!, $id: ID!, $name: String!) {
    renameListItem(kind: $kind, id: $id, name: $name) { ${ITEM} }
  }`,
  variables: () => ({ kind, id, name }),
  command: "renameListItem",
  args: () => ({ kind, id, name }),
})

const deleteListItem = (kind: Kind, id: number): Step => ({
  name: `deleteListItem ${kind} ${id}`,
  graphql: `mutation ($kind: ListKind!, $id: ID!) { deleteListItem(kind: $kind, id: $id) }`,
  variables: () => ({ kind, id }),
  command: "deleteListItem",
  args: () => ({ kind, id }),
  returnsId: true,
})

const artists = (name: string, args: Args = {}): Step => ({
  name: `artists ${name}`,
  graphql: `query ($page: Int, $perPage: Int, $sort: String, $q: String) {
    artists(page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total items { ${ARTIST} alternateNames { ${ARTIST_NAME} } }
    }
  }`,
  variables: () => args,
  command: "artists",
  args: () => args,
})

const artist = (id: number): Step => ({
  name: `artist ${id}`,
  graphql: `query ($id: ID!) {
    artist(id: $id) {
      id name nameLanguage mainNameUses
      alternateNames { ${ARTIST_NAME} uses }
      picture { ${FILE} }
      files { note file { ${FILE} } }
    }
  }`,
  variables: () => ({ id }),
  command: "artist",
  args: () => ({ id }),
})

const artistOptions = (q: string): Step => ({
  name: `artistOptions "${q}"`,
  graphql: `query ($q: String) {
    artists(q: $q, perPage: 20, sort: "name") {
      items { ${ARTIST} alternateNames { ${ARTIST_NAME} } }
    }
  }`,
  variables: () => ({ q }),
  command: "artistOptions",
  args: () => ({ q }),
  fromGraphql: (page) => (page as { items: unknown }).items,
})

const artistNames = (id: number): Step => ({
  name: `artistNames ${id}`,
  graphql: `query ($id: ID!) { artist(id: $id) { alternateNames { ${ARTIST_NAME} } } }`,
  variables: () => ({ id }),
  command: "artistNames",
  args: () => ({ id }),
  fromGraphql: (found) =>
    (found as { alternateNames: unknown[] } | null)?.alternateNames ?? [],
})

const createArtist = (name: string, input: Args): Step => ({
  name: `createArtist ${name}`,
  graphql: `mutation ($input: ArtistInput!) { createArtist(input: $input) { ${ARTIST} } }`,
  variables: () => ({ input }),
  command: "createArtist",
  args: () => input,
})

const updateArtist = (name: string, id: number, input: Args): Step => ({
  name: `updateArtist ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: ArtistInput!) {
    updateArtist(id: $id, input: $input) { ${ARTIST} }
  }`,
  variables: () => ({ id, input }),
  command: "updateArtist",
  args: () => ({ id, ...input }),
})

const setPicture = (
  owner: "Artist" | "Label",
  id: number,
  picture: Args
): Step => ({
  name: `set${owner}Picture ${id} ${JSON.stringify(picture)}`,
  graphql: `mutation ($id: ID!, $picture: FileRefInput!) {
    set${owner}Picture(id: $id, picture: $picture) { ${owner === "Artist" ? ARTIST : LABEL} }
  }`,
  variables: () => ({ id, picture }),
  command: `set${owner}Picture`,
  args: () => ({ id, picture }),
})

const deleteNamed = (owner: "Artist" | "Label", id: number): Step => ({
  name: `delete${owner} ${id}`,
  graphql: `mutation ($id: ID!) { delete${owner}(id: $id) }`,
  variables: () => ({ id }),
  command: `delete${owner}`,
  args: () => ({ id }),
  returnsId: true,
})

const labels = (name: string, args: Args = {}): Step => ({
  name: `labels ${name}`,
  graphql: `query ($page: Int, $perPage: Int, $sort: String, $q: String) {
    labels(page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total items { ${LABEL} alternateNames { ${LABEL_NAME} } }
    }
  }`,
  variables: () => args,
  command: "labels",
  args: () => args,
})

const label = (id: number): Step => ({
  name: `label ${id}`,
  graphql: `query ($id: ID!) {
    label(id: $id) {
      id name nameLanguage alternateNames { ${LABEL_NAME} } picture { ${FILE} }
    }
  }`,
  variables: () => ({ id }),
  command: "label",
  args: () => ({ id }),
})

const labelOptions = (q: string): Step => ({
  name: `labelOptions "${q}"`,
  graphql: `query ($q: String) {
    labels(q: $q, perPage: 20, sort: "name") {
      items { ${LABEL} alternateNames { ${LABEL_NAME} } }
    }
  }`,
  variables: () => ({ q }),
  command: "labelOptions",
  args: () => ({ q }),
  fromGraphql: (page) => (page as { items: unknown }).items,
})

const createLabel = (name: string, input: Args): Step => ({
  name: `createLabel ${name}`,
  graphql: `mutation ($input: LabelInput!) { createLabel(input: $input) { ${LABEL} } }`,
  variables: () => ({ input }),
  command: "createLabel",
  args: () => input,
})

const updateLabel = (name: string, id: number, input: Args): Step => ({
  name: `updateLabel ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: LabelInput!) {
    updateLabel(id: $id, input: $input) { ${LABEL} }
  }`,
  variables: () => ({ id, input }),
  command: "updateLabel",
  args: () => ({ id, ...input }),
})

const languages: Step = {
  name: "languages",
  graphql: "{ languages }",
  command: "languages",
}

// The seed's ids: Drake is artist 1, Alika 6, Ryo Fukui 7, Ryo Fukui Trio 8
// and Ann Ronell 10. Alika's names are Alika Milova (1, a real name) and
// ALIKA (2), credited on virmalised; Ryo Fukui's are 福居良 (3) and R. Fukui
// (4); Ann Ronell's is A. Ronnell (7). Nadja (label 6) is on Scenery
// (release 5), which has the formats LP and Album.
const FORMATS = "release_formats"
const MEDIA = "release_media"

export const catalog: Phase = {
  // Credit lines on Scenery: Ryo Fukui as R. Fukui, and Ann Ronell as A.
  // Ronnell, then Ryo Fukui with his main name
  prepare: [
    {
      graphql: `mutation {
        setReleaseCredits(id: 5, input: { credits: [
          { role: "Piano", artists: [{ artistId: 7, creditedAs: "R. Fukui" }] },
          { role: "Composed By", artists: [{ artistId: 10, creditedAs: "A. Ronnell" }, { artistId: 7 }] }
        ] }) { id }
      }`,
    },
  ],
  steps: [
    listItems("by default", FORMATS),
    listItems("by name, searched", FORMATS, { sort: "name", q: "l" }),
    listItems("by name, descending", MEDIA, { sort: "-name" }),
    listItems("paged", MEDIA, { page: 2, perPage: 3 }),
    listItems("by uses", FORMATS, { sort: "uses" }),
    listItems("on page 0", FORMATS, { page: 0 }),
    listItem(FORMATS, 1),
    listItem(MEDIA, 3),
    listItem(MEDIA, 999),
    listItemOptions(FORMATS, "a"),
    listItemOptions(MEDIA, ""),
    createListItem(FORMATS, " Box  Set "),
    createListItem(FORMATS, "box  set"),
    createListItem(MEDIA, "Cafe\u0301 Tape"),
    createListItem(MEDIA, "  "),
    renameListItem(FORMATS, 9, "Boxset"),
    renameListItem(MEDIA, 1, "cd"),
    renameListItem(FORMATS, 999, "Nothing"),
    deleteListItem(FORMATS, 1),
    deleteListItem(MEDIA, 3),
    deleteListItem(FORMATS, 9),
    deleteListItem(MEDIA, 999),
    listItems("by name", FORMATS, { sort: "name" }),
    listItems("by name", MEDIA, { sort: "name" }),

    artists("by default"),
    artists("by name, searched", { sort: "name", q: "FUKUI" }),
    artists("found by an alternate name", { q: "福居" }),
    artists("paged", { q: "a", page: 2, perPage: 2, sort: "-name" }),
    artists("by picture", { sort: "picture" }),
    artist(1),
    artist(6),
    artist(7),
    artist(999),
    artistOptions("a"),
    artistOptions("いと"),
    artistNames(6),
    artistNames(999),

    createArtist("with names, a language and file links", {
      name: " Majid  Jordan ",
      nameLanguage: "EN-us",
      alternateNames: [
        { name: "MJ" },
        { name: "  " },
        { name: "Cafe\u0301", language: "fr", note: " a note " },
      ],
      files: [
        { path: "./music/Drake//interview.pdf", note: " Interview " },
        { fileId: 2 },
      ],
    }),
    artist(14),
    // Majid Jordan's names: MJ (11) and Café (12)
    updateArtist("keeping, adding and removing names", 14, {
      name: "Majid Jordan",
      alternateNames: [
        { id: 12, name: "Café", language: "fr", note: "a note" },
        { name: "Majid & Jordan" },
      ],
      files: [{ fileId: 2, note: "Drake's" }],
    }),
    updateArtist("swapping two names", 14, {
      name: "Majid Jordan",
      alternateNames: [
        { id: 12, name: "Majid & Jordan" },
        { id: 13, name: "Café" },
      ],
    }),
    updateArtist("with another artist's name", 14, {
      name: "Majid Jordan",
      alternateNames: [{ id: 3, name: "福居良" }],
    }),
    updateArtist("leaving the names out", 14, { name: "Majid Jordan" }),
    artist(14),
    createArtist("only a name", { name: "Zac Efron" }),
    createArtist("with a name twice", {
      name: "Alika",
      alternateNames: [{ name: "ALIKA" }, { name: " ALIKA" }],
    }),
    createArtist("with the main name", {
      name: "Alika",
      alternateNames: [{ name: "Alika" }],
    }),
    createArtist("with the main name as a real name", {
      name: "Alika Milova",
      alternateNames: [{ name: "Alika Milova", realName: true }],
    }),
    createArtist("with a malformed language", {
      name: "Ryo",
      nameLanguage: "ja_JP",
    }),
    createArtist("with und", {
      name: "Ryo",
      alternateNames: [{ name: "Fukui", language: "und" }],
    }),
    createArtist("with a long language", {
      name: "Ryo",
      nameLanguage: "japanese",
    }),
    createArtist("with an alias language", {
      name: "Ryo",
      nameLanguage: "jpn",
      alternateNames: [{ name: "R.", language: "iw" }],
    }),
    createArtist("with a missing file", {
      name: "Drake",
      files: [{ path: "music/Drake/gone.pdf" }],
    }),
    createArtist("with a blank name", { name: "  " }),
    createArtist("with a picture that isn't an image", {
      name: "Drake",
      pictureId: 1,
    }),

    updateArtist("that isn't there", 999, { name: "Nobody" }),

    // Make main, on Alika: ALIKA becomes the main name, and Alika takes its
    // place, keeping it on the release that showed it
    updateArtist("making ALIKA the main name", 6, {
      name: "ALIKA",
      alternateNames: [
        { id: 1, name: "Alika Milova", realName: true },
        { name: "Alika" },
      ],
      keepOldName: true,
    }),
    artist(6),
    // Renamed, keeping the old name on the links that showed it
    updateArtist("renamed, keeping the old name", 7, {
      name: "Ryo Fukui (piano)",
      nameLanguage: "en",
      keepOldName: true,
    }),
    artist(7),
    // Renamed to an alternate name, which is promoted
    updateArtist("renamed to an alternate name", 7, { name: "R. Fukui" }),
    artist(7),
    // Renamed without keeping the old name
    updateArtist("renamed", 7, { name: "Ryo Fukui" }),
    artist(7),
    // A misprint, fixed on its links
    updateArtist("with a name renamed", 10, {
      name: "Ann Ronell",
      alternateNames: [{ id: 7, name: "A. Ronell", note: "misprint" }],
    }),
    // A name that's credited, removed
    updateArtist("with a credited name removed", 10, {
      name: "Ann Ronell",
      alternateNames: [],
    }),
    artist(10),

    setPicture("Artist", 14, { path: "music/Drake/drake.jpg" }),
    setPicture("Artist", 14, { path: "music/Drake/notes.txt" }),
    setPicture("Artist", 999, { path: "music/Drake/logo.png" }),
    setPicture("Artist", 2, { fileId: 2 }),
    setPicture("Artist", 2, {}),
    setPicture("Artist", 2, { path: "music/Drake/gone.jpg" }),
    deleteNamed("Artist", 7),
    deleteNamed("Artist", 14),
    deleteNamed("Artist", 999),
    artist(14),
    artists("by name", { sort: "name" }),

    labels("by default"),
    labels("searched", { q: "very" }),
    labels("by logo", { sort: "logo" }),
    label(1),
    label(999),
    labelOptions("re"),
    createLabel("with names", {
      name: "Trio Records",
      nameLanguage: "en",
      alternateNames: [
        { name: "トリオレコード", language: "JA" },
        { name: "Trio" },
        { name: " " },
      ],
    }),
    createLabel("with a name twice", {
      name: "Nadja",
      alternateNames: [{ name: "ナジャ" }, { name: "ナジャ " }],
    }),
    createLabel("with the main name", {
      name: "Nadja",
      alternateNames: [{ name: "Nadja" }],
    }),
    createLabel("with a malformed language", {
      name: "Nadja",
      nameLanguage: "en--US",
    }),
    label(7),
    updateLabel("renamed to an alternate name", 7, { name: "Trio" }),
    updateLabel("with names replaced", 7, {
      name: "Trio Records",
      alternateNames: [{ name: "Trio", note: "former name" }],
    }),
    updateLabel("that isn't there", 999, { name: "Nobody" }),
    label(7),
    setPicture("Label", 7, { path: "music/Drake/logo.png" }),
    setPicture("Label", 7, { path: "music/Drake/notes.txt" }),
    setPicture("Label", 999, { fileId: 2 }),
    label(7),
    deleteNamed("Label", 6),
    deleteNamed("Label", 7),
    labels("by name", { sort: "name" }),

    languages,
  ],
}
