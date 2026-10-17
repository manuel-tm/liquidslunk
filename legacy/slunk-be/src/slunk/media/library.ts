// The user's library on disk. Slunk only ever reads it: nothing here opens a
// file for writing, or changes one.
import { createHash } from "node:crypto"
import { createReadStream, type Dirent, type Stats } from "node:fs"
import { readdir, stat } from "node:fs/promises"
import { dirname, join, relative, resolve } from "node:path"
import { type FileType, fileTypeOf } from "./file_types.ts"
import type { FileFacts } from "./repository.ts"

export type DiskFile = {
  name: string
  path: string
  type: FileType
  sizeBytes: number
}

export type DiskFolder = {
  path: string
  absolutePath: string
  /** Null at the top of the disk */
  parent: string | null
  folders: { name: string; path: string }[]
  files: DiskFile[]
}

export type Library = ReturnType<typeof createLibrary>

/** The library around the database at `dbPath`: paths are relative to its folder. */
export function createLibrary(dbPath: string) {
  const root = dirname(dbPath)
  // Slunk's own files, which aren't part of the library
  const own = new Set(
    ["", "-wal", "-shm", "-journal"]
      .map((suffix) => dbPath + suffix)
      .concat(join(root, "slunk-backups"))
  )
  const skipped = (entry: Dirent, absolute: string) =>
    entry.name.startsWith(".") || own.has(absolute)

  /** Relative to the database's folder, normalized (design §4.1). "" is that folder. */
  function normalize(path: string): string {
    return relative(root, resolve(root, path))
  }

  function absolute(path: string): string {
    return resolve(root, path)
  }

  /** A folder's subfolders and the files directly in it. Hidden files are left out. */
  async function listFolder(path: string): Promise<DiskFolder> {
    const folderPath = normalize(path)
    const folder = absolute(folderPath)
    const folders: DiskFolder["folders"] = []
    const files: DiskFile[] = []
    for (const entry of await read(folder, path)) {
      const entryPath = join(folder, entry.name)
      if (skipped(entry, entryPath)) continue
      // Follows symlinks; a broken one is left out
      const info = await stat(entryPath).catch(() => null)
      if (info?.isDirectory()) {
        folders.push({ name: entry.name, path: normalize(entryPath) })
      } else if (info?.isFile()) {
        files.push(diskFile(entry.name, entryPath, info))
      }
    }
    return {
      path: folderPath,
      absolutePath: folder,
      parent: dirname(folder) === folder ? null : normalize(dirname(folder)),
      folders: folders.sort((a, b) => compare(a.name, b.name)),
      files: files.sort((a, b) => compare(a.name, b.name)),
    }
  }

  /**
   * Every file in a folder and its subfolders, by path. Hidden files and
   * folders are left out, and symlinked folders aren't followed, so a link
   * loop can't make it run forever.
   */
  async function scanFolder(path: string): Promise<DiskFile[]> {
    const files: DiskFile[] = []
    async function walk(folder: string) {
      const entries = (await read(folder, normalize(folder))).filter(
        (entry) => !skipped(entry, join(folder, entry.name))
      )
      // A folder's files are read together, which is much faster on a big
      // library than one after another
      await Promise.all(
        entries.map(async (entry) => {
          const entryPath = join(folder, entry.name)
          if (entry.isFile() || entry.isSymbolicLink()) {
            const info = await stat(entryPath).catch(() => null)
            if (info?.isFile()) {
              files.push(diskFile(entry.name, entryPath, info))
            }
          }
        })
      )
      for (const entry of entries) {
        if (entry.isDirectory()) await walk(join(folder, entry.name))
      }
    }
    await walk(absolute(normalize(path)))
    return files.sort((a, b) => compare(a.path, b.path))
  }

  /** Reads a file's size, mtime and SHA-256. */
  async function readFacts(path: string): Promise<FileFacts> {
    const file = absolute(path)
    const info = await stat(file).catch(() => null)
    if (!info?.isFile()) throw new Error(`There's no file at ${path}`)
    const hash = createHash("sha256")
    for await (const chunk of createReadStream(file)) hash.update(chunk)
    return {
      sizeBytes: info.size,
      mtime: Math.floor(info.mtimeMs / 1000),
      sha256: hash.digest("hex"),
    }
  }

  async function exists(path: string): Promise<boolean> {
    const info = await stat(absolute(path)).catch(() => null)
    return info?.isFile() ?? false
  }

  /** A file's size and mtime, as registering reads them, or null if it's gone. */
  async function statFile(
    path: string
  ): Promise<{ sizeBytes: number; mtime: number } | null> {
    const info = await stat(absolute(path)).catch(() => null)
    if (!info?.isFile()) return null
    return { sizeBytes: info.size, mtime: Math.floor(info.mtimeMs / 1000) }
  }

  function diskFile(name: string, file: string, info: Stats): DiskFile {
    return {
      name,
      path: normalize(file),
      type: fileTypeOf(name),
      sizeBytes: info.size,
    }
  }

  return {
    normalize,
    absolute,
    listFolder,
    scanFolder,
    readFacts,
    exists,
    statFile,
  }
}

async function read(folder: string, shown: string): Promise<Dirent[]> {
  try {
    return await readdir(folder, { withFileTypes: true })
  } catch {
    throw new Error(`Can't read the folder ${shown || "."}`)
  }
}

/** Like a file browser sorts: "2 Song" before "10 Song". */
function compare(a: string, b: string): number {
  return a.localeCompare(b, undefined, { numeric: true })
}
