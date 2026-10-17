import type { FileType } from "./file_types.ts"

export type FileRecord = {
  id: number
  /** Relative to the database's folder (design §4.1) */
  path: string
  type: FileType
  sizeBytes: number
  /** Epoch seconds */
  mtime: number
  sha256: string
  insertedAt: number
  updatedAt: number
}

/** What Slunk reads from a file on disk. */
export type FileFacts = Pick<FileRecord, "sizeBytes" | "mtime" | "sha256">

export type NewFile = Pick<FileRecord, "path" | "type"> & FileFacts

/** A row that uses a file, which keeps the file from being deleted. */
export type FileUse = {
  kind:
    | "track_audio"
    | "release_cover"
    | "artist_picture"
    | "label_picture"
    | "release_link"
    | "artist_link"
    | "track_link"
  /** The track, release, artist or label */
  entityId: number
  name: string
  /** A file link's note */
  note: string | null
}

export type FileQuery = {
  offset: number
  limit: number
  sort: "id" | "path"
  descending: boolean
  /** Part of the path, ignoring case */
  search?: string
  type?: FileType
  /** Only files nothing uses */
  unused?: boolean
  /** Only files that aren't a track's audio */
  withoutTrack?: boolean
}

/** What the media context needs from storage. src/repo/ implements it. */
export interface FilesRepository {
  list(query: FileQuery): Promise<{ files: FileRecord[]; total: number }>
  get(id: number): Promise<FileRecord | null>
  getMany(ids: readonly number[]): Promise<FileRecord[]>
  findByPaths(paths: string[]): Promise<FileRecord[]>
  /** Every registered file, by path, for the library check */
  all(): Promise<FileRecord[]>
  insert(files: NewFile[]): Promise<FileRecord[]>
  /** Also sets `updatedAt`. Null if there's no such file. */
  update(id: number, changes: Partial<NewFile>): Promise<FileRecord | null>
  delete(id: number): Promise<void>
  uses(id: number): Promise<FileUse[]>
  counts(): Promise<{ total: number; unused: number }>
  /** Whether a registered path starts with `prefix`, or doesn't. */
  hasPathStartingWith(prefix: string, negate?: boolean): Promise<boolean>
}

/** What a file link belongs to (design §4.3 File links). */
export type LinkOwner = "release" | "artist" | "track"

export type FileLink = { fileId: number; note: string | null }

export interface FileLinksRepository {
  /** Links by owner, in the order they were added */
  of(owner: LinkOwner, ids: readonly number[]): Promise<Map<number, FileLink[]>>
  /** Replaces the owner's links */
  set(owner: LinkOwner, id: number, links: FileLink[]): Promise<void>
}
