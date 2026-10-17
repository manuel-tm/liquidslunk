import { FileIcon, FileMusicIcon, ImageIcon } from "lucide-react"

import type { FileType } from "@/gql/graphql"

export const FILE_TYPE_LABELS: Record<FileType, string> = {
  music: "Music",
  image: "Image",
  other: "Other",
}

export function FileTypeIcon({ type }: { type: FileType }) {
  if (type === "music") return <FileMusicIcon />
  if (type === "image") return <ImageIcon />
  return <FileIcon />
}
