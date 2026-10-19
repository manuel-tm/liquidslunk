// Phase 5's steps: tracks, a release's track list and disc titles, and the
// import, from its suggestions to its saves. They run after phase 4's: track
// 1 is on Nothing Was the Same (release 1), with an alternate title, artists
// 12 and 13, a credit line, disc 1's title, and phase 3's tags (text tag 1
// Genre, number tag 1 Score). Files 5, 6, 8, 9, 14 and 15 are music nothing
// uses, 2 and 7 images, and 10 and 11 other files. Artists 4 and 15 are both
// named Zac Efron, and artist 6 is ALIKA, with the alternate name Alika.
// music/Import/ is the folder imported (index.ts, `makeFiles`).
import type { Phase, Step } from "./media.ts"

const FILE = "id path name type sizeBytes mtime sha256"
const RELEASE =
  "id title titleLanguage released catalogNumber variousArtists cover { id }"
const TITLES = "alternateTitles { title language printed }"
const CREDITED = "artist { id name nameLanguage } creditedAs { name language }"
const NAMED_CREDITED =
  "artist { id name nameLanguage alternateNames { name language } } creditedAs { name language }"
const CREDITS = `credits { role artists { ${CREDITED} } }`
const TAGS = "tags { tag { id type name multiple } values }"
const HEADER = "id title titleLanguage"
const TRACK =
  "id title titleLanguage durationMs cdNumber trackNumber release { id } file { id }"
const SUGGESTED =
  "name matches creditedAs otherNames artist { id name nameLanguage }"

type Args = Record<string, unknown>
type Data = Record<string, unknown>

/** A track as a save returns it, from GraphQL's, whose ids are strings. */
const savedTrack = (data: unknown) => {
  const { release, file, ...track } = data as Data & {
    release: { id: string }
    file: { id: string }
  }
  return { ...track, releaseId: Number(release.id), fileId: Number(file.id) }
}

const trackList = (name: string, args: Args = {}): Step => ({
  name: `tracks ${name}`,
  graphql: `query (
    $page: Int, $perPage: Int, $sort: String, $q: String, $release: ID,
    $tags: [TagFilterInput!], $releaseId: ID!, $withRelease: Boolean!
  ) {
    tracks(page: $page, perPage: $perPage, sort: $sort, q: $q, release: $release, tags: $tags) {
      total items {
        id title titleLanguage ${TITLES} durationMs cdNumber trackNumber
        release { ${HEADER} } performers { ${CREDITED} } file { id path }
      }
    }
    filterRelease: release(id: $releaseId) @include(if: $withRelease) { id name: title }
  }`,
  variables: () => ({
    ...args,
    releaseId: args.release ?? 0,
    withRelease: args.release !== undefined,
  }),
  command: "tracks",
  args: () => args,
  whole: true,
  fromGraphql: (data) => {
    const { tracks, filterRelease } = data as Data & { tracks: object }
    return { ...tracks, filterRelease: filterRelease ?? null }
  },
})

const trackPage = (id: number): Step => ({
  name: `trackPage ${id}`,
  graphql: `query ($id: ID!) {
    track(id: $id) {
      id title titleLanguage ${TITLES} durationMs cdNumber trackNumber
      release { ${HEADER} } performers { ${NAMED_CREDITED} } features { ${CREDITED} }
      ${CREDITS} file { id path } ${TAGS} files { note file { ${FILE} } }
    }
  }`,
  variables: () => ({ id }),
  command: "trackPage",
  args: () => ({ id }),
})

const trackForm = (
  name: string,
  args: { id?: number; release?: number; file?: number }
): Step => ({
  name: `trackForm ${name}`,
  graphql: `query (
    $id: ID!, $isEdit: Boolean!, $releaseId: ID!, $withRelease: Boolean!,
    $fileId: ID!, $withFile: Boolean!
  ) {
    track(id: $id) @include(if: $isEdit) {
      id title titleLanguage ${TITLES} cdNumber trackNumber
      release { ${HEADER} } file { id path }
      artists { ${CREDITED} } features { ${CREDITED} } ${CREDITS}
      files { note file { ${FILE} } } ${TAGS}
    }
    prefillRelease: release(id: $releaseId) @include(if: $withRelease) { ${HEADER} }
    prefillFile: file(id: $fileId) @include(if: $withFile) { ${FILE} }
  }`,
  variables: () => ({
    id: args.id ?? 0,
    isEdit: args.id !== undefined,
    releaseId: args.release ?? 0,
    withRelease: args.release !== undefined,
    fileId: args.file ?? 0,
    withFile: args.file !== undefined,
  }),
  command: "trackForm",
  args: () => args,
  whole: true,
  fromGraphql: (data) => {
    const { track, prefillRelease, prefillFile } = data as Data
    return {
      track: track ?? null,
      prefillRelease: prefillRelease ?? null,
      prefillFile: prefillFile ?? null,
    }
  },
})

const releaseTracks = (id: number, folder?: string): Step => ({
  name: `releaseTracks ${id}${folder === undefined ? "" : ` from "${folder}"`}`,
  graphql: `query ($id: ID!, $folder: String!, $withFolder: Boolean!) {
    release(id: $id) {
      id title variousArtists artists { ${CREDITED} }
      discTitles { cdNumber title language ${TITLES} }
      tracks {
        id cdNumber trackNumber title titleLanguage ${TITLES} durationMs
        file { id path } artists { ${CREDITED} } features { ${CREDITED} }
      }
    }
    trackSuggestions(folder: $folder) @include(if: $withFolder) {
      path cdNumber discTitle trackNumber title durationMs file { id }
      track { id release { ${HEADER} } } artists { ${SUGGESTED} }
    }
  }`,
  variables: () => ({
    id,
    folder: folder ?? "",
    withFolder: folder !== undefined,
  }),
  command: "releaseTracks",
  args: () => (folder === undefined ? { id } : { id, folder }),
  whole: true,
  fromGraphql: (data) => {
    const { release, trackSuggestions } = data as Data & {
      release: object | null
    }
    return release
      ? { ...release, trackSuggestions: trackSuggestions ?? null }
      : null
  },
})

const releaseSuggestion = (folder: string): Step => ({
  name: `releaseSuggestion "${folder}"`,
  graphql: `query ($folder: String!) {
    releaseSuggestion(folder: $folder) {
      title released music images others artists { ${SUGGESTED} }
      sameTitle { id title artists { ${CREDITED} } }
    }
  }`,
  variables: () => ({ folder }),
  command: "releaseSuggestion",
  args: () => ({ folder }),
})

const importFiles = (id: number, folder: string): Step => ({
  name: `importFiles ${id} from "${folder}"`,
  graphql: `query ($id: ID!, $folder: String!) {
    release(id: $id) { id title cover { ${FILE} } files { note file { ${FILE} } } }
    folderFiles(path: $folder) { name path type sizeBytes file { id } }
  }`,
  variables: () => ({ id, folder }),
  command: "importFiles",
  args: () => ({ id, folder }),
  whole: true,
  fromGraphql: (data) => {
    const { release, folderFiles } = data as Data & { release: object | null }
    return release ? { ...release, folderFiles } : null
  },
})

const releasePage = (id: number): Step => ({
  name: `releasePage ${id}`,
  graphql: `query ($id: ID!) {
    release(id: $id) {
      id title titleLanguage ${TITLES} released catalogNumber variousArtists
      fileCount durationMs blockers cover { id }
      artists { ${NAMED_CREDITED} }
      labels { id name nameLanguage } formats { id name } media { id name }
      ${CREDITS} ${TAGS}
      discTitles { cdNumber title language ${TITLES} }
      tracks {
        id title titleLanguage ${TITLES} durationMs cdNumber trackNumber
        file { id path }
        artists { ${NAMED_CREDITED} } features { ${CREDITED} } ${CREDITS}
      }
    }
  }`,
  variables: () => ({ id }),
  command: "releasePage",
  args: () => ({ id }),
})

const artistNames = (id: number): Step => ({
  name: `artistNames ${id}`,
  graphql: `query ($id: ID!) {
    artist(id: $id) { alternateNames { id name language realName note } }
  }`,
  variables: () => ({ id }),
  command: "artistNames",
  args: () => ({ id }),
  fromGraphql: (artist) =>
    (artist as { alternateNames: unknown[] } | null)?.alternateNames ?? [],
})

const catalogTotals: Step = {
  name: "catalogTotals",
  graphql: `query { catalogTotals { releases artists tracks labels durationMs } }`,
  command: "catalogTotals",
}

const files: Step = {
  name: "files by path",
  graphql: `query {
    files(sort: "path", perPage: 200) {
      total items { id path type sizeBytes mtime sha256 track { id title } }
    }
  }`,
  command: "files",
  args: () => ({ sort: "path", perPage: 200 }),
}

const createRelease = (name: string, input: Args): Step => ({
  name: `createRelease ${name}`,
  graphql: `mutation ($input: ReleaseInput!) { createRelease(input: $input) { ${RELEASE} } }`,
  variables: () => ({ input }),
  command: "createRelease",
  args: () => input,
})

const addArtistName = (name: string, args: Args): Step => ({
  name: `addArtistName ${name}`,
  graphql: `mutation ($id: ID!, $name: String!, $language: String) {
    addArtistName(id: $id, name: $name, language: $language) { id name nameLanguage picture { id } }
  }`,
  variables: () => args,
  command: "addArtistName",
  args: () => args,
})

const setReleaseTracks = (
  name: string,
  id: number,
  tracks: Args[],
  discTitles?: Args[]
): Step => ({
  name: `setReleaseTracks ${id} ${name}`,
  graphql: `mutation ($id: ID!, $tracks: [ReleaseTrackInput!]!, $discTitles: [DiscTitleInput!]) {
    setReleaseTracks(id: $id, tracks: $tracks, discTitles: $discTitles) { ${RELEASE} }
  }`,
  variables: () =>
    discTitles === undefined ? { id, tracks } : { id, tracks, discTitles },
  command: "setReleaseTracks",
  args: () =>
    discTitles === undefined ? { id, tracks } : { id, tracks, discTitles },
})

const createTrack = (name: string, input: Args): Step => ({
  name: `createTrack ${name}`,
  graphql: `mutation ($input: TrackInput!) { createTrack(input: $input) { ${TRACK} } }`,
  variables: () => ({ input }),
  command: "createTrack",
  args: () => input,
  fromGraphql: savedTrack,
})

const updateTrack = (name: string, id: number, input: Args): Step => ({
  name: `updateTrack ${id} ${name}`,
  graphql: `mutation ($id: ID!, $input: TrackInput!) {
    updateTrack(id: $id, input: $input) { ${TRACK} }
  }`,
  variables: () => ({ id, input }),
  command: "updateTrack",
  args: () => ({ id, ...input }),
  fromGraphql: savedTrack,
})

const deleteTrack = (id: number): Step => ({
  name: `deleteTrack ${id}`,
  graphql: `mutation ($id: ID!) { deleteTrack(id: $id) }`,
  variables: () => ({ id }),
  command: "deleteTrack",
  args: () => ({ id }),
  returnsId: true,
})

const credited = (artistId: number, creditedAs?: string) =>
  creditedAs === undefined ? { artistId } : { artistId, creditedAs }

/** A row for a track the release has. */
const kept = (
  id: number,
  cdNumber: number,
  trackNumber: number,
  title: string
) => ({
  id,
  cdNumber,
  trackNumber,
  title,
})

// The release the import creates, its tracks, then the track createTrack makes
const IMPORTED = 7
const [IT_COULD, EARLY, MELLOW, LIVE] = [2, 3, 4, 5]
const AUTUMN = 6
const FOLDER = "music/Import"

// The import's rows, as its Tracks step saves them: titles split at " = ",
// and artists as the tags matched them
const IMPORTED_ROWS: Args[] = [
  {
    path: `${FOLDER}/01 It Could Happen to You.flac`,
    trackNumber: 1,
    title: "It Could Happen to You",
    alternateTitles: [
      { title: "イット・クッド・ハプン・トゥ・ユー", language: "ja" },
    ],
    artists: [credited(6, "alika")],
    features: [credited(12, "いとう・まさたか")],
  },
  {
    path: `./${FOLDER}//02 Early Summer.flac`,
    cdNumber: 1,
    trackNumber: 2,
    title: "Early Summer",
    titleLanguage: "EN",
    alternateTitles: [{ title: " 初夏 " }, { title: " " }],
    artists: [credited(8, "ryo fukui trio"), credited(8)],
  },
  {
    path: `${FOLDER}/CD2/03 Mellow Dream.flac`,
    cdNumber: 2,
    trackNumber: 3,
    title: "Mellow Dream",
  },
  { fileId: 14, cdNumber: 2, trackNumber: 4, title: " Mellow Dream (Live) " },
]

export const tracks: Phase = {
  prepare: [],
  steps: [
    // The track list and pages
    trackList("by default"),
    trackList("by title", { sort: "title" }),
    trackList("searched", { q: "SUMMER" }),
    trackList("searched by an alternate title", { q: "初" }),
    trackList("of a release", { release: 1 }),
    trackList("of a release that isn't there", { release: 999 }),
    trackList("by tags", {
      tags: [
        { type: "number", tagId: 1, min: "8" },
        { type: "text", tagId: 1, equals: "BEBOP" },
      ],
    }),
    trackList("by a tag filter that's wrong", {
      tags: [{ type: "date", tagId: 1, equals: "2024-13" }],
    }),
    trackList("by a column it can't", { sort: "duration" }),
    trackList("paged", { page: 2, perPage: 1 }),
    trackPage(1),
    trackPage(999),
    trackForm("of a track", { id: 1 }),
    trackForm("of a track that isn't there", { id: 999 }),
    trackForm("prefilled", { release: 5, file: 14 }),
    trackForm("prefilled with what isn't there", { release: 999, file: 999 }),
    trackForm("empty", {}),
    releaseTracks(1),
    releaseTracks(2),
    releaseTracks(999),
    releaseTracks(1, FOLDER),
    releaseTracks(999, FOLDER),
    releaseTracks(1, "nowhere"),

    // The import's suggestions
    releaseSuggestion(FOLDER),
    releaseSuggestion("music/Ryo Fukui"),
    releaseSuggestion("./music//Alika/"),
    releaseSuggestion("music/Import/CD2"),
    releaseSuggestion(""),
    releaseSuggestion("nowhere"),
    importFiles(5, FOLDER),
    importFiles(1, "music/Ryo Fukui"),
    importFiles(999, FOLDER),
    importFiles(5, "nowhere"),

    // Importing: the release, a tag's other name, the tracks
    createRelease("from the folder, its title split", {
      title: "Scenery",
      alternateTitles: [{ title: "シーナリィ", language: "ja" }],
      released: "1976",
      artists: [credited(7)],
    }),
    addArtistName("from a tag's other part", {
      id: 7,
      name: " Fukui Ryo ",
      language: null,
    }),
    addArtistName("in Japanese", {
      id: 8,
      name: "ふくいりょうトリオ",
      language: "JA",
    }),
    addArtistName("it has already", { id: 7, name: "Fukui Ryo" }),
    addArtistName("that's its main name", { id: 7, name: "Ryo Fukui" }),
    addArtistName("that's blank", { id: 7, name: " " }),
    addArtistName("with und", { id: 7, name: "Ryo", language: "und" }),
    addArtistName("of an artist that isn't there", { id: 999, name: "X" }),
    artistNames(7),
    artistNames(8),
    releaseTracks(IMPORTED, FOLDER),
    setReleaseTracks("from the folder", IMPORTED, IMPORTED_ROWS, [
      {
        cdNumber: 1,
        title: " Side A ",
        alternateTitles: [{ title: "A面", language: "ja" }],
      },
      { cdNumber: 2, title: " " },
    ]),
    releaseTracks(IMPORTED),
    releaseTracks(IMPORTED, FOLDER),
    artistNames(6),
    artistNames(8),
    files,

    // Saves that are refused, and save nothing
    setReleaseTracks("of a release that isn't there", 999, []),
    setReleaseTracks("with another release's track", IMPORTED, [
      kept(1, 1, 1, "Early Summer"),
    ]),
    setReleaseTracks("with an artist that performs and is featured", IMPORTED, [
      {
        ...kept(IT_COULD, 1, 1, "It"),
        artists: [credited(6)],
        features: [credited(6)],
      },
    ]),
    setReleaseTracks("with an artist that isn't there", IMPORTED, [
      { ...kept(IT_COULD, 1, 1, "It"), artists: [credited(999)] },
    ]),
    setReleaseTracks("with a number twice", IMPORTED, [
      kept(IT_COULD, 1, 1, "It"),
      kept(EARLY, 1, 1, "Early"),
    ]),
    setReleaseTracks("with another track's audio", IMPORTED, [
      { fileId: 1, trackNumber: 9, title: "Again" },
    ]),
    setReleaseTracks("with an image", IMPORTED, [
      { fileId: 2, trackNumber: 9, title: "Cover" },
    ]),
    setReleaseTracks("with a text file by path", IMPORTED, [
      { path: `${FOLDER}/notes.txt`, trackNumber: 9, title: "Notes" },
    ]),
    setReleaseTracks("with a missing file", IMPORTED, [
      { path: `${FOLDER}/gone.flac`, trackNumber: 9, title: "Gone" },
    ]),
    setReleaseTracks("with a file that isn't registered", IMPORTED, [
      { fileId: 999, trackNumber: 9, title: "Nothing" },
    ]),
    setReleaseTracks("with no file", IMPORTED, [
      { trackNumber: 9, title: "No file" },
    ]),
    setReleaseTracks("with disc 0", IMPORTED, [kept(IT_COULD, 0, 1, "It")]),
    setReleaseTracks("with und", IMPORTED, [
      { ...kept(IT_COULD, 1, 1, "It"), titleLanguage: "und" },
    ]),
    setReleaseTracks("with a blank title", IMPORTED, [
      kept(IT_COULD, 1, 1, " "),
    ]),
    setReleaseTracks("with its title as an alternate", IMPORTED, [
      { ...kept(IT_COULD, 1, 1, "It"), alternateTitles: [{ title: "It" }] },
    ]),
    setReleaseTracks(
      "with two titles for a disc",
      IMPORTED,
      [],
      [
        { cdNumber: 1, title: "A" },
        { cdNumber: 1, title: "B" },
      ]
    ),
    setReleaseTracks(
      "with a disc's alternate titles but no title",
      IMPORTED,
      [],
      [{ cdNumber: 1, title: "", alternateTitles: [{ title: "A面" }] }]
    ),
    setReleaseTracks(
      "with a disc title in a bad language",
      IMPORTED,
      [],
      [{ cdNumber: 1, title: "A", language: "ja_JP" }]
    ),
    setReleaseTracks(
      "with a title for a disc with no tracks",
      IMPORTED,
      [
        kept(IT_COULD, 1, 1, "It"),
        kept(EARLY, 1, 2, "Early"),
        kept(MELLOW, 2, 3, "Mellow"),
        kept(LIVE, 2, 4, "Live"),
      ],
      [{ cdNumber: 3, title: "Bonus" }]
    ),
    releaseTracks(IMPORTED),

    // Renumbering, leaving one out, and moving a disc's last track away
    setReleaseTracks(
      "swapped, with one left out",
      IMPORTED,
      [
        kept(EARLY, 1, 1, "Early Summer"),
        {
          ...kept(IT_COULD, 1, 2, "It Could Happen to You"),
          titleLanguage: null,
        },
        { ...kept(MELLOW, 2, 1, "Mellow Dream"), alternateTitles: [] },
      ],
      [
        { cdNumber: 1, title: "Side A" },
        { cdNumber: 2, title: "Side B", language: "EN" },
      ]
    ),
    releaseTracks(IMPORTED),
    setReleaseTracks(
      "with disc 2's last track moved, titles left out",
      IMPORTED,
      [
        kept(EARLY, 1, 1, "初夏"),
        kept(IT_COULD, 1, 2, "It Could Happen to You"),
        kept(MELLOW, 1, 3, "Mellow Dream"),
      ]
    ),
    releaseTracks(IMPORTED),

    // A track on its own
    createTrack("with everything", {
      releaseId: IMPORTED,
      fileId: 6,
      trackNumber: 4,
      title: " Autumn Leaves ",
      titleLanguage: "EN",
      alternateTitles: [
        { title: "Les Feuilles mortes", language: "fr", printed: false },
      ],
      artists: [credited(11, "J. Kosma")],
      features: [credited(13, "Teruo")],
      credits: [
        { role: "Composed By", artists: [credited(11)] },
        {
          role: " Lyrics By ",
          artists: [credited(9, "J. V. Heusen"), credited(9)],
        },
      ],
      files: [{ path: `${FOLDER}/notes.txt`, note: " Lyrics " }, { fileId: 7 }],
      tags: [
        { type: "text", tagId: 1, values: ["Jazz", "jazz", " "] },
        { type: "number", tagId: 1, values: ["8.50"] },
      ],
    }),
    trackPage(AUTUMN),
    trackForm("of the new track", { id: AUTUMN }),
    createTrack("from a file that isn't there", {
      releaseId: IMPORTED,
      fileId: 999,
      trackNumber: 5,
      title: "X",
    }),
    createTrack("from an image", {
      releaseId: IMPORTED,
      fileId: 2,
      trackNumber: 5,
      title: "X",
    }),
    createTrack("from another track's audio", {
      releaseId: IMPORTED,
      fileId: 6,
      trackNumber: 5,
      title: "X",
    }),
    createTrack("with a number that's taken", {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 1,
      title: "X",
    }),
    createTrack("on a release that isn't there", {
      releaseId: 999,
      fileId: 8,
      trackNumber: 1,
      title: "X",
    }),
    createTrack("with a bad language", {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 5,
      title: "X",
      titleLanguage: "japanese",
    }),
    createTrack("with an artist that performs and is featured", {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 5,
      title: "X",
      artists: [credited(11)],
      features: [credited(11)],
    }),
    createTrack("with two values for a single-value tag", {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 5,
      title: "X",
      tags: [{ type: "number", tagId: 1, values: ["1", "2"] }],
    }),
    createTrack("with a credit with no role", {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 5,
      title: "X",
      credits: [{ role: " ", artists: [credited(11)] }],
    }),
    createTrack("with a link to a file that isn't there", {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 5,
      title: "X",
      files: [{ path: `${FOLDER}/gone.txt` }],
    }),
    updateTrack("renamed to its alternate title, the rest left out", AUTUMN, {
      releaseId: IMPORTED,
      fileId: 6,
      trackNumber: 4,
      title: "Les Feuilles mortes",
    }),
    trackPage(AUTUMN),
    updateTrack("moved, with a new file and its tags cleared", AUTUMN, {
      releaseId: 5,
      fileId: 8,
      cdNumber: 2,
      trackNumber: 9,
      title: "Autumn Leaves",
      titleLanguage: null,
      alternateTitles: [],
      artists: [credited(7)],
      tags: [],
    }),
    trackPage(AUTUMN),
    updateTrack("that isn't there", 999, {
      releaseId: IMPORTED,
      fileId: 8,
      trackNumber: 1,
      title: "X",
    }),
    updateTrack("to an image", AUTUMN, {
      releaseId: 5,
      fileId: 2,
      trackNumber: 9,
      title: "X",
    }),
    trackList("of the release it moved to", { release: 5 }),
    releasePage(5),
    releasePage(IMPORTED),
    catalogTotals,

    // Deleting
    deleteTrack(AUTUMN),
    deleteTrack(999),
    trackPage(AUTUMN),
    releaseTracks(IMPORTED, FOLDER),
    trackList("at the end", { sort: "title" }),
    files,
    catalogTotals,
  ],
}
