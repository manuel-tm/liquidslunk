// The catalog context: artists, labels, releases with their formats, media
// and labels, tracks, and credits (design §6).
import { basename, dirname } from "node:path"
import type {
  AlternateName,
  AlternateNameValues,
  AlternateTitle,
  Artist,
  ArtistLink,
  CatalogStore,
  CreditOwner,
  DiscTitle,
  Label,
  ListKind,
  Release,
  ReleaseLink,
  TrackArtist,
} from "./catalog/repository.ts"
import { formatDate, type ParsedDate, parseDate } from "./dates.ts"
import { languageTag, scriptLanguage } from "./languages.ts"
import type { Media } from "./media.ts"
import type { FileRecord, LinkOwner } from "./media/repository.ts"
import { type ListArgs, pagingOf, searchOf, sortOf } from "./paging.ts"
import type { Tagging, TagValuesInput } from "./tagging.ts"
import type { TagFilter } from "./tagging/repository.ts"
import { cleanOrNull, cleanText } from "./text.ts"

export type Catalog = ReturnType<typeof createCatalog>

/** A file by id, or by a path to register first. */
export type FileRef = { fileId?: number | null; path?: string | null }

export type FileLinkInput = FileRef & { note?: string | null }

/**
 * An artist as credited (3_alternate_names.md §6): `creditedAs` is the name
 * printed. Blank, or the artist's main name, means the main name.
 */
export type CreditedArtistInput = {
  artistId: number
  creditedAs?: string | null
}

/** A credit line: a role as printed, and its artists in order. */
export type CreditInput = { role: string; artists: CreditedArtistInput[] }

/** An alternate name (3_alternate_names.md §6). With an id, one the owner has. */
export type AlternateNameInput = {
  id?: number | null
  name: string
  /** A BCP 47 tag. Blank means unknown. */
  language?: string | null
  /** Artists only */
  realName?: boolean | null
  note?: string | null
}

export type ArtistInput = {
  name: string
  nameLanguage?: string | null
  pictureId?: number | null
  files: FileLinkInput[]
  /** Replaces the alternate names. Left out, they stay as they are. */
  alternateNames?: AlternateNameInput[] | null
  /**
   * On a rename, credits the links that showed the old main name with it, so
   * they keep showing it
   */
  keepOldName?: boolean | null
}

export type LabelInput = {
  name: string
  nameLanguage?: string | null
  pictureId?: number | null
  /** Replaces the alternate names. Left out, they stay as they are. */
  alternateNames?: AlternateNameInput[] | null
}

/** Artists and labels both have alternate names. */
type NamedKind = "artist" | "label"

/** An alternate title (3_alternate_names.md §6). */
export type AlternateTitleInput = {
  title: string
  /** A BCP 47 tag. Blank means unknown. */
  language?: string | null
  /** On the release as published. True when left out. */
  printed?: boolean | null
}

/**
 * A main title's language and alternate titles, on a release, a track or a
 * disc. Left out (undefined), each stays as it is.
 */
type TitlesInput = {
  titleLanguage?: string | null
  alternateTitles?: AlternateTitleInput[] | null
}

/** A disc's own title, its language, and its alternate titles. */
export type DiscTitleInput = {
  cdNumber: number
  title: string
  language?: string | null
  alternateTitles?: AlternateTitleInput[] | null
}

export type ReleaseInput = TitlesInput & {
  title: string
  /** `2006`, `2006-01` or `2006-01-10` */
  released?: string | null
  catalogNumber?: string | null
  variousArtists: boolean
  artists: CreditedArtistInput[]
  labelIds: number[]
  formatIds: number[]
  mediumIds: number[]
}

/** A row of a release's tracks: an existing track, or a new one from a file. */
export type ReleaseTrackInput = FileRef &
  TitlesInput & {
    id?: number | null
    cdNumber: number
    trackNumber: number
    title: string
    artists: CreditedArtistInput[]
    features: CreditedArtistInput[]
  }

/** The artist a tag suggests, for the import (3_alternate_names.md §7.7). */
export type ArtistSuggestion = {
  /** The name matched: the tag, or its first part when it has " = " */
  name: string
  /** The artist, when exactly one matches */
  artist: Artist | null
  /** The name to credit it with, when that isn't the main name */
  creditedAs: string | null
  /** How many artists matched equally well: more than one picks none */
  matches: number
  /** The tag's other parts that the artist doesn't have as names yet */
  otherNames: string[]
}

export type TrackInput = TitlesInput & {
  releaseId: number
  fileId: number
  cdNumber: number
  trackNumber: number
  title: string
  artists: CreditedArtistInput[]
  features: CreditedArtistInput[]
  credits: CreditInput[]
  files: FileLinkInput[]
  /** Replaces the track's tags. Left out, they stay as they are. */
  tags?: TagValuesInput[] | null
}

const LIST_NAMES: Record<ListKind, string> = {
  release_formats: "format",
  release_media: "medium",
}

// How many roles the credits editor suggests
const ROLE_SUGGESTIONS = 20

// Moves track numbers clear of the ones being saved, so renumbering a
// release never trips its unique (release, disc, number) index midway
const RENUMBER_OFFSET = 1_000_000

export function createCatalog({
  store,
  media,
  tagging,
}: {
  store: CatalogStore
  media: Media
  tagging: Tagging
}) {
  /** Registered files for refs, registering paths first. Reads outside the transaction. */
  async function prepareRefs(refs: FileRef[]) {
    const paths = refs.flatMap((ref) =>
      ref.fileId == null && ref.path ? [ref.path] : []
    )
    return media.prepareFiles(paths)
  }

  /** The file id of a ref, after `prepared` has been inserted in `tx`. */
  function refId(ref: FileRef, registered: Map<string, FileRecord>): number {
    if (ref.fileId != null) return ref.fileId
    if (!ref.path) throw new Error("A file needs an id or a path")
    const file = registered.get(media.normalizePath(ref.path))
    if (!file) throw new Error(`There's no file at ${ref.path}`)
    return file.id
  }

  async function setFileLinks(
    tx: CatalogStore,
    owner: LinkOwner,
    id: number,
    links: FileLinkInput[],
    registered: Map<string, FileRecord>
  ) {
    await tx.fileLinks.set(
      owner,
      id,
      links.map((link) => ({
        fileId: refId(link, registered),
        note: cleanOrNull(link.note),
      }))
    )
  }

  /**
   * Sets a cover, picture or logo from a ref, registering a path in the same
   * transaction: a file that can't be used isn't left registered.
   */
  async function setImage<T>(
    what: string,
    id: number,
    ref: FileRef,
    update: (tx: CatalogStore, fileId: number) => Promise<T | null>
  ): Promise<T> {
    const prepared = await prepareRefs([ref])
    return store.transaction(async (tx) => {
      const registered = await media.insertPrepared(tx, prepared)
      const updated = await update(tx, refId(ref, registered))
      if (!updated) throw notFound(what, id)
      return updated
    })
  }

  /** A track's artists and features, as credited, to resolve. */
  function trackArtists(
    artists: CreditedArtistInput[],
    features: CreditedArtistInput[]
  ) {
    const featured = new Set(features.map((link) => link.artistId))
    if (artists.some((link) => featured.has(link.artistId))) {
      throw new Error(
        "An artist can't both perform a track and be featured on it"
      )
    }
    return [
      ...firstOfEach(artists).map((link) => ({ ...link, featured: false })),
      ...firstOfEach(features).map((link) => ({ ...link, featured: true })),
    ]
  }

  /**
   * Resolves the names artists are credited with (3_alternate_names.md §6),
   * in one transaction: blank or the main name is the main name, one of the
   * artist's alternate names is that name, and anything else is added to
   * them. Names compare exactly, case included. A name it adds is
   * remembered, so two links can be credited with it.
   */
  function creditedNames(tx: CatalogStore) {
    const known = new Map<number, { artist: Artist; names: AlternateName[] }>()

    async function load(ids: number[]) {
      const missing = [...new Set(ids)].filter((id) => !known.has(id))
      if (missing.length === 0) return
      const artists = await tx.artists.getMany(missing)
      const names = await tx.artists.alternateNamesOf(missing)
      for (const artist of artists) {
        known.set(artist.id, { artist, names: names.get(artist.id) ?? [] })
      }
      const absent = missing.find((id) => !known.has(id))
      if (absent !== undefined) throw notFound("artist", absent)
    }

    return async function resolve<T extends CreditedArtistInput>(
      links: T[]
    ): Promise<(Omit<T, "creditedAs"> & ArtistLink)[]> {
      await load(links.map((link) => link.artistId))
      const resolved = []
      for (const { creditedAs, ...link } of links) {
        const { artist, names } = known.get(link.artistId)!
        const printed = cleanOrNull(creditedAs)
        let creditedNameId: number | null = null
        if (printed !== null && printed !== artist.name) {
          let name = names.find((n) => n.name === printed)
          if (!name) {
            name = await tx.artists.addAlternateName(artist.id, {
              name: printed,
              language: scriptLanguage(printed),
              realName: false,
              note: null,
            })
            names.push(name)
          }
          creditedNameId = name.id
        }
        resolved.push({ ...link, creditedNameId })
      }
      return resolved
    }
  }

  /**
   * The artist each tag names (3_alternate_names.md §7.7), matching in turn:
   * the main name exactly, an alternate name exactly, credited with it, or
   * either one ignoring case, credited as tagged, so "ALIKA" is still
   * printed. The first step that finds anyone decides, and finding more than
   * one artist picks none. A tag with " = " is matched by its first part, and
   * its other parts are offered as names of the artist it matched.
   */
  async function suggestArtists(tags: string[]): Promise<ArtistSuggestion[]> {
    const parsed = tags.map((tag) => {
      const [name, ...others] = equalsParts(cleanText(tag))
      return { name: name ?? "", others }
    })
    const found = await store.artists.findByNames(
      parsed.flatMap(({ name }) => (name ? [name] : []))
    )
    const names = await store.artists.alternateNamesOf([
      ...new Set(found.map((match) => match.item.id)),
    ])
    return parsed.map(({ name, others }) => {
      const matches = found.filter(
        (match) =>
          nocase(match.alternate?.name ?? match.item.name) === nocase(name)
      )
      const steps = [
        matches.filter((m) => !m.alternate && m.item.name === name),
        matches.filter((m) => m.alternate?.name === name),
        matches,
      ]
      const step = steps.findIndex((step) => step.length > 0)
      const artists = new Map(
        (steps[step] ?? []).map((match) => [match.item.id, match.item])
      )
      if (artists.size !== 1) {
        return {
          name,
          artist: null,
          creditedAs: null,
          matches: artists.size,
          otherNames: [],
        }
      }
      const [artist] = artists.values()
      const known = new Set([
        artist.name,
        name,
        ...(names.get(artist.id) ?? []).map((known) => known.name),
      ])
      return {
        name,
        artist,
        // Only a step-1 match shows the main name
        creditedAs: step === 0 ? null : name,
        matches: 1,
        otherNames: others.filter((other) => !known.has(other)),
      }
    })
  }

  /**
   * Releases with any of these titles, main or alternate, ignoring case: a
   * folder tagged "Scenery = シーナリィ" finds "Scenery".
   */
  async function releasesTitledAny(titles: string[]) {
    const found = new Map<number, Release>()
    for (const title of new Set(titles.filter(Boolean))) {
      for (const release of await store.releases.findByTitle(title)) {
        found.set(release.id, release)
      }
    }
    return [...found.values()]
  }

  /** Credit lines with their artists' credited names resolved. */
  async function resolvedLines(
    credited: ReturnType<typeof creditedNames>,
    credits: CreditInput[]
  ) {
    const lines = []
    for (const { role, artists } of creditLines(credits)) {
      lines.push({ role, artists: await credited(artists) })
    }
    return lines
  }

  function releaseValues(input: ReleaseInput) {
    const released = input.released?.trim()
    return {
      title: cleanText(input.title),
      titleLanguage: languageTag(input.titleLanguage),
      ...(released
        ? releaseDate(parseDate(released))
        : { releaseDate: null, releaseDatePrecision: null }),
      catalogNumber: cleanOrNull(input.catalogNumber),
      variousArtists: input.variousArtists,
    }
  }

  async function setReleaseLinks(
    tx: CatalogStore,
    id: number,
    input: ReleaseInput
  ) {
    const credited = creditedNames(tx)
    await tx.releases.setArtists(id, await credited(firstOfEach(input.artists)))
    await tx.releases.setLinks("labels", id, unique(input.labelIds))
    await tx.releases.setLinks("formats", id, unique(input.formatIds))
    await tx.releases.setLinks("media", id, unique(input.mediumIds))
  }

  async function durationOf(fileId: number) {
    const file = await store.files.get(fileId)
    if (!file) throw new Error(`There's no file ${fileId}`)
    if (file.type !== "music")
      throw new Error(`${file.path} isn't a music file`)
    return (await media.readAudio(file)).durationMs
  }

  const notFound = (what: string, id: number) =>
    new Error(`There's no ${what} ${id}`)

  /**
   * Saves an artist's alternate names, after its main name
   * (3_alternate_names.md §6). Links keep showing what they showed:
   * - with `keepOldName`, a rename credits the links that showed the old main
   *   name with it, as an alternate name;
   * - a name that goes leaves its links showing the main name, and so does a
   *   real name that's now the main name.
   *
   * Left out, the names stay as they are, except one that's now the main
   * name: it's promoted, and goes unless it's a real name.
   */
  async function saveArtistNames(
    tx: CatalogStore,
    before: Artist | null,
    artist: Artist,
    input: AlternateNameInput[] | null | undefined,
    keepOldName: boolean
  ) {
    const id = artist.id
    const namesNow = async () =>
      (await tx.artists.alternateNamesOf([id])).get(id) ?? []
    let existing = await namesNow()

    // Before anything else, so links whose name goes below don't get it
    let kept: AlternateName | null = null
    if (
      before &&
      keepOldName &&
      before.name !== artist.name &&
      (await tx.artists.mainNameUses(id)) > 0
    ) {
      kept =
        existing.find((name) => name.name === before.name) ??
        (await tx.artists.addAlternateName(id, {
          name: before.name,
          language: before.nameLanguage,
          realName: false,
          note: null,
        }))
      await tx.artists.creditMainNameLinks(id, kept.id)
      existing = await namesNow()
    }

    let list: AlternateNameValues[] = input
      ? alternateNameRows("artist", artist.name, input)
      : existing.filter((name) => name.name !== artist.name || name.realName)
    const claimed = new Set(
      list.flatMap((name) => (name.id != null ? [name.id] : []))
    )
    // A new name the artist already has keeps its row, and so its links
    list = list.map((name) => {
      if (name.id != null) return name
      const same = existing.find(
        (other) => other.name === name.name && !claimed.has(other.id)
      )
      if (!same) return name
      claimed.add(same.id)
      return { ...name, id: same.id }
    })
    if (kept && !claimed.has(kept.id)) {
      list.push(kept)
      claimed.add(kept.id)
    }

    const going = existing
      .filter((name) => !claimed.has(name.id))
      .map((name) => name.id)
    const nowMain = list.flatMap((name) =>
      name.id != null && name.name === artist.name ? [name.id] : []
    )
    await tx.artists.uncredit(id, [...going, ...nowMain])
    await tx.artists.setAlternateNames(id, list)
  }

  /**
   * Saves a label's alternate names, after its main name. Left out, they stay
   * as they are, except one that's now the main name.
   */
  async function saveLabelNames(
    tx: CatalogStore,
    label: Label,
    input: AlternateNameInput[] | null | undefined
  ) {
    const existing = (await tx.labels.alternateNamesOf([label.id])).get(
      label.id
    )
    await tx.labels.setAlternateNames(
      label.id,
      input
        ? alternateNameRows("label", label.name, input)
        : (existing ?? []).filter((name) => name.name !== label.name)
    )
  }

  /**
   * Saves a release's or track's alternate titles, after its main title
   * (3_alternate_names.md §6). Left out, they stay as they are, except one
   * that's now the main title, which goes.
   */
  async function saveAlternateTitles(
    repository: CatalogStore["releases"] | CatalogStore["tracks"],
    id: number,
    main: string,
    input: AlternateTitleInput[] | null | undefined,
    what: string
  ) {
    if (input) {
      await repository.setAlternateTitles(
        id,
        alternateTitleRows(main, input, what)
      )
      return
    }
    const existing = (await repository.alternateTitlesOf([id])).get(id) ?? []
    if (existing.some((title) => title.title === main)) {
      await repository.setAlternateTitles(
        id,
        existing.filter((title) => title.title !== main)
      )
    }
  }

  /** What keeps an artist from being deleted. */
  async function artistBlockers(id: number) {
    const { releases, tracks, credits } = await store.artists.usage(id)
    return [
      releases && `Release artist on ${count(releases, "release")}`,
      tracks && `Performs on ${count(tracks, "track")}`,
      credits && `Credited ${count(credits, "time")}`,
    ].filter((reason): reason is string => Boolean(reason))
  }

  return {
    // Lists: release formats, release media

    listItems: (kind: ListKind, args: ListArgs) =>
      store.lists.list(kind, {
        ...pagingOf(args),
        ...sortOf(args.sort, ["name"] as const, "Lists"),
        search: searchOf(args.q),
      }),
    getListItem: (kind: ListKind, id: number) => store.lists.get(kind, id),
    listItemsByIds: (kind: ListKind, ids: readonly number[]) =>
      store.lists.getMany(kind, ids),
    listItemUses: (kind: ListKind, id: number) => store.lists.uses(kind, id),
    createListItem: (kind: ListKind, name: string) =>
      store.lists.insert(kind, cleanText(name)),
    async renameListItem(kind: ListKind, id: number, name: string) {
      const item = await store.lists.update(kind, id, cleanText(name))
      if (!item) throw notFound(LIST_NAMES[kind], id)
      return item
    },
    async listItemBlockers(kind: ListKind, id: number) {
      const uses = await store.lists.uses(kind, id)
      return uses === 0 ? [] : [`Used by ${count(uses, "release")}`]
    },
    async deleteListItem(kind: ListKind, id: number) {
      const uses = await store.lists.uses(kind, id)
      if (uses > 0) {
        throw new Error(
          `The ${LIST_NAMES[kind]} is used by ${count(uses, "release")}`
        )
      }
      await store.lists.delete(kind, id)
    },

    // Artists and labels

    listArtists: (args: ListArgs) =>
      store.artists.list({
        ...pagingOf(args),
        ...sortOf(args.sort, ["name"] as const, "Artists"),
        search: searchOf(args.q),
      }),
    getArtist: (id: number) => store.artists.get(id),
    artistsByIds: (ids: readonly number[]) => store.artists.getMany(ids),

    async createArtist(input: ArtistInput) {
      const values = namedValues(input)
      const prepared = await prepareRefs(input.files)
      return store.transaction(async (tx) => {
        const registered = await media.insertPrepared(tx, prepared)
        const artist = await tx.artists.insert(values)
        await saveArtistNames(tx, null, artist, input.alternateNames, false)
        await setFileLinks(tx, "artist", artist.id, input.files, registered)
        return artist
      })
    },

    async updateArtist(id: number, input: ArtistInput) {
      const values = namedValues(input)
      const prepared = await prepareRefs(input.files)
      return store.transaction(async (tx) => {
        const before = await tx.artists.get(id)
        if (!before) throw notFound("artist", id)
        const registered = await media.insertPrepared(tx, prepared)
        const artist = (await tx.artists.update(id, values))!
        await saveArtistNames(
          tx,
          before,
          artist,
          input.alternateNames,
          input.keepOldName ?? false
        )
        await setFileLinks(tx, "artist", id, input.files, registered)
        return artist
      })
    },

    /**
     * Adds an alternate name after the artist's others, like one the import
     * offers (3_alternate_names.md §7.7). Refuses one the artist has already.
     */
    async addArtistName(
      id: number,
      input: { name: string; language?: string | null }
    ) {
      const name = cleanText(input.name)
      if (!name) throw new Error("The name is blank")
      const language = languageTag(input.language)
      return store.transaction(async (tx) => {
        const artist = await tx.artists.get(id)
        if (!artist) throw notFound("artist", id)
        if (name === artist.name) {
          throw new Error(`"${name}" is already the artist's main name`)
        }
        const names = (await tx.artists.alternateNamesOf([id])).get(id) ?? []
        if (names.some((known) => known.name === name)) {
          throw new Error(`${artist.name} already has the name "${name}"`)
        }
        await tx.artists.addAlternateName(id, {
          name,
          language,
          realName: false,
          note: null,
        })
        return artist
      })
    },

    /** Alternate names by artist, in order. */
    artistNamesOf: (ids: readonly number[]) =>
      store.artists.alternateNamesOf(ids),
    /** Artists' alternate names, by their ids. */
    artistNamesByIds: (ids: readonly number[]) =>
      store.artists.alternateNamesByIds(ids),
    /** How many artist links are credited with each name. */
    artistNameUses: (ids: readonly number[]) => store.artists.nameUses(ids),
    /** How many of an artist's links show its main name. */
    mainNameUses: (id: number) => store.artists.mainNameUses(id),

    /** Sets only an artist's picture, registering a path. */
    setArtistPicture: (id: number, picture: FileRef) =>
      setImage("artist", id, picture, (tx, fileId) =>
        tx.artists.update(id, { picture: fileId })
      ),

    artistBlockers,

    async deleteArtist(id: number) {
      const blockers = await artistBlockers(id)
      if (blockers.length > 0) {
        throw new Error(
          `The artist is still used: ${blockers.join(", ").toLowerCase()}`
        )
      }
      await store.artists.delete(id)
    },

    /** Releases where the artist is on tracks without being a release artist. */
    async artistAppearances(id: number) {
      const rows = await store.artists.appearances(id)
      const byRelease = new Map<number, number[]>()
      for (const { releaseId, trackId } of rows) {
        byRelease.set(releaseId, [...(byRelease.get(releaseId) ?? []), trackId])
      }
      return [...byRelease].map(([releaseId, trackIds]) => ({
        releaseId,
        trackIds,
      }))
    },

    artistCredits: (id: number) => store.artists.credits(id),
    artistFormatCounts: (id: number) => store.artists.formatCounts(id),

    listLabels: (args: ListArgs) =>
      store.labels.list({
        ...pagingOf(args),
        ...sortOf(args.sort, ["name"] as const, "Labels"),
        search: searchOf(args.q),
      }),
    getLabel: (id: number) => store.labels.get(id),
    labelsByIds: (ids: readonly number[]) => store.labels.getMany(ids),
    labelReleaseCount: (id: number) => store.labels.releaseCount(id),
    async createLabel(input: LabelInput) {
      const values = namedValues(input)
      return store.transaction(async (tx) => {
        const label = await tx.labels.insert(values)
        await saveLabelNames(tx, label, input.alternateNames)
        return label
      })
    },
    async updateLabel(id: number, input: LabelInput) {
      const values = namedValues(input)
      return store.transaction(async (tx) => {
        const label = await tx.labels.update(id, values)
        if (!label) throw notFound("label", id)
        await saveLabelNames(tx, label, input.alternateNames)
        return label
      })
    },
    /** Alternate names by label, in order. */
    labelNamesOf: (ids: readonly number[]) =>
      store.labels.alternateNamesOf(ids),

    /** Language tags in use on names, the most used first, for suggestions. */
    languages: () => store.stats.languages(),
    /** Sets only a label's logo, registering a path. */
    setLabelPicture: (id: number, picture: FileRef) =>
      setImage("label", id, picture, (tx, fileId) =>
        tx.labels.update(id, { picture: fileId })
      ),
    async labelBlockers(id: number) {
      const releases = await store.labels.releaseCount(id)
      return releases ? [`On ${count(releases, "release")}`] : []
    },
    async deleteLabel(id: number) {
      const releases = await store.labels.releaseCount(id)
      if (releases > 0)
        throw new Error(`The label is on ${count(releases, "release")}`)
      await store.labels.delete(id)
    },

    // Releases

    listReleases: (args: ListArgs & ReleaseFilterArgs) =>
      store.releases.list({
        ...pagingOf(args),
        ...sortOf(args.sort, ["title", "released"] as const, "Releases"),
        ...releaseFilters(args),
      }),
    /** Every matching release's id, to select them all for a bulk edit. */
    releaseIds: (args: ListArgs & ReleaseFilterArgs) =>
      store.releases.ids(releaseFilters(args)),
    getRelease: (id: number) => store.releases.get(id),
    releasesByIds: (ids: readonly number[]) => store.releases.getMany(ids),
    releasesTitled: (title: string) => store.releases.findByTitle(title),
    releaseLinksOf: (link: ReleaseLink, ids: readonly number[]) =>
      store.releases.linksOf(link, ids),
    /** Alternate titles by release, in order. */
    releaseTitlesOf: (ids: readonly number[]) =>
      store.releases.alternateTitlesOf(ids),
    /** Alternate titles by track, in order. */
    trackTitlesOf: (ids: readonly number[]) =>
      store.tracks.alternateTitlesOf(ids),
    /** Release artists, as credited, by release. */
    releaseArtistsOf: (ids: readonly number[]) => store.releases.artistsOf(ids),
    discTitlesOf: (ids: readonly number[]) => store.releases.discTitlesOf(ids),

    /** `2006`, `2006-01` or `2006-01-10`, with only the known parts */
    released: (release: Release) =>
      release.releaseDate === null || release.releaseDatePrecision === null
        ? null
        : formatDate(release.releaseDate, release.releaseDatePrecision),

    async createRelease(input: ReleaseInput) {
      const values = releaseValues(input)
      return store.transaction(async (tx) => {
        const release = await tx.releases.insert({
          ...values,
          coverPicture: null,
        })
        await saveAlternateTitles(
          tx.releases,
          release.id,
          release.title,
          input.alternateTitles,
          "the release"
        )
        await setReleaseLinks(tx, release.id, input)
        return release
      })
    },

    async updateRelease(id: number, input: ReleaseInput) {
      const { titleLanguage, ...values } = releaseValues(input)
      return store.transaction(async (tx) => {
        const release = await tx.releases.update(id, {
          ...values,
          // Left out, it stays as it is
          ...(input.titleLanguage !== undefined && { titleLanguage }),
        })
        if (!release) throw notFound("release", id)
        await saveAlternateTitles(
          tx.releases,
          id,
          release.title,
          input.alternateTitles,
          "the release"
        )
        await setReleaseLinks(tx, id, input)
        return release
      })
    },

    async releaseBlockers(id: number) {
      const tracks = await store.releases.trackCount(id)
      return tracks ? [`Has ${count(tracks, "track")}`] : []
    },

    async deleteRelease(id: number) {
      const tracks = await store.releases.trackCount(id)
      if (tracks > 0) {
        throw new Error(
          `The release still has ${count(tracks, "track")}. Delete them first.`
        )
      }
      await store.releases.delete(id)
    },

    /**
     * Saves a release's whole track list: updates the rows with an id,
     * creates tracks for the others (registering files given by path), and
     * deletes the tracks left out. Their files stay registered. Disc titles,
     * when given, replace the release's.
     */
    async setReleaseTracks(
      id: number,
      rows: ReleaseTrackInput[],
      discTitles?: DiscTitleInput[] | null
    ) {
      const release = await store.releases.get(id)
      if (!release) throw notFound("release", id)
      const titles = discTitles ? discTitleRows(discTitles) : null
      const fresh = rows.filter((row) => row.id == null)
      const prepared = await prepareRefs(fresh)
      // Durations are read now, before the transaction holds the write lock
      const durations = new Map<string, number | null>()
      for (const row of fresh) {
        const file =
          row.fileId != null ? await store.files.get(row.fileId) : null
        const path = file?.path ?? media.normalizePath(row.path ?? "")
        durations.set(path, (await media.readAudioAt(path)).durationMs)
      }

      return store.transaction(async (tx) => {
        const credited = creditedNames(tx)
        const existing = (await tx.tracks.ofReleases([id])).get(id) ?? []
        const kept = new Set(
          rows.flatMap((row) => (row.id != null ? [row.id] : []))
        )
        for (const track of existing) {
          if (!kept.has(track.id)) await tx.tracks.delete(track.id)
        }
        for (const trackId of kept) {
          if (!existing.some((track) => track.id === trackId)) {
            throw new Error(`Track ${trackId} isn't on this release`)
          }
        }
        await tx.tracks.shiftNumbers(id, RENUMBER_OFFSET)
        const registered = await media.insertPrepared(tx, prepared)

        for (const row of rows) {
          const values = {
            cdNumber: row.cdNumber,
            trackNumber: row.trackNumber,
            title: cleanText(row.title),
            // Left out, it stays as it is
            ...(row.titleLanguage !== undefined && {
              titleLanguage: languageTag(row.titleLanguage),
            }),
          }
          let trackId = row.id
          if (trackId != null) {
            await tx.tracks.update(trackId, values)
          } else {
            const fileId = refId(row, registered)
            const file = await tx.files.get(fileId)
            const track = await tx.tracks.insert({
              titleLanguage: null,
              ...values,
              releaseId: id,
              fileId,
              durationMs: durations.get(file?.path ?? "") ?? null,
            })
            trackId = track.id
          }
          await saveAlternateTitles(
            tx.tracks,
            trackId,
            values.title,
            row.alternateTitles,
            `track ${row.trackNumber}`
          )
          await tx.tracks.setArtists(
            trackId,
            await credited(trackArtists(row.artists, row.features))
          )
        }
        // After the tracks, since a title needs a track on its disc
        if (titles) await tx.releases.setDiscTitles(id, titles)
        return release
      })
    },

    /** Replaces a release's credits, and its tracks' credits. */
    async setReleaseCredits(
      id: number,
      input: {
        credits: CreditInput[]
        tracks: { trackId: number; credits: CreditInput[] }[]
      }
    ) {
      return store.transaction(async (tx) => {
        const release = await tx.releases.get(id)
        if (!release) throw notFound("release", id)
        const tracks = new Set(
          ((await tx.tracks.ofReleases([id])).get(id) ?? []).map((t) => t.id)
        )
        const credited = creditedNames(tx)
        await tx.credits.set(
          "release",
          id,
          await resolvedLines(credited, input.credits)
        )
        for (const { trackId, credits } of input.tracks) {
          if (!tracks.has(trackId)) {
            throw new Error(`Track ${trackId} isn't on this release`)
          }
          await tx.credits.set(
            "track",
            trackId,
            await resolvedLines(credited, credits)
          )
        }
        return release
      })
    },

    /** Sets a release's cover and replaces its file links, registering paths. */
    async setReleaseFiles(
      id: number,
      input: { cover?: FileRef | null; files: FileLinkInput[] }
    ) {
      const refs = [...(input.cover ? [input.cover] : []), ...input.files]
      const prepared = await prepareRefs(refs)
      return store.transaction(async (tx) => {
        const registered = await media.insertPrepared(tx, prepared)
        const release = await tx.releases.update(id, {
          coverPicture: input.cover ? refId(input.cover, registered) : null,
        })
        if (!release) throw notFound("release", id)
        await setFileLinks(tx, "release", id, input.files, registered)
        return release
      })
    },

    /** Sets only a release's cover, keeping its file links, registering a path. */
    setReleaseCover: (id: number, cover: FileRef) =>
      setImage("release", id, cover, (tx, fileId) =>
        tx.releases.update(id, { coverPicture: fileId })
      ),

    // Tracks

    listTracks: (
      args: ListArgs & { release?: number | null; tags?: TagFilter[] }
    ) =>
      store.tracks.list({
        ...pagingOf(args),
        ...sortOf(args.sort, ["title"] as const, "Tracks"),
        search: searchOf(args.q),
        release: args.release ?? undefined,
        tags: args.tags,
      }),
    getTrack: (id: number) => store.tracks.get(id),
    tracksByIds: (ids: readonly number[]) => store.tracks.getMany(ids),
    tracksOfReleases: (ids: readonly number[]) => store.tracks.ofReleases(ids),
    tracksOfFiles: (fileIds: readonly number[]) =>
      store.tracks.ofFiles(fileIds),
    trackArtistsOf: (ids: readonly number[]) => store.tracks.artistsOf(ids),

    async createTrack(input: TrackInput) {
      const durationMs = await durationOf(input.fileId)
      const prepared = await prepareRefs(input.files)
      return store.transaction(async (tx) => {
        const registered = await media.insertPrepared(tx, prepared)
        const track = await tx.tracks.insert({
          title: cleanText(input.title),
          titleLanguage: languageTag(input.titleLanguage),
          cdNumber: input.cdNumber,
          trackNumber: input.trackNumber,
          releaseId: input.releaseId,
          fileId: input.fileId,
          durationMs,
        })
        await saveAlternateTitles(
          tx.tracks,
          track.id,
          track.title,
          input.alternateTitles,
          "the track"
        )
        const credited = creditedNames(tx)
        await tx.tracks.setArtists(
          track.id,
          await credited(trackArtists(input.artists, input.features))
        )
        await tx.credits.set(
          "track",
          track.id,
          await resolvedLines(credited, input.credits)
        )
        await setFileLinks(tx, "track", track.id, input.files, registered)
        if (input.tags) {
          await tagging.setTagsWith(tx, "track", track.id, input.tags)
        }
        return track
      })
    },

    async updateTrack(id: number, input: TrackInput) {
      const current = await store.tracks.get(id)
      if (!current) throw notFound("track", id)
      // A new audio file means a new duration
      const durationMs =
        input.fileId === current.fileId
          ? current.durationMs
          : await durationOf(input.fileId)
      const prepared = await prepareRefs(input.files)
      return store.transaction(async (tx) => {
        const registered = await media.insertPrepared(tx, prepared)
        const track = await tx.tracks.update(id, {
          title: cleanText(input.title),
          // Left out, it stays as it is
          ...(input.titleLanguage !== undefined && {
            titleLanguage: languageTag(input.titleLanguage),
          }),
          cdNumber: input.cdNumber,
          trackNumber: input.trackNumber,
          releaseId: input.releaseId,
          fileId: input.fileId,
          durationMs,
        })
        if (!track) throw notFound("track", id)
        await saveAlternateTitles(
          tx.tracks,
          id,
          track.title,
          input.alternateTitles,
          "the track"
        )
        const credited = creditedNames(tx)
        await tx.tracks.setArtists(
          id,
          await credited(trackArtists(input.artists, input.features))
        )
        await tx.credits.set(
          "track",
          id,
          await resolvedLines(credited, input.credits)
        )
        await setFileLinks(tx, "track", id, input.files, registered)
        if (input.tags) await tagging.setTagsWith(tx, "track", id, input.tags)
        return track
      })
    },

    deleteTrack: (id: number) => store.tracks.delete(id),

    // The library check's catalog part (design §8, phase 4)

    releasesWithoutArtists: () => store.releases.withoutArtists(),
    tracksWithoutPerformers: () => store.tracks.withoutPerformers(),

    /**
     * Reads the durations again of the tracks whose audio these files are,
     * after the files changed on disk.
     */
    async readDurations(fileIds: number[]) {
      const tracks = await store.tracks.ofFiles(fileIds)
      for (const [fileId, track] of tracks) {
        const file = await store.files.get(fileId)
        if (!file) continue
        const { durationMs } = await media.readAudio(file)
        if (durationMs !== track.durationMs) {
          await store.tracks.update(track.id, { durationMs })
        }
      }
    },

    /**
     * Who performs a track (design §4.3), as credited: on a regular release
     * with no artists of its own, the release's, with their credited names.
     */
    performers(
      release: Release,
      own: TrackArtist[],
      releaseArtists: ArtistLink[]
    ): ArtistLink[] {
      const performing = own
        .filter((a) => !a.featured)
        .map(({ artistId, creditedNameId }) => ({ artistId, creditedNameId }))
      if (performing.length > 0 || release.variousArtists) return performing
      return releaseArtists
    },

    creditsOf: (owner: CreditOwner, ids: readonly number[]) =>
      store.credits.of(owner, ids),
    /** Roles in use, for the credits editor's suggestions. */
    creditRoles: (q?: string | null) =>
      store.credits.roles({ search: searchOf(q), limit: ROLE_SUGGESTIONS }),
    totals: () => store.stats.totals(),

    // Import suggestions (design §7.9), from embedded tags or else names

    /** What a folder suggests for a new release. */
    async suggestRelease(folder: string) {
      const files = await media.scanFolder(folder)
      const music = files.filter((file) => file.type === "music")
      const tags = await Promise.all(
        music.slice(0, 50).map((file) => media.readAudioAt(file.path))
      )
      const album = mostCommon(tags.map((t) => t.tags.album))
      const date = mostCommon(tags.map((t) => t.tags.date))
      const names = mostCommon(
        tags.map((t) => {
          const names = t.tags.albumArtists.length
            ? t.tags.albumArtists
            : t.tags.artists
          return names.length ? names.join("\u0000") : null
        })
      )?.split("\u0000")
      const normalized = media.normalizePath(folder)
      const title =
        album ?? (normalized === "" ? null : basename(normalized)) ?? ""
      return {
        title,
        released: date && isValidDate(date) ? date : null,
        artists: await suggestArtists(names ?? []),
        sameTitle: await releasesTitledAny([title, ...equalsParts(title)]),
        music: music.length,
        images: files.filter((file) => file.type === "image").length,
        others: files.filter((file) => file.type === "other").length,
      }
    },

    /** Rows for a folder's audio files: from tags, or else the file's name. */
    async suggestTracks(folder: string) {
      const normalized = media.normalizePath(folder)
      const files = (await media.scanFolder(folder)).filter(
        (f) => f.type === "music"
      )
      const tracks = await store.tracks.ofFiles(
        files.flatMap((file) => (file.file ? [file.file.id] : []))
      )
      const numbers = new Map<number, number>()
      const read = []
      for (const file of files) {
        read.push({ file, ...(await media.readAudioAt(file.path)) })
      }
      // Every row's artist tags are matched at once
      const artists = await suggestArtists(read.flatMap((r) => r.tags.artists))
      const rows = []
      for (const { file, durationMs, tags } of read) {
        const fromName = fromFileName(file.path, normalized)
        const cdNumber = tags.discNumber ?? fromName.cdNumber
        const position = (numbers.get(cdNumber) ?? 0) + 1
        numbers.set(cdNumber, position)
        rows.push({
          path: file.path,
          file: file.file,
          track: file.file ? (tracks.get(file.file.id) ?? null) : null,
          cdNumber,
          discTitle: tags.discTitle,
          trackNumber: tags.trackNumber ?? fromName.trackNumber ?? position,
          title: tags.title ?? fromName.title,
          artists: artists.splice(0, tags.artists.length),
          durationMs,
        })
      }
      return rows
    },
  }
}

/** The release list's filters, as the API gives them. */
type ReleaseFilterArgs = {
  format?: number | null
  medium?: number | null
  label?: number | null
  artist?: number | null
  tags?: TagFilter[]
}

function releaseFilters(args: ListArgs & ReleaseFilterArgs) {
  return {
    search: searchOf(args.q),
    format: args.format ?? undefined,
    medium: args.medium ?? undefined,
    label: args.label ?? undefined,
    artist: args.artist ?? undefined,
    tags: args.tags,
  }
}

/** `CD2/03 Title.flac` → disc 2, track 3, "Title" (design §7.9). */
function fromFileName(path: string, folder: string) {
  const relative =
    folder && path.startsWith(`${folder}/`)
      ? path.slice(folder.length + 1)
      : path
  const disc = /(?:^|\/)(?:cd|disc|disk)\s*(\d+)\//i.exec(
    `${dirname(relative)}/`
  )
  const name = basename(relative).replace(/\.[^.]+$/, "")
  const numbered = /^(\d{1,3})(?:[\s._-]+(.*))?$/.exec(name)
  return {
    cdNumber: disc ? Number(disc[1]) || 1 : 1,
    trackNumber: numbered ? Number(numbered[1]) || null : null,
    title: (numbered?.[2] ?? name).trim() || name,
  }
}

/** An artist's or label's own columns, as they're saved. */
function namedValues(input: {
  name: string
  nameLanguage?: string | null
  pictureId?: number | null
}) {
  return {
    name: cleanText(input.name),
    nameLanguage: languageTag(input.nameLanguage),
    picture: input.pictureId ?? null,
  }
}

/**
 * Alternate names as they're saved (3_alternate_names.md §6): cleaned, a
 * blank one left out, and languages in canonical form. Refuses a name given
 * twice, and one that's the main name, unless it's a real name: an artist who
 * performs under their real name can still record it. Labels have no real
 * names. Names compare exactly, case included, so "ALIKA" isn't "Alika".
 */
function alternateNameRows(
  what: NamedKind,
  main: string,
  names: AlternateNameInput[]
): AlternateNameValues[] {
  const seen = new Set<string>()
  return names.flatMap((input) => {
    const name = cleanText(input.name)
    if (!name) return []
    const realName = what === "artist" && Boolean(input.realName)
    if (seen.has(name)) throw new Error(`"${name}" is listed twice`)
    seen.add(name)
    if (name === main && !realName) {
      throw new Error(`"${name}" is already the ${what}'s main name`)
    }
    return [
      {
        id: input.id ?? null,
        name,
        language: languageTag(input.language),
        realName,
        note: cleanOrNull(input.note),
      },
    ]
  })
}

function releaseDate({ epoch, precision }: ParsedDate) {
  return { releaseDate: epoch, releaseDatePrecision: precision }
}

function isValidDate(text: string) {
  try {
    parseDate(text)
    return true
  } catch {
    return false
  }
}

/**
 * A tag's parts around " = ", as Discogs joins a name or title in two
 * languages: "Ryo Fukui = 福居良" (3_alternate_names.md §7.7). The "=" needs
 * spaces on both sides, since it's part of titles like "E=MC²". Empty and
 * repeated parts are left out.
 */
export function equalsParts(text: string): string[] {
  const parts = text.split(/\s+=\s+/u).map((part) => part.trim())
  return [...new Set(parts.filter(Boolean))]
}

/** Lowercase in ASCII only, as SQLite's NOCASE compares. */
function nocase(text: string) {
  return text.replace(/[A-Z]/g, (letter) => letter.toLowerCase())
}

function mostCommon(values: (string | null)[]): string | null {
  const counts = new Map<string, number>()
  for (const value of values) {
    if (value) counts.set(value, (counts.get(value) ?? 0) + 1)
  }
  let best: string | null = null
  for (const [value, n] of counts)
    if (best === null || n > counts.get(best)!) best = value
  return best
}

function unique(ids: number[]): number[] {
  return [...new Set(ids)]
}

/** Each artist once, as first given: an artist appears once per list. */
function firstOfEach<T extends { artistId: number }>(links: T[]): T[] {
  const seen = new Set<number>()
  return links.filter((link) => {
    if (seen.has(link.artistId)) return false
    seen.add(link.artistId)
    return true
  })
}

/**
 * Credit lines as they're saved (2_credits.md §6): roles trimmed, and an
 * artist given twice on a line kept once, with the first name it's credited
 * with. Refuses an empty role, a line with no artists, and two lines with the
 * same role, ignoring case.
 */
function creditLines(credits: CreditInput[]): CreditInput[] {
  const roles = new Set<string>()
  return credits.map(({ role, artists }) => {
    const trimmed = cleanText(role)
    if (!trimmed) throw new Error("A credit needs a role")
    if (artists.length === 0) {
      throw new Error(`The ${trimmed} credit needs an artist`)
    }
    const key = nocaseKey(trimmed)
    if (roles.has(key)) {
      throw new Error(
        `Two credits have the role ${trimmed}, ignoring case. Put their artists on one line.`
      )
    }
    roles.add(key)
    return { role: trimmed, artists: firstOfEach(artists) }
  })
}

/**
 * Disc titles as they're saved: trimmed, and a blank one left out, since a
 * disc without a title has no row. Refuses two titles for one disc, and
 * alternate titles on a disc with no title, since they belong to its title.
 */
function discTitleRows(titles: DiscTitleInput[]): DiscTitle[] {
  const discs = new Set<number>()
  return titles.flatMap(({ cdNumber, title, language, alternateTitles }) => {
    const trimmed = cleanText(title)
    const what = `disc ${cdNumber}`
    if (!trimmed) {
      if (alternateTitles?.some((t) => cleanText(t.title))) {
        throw new Error(`Disc ${cdNumber} has alternate titles but no title`)
      }
      return []
    }
    if (discs.has(cdNumber)) throw new Error(`Disc ${cdNumber} has two titles`)
    discs.add(cdNumber)
    return [
      {
        cdNumber,
        title: trimmed,
        language: languageTag(language),
        alternateTitles: alternateTitleRows(
          trimmed,
          alternateTitles ?? [],
          what
        ),
      },
    ]
  })
}

/**
 * Alternate titles as they're saved (3_alternate_names.md §6): cleaned, a
 * blank one left out, and languages in canonical form. Refuses a title given
 * twice, and one that's the main title. Titles compare exactly, case
 * included.
 */
function alternateTitleRows(
  main: string,
  titles: AlternateTitleInput[],
  what: string
): AlternateTitle[] {
  const seen = new Set<string>()
  return titles.flatMap((input) => {
    const title = cleanText(input.title)
    if (!title) return []
    if (seen.has(title)) {
      throw new Error(`"${title}" is listed twice on ${what}`)
    }
    seen.add(title)
    if (title === main) {
      throw new Error(`"${title}" is already the main title of ${what}`)
    }
    return [
      {
        title,
        language: languageTag(input.language),
        printed: input.printed ?? true,
      },
    ]
  })
}

/** Text compared ignoring ASCII case, like SQLite's NOCASE. */
function nocaseKey(text: string): string {
  return text.replace(/[A-Z]/g, (c) => c.toLowerCase())
}

function count(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? "" : "s"}`
}
