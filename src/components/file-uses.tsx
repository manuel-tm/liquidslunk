import { Link } from "react-router"

import type { FileUseKind } from "@/gql/graphql"

const USE_LABELS: Record<FileUseKind, string> = {
  track_audio: "Audio of track",
  release_cover: "Cover of release",
  artist_picture: "Picture of artist",
  label_picture: "Logo of label",
  release_link: "Linked to release",
  artist_link: "Linked to artist",
  track_link: "Linked to track",
}

const USE_URLS: Record<FileUseKind, string> = {
  track_audio: "/tracks",
  release_cover: "/releases",
  artist_picture: "/artists",
  label_picture: "/labels",
  release_link: "/releases",
  artist_link: "/artists",
  track_link: "/tracks",
}

export type FileUse = {
  kind: FileUseKind
  entityId: string
  name: string
  note?: string | null
}

/** What uses a file, each linking to its page. */
export function FileUses({ uses }: { uses: FileUse[] }) {
  return (
    <ul className="flex flex-col gap-1">
      {uses.map((use) => (
        <li key={`${use.kind}-${use.entityId}`}>
          <span className="text-muted-foreground">{USE_LABELS[use.kind]}</span>{" "}
          <Link
            to={`${USE_URLS[use.kind]}/${use.entityId}`}
            className="hover:underline"
          >
            {use.name}
          </Link>
          {use.note && (
            <span className="text-muted-foreground"> ({use.note})</span>
          )}
        </li>
      ))}
    </ul>
  )
}
