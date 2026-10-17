import type {
  FileLinksRepository,
  FilesRepository,
} from "../media/repository.ts"
import type { ListQuery, Page } from "../paging.ts"
import type { DatePrecision } from "../dates.ts"
import type { TagFilter, TagsRepository } from "../tagging/repository.ts"

export type Artist = {
  id: number
  /** The main name (3_alternate_names.md §4.2) */
  name: string
  /** A BCP 47 tag, or null when it isn't known */
  nameLanguage: string | null
  /** A file id */
  picture: number | null
  insertedAt: number
  updatedAt: number
}

/** Labels have the same shape: `picture` is the logo. */
export type Label = Artist

/**
 * A name an artist or label goes by, besides its main one
 * (3_alternate_names.md §4.2). A label's is never a real name.
 */
export type AlternateName = {
  id: number
  name: string
  /** A BCP 47 tag, or null when it isn't known */
  language: string | null
  /** A legal or birth name */
  realName: boolean
  note: string | null
}

/** An alternate name to save. With an id, it's one the owner has. */
export type AlternateNameValues = Omit<AlternateName, "id"> & {
  id?: number | null
}

export type Release = {
  id: number
  /** The main title (3_alternate_names.md §4.2) */
  title: string
  /** Its language, and its discs' and tracks' unless they have their own */
  titleLanguage: string | null
  /** Epoch seconds, midnight UTC */
  releaseDate: number | null
  releaseDatePrecision: DatePrecision | null
  catalogNumber: string | null
  /** A file id */
  coverPicture: number | null
  variousArtists: boolean
  insertedAt: number
  updatedAt: number
}

export type Track = {
  id: number
  title: string
  /** Null: the release's title language */
  titleLanguage: string | null
  durationMs: number | null
  trackNumber: number
  cdNumber: number
  releaseId: number
  fileId: number
  insertedAt: number
  updatedAt: number
}

/**
 * An artist named on a release, a track or a credit line, and the alternate
 * name printed there: null for the main name (3_alternate_names.md §4.2).
 */
export type ArtistLink = { artistId: number; creditedNameId: number | null }

export type TrackArtist = ArtistLink & { featured: boolean }

/**
 * A title besides the main one, of a release, a track or a disc
 * (3_alternate_names.md §4.2). `printed` says it's on the release as
 * published, rather than added in Slunk, like a translation.
 */
export type AlternateTitle = {
  title: string
  language: string | null
  printed: boolean
}

/** A disc's own title, as printed. Only discs with a title have one. */
export type DiscTitle = {
  cdNumber: number
  title: string
  /** Null: the release's title language */
  language: string | null
  alternateTitles: AlternateTitle[]
}

/** A credit line: a role as printed, and the artists who hold it, in order. */
export type Credit = { role: string; artists: ArtistLink[] }

/** The lists the user manages, which are just names. */
export type ListKind = "release_formats" | "release_media"

export type ListItem = { kind: ListKind; id: number; name: string }

/** A release the artist is on as a track artist only, with those tracks. */
export type Appearance = { releaseId: number; trackIds: number[] }

/** A credit line the artist is on */
export type ArtistCredit = {
  role: string
  releaseId: number
  trackId: number | null
  /** The alternate name the artist is credited with on it */
  creditedNameId: number | null
}

export type FormatCount = { formatId: number; count: number }

export type NamedValues = {
  name: string
  nameLanguage: string | null
  picture: number | null
}

/** An artist or label found by a name: an alternate name, or null for the main one. */
export type NameMatch<T> = { item: T; alternate: AlternateName | null }

/** Artists and labels: named entities with a picture, and alternate names. */
export interface NamedRepository<T> {
  /** The search matches the main name or an alternate name */
  list(query: ListQuery<"name">): Promise<Page<T>>
  get(id: number): Promise<T | null>
  getMany(ids: readonly number[]): Promise<T[]>
  /**
   * Who has one of these names, as main or alternate name, ignoring ASCII
   * case as SQLite's NOCASE does: a match for each such name
   */
  findByNames(names: string[]): Promise<NameMatch<T>[]>
  insert(values: NamedValues): Promise<T>
  update(id: number, values: Partial<NamedValues>): Promise<T | null>
  delete(id: number): Promise<void>
  /** Alternate names by owner, in order */
  alternateNamesOf(
    ids: readonly number[]
  ): Promise<Map<number, AlternateName[]>>
  /**
   * Replaces an owner's alternate names, in order. A name with an id keeps its
   * row, the others are added, and the names left out are deleted.
   */
  setAlternateNames(id: number, names: AlternateNameValues[]): Promise<void>
}

export interface ArtistsRepository extends NamedRepository<Artist> {
  /** Alternate names, by their ids */
  alternateNamesByIds(ids: readonly number[]): Promise<AlternateName[]>
  /** Adds an alternate name after the artist's others */
  addAlternateName(
    artistId: number,
    values: Omit<AlternateNameValues, "id">
  ): Promise<AlternateName>
  /** How many artist links are credited with each name */
  nameUses(nameIds: readonly number[]): Promise<Map<number, number>>
  /** How many of the artist's links show its main name */
  mainNameUses(artistId: number): Promise<number>
  /** Credits the artist's links that show its main name with a name instead */
  creditMainNameLinks(artistId: number, nameId: number): Promise<void>
  /** Credits the artist's links that use these names with the main name */
  uncredit(artistId: number, nameIds: number[]): Promise<void>
  /** Where the artist is used, which keeps it from being deleted */
  usage(
    id: number
  ): Promise<{ releases: number; tracks: number; credits: number }>
  /** Releases where the artist is on tracks but isn't a release artist */
  appearances(id: number): Promise<{ releaseId: number; trackId: number }[]>
  credits(id: number): Promise<ArtistCredit[]>
  /** How many of the artist's releases have each format */
  formatCounts(id: number): Promise<FormatCount[]>
}

export interface LabelsRepository extends NamedRepository<Label> {
  releaseCount(id: number): Promise<number>
}

/** A release list's search and filters. They all have to match. */
export type ReleaseFilters = {
  search?: string
  format?: number
  medium?: number
  label?: number
  artist?: number
  tags?: TagFilter[]
}

export type ReleaseQuery = ListQuery<"title" | "released"> & ReleaseFilters

export type ReleaseValues = Omit<Release, "id" | "insertedAt" | "updatedAt">

/**
 * The rows that link a release to artists, labels, formats or media. A
 * release's artists also have credited names: see `artistsOf`.
 */
export type ReleaseLink = "artists" | "labels" | "formats" | "media"

export interface ReleasesRepository {
  list(query: ReleaseQuery): Promise<Page<Release>>
  /** Every matching release's id, newest first, for selecting them all */
  ids(query: ReleaseFilters): Promise<number[]>
  get(id: number): Promise<Release | null>
  getMany(ids: readonly number[]): Promise<Release[]>
  /** Same title, or alternate title, ignoring case */
  findByTitle(title: string): Promise<Release[]>
  insert(values: ReleaseValues): Promise<Release>
  update(id: number, values: Partial<ReleaseValues>): Promise<Release | null>
  delete(id: number): Promise<void>
  /** Linked ids by release, in display order */
  linksOf(
    link: ReleaseLink,
    releaseIds: readonly number[]
  ): Promise<Map<number, number[]>>
  /** Replaces the links; their order is the display order */
  setLinks(
    link: Exclude<ReleaseLink, "artists">,
    releaseId: number,
    ids: number[]
  ): Promise<void>
  /** Release artists by release, in display order */
  artistsOf(releaseIds: readonly number[]): Promise<Map<number, ArtistLink[]>>
  /** Replaces the release's artists; their order is the display order */
  setArtists(releaseId: number, artists: ArtistLink[]): Promise<void>
  /** Alternate titles by release, in order */
  alternateTitlesOf(
    releaseIds: readonly number[]
  ): Promise<Map<number, AlternateTitle[]>>
  /** Replaces the release's alternate titles, in order */
  setAlternateTitles(releaseId: number, titles: AlternateTitle[]): Promise<void>
  /** Disc titles by release, by disc number, with their alternate titles */
  discTitlesOf(releaseIds: readonly number[]): Promise<Map<number, DiscTitle[]>>
  /** Replaces the disc titles. Each disc needs a track already. */
  setDiscTitles(releaseId: number, titles: DiscTitle[]): Promise<void>
  trackCount(id: number): Promise<number>
  /** Releases with no artist of their own, for the library check */
  withoutArtists(): Promise<Release[]>
}

export type TrackValues = Omit<Track, "id" | "insertedAt" | "updatedAt">

export interface TracksRepository {
  list(
    query: ListQuery<"title"> & { release?: number; tags?: TagFilter[] }
  ): Promise<Page<Track>>
  get(id: number): Promise<Track | null>
  getMany(ids: readonly number[]): Promise<Track[]>
  /** The track each file is the audio of, by file id */
  ofFiles(fileIds: readonly number[]): Promise<Map<number, Track>>
  /** By release, in order: disc, then number */
  ofReleases(releaseIds: readonly number[]): Promise<Map<number, Track[]>>
  insert(values: TrackValues): Promise<Track>
  update(id: number, values: Partial<TrackValues>): Promise<Track | null>
  delete(id: number): Promise<void>
  /** Moves a release's track numbers out of the way, to renumber them */
  shiftNumbers(releaseId: number, by: number): Promise<void>
  /** Alternate titles by track, in order */
  alternateTitlesOf(
    trackIds: readonly number[]
  ): Promise<Map<number, AlternateTitle[]>>
  /** Replaces the track's alternate titles, in order */
  setAlternateTitles(trackId: number, titles: AlternateTitle[]): Promise<void>
  /** Own artists by track, in display order */
  artistsOf(trackIds: readonly number[]): Promise<Map<number, TrackArtist[]>>
  setArtists(trackId: number, artists: TrackArtist[]): Promise<void>
  /**
   * Tracks on various-artists releases with no non-featured artist of their
   * own, so nobody performs them (design §4.3), for the library check
   */
  withoutPerformers(): Promise<Track[]>
}

export interface ListsRepository {
  list(kind: ListKind, query: ListQuery<"name">): Promise<Page<ListItem>>
  get(kind: ListKind, id: number): Promise<ListItem | null>
  getMany(kind: ListKind, ids: readonly number[]): Promise<ListItem[]>
  insert(kind: ListKind, name: string): Promise<ListItem>
  update(kind: ListKind, id: number, name: string): Promise<ListItem | null>
  delete(kind: ListKind, id: number): Promise<void>
  /** Releases with the format or medium */
  uses(kind: ListKind, id: number): Promise<number>
}

export type CreditOwner = "release" | "track"

export interface CreditsRepository {
  /** Credit lines by release or track, in display order */
  of(owner: CreditOwner, ids: readonly number[]): Promise<Map<number, Credit[]>>
  /**
   * Replaces the lines, in display order. Call it inside a transaction: the
   * database checks that each line has an artist at commit.
   */
  set(owner: CreditOwner, id: number, credits: Credit[]): Promise<void>
  /** Roles in use on releases and tracks, each spelling once, the most used first */
  roles(query: { search?: string; limit: number }): Promise<string[]>
}

export interface StatsRepository {
  totals(): Promise<{
    releases: number
    artists: number
    tracks: number
    labels: number
    durationMs: number
  }>
  /** Language tags in use on names and titles, the most used first */
  languages(): Promise<string[]>
}

/** Every repository the catalog needs, and a way to use them in one transaction. */
export interface CatalogStore {
  artists: ArtistsRepository
  labels: LabelsRepository
  releases: ReleasesRepository
  tracks: TracksRepository
  lists: ListsRepository
  credits: CreditsRepository
  stats: StatsRepository
  files: FilesRepository
  fileLinks: FileLinksRepository
  /** A track's tags are saved with the track, through tagging */
  tags: TagsRepository
  /** Runs `fn` in one transaction, with repositories bound to it. */
  transaction<T>(fn: (store: CatalogStore) => Promise<T>): Promise<T>
}
