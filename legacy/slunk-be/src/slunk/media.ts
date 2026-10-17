// The media context: registered files, and the library on disk they come
// from. Slunk only reads the files; it records them in its database.
import { posix } from "node:path"
import { fileTypeOf, type FileType } from "./media/file_types.ts"
import type { DiskFile, Library } from "./media/library.ts"
import { type ListArgs, pagingOf, searchOf, sortOf } from "./paging.ts"
import { readAudio } from "./media/audio.ts"
import type {
  FileLinksRepository,
  FileRecord,
  FilesRepository,
  FileUse,
  LinkOwner,
  NewFile,
} from "./media/repository.ts"

export type Media = ReturnType<typeof createMedia>

/** Files read from disk and ready to insert, and those already registered. */
export type PreparedFiles = { existing: FileRecord[]; fresh: NewFile[] }

/** What the library check found on disk (design §8, phase 4). */
export type FileCheck = {
  /** How many registered files were checked */
  fileCount: number
  /**
   * Registered files that are gone, with unregistered files of the same
   * content, those with the same name first. `likelyMove` is the only one,
   * or the only one with the same name, and null when that's unclear.
   */
  missing: { file: FileRecord; movedTo: string[]; likelyMove: string | null }[]
  /** Registered files whose size or mtime isn't what was read */
  changed: { file: FileRecord; sizeBytes: number; mtime: number }[]
  /** Unregistered files in folders that hold registered ones, by folder */
  unregistered: { path: string; files: DiskFile[] }[]
}

// How many files are read from disk at once
const PARALLEL = 64

export function createMedia({
  files,
  fileLinks,
  library,
}: {
  files: FilesRepository
  fileLinks: FileLinksRepository
  library: Library
}) {
  /** Registered files at these paths, by path. */
  async function registered(paths: string[]): Promise<Map<string, FileRecord>> {
    const found = await files.findByPaths(paths)
    return new Map(found.map((file) => [file.path, file]))
  }

  /** Whether any registered file is in the folder or its subfolders. */
  function holdsRegisteredFiles(folder: string): Promise<boolean> {
    // A folder at or above the database's folder holds everything but the
    // paths that climb out of it: "../.." holds all but "../../../…"
    const ups = folder === "" ? 0 : folder.split("/").length
    if (folder === "" || /^\.\.(\/\.\.)*$/.test(folder)) {
      return files.hasPathStartingWith("../".repeat(ups + 1), true)
    }
    return files.hasPathStartingWith(`${folder}/`)
  }

  /** Reads the files at these paths that aren't registered yet. Writes nothing. */
  async function prepareFiles(paths: string[]): Promise<PreparedFiles> {
    const wanted = [...new Set(paths.map(library.normalize))]
    const known = await registered(wanted)
    const fresh: NewFile[] = []
    for (const path of wanted.filter((path) => !known.has(path))) {
      if (path === "") throw new Error("That's a folder, not a file")
      const facts = await library.readFacts(path)
      fresh.push({ path, type: fileTypeOf(path), ...facts })
    }
    return { existing: [...known.values()], fresh }
  }

  /** Inserts prepared files, maybe in a transaction. Returns them all by path. */
  async function insertPrepared(
    store: { files: FilesRepository },
    { existing, fresh }: PreparedFiles
  ): Promise<Map<string, FileRecord>> {
    const byPath = new Map(existing.map((file) => [file.path, file]))
    for (const file of await store.files.insert(fresh))
      byPath.set(file.path, file)
    return byPath
  }

  /** Unregistered files that hold the same bytes as missing ones, by hash. */
  async function movedFiles(
    all: FileRecord[],
    missing: FileRecord[],
    known: Set<string>,
    listed: DiskFile[]
  ): Promise<Map<string, string[]>> {
    // A moved file can be anywhere in the library: the database's folder,
    // and the top folders outside it that hold registered files
    const roots = new Set([""])
    for (const { path } of all) {
      const outside = /^((?:\.\.\/)+)([^/]+)\//.exec(path)
      if (outside) roots.add(outside[1] + outside[2])
    }
    const onDisk = new Map(listed.map((file) => [file.path, file]))
    for (const root of roots) {
      const found = await library.scanFolder(root).catch(() => [])
      for (const file of found) onDisk.set(file.path, file)
    }

    // Only files of a missing file's size are worth hashing
    const sizes = new Set(missing.map((file) => file.sizeBytes))
    const candidates = [...onDisk.values()].filter(
      (file) => !known.has(file.path) && sizes.has(file.sizeBytes)
    )
    const byHash = new Map<string, string[]>()
    await inBatches(candidates, async (file) => {
      const facts = await library.readFacts(file.path).catch(() => null)
      if (facts)
        byHash.set(facts.sha256, [
          ...(byHash.get(facts.sha256) ?? []),
          file.path,
        ])
    })
    return byHash
  }

  /**
   * Changes a file's type, or its path to relink a file that moved, in
   * which case its size, mtime and hash are read again.
   */
  async function updateFile(
    id: number,
    changes: { path?: string | null; type?: FileType | null }
  ): Promise<FileRecord> {
    const file = await files.get(id)
    if (!file) throw new Error(`There's no file ${id}`)
    const update: Partial<NewFile> = {}
    if (changes.path != null) {
      const path = library.normalize(changes.path)
      if (path !== file.path) {
        const [taken] = await files.findByPaths([path])
        if (taken) {
          throw new Error(`File ${taken.id} is already registered at ${path}`)
        }
        Object.assign(update, { path }, await library.readFacts(path))
      }
    }
    if (changes.type != null) update.type = changes.type
    const updated = await files.update(id, update)
    if (!updated) throw new Error(`There's no file ${id}`)
    return updated
  }

  return {
    normalizePath: (path: string) => library.normalize(path),
    prepareFiles,
    insertPrepared,
    filesByIds: (ids: readonly number[]) => files.getMany(ids),
    fileLinksOf: (owner: LinkOwner, ids: readonly number[]) =>
      fileLinks.of(owner, ids),
    /** An audio file's duration and embedded tags. */
    readAudio: (file: FileRecord) => readAudio(library.absolute(file.path)),
    readAudioAt: (path: string) => readAudio(library.absolute(path)),

    /** A page of files. */
    listFiles(
      args: ListArgs & {
        type?: FileType | null
        unused?: boolean | null
        withoutTrack?: boolean | null
      }
    ) {
      return files.list({
        ...pagingOf(args),
        ...sortOf(args.sort, ["path"] as const, "Files"),
        search: searchOf(args.q),
        type: args.type ?? undefined,
        unused: args.unused ?? undefined,
        withoutTrack: args.withoutTrack ?? undefined,
      })
    },

    getFile: (id: number) => files.get(id),
    fileCounts: () => files.counts(),
    fileUses: (id: number): Promise<FileUse[]> => files.uses(id),
    fileOnDisk: (file: FileRecord) => library.exists(file.path),
    absolutePath: (file: FileRecord) => library.absolute(file.path),

    /** A folder's subfolders and files, marking what's registered. */
    async browseFolder(path: string) {
      const folder = await library.listFolder(path)
      const known = await registered(folder.files.map((file) => file.path))
      return {
        ...folder,
        folders: await Promise.all(
          folder.folders.map(async (sub) => ({
            ...sub,
            hasRegisteredFiles: await holdsRegisteredFiles(sub.path),
          }))
        ),
        files: folder.files.map((file) => ({
          ...file,
          file: known.get(file.path) ?? null,
        })),
      }
    },

    /** Every file in a folder and its subfolders, with its registration. */
    async scanFolder(path: string) {
      const found = await library.scanFolder(path)
      const known = await registered(found.map((file) => file.path))
      return found.map((file) => ({
        ...file,
        file: known.get(file.path) ?? null,
      }))
    },

    updateFile,

    /**
     * Registers files on disk: reads their size, mtime and hash, and types
     * them by extension. Paths already registered are returned as they are.
     */
    async registerFiles(paths: string[]): Promise<FileRecord[]> {
      const byPath = await insertPrepared({ files }, await prepareFiles(paths))
      return [...new Set(paths.map(library.normalize))].map((path) =>
        byPath.get(path)!
      )
    },

    /**
     * The library check's file part. It reads each registered file's size
     * and mtime, and lists the folders they're in. When some are missing, it
     * scans the library and hashes files of the same size, to find where they
     * moved. Nothing is written.
     */
    async checkFiles(): Promise<FileCheck> {
      const all = await files.all()
      const known = new Set(all.map((file) => file.path))
      const missing: FileRecord[] = []
      const changed: FileCheck["changed"] = []
      await inBatches(all, async (file) => {
        const now = await library.statFile(file.path)
        if (!now) missing.push(file)
        else if (now.sizeBytes !== file.sizeBytes || now.mtime !== file.mtime) {
          changed.push({ file, ...now })
        }
      })

      // The folders that hold registered files, and what else is in them
      const folders = [...new Set(all.map((file) => folderOf(file.path)))]
      const listed: DiskFile[] = []
      for (const folder of folders) {
        const found = await library.listFolder(folder).catch(() => null)
        if (found) listed.push(...found.files)
      }

      const moved =
        missing.length > 0
          ? await movedFiles(all, missing, known, listed)
          : new Map<string, string[]>()
      const movedTo = new Set([...moved.values()].flat())

      const unregistered = new Map<string, DiskFile[]>()
      for (const file of listed) {
        if (known.has(file.path) || movedTo.has(file.path)) continue
        const folder = folderOf(file.path)
        unregistered.set(folder, [...(unregistered.get(folder) ?? []), file])
      }

      const byPath = (a: { path: string }, b: { path: string }) =>
        a.path.localeCompare(b.path, undefined, { numeric: true })
      return {
        fileCount: all.length,
        missing: missing
          .sort((a, b) => byPath(a, b))
          .map((file) => candidatesFor(file, moved.get(file.sha256) ?? [])),
        changed: changed.sort((a, b) => byPath(a.file, b.file)),
        unregistered: [...unregistered]
          .map(([path, found]) => ({ path, files: found }))
          .sort(byPath),
      }
    },

    /**
     * Points files at new paths, for files that moved. Each one is read
     * again, as when a file's path is edited.
     */
    async relinkFiles(moves: { fileId: number; path: string }[]) {
      const relinked: FileRecord[] = []
      for (const { fileId, path } of moves) {
        relinked.push(await updateFile(fileId, { path }))
      }
      return relinked
    },

    /** Reads files' size, mtime and hash again, for files that changed. */
    async rehashFiles(ids: number[]): Promise<FileRecord[]> {
      const found = await files.getMany(ids)
      const facts = new Map<number, Awaited<ReturnType<Library["readFacts"]>>>()
      // Read first, so a file that's gone fails before anything is saved
      await inBatches(found, async (file) => {
        facts.set(file.id, await library.readFacts(file.path))
      })
      const rehashed: FileRecord[] = []
      for (const file of found) {
        const updated = await files.update(file.id, facts.get(file.id)!)
        if (updated) rehashed.push(updated)
      }
      return rehashed
    },

    /** Deletes a file's record. The file on disk isn't touched. */
    async deleteFile(id: number): Promise<void> {
      const uses = await files.uses(id)
      if (uses.length > 0) {
        throw new Error(
          `The file is still used: ${uses.map(describeUse).join(", ")}`
        )
      }
      await files.delete(id)
    },
  }
}

/** A missing file's candidates, the ones with its name first. */
function candidatesFor(file: FileRecord, paths: string[]) {
  const name = posix.basename(file.path)
  const sameName = paths.filter((path) => posix.basename(path) === name)
  return {
    file,
    movedTo: [...sameName, ...paths.filter((path) => !sameName.includes(path))],
    likelyMove:
      paths.length === 1
        ? paths[0]
        : sameName.length === 1
          ? sameName[0]
          : null,
  }
}

/** The folder a path is in: "" for the database's folder. */
function folderOf(path: string): string {
  const folder = posix.dirname(path)
  return folder === "." ? "" : folder
}

/** Runs `fn` over items, PARALLEL at a time. */
async function inBatches<T>(items: T[], fn: (item: T) => Promise<void>) {
  for (let i = 0; i < items.length; i += PARALLEL) {
    await Promise.all(items.slice(i, i + PARALLEL).map(fn))
  }
}

function describeUse({ kind, name }: FileUse): string {
  const what = {
    track_audio: "the audio of track",
    release_cover: "the cover of",
    artist_picture: "the picture of",
    label_picture: "the logo of",
    release_link: "linked to release",
    artist_link: "linked to artist",
    track_link: "linked to track",
  }[kind]
  return `${what} "${name}"`
}
