import type { ListKind } from "@/bindings"

/** The two lists the user manages, which share their pages. */
export const LISTS: Record<
  ListKind,
  {
    kind: ListKind
    path: string
    title: string
    one: string
    /** The release list's filter for it */
    releaseFilter: "format" | "medium"
  }
> = {
  release_formats: {
    kind: "release_formats",
    path: "/release-formats",
    title: "Release formats",
    one: "format",
    releaseFilter: "format",
  },
  release_media: {
    kind: "release_media",
    path: "/release-media",
    title: "Release media",
    one: "medium",
    releaseFilter: "medium",
  },
}
