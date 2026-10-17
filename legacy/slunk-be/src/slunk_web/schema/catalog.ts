import type { Catalog, FileLinkInput, FileRef } from "../../slunk/catalog.ts"
import type { ListKind } from "../../slunk/catalog/repository.ts"
import { loadAll, loadRequired } from "./loaders.ts"
import type { Resolvers } from "./resolvers_types.ts"
import { tagFilters, tagValues } from "./tagging.ts"

const LIST_TYPES = {
  release_formats: "ReleaseFormat",
  release_media: "ReleaseMedium",
} as const

/** A GraphQL id, as the database's number. */
const n = (id: string | number) => Number(id)
const ids = (list: readonly (string | number)[]) => list.map(n)
const optional = (id: string | number | null | undefined) =>
  id == null ? null : n(id)

const fileRef = (ref: {
  fileId?: string | null
  path?: string | null
}): FileRef => ({
  fileId: optional(ref.fileId),
  path: ref.path,
})
const fileLinks = (
  links: {
    fileId?: string | null
    path?: string | null
    note?: string | null
  }[]
): FileLinkInput[] =>
  links.map((link) => ({ ...fileRef(link), note: link.note }))
/** Artists as credited, with their ids as numbers. */
const credited = (list: { artistId: string; creditedAs?: string | null }[]) =>
  list.map((link) => ({
    artistId: n(link.artistId),
    creditedAs: link.creditedAs,
  }))
const credits = (
  list: {
    role: string
    artists: { artistId: string; creditedAs?: string | null }[]
  }[]
) => list.map((c) => ({ role: c.role, artists: credited(c.artists) }))

/** `uses` and `blockers` for either list type. */
const listItemFields = {
  uses: (
    item: { kind: ListKind; id: number },
    _args: unknown,
    { catalog }: { catalog: Catalog }
  ) => catalog.listItemUses(item.kind, item.id),
  blockers: (
    item: { kind: ListKind; id: number },
    _args: unknown,
    { catalog }: { catalog: Catalog }
  ) => catalog.listItemBlockers(item.kind, item.id),
}

export const catalogResolvers: Resolvers = {
  Query: {
    artists: (_parent, args, { catalog }) => catalog.listArtists(args),
    artist: (_parent, { id }, { catalog }) => catalog.getArtist(n(id)),
    labels: (_parent, args, { catalog }) => catalog.listLabels(args),
    label: (_parent, { id }, { catalog }) => catalog.getLabel(n(id)),
    releases: (_parent, args, { catalog, tagging }) =>
      catalog.listReleases({
        ...args,
        format: optional(args.format),
        medium: optional(args.medium),
        label: optional(args.label),
        artist: optional(args.artist),
        tags: tagFilters(tagging, args.tags),
      }),
    release: (_parent, { id }, { catalog }) => catalog.getRelease(n(id)),
    releaseIds: async (_parent, args, { catalog, tagging }) =>
      catalog.releaseIds({
        ...args,
        format: optional(args.format),
        medium: optional(args.medium),
        label: optional(args.label),
        artist: optional(args.artist),
        tags: tagFilters(tagging, args.tags),
      }),
    releasesTitled: (_parent, { title }, { catalog }) =>
      catalog.releasesTitled(title),
    tracks: (_parent, args, { catalog, tagging }) =>
      catalog.listTracks({
        ...args,
        release: optional(args.release),
        tags: tagFilters(tagging, args.tags),
      }),
    track: (_parent, { id }, { catalog }) => catalog.getTrack(n(id)),
    listItems: (_parent, { kind, ...args }, { catalog }) =>
      catalog.listItems(kind, args),
    listItem: (_parent, { kind, id }, { catalog }) =>
      catalog.getListItem(kind, n(id)),
    creditRoles: (_parent, { q }, { catalog }) => catalog.creditRoles(q),
    catalogTotals: (_parent, _args, { catalog }) => catalog.totals(),
    languages: (_parent, _args, { catalog }) => catalog.languages(),
    releaseSuggestion: (_parent, { folder }, { catalog }) =>
      catalog.suggestRelease(folder),
    trackSuggestions: (_parent, { folder }, { catalog }) =>
      catalog.suggestTracks(folder),
  },

  Mutation: {
    createArtist: (_parent, { input }, { catalog }) =>
      catalog.createArtist(artistInput(input)),
    updateArtist: (_parent, { id, input }, { catalog }) =>
      catalog.updateArtist(n(id), artistInput(input)),
    setArtistPicture: (_parent, { id, picture }, { catalog }) =>
      catalog.setArtistPicture(n(id), fileRef(picture)),
    addArtistName: (_parent, { id, ...name }, { catalog }) =>
      catalog.addArtistName(n(id), name),
    deleteArtist: async (_parent, { id }, { catalog }) => {
      await catalog.deleteArtist(n(id))
      return id
    },
    createLabel: (_parent, { input }, { catalog }) =>
      catalog.createLabel(labelInput(input)),
    updateLabel: (_parent, { id, input }, { catalog }) =>
      catalog.updateLabel(n(id), labelInput(input)),
    setLabelPicture: (_parent, { id, picture }, { catalog }) =>
      catalog.setLabelPicture(n(id), fileRef(picture)),
    deleteLabel: async (_parent, { id }, { catalog }) => {
      await catalog.deleteLabel(n(id))
      return id
    },
    createRelease: (_parent, { input }, { catalog }) =>
      catalog.createRelease(releaseInput(input)),
    updateRelease: (_parent, { id, input }, { catalog }) =>
      catalog.updateRelease(n(id), releaseInput(input)),
    deleteRelease: async (_parent, { id }, { catalog }) => {
      await catalog.deleteRelease(n(id))
      return id
    },
    setReleaseTracks: (_parent, { id, tracks, discTitles }, { catalog }) =>
      catalog.setReleaseTracks(
        n(id),
        tracks.map((row) => ({
          ...fileRef(row),
          id: optional(row.id),
          cdNumber: row.cdNumber,
          trackNumber: row.trackNumber,
          title: row.title,
          // Left out (undefined), they stay as they are
          titleLanguage: row.titleLanguage,
          alternateTitles: row.alternateTitles,
          artists: credited(row.artists),
          features: credited(row.features),
        })),
        discTitles
      ),
    setReleaseFiles: (_parent, { id, input }, { catalog }) =>
      catalog.setReleaseFiles(n(id), {
        cover: input.cover ? fileRef(input.cover) : null,
        files: fileLinks(input.files),
      }),
    setReleaseCover: (_parent, { id, cover }, { catalog }) =>
      catalog.setReleaseCover(n(id), fileRef(cover)),
    setReleaseCredits: (_parent, { id, input }, { catalog }) =>
      catalog.setReleaseCredits(n(id), {
        credits: credits(input.credits),
        tracks: input.tracks.map((track) => ({
          trackId: n(track.trackId),
          credits: credits(track.credits),
        })),
      }),
    createTrack: (_parent, { input }, { catalog }) =>
      catalog.createTrack(trackInput(input)),
    updateTrack: (_parent, { id, input }, { catalog }) =>
      catalog.updateTrack(n(id), trackInput(input)),
    deleteTrack: async (_parent, { id }, { catalog }) => {
      await catalog.deleteTrack(n(id))
      return id
    },
    createListItem: (_parent, { kind, name }, { catalog }) =>
      catalog.createListItem(kind, name),
    renameListItem: (_parent, { kind, id, name }, { catalog }) =>
      catalog.renameListItem(kind, n(id), name),
    deleteListItem: async (_parent, { kind, id }, { catalog }) => {
      await catalog.deleteListItem(kind, n(id))
      return id
    },
  },

  Artist: {
    alternateNames: (artist, _args, { loaders }) =>
      loaders.artistNames.load(artist.id),
    mainNameUses: (artist, _args, { catalog }) =>
      catalog.mainNameUses(artist.id),
    picture: (artist, _args, { loaders }) =>
      artist.picture === null ? null : loaders.file.load(artist.picture),
    files: (artist, _args, { loaders }) => loaders.artistFiles.load(artist.id),
    blockers: (artist, _args, { catalog }) => catalog.artistBlockers(artist.id),
    appearances: (artist, _args, { catalog }) =>
      catalog.artistAppearances(artist.id),
    credits: (artist, _args, { catalog }) => catalog.artistCredits(artist.id),
    formatCounts: (artist, _args, { catalog }) =>
      catalog.artistFormatCounts(artist.id),
    counts: async (artist, _args, { catalog, loaders }) => ({
      releases: (await catalog.listReleases({ artist: artist.id, perPage: 1 }))
        .total,
      appearances: (await catalog.artistAppearances(artist.id)).length,
      credits: (await catalog.artistCredits(artist.id)).length,
      files:
        (await loaders.artistFiles.load(artist.id)).length +
        (artist.picture === null ? 0 : 1),
    }),
  },

  Appearance: {
    release: (appearance, _args, { loaders }) =>
      loadRequired(loaders.release, appearance.releaseId),
    tracks: (appearance, _args, { loaders }) =>
      loadAll(loaders.track, appearance.trackIds),
  },

  ArtistCredit: {
    creditedAs: (credit, _args, { loaders }) =>
      credit.creditedNameId === null
        ? null
        : loaders.artistName.load(credit.creditedNameId),
    release: (credit, _args, { loaders }) =>
      loadRequired(loaders.release, credit.releaseId),
    track: (credit, _args, { loaders }) =>
      credit.trackId === null ? null : loaders.track.load(credit.trackId),
  },

  FormatCount: {
    format: (count, _args, { loaders }) =>
      loadRequired(loaders.format, count.formatId),
  },

  Label: {
    alternateNames: (label, _args, { loaders }) =>
      loaders.labelNames.load(label.id),
    picture: (label, _args, { loaders }) =>
      label.picture === null ? null : loaders.file.load(label.picture),
    releaseCount: (label, _args, { catalog }) =>
      catalog.labelReleaseCount(label.id),
    blockers: (label, _args, { catalog }) => catalog.labelBlockers(label.id),
  },

  Release: {
    alternateTitles: (release, _args, { loaders }) =>
      loaders.releaseTitles.load(release.id),
    released: (release, _args, { catalog }) => catalog.released(release),
    cover: (release, _args, { loaders }) =>
      release.coverPicture === null
        ? null
        : loaders.file.load(release.coverPicture),
    artists: (release, _args, { loaders }) =>
      loaders.releaseArtists.load(release.id),
    labels: async (release, _args, { loaders }) =>
      loadAll(loaders.label, await loaders.releaseLabels.load(release.id)),
    formats: async (release, _args, { loaders }) =>
      loadAll(loaders.format, await loaders.releaseFormats.load(release.id)),
    media: async (release, _args, { loaders }) =>
      loadAll(loaders.medium, await loaders.releaseMedia.load(release.id)),
    tracks: (release, _args, { loaders }) =>
      loaders.releaseTracks.load(release.id),
    discTitles: (release, _args, { loaders }) =>
      loaders.releaseDiscTitles.load(release.id),
    credits: (release, _args, { loaders }) =>
      loaders.releaseCredits.load(release.id),
    files: (release, _args, { loaders }) =>
      loaders.releaseFiles.load(release.id),
    fileCount: async (release, _args, { loaders }) => {
      const links = await loaders.releaseFiles.load(release.id)
      const files = new Set(links.map((link) => link.fileId))
      if (release.coverPicture !== null) files.add(release.coverPicture)
      return files.size
    },
    durationMs: async (release, _args, { loaders }) => {
      const durations = (await loaders.releaseTracks.load(release.id)).flatMap(
        (track) => (track.durationMs === null ? [] : [track.durationMs])
      )
      return durations.length ? durations.reduce((a, b) => a + b, 0) : null
    },
    blockers: (release, _args, { catalog }) =>
      catalog.releaseBlockers(release.id),
  },

  Track: {
    alternateTitles: (track, _args, { loaders }) =>
      loaders.trackTitles.load(track.id),
    release: (track, _args, { loaders }) =>
      loadRequired(loaders.release, track.releaseId),
    file: (track, _args, { loaders }) =>
      loadRequired(loaders.file, track.fileId),
    artists: async (track, _args, { loaders }) =>
      (await loaders.trackArtists.load(track.id)).filter((a) => !a.featured),
    features: async (track, _args, { loaders }) =>
      (await loaders.trackArtists.load(track.id)).filter((a) => a.featured),
    performers: async (track, _args, { catalog, loaders }) => {
      const [release, own, releaseArtists] = await Promise.all([
        loadRequired(loaders.release, track.releaseId),
        loaders.trackArtists.load(track.id),
        loaders.releaseArtists.load(track.releaseId),
      ])
      return catalog.performers(release, own, releaseArtists)
    },
    credits: (track, _args, { loaders }) => loaders.trackCredits.load(track.id),
    files: (track, _args, { loaders }) => loaders.trackFiles.load(track.id),
  },

  CreditedArtist: {
    artist: (link, _args, { loaders }) =>
      loadRequired(loaders.artist, link.artistId),
    creditedAs: (link, _args, { loaders }) =>
      link.creditedNameId === null
        ? null
        : loaders.artistName.load(link.creditedNameId),
  },

  ArtistName: {
    uses: (name, _args, { loaders }) => loaders.artistNameUses.load(name.id),
  },

  ListItem: {
    __resolveType: (item) => LIST_TYPES[item.kind],
  },
  ReleaseFormat: listItemFields,
  ReleaseMedium: listItemFields,

  FileLink: {
    file: (link, _args, { loaders }) => loadRequired(loaders.file, link.fileId),
  },
}

type NameInput = {
  id?: string | null
  name: string
  language?: string | null
  realName?: boolean
  note?: string | null
}

function artistInput(input: {
  name: string
  nameLanguage?: string | null
  pictureId?: string | null
  files: Parameters<typeof fileLinks>[0]
  alternateNames?: NameInput[] | null
  keepOldName: boolean
}) {
  return {
    ...labelInput(input),
    files: fileLinks(input.files),
    keepOldName: input.keepOldName,
  }
}

function labelInput(input: {
  name: string
  nameLanguage?: string | null
  pictureId?: string | null
  alternateNames?: NameInput[] | null
}) {
  return {
    name: input.name,
    nameLanguage: input.nameLanguage,
    pictureId: optional(input.pictureId),
    alternateNames: input.alternateNames?.map((name) => ({
      ...name,
      id: optional(name.id),
    })),
  }
}

/** An input's title language and alternate titles. */
type TitlesInput = {
  titleLanguage?: string | null
  alternateTitles?:
    { title: string; language?: string | null; printed: boolean }[] | null
}

function releaseInput(
  input: TitlesInput & {
    title: string
    released?: string | null
    catalogNumber?: string | null
    variousArtists: boolean
    artists: { artistId: string; creditedAs?: string | null }[]
    labelIds: string[]
    formatIds: string[]
    mediumIds: string[]
  }
) {
  return {
    title: input.title,
    titleLanguage: input.titleLanguage,
    alternateTitles: input.alternateTitles,
    released: input.released,
    catalogNumber: input.catalogNumber,
    variousArtists: input.variousArtists,
    artists: credited(input.artists),
    labelIds: ids(input.labelIds),
    formatIds: ids(input.formatIds),
    mediumIds: ids(input.mediumIds),
  }
}

function trackInput(
  input: TitlesInput & {
    releaseId: string
    fileId: string
    cdNumber: number
    trackNumber: number
    title: string
    artists: { artistId: string; creditedAs?: string | null }[]
    features: { artistId: string; creditedAs?: string | null }[]
    credits: Parameters<typeof credits>[0]
    files: {
      fileId?: string | null
      path?: string | null
      note?: string | null
    }[]
    tags?: Parameters<typeof tagValues>[0] | null
  }
) {
  return {
    releaseId: n(input.releaseId),
    fileId: n(input.fileId),
    cdNumber: input.cdNumber,
    trackNumber: input.trackNumber,
    title: input.title,
    titleLanguage: input.titleLanguage,
    alternateTitles: input.alternateTitles,
    artists: credited(input.artists),
    features: credited(input.features),
    credits: credits(input.credits),
    files: fileLinks(input.files),
    tags: input.tags ? tagValues(input.tags) : null,
  }
}
