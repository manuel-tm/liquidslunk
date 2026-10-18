// Phase 4's steps: releases, their pages and edit tabs, the artist and label
// pages, the bulk edits and the dashboard's totals. They run after phase
// 3's, on the seeded library: Nothing Was the Same (release 1) with track 1,
// High School Musical (2), Alika (3), virmalised (4) and Scenery (5), whose
// credit lines phase 2 added. Formats are 1 Album to 8 Soundtrack, media 1
// Cassette to 4 Vinyl. Phase 3 left text tag 1 Genre, number tag 1 Score and
// date tag 1 Bought on some of them.
import type { Phase, Step } from "./media.ts"

const FILE = "id path name type sizeBytes mtime sha256"
const RELEASE =
  "id title titleLanguage released catalogNumber variousArtists cover { id }"
const TITLES = "alternateTitles { title language printed }"
const CREDITED = "artist { id name nameLanguage } creditedAs { name language }"
const NAMED_CREDITED =
  "artist { id name nameLanguage alternateNames { name language } } creditedAs { name language }"
const CREDITS = `credits { role artists { ${CREDITED} } }`
const LIST_ITEM = `${RELEASE} ${TITLES} artists { ${CREDITED} } formats { id name } media { id name } labels { id name nameLanguage }`

type Args = Record<string, unknown>

/** A filter's name, as the list's GraphQL asks for it beside the page. */
type Named = { id: string; name: string } | null | undefined
type Listed = {
  releases: object
  filterFormat?: Named
  filterMedium?: Named
  filterLabel?: Named
}

const page = (data: Listed) => ({
  ...data.releases,
  filterFormat: data.filterFormat ?? null,
  filterMedium: data.filterMedium ?? null,
  filterLabel: data.filterLabel ?? null,
})

const releaseList = (name: string, args: Args = {}): Step => ({
  name: `releases ${name}`,
  graphql: `query (
    $page: Int, $perPage: Int, $sort: String, $q: String,
    $format: ID, $medium: ID, $label: ID, $artist: ID, $tags: [TagFilterInput!],
    $formatId: ID!, $mediumId: ID!, $labelId: ID!,
    $withFormat: Boolean!, $withMedium: Boolean!, $withLabel: Boolean!
  ) {
    releases(
      page: $page, perPage: $perPage, sort: $sort, q: $q,
      format: $format, medium: $medium, label: $label, artist: $artist, tags: $tags
    ) { total items { ${LIST_ITEM} } }
    filterFormat: listItem(kind: release_formats, id: $formatId) @include(if: $withFormat) { id name }
    filterMedium: listItem(kind: release_media, id: $mediumId) @include(if: $withMedium) { id name }
    filterLabel: label(id: $labelId) @include(if: $withLabel) { id name }
  }`,
  variables: () => ({
    ...args,
    formatId: args.format ?? 0,
    mediumId: args.medium ?? 0,
    labelId: args.label ?? 0,
    withFormat: args.format !== undefined,
    withMedium: args.medium !== undefined,
    withLabel: args.label !== undefined,
  }),
  command: "releases",
  args: () => args,
  whole: true,
  fromGraphql: (data) => page(data as Listed),
})

const releaseIds = (name: string, args: Args = {}): Step => ({
  name: `releaseIds ${name}`,
  graphql: `query ($q: String, $format: ID, $medium: ID, $label: ID, $artist: ID, $tags: [TagFilterInput!]) {
    releaseIds(q: $q, format: $format, medium: $medium, label: $label, artist: $artist, tags: $tags)
  }`,
  variables: () => args,
  command: "releaseIds",
  args: () => args,
  fromGraphql: (ids) => (ids as string[]).map(Number),
})

/** A query of one release, by id, as a command of the same name returns it. */
const ofRelease =
  (command: string, fields: string) =>
  (id: number): Step => ({
    name: `${command} ${id}`,
    graphql: `query ($id: ID!) { release(id: $id) { ${fields} } }`,
    variables: () => ({ id }),
    command,
    args: () => ({ id }),
  })

const releasePage = ofRelease(
  "releasePage",
  `id title titleLanguage ${TITLES} released catalogNumber variousArtists
  fileCount durationMs blockers cover { id }
  artists { ${NAMED_CREDITED} }
  labels { id name nameLanguage } formats { id name } media { id name }
  ${CREDITS}
  tags { tag { id type name multiple } values }
  discTitles { cdNumber title language ${TITLES} }
  tracks {
    id title titleLanguage ${TITLES} durationMs cdNumber trackNumber
    file { id path }
    artists { ${NAMED_CREDITED} } features { ${CREDITED} } ${CREDITS}
  }`
)
const releaseHeader = ofRelease("releaseHeader", "id title titleLanguage")
const release = ofRelease(
  "release",
  `id title titleLanguage ${TITLES} released variousArtists catalogNumber
  artists { ${CREDITED} } labels { id name nameLanguage } formats { id name } media { id name }`
)
const releaseFiles = ofRelease(
  "releaseFiles",
  `id title cover { ${FILE} } files { note file { ${FILE} } }`
)
const releaseCredits = ofRelease(
  "releaseCredits",
  `id title ${CREDITS} tracks { id cdNumber trackNumber title ${CREDITS} }`
)

const releaseOptions = (q: string): Step => ({
  name: `releaseOptions "${q}"`,
  graphql: `query ($q: String) {
    releases(q: $q, perPage: 20, sort: "title") { items { id title cover { id } } }
  }`,
  variables: () => ({ q }),
  command: "releaseOptions",
  args: () => ({ q }),
  fromGraphql: (data) => (data as { items: unknown }).items,
})

const creditRoles = (q: string): Step => ({
  name: `creditRoles "${q}"`,
  graphql: `query ($q: String) { creditRoles(q: $q) }`,
  variables: () => ({ q }),
  command: "creditRoles",
  args: () => ({ q }),
})

const catalogTotals: Step = {
  name: "catalogTotals",
  graphql: `query { catalogTotals { releases artists tracks labels durationMs } }`,
  command: "catalogTotals",
}

const artistHeader = (id: number): Step => ({
  name: `artistHeader ${id}`,
  graphql: `query ($id: ID!) {
    artist(id: $id) {
      id name nameLanguage alternateNames { id name language realName note }
      picture { id } blockers counts { releases appearances credits files }
    }
  }`,
  variables: () => ({ id }),
  command: "artistHeader",
  args: () => ({ id }),
})

const artistReleases = (name: string, id: number, args: Args = {}): Step => ({
  name: `artistReleases ${id} ${name}`,
  graphql: `query (
    $id: ID!, $page: Int, $perPage: Int, $sort: String, $q: String, $format: ID,
    $formatId: ID!, $withFormat: Boolean!
  ) {
    artist(id: $id) { formatCounts { format { id name } count } }
    releases(artist: $id, page: $page, perPage: $perPage, sort: $sort, q: $q, format: $format) {
      total items { ${LIST_ITEM} }
    }
    filterFormat: listItem(kind: release_formats, id: $formatId) @include(if: $withFormat) { id name }
  }`,
  variables: () => ({
    id,
    ...args,
    formatId: args.format ?? 0,
    withFormat: args.format !== undefined,
  }),
  command: "artistReleases",
  args: () => ({ id, ...args }),
  whole: true,
  fromGraphql: (data) => {
    const { artist, ...listed } = data as Listed & {
      artist: { formatCounts: unknown[] } | null
    }
    return {
      formatCounts: artist?.formatCounts ?? [],
      releases: page(listed),
    }
  },
})

const artistAppearances = (id: number): Step => ({
  name: `artistAppearances ${id}`,
  graphql: `query ($id: ID!) {
    artist(id: $id) {
      appearances {
        release { id title titleLanguage released cover { id } artists { ${CREDITED} } }
        tracks { id title titleLanguage artists { ${CREDITED} } features { ${CREDITED} } }
      }
    }
  }`,
  variables: () => ({ id }),
  command: "artistAppearances",
  args: () => ({ id }),
  fromGraphql: (artist) =>
    (artist as { appearances: unknown[] } | null)?.appearances ?? [],
})

const artistCredits = (id: number): Step => ({
  name: `artistCredits ${id}`,
  graphql: `query ($id: ID!) {
    artist(id: $id) {
      credits {
        role creditedAs { name language }
        release { id title titleLanguage released }
        track { id title titleLanguage }
      }
    }
  }`,
  variables: () => ({ id }),
  command: "artistCredits",
  args: () => ({ id }),
  fromGraphql: (artist) =>
    (artist as { credits: unknown[] } | null)?.credits ?? [],
})

const labelPage = (id: number, args: Args = {}): Step => ({
  name: `labelPage ${id}`,
  graphql: `query ($id: ID!, $page: Int, $perPage: Int, $sort: String) {
    label(id: $id) {
      id name nameLanguage alternateNames { name language note } picture { id } blockers
    }
    releases(label: $id, page: $page, perPage: $perPage, sort: $sort) {
      total items { ${LIST_ITEM} }
    }
    filterLabel: label(id: $id) { id name }
  }`,
  variables: () => ({ id, ...args }),
  command: "labelPage",
  args: () => ({ id, ...args }),
  whole: true,
  fromGraphql: (data) => {
    const { label, ...listed } = data as Listed & { label: object | null }
    return label ? { ...label, releases: page(listed) } : null
  },
})

const createRelease = (name: string, input: Args): Step => ({
  name: `createRelease ${name}`,
  graphql: `mutation ($input: ReleaseInput!) { createRelease(input: $input) { ${RELEASE} } }`,
  variables: () => ({ input }),
  command: "createRelease",
  args: () => input,
})

const updateRelease = (name: string, id: number, input: Args): Step => ({
  name: `updateRelease ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: ReleaseInput!) {
    updateRelease(id: $id, input: $input) { ${RELEASE} }
  }`,
  variables: () => ({ id, input }),
  command: "updateRelease",
  args: () => ({ id, ...input }),
})

const deleteRelease = (id: number): Step => ({
  name: `deleteRelease ${id}`,
  graphql: `mutation ($id: ID!) { deleteRelease(id: $id) }`,
  variables: () => ({ id }),
  command: "deleteRelease",
  args: () => ({ id }),
  returnsId: true,
})

const setReleaseCover = (name: string, id: number, cover: Args): Step => ({
  name: `setReleaseCover ${id} ${name}`,
  graphql: `mutation ($id: ID!, $cover: FileRefInput!) {
    setReleaseCover(id: $id, cover: $cover) { ${RELEASE} }
  }`,
  variables: () => ({ id, cover }),
  command: "setReleaseCover",
  args: () => ({ id, cover }),
})

const setReleaseFiles = (
  name: string,
  id: number,
  input: { cover: Args | null; files: Args[] }
): Step => ({
  name: `setReleaseFiles ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: ReleaseFilesInput!) {
    setReleaseFiles(id: $id, input: $input) { ${RELEASE} }
  }`,
  variables: () => ({ id, input }),
  command: "setReleaseFiles",
  args: () => ({ id, ...input }),
})

const setReleaseCredits = (
  name: string,
  id: number,
  input: { credits: Args[]; tracks?: Args[] }
): Step => ({
  name: `setReleaseCredits ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: ReleaseCreditsInput!) {
    setReleaseCredits(id: $id, input: $input) { ${RELEASE} }
  }`,
  variables: () => ({ id, input }),
  command: "setReleaseCredits",
  args: () => ({ id, ...input }),
})

const bulkTag = (
  mutation: "setTagOnReleases" | "removeTagFromReleases",
  name: string,
  args: { ids: number[]; type: string; tagId: number; value?: string }
): Step => ({
  name: `${mutation} ${name}`,
  graphql:
    mutation === "setTagOnReleases"
      ? `mutation ($ids: [ID!]!, $type: TagType!, $tagId: ID!, $value: String!) {
          setTagOnReleases(ids: $ids, type: $type, tagId: $tagId, value: $value)
        }`
      : `mutation ($ids: [ID!]!, $type: TagType!, $tagId: ID!, $value: String) {
          removeTagFromReleases(ids: $ids, type: $type, tagId: $tagId, value: $value)
        }`,
  variables: () => args,
  command: mutation,
  args: () => args,
})

const credited = (artistId: number, creditedAs?: string) =>
  creditedAs === undefined ? { artistId } : { artistId, creditedAs }

// The release this phase creates
const KIND = 6

export const releases: Phase = {
  // Track 1 gets artists, a feature, a credit line, an alternate title and a
  // disc title, through GraphQL: tracks are saved through it until phase 5.
  // Artists 11 to 13 are ones the earlier phases don't look at.
  prepare: [
    {
      graphql: `mutation {
        updateTrack(id: 1, input: {
          releaseId: 1, fileId: 1, trackNumber: 1, title: "Early Summer",
          artists: [{ artistId: 12, creditedAs: "いとう・まさたか" }],
          features: [{ artistId: 13 }],
          credits: [{ role: "Bass", artists: [{ artistId: 11, creditedAs: "J. Kosma" }] }]
        }) { id }
      }`,
    },
    {
      graphql: `mutation {
        setReleaseTracks(id: 1, tracks: [{
          id: 1, cdNumber: 1, trackNumber: 1, title: "Early Summer",
          alternateTitles: [{ title: "初夏", language: "ja" }],
          artists: [{ artistId: 12, creditedAs: "いとう・まさたか" }],
          features: [{ artistId: 13 }]
        }], discTitles: [{
          cdNumber: 1, title: "Side A", alternateTitles: [{ title: "A面", language: "ja" }]
        }]) { id }
      }`,
    },
  ],
  steps: [
    // The list
    releaseList("by default"),
    releaseList("by title", { sort: "title" }),
    releaseList("by date", { sort: "released" }),
    releaseList("by date, newest first", { sort: "-released" }),
    releaseList("paged", { page: 2, perPage: 2, sort: "title" }),
    releaseList("by a column it can't", { sort: "label" }),
    releaseList("on page 0", { page: 0 }),
    releaseList("searched by an alternate title", { q: "シーナ" }),
    releaseList("searched", { q: "ni" }),
    releaseList("by format and medium", { format: 1, medium: 2 }),
    releaseList("by label", { label: 1 }),
    releaseList("by artist", { artist: 7 }),
    releaseList("by a format that isn't there", { format: 999 }),
    releaseList("by tags", {
      tags: [
        { type: "number", tagId: 1, min: "8" },
        { type: "text", tagId: 1, contains: "o" },
      ],
    }),
    releaseList("by a date tag", {
      tags: [{ type: "date", tagId: 1, equals: "2024" }],
    }),
    releaseList("by a tag filter that's wrong", {
      tags: [{ type: "text", tagId: 1, min: "8" }],
    }),
    releaseIds("of every release"),
    releaseIds("filtered", { q: "e", format: 1 }),
    releaseIds("by tags", {
      tags: [{ type: "text", tagId: 1, equals: "Pop" }],
    }),

    // A release
    releasePage(1),
    releasePage(2),
    releasePage(5),
    releasePage(999),
    releaseHeader(5),
    releaseHeader(999),
    release(5),
    release(999),
    releaseFiles(1),
    releaseFiles(999),
    releaseCredits(1),
    releaseCredits(5),
    releaseCredits(999),
    releaseOptions(""),
    releaseOptions("s"),
    creditRoles(""),
    creditRoles(" b"),
    catalogTotals,

    // The artist and label pages
    artistHeader(7),
    artistHeader(12),
    artistHeader(999),
    artistReleases("by default", 7),
    artistReleases("by date, of a format", 7, { sort: "released", format: 1 }),
    artistReleases("of an artist that isn't there", 999),
    artistAppearances(12),
    artistAppearances(13),
    artistAppearances(999),
    artistCredits(7),
    artistCredits(10),
    artistCredits(11),
    labelPage(1),
    labelPage(6, { sort: "released" }),
    labelPage(999),

    // Saving a release
    createRelease("with everything", {
      title: " Kind of Blue ",
      titleLanguage: "EN",
      alternateTitles: [
        { title: "カインド・オブ・ブルー", language: "ja-Kana" },
        { title: " " },
        { title: "Kind of Blue (Legacy)", printed: false },
      ],
      released: " 1959-08-17 ",
      catalogNumber: " CL 1355 ",
      artists: [
        credited(7, "R. Fukui"),
        credited(9, "J. Van Heusen"),
        credited(7),
        credited(12, " Masataka "),
      ],
      labelIds: [1, 6, 1],
      formatIds: [5, 1],
      mediumIds: [4],
    }),
    releasePage(KIND),
    artistHeader(12),
    createRelease("with a malformed date", {
      title: "Bad",
      released: "1959-02-30",
      artists: [],
    }),
    createRelease("with und", {
      title: "Bad",
      titleLanguage: "und",
      released: "59",
      artists: [],
    }),
    createRelease("with an artist that isn't there", {
      title: "Bad",
      artists: [credited(999)],
    }),
    createRelease("with a label that isn't there", {
      title: "Bad",
      artists: [],
      labelIds: [999],
    }),
    createRelease("with a blank title", { title: " ", artists: [] }),
    createRelease("with a title listed twice", {
      title: "Bad",
      alternateTitles: [{ title: "Twice" }, { title: "Twice " }],
      artists: [],
    }),
    createRelease("with its main title as an alternate", {
      title: "Bad",
      alternateTitles: [{ title: "Bad" }],
      artists: [],
    }),
    updateRelease("renamed to an alternate title, language left out", KIND, {
      title: "Kind of Blue (Legacy)",
      released: "1959",
      artists: [credited(7, "R. Fukui")],
      variousArtists: true,
    }),
    release(KIND),
    updateRelease("with the language and date cleared", KIND, {
      title: "Kind of Blue",
      titleLanguage: null,
      released: " ",
      catalogNumber: "",
      artists: [credited(9)],
      alternateTitles: [],
    }),
    release(KIND),
    updateRelease("that isn't there", 999, { title: "Nobody", artists: [] }),

    // Its cover and files
    setReleaseCover("from a registered path", KIND, {
      path: "music/Drake/drake.jpg",
    }),
    setReleaseCover("from a text file", KIND, {
      path: "music/Ryo Fukui/rip.log",
    }),
    setReleaseCover("from nothing", KIND, {}),
    setReleaseCover("of a release that isn't there", 999, { fileId: 2 }),
    setReleaseFiles("with a cover and links", KIND, {
      cover: { fileId: 2 },
      files: [
        { path: "music/Ryo Fukui/10 Mellow Dream.flac", note: " Bonus " },
        { fileId: 3, note: "" },
        { path: "music/Alika/Café.flac" },
      ],
    }),
    releaseFiles(KIND),
    setReleaseFiles("with a link to a file that isn't there", KIND, {
      cover: null,
      files: [{ path: "music/nothing.flac" }],
    }),
    setReleaseFiles("with no cover", KIND, {
      cover: null,
      files: [{ fileId: 3 }],
    }),
    setReleaseFiles("of a release that isn't there", 999, {
      cover: null,
      files: [],
    }),
    releaseFiles(KIND),

    // Its credits
    setReleaseCredits("with names to resolve", KIND, {
      credits: [
        {
          role: " Trumpet ",
          artists: [
            credited(7, "R. Fukui"),
            credited(7),
            credited(10, "A. R."),
          ],
        },
        { role: "Piano", artists: [credited(9)] },
      ],
    }),
    setReleaseCredits("on its track", 1, {
      credits: [{ role: "Mixed By", artists: [credited(2)] }],
      tracks: [
        {
          trackId: 1,
          credits: [
            { role: "Engineer", artists: [credited(11, "J. Kosma")] },
            { role: "mixed by", artists: [credited(13)] },
          ],
        },
      ],
    }),
    releaseCredits(KIND),
    releaseCredits(1),
    setReleaseCredits("with a blank role", KIND, {
      credits: [{ role: " ", artists: [credited(7)] }],
    }),
    setReleaseCredits("with no artist", KIND, {
      credits: [{ role: "Piano", artists: [] }],
    }),
    setReleaseCredits("with a role twice", KIND, {
      credits: [
        { role: "Piano", artists: [credited(7)] },
        { role: "PIANO", artists: [credited(9)] },
      ],
    }),
    setReleaseCredits("with another release's track", KIND, {
      credits: [],
      tracks: [{ trackId: 1, credits: [] }],
    }),
    setReleaseCredits("with an artist that isn't there", KIND, {
      credits: [{ role: "Piano", artists: [credited(999)] }],
    }),
    setReleaseCredits("of a release that isn't there", 999, { credits: [] }),
    creditRoles(""),
    artistCredits(7),
    artistHeader(10),

    // Bulk edits
    bulkTag("setTagOnReleases", "a multi-value tag", {
      ids: [1, 2, KIND, 999],
      type: "text",
      tagId: 1,
      value: " Jazz ",
    }),
    bulkTag("setTagOnReleases", "a single-value tag", {
      ids: [1, 2, KIND],
      type: "number",
      tagId: 1,
      value: "8.0",
    }),
    bulkTag("setTagOnReleases", "with no value", {
      ids: [1],
      type: "number",
      tagId: 1,
      value: " ",
    }),
    bulkTag("setTagOnReleases", "with a bad value", {
      ids: [1],
      type: "date",
      tagId: 1,
      value: "2024-13",
    }),
    bulkTag("setTagOnReleases", "with a tag that isn't there", {
      ids: [1],
      type: "text",
      tagId: 999,
      value: "Rock",
    }),
    bulkTag("removeTagFromReleases", "one value", {
      ids: [1, 2, KIND],
      type: "text",
      tagId: 1,
      value: "JAZZ",
    }),
    bulkTag("removeTagFromReleases", "every value", {
      ids: [KIND],
      type: "number",
      tagId: 1,
    }),
    releasePage(KIND),

    // Deleting
    deleteRelease(1),
    deleteRelease(KIND),
    deleteRelease(999),
    releasePage(KIND),
    releaseList("at the end", { sort: "title" }),
    catalogTotals,
  ],
}
