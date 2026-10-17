import { extname } from "node:path"

export type FileType = "music" | "image" | "other"

const MUSIC = new Set([
  "aac",
  "aif",
  "aiff",
  "alac",
  "ape",
  "dff",
  "dsf",
  "flac",
  "m4a",
  "mp3",
  "mpc",
  "oga",
  "ogg",
  "opus",
  "wav",
  "wma",
  "wv",
])

const IMAGES = new Set([
  "avif",
  "bmp",
  "gif",
  "heic",
  "jpeg",
  "jpg",
  "png",
  "svg",
  "tif",
  "tiff",
  "webp",
])

/** A file's type, from its extension. Anything unknown is `other`. */
export function fileTypeOf(path: string): FileType {
  const extension = extname(path).slice(1).toLowerCase()
  if (MUSIC.has(extension)) return "music"
  if (IMAGES.has(extension)) return "image"
  return "other"
}
