import { Link } from "react-router"

import { Cover } from "@/components/cover"
import { SortableHead } from "@/components/list/sortable-head"
import { artistUrl, labelUrl, Names } from "@/components/names"
import { ReleasePlayButton } from "@/components/player/play-buttons"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { ListedAlternateTitle, Title } from "@/components/titles"
import type { CreditedLink } from "@/lib/credited"
import type { AlternateTitle } from "@/lib/titles"
import { formatDateTime, formatReleased } from "@/lib/format"

type Named = { id: number; name: string }

/** A release, with whichever fields the page's command returns. */
export type ReleaseRow = {
  id: number
  title: string
  titleLanguage?: string | null
  alternateTitles?: AlternateTitle[]
  released?: string | null
  catalogNumber?: string | null
  variousArtists?: boolean
  insertedAt?: number
  updatedAt?: number
  cover?: { id: number } | null
  /** As credited */
  artists?: CreditedLink[]
  formats?: Named[]
  media?: Named[]
  labels?: Named[]
}

export type ReleaseColumn =
  | "id"
  | "cover"
  | "title"
  | "artists"
  | "released"
  | "formats"
  | "media"
  | "labels"
  | "catalogNumber"
  | "variousArtists"
  | "added"
  | "updated"

const HEADS: Record<ReleaseColumn, string> = {
  id: "Id",
  cover: "",
  title: "Title",
  artists: "Artists",
  released: "Released",
  formats: "Format",
  media: "Media",
  labels: "Label",
  catalogNumber: "Cat#",
  variousArtists: "Various artists",
  added: "Added",
  updated: "Updated",
}

const SORTABLE = new Set<ReleaseColumn>(["title", "released"])

/** Checkboxes on the rows, for a bulk edit. */
export type Selection = {
  selected: Set<number>
  onChange: (ids: number[], selected: boolean) => void
}

/**
 * The release table: the release list's, also shown on label and artist pages
 * with fewer columns. `withArtists` names every artist in the title cell, as on
 * an artist page. With `selection`, each row gets a checkbox, and the header
 * one selects the page. Artists show as credited, so on an artist page the
 * title cell says when a release credits the artist under another name: "ALIKA
 * – virmalised". Under each title is an alternate title, the one `q` matched or
 * the first. A cover shows a play button on hover or focus, which plays the
 * release (frontend.md, "The player").
 */
export function ReleaseTable({
  releases,
  columns,
  sort,
  onSort,
  withArtists = false,
  selection,
  q,
}: {
  releases: ReleaseRow[]
  columns: ReleaseColumn[]
  sort?: string | null
  onSort?: (sort: string | null) => void
  withArtists?: boolean
  selection?: Selection
  /** The list's search, which picks the alternate title a row shows */
  q?: string | null
}) {
  const names = (items: Named[] | undefined) =>
    items?.map((i) => i.name).join(", ")
  const pageIds = releases.map((release) => release.id)
  const onPage = pageIds.filter((id) => selection?.selected.has(id)).length

  return (
    <Table>
      <TableHeader>
        <TableRow>
          {selection && (
            <TableHead className="w-8">
              <Checkbox
                aria-label="Select this page"
                checked={onPage > 0 && onPage === pageIds.length}
                indeterminate={onPage > 0 && onPage < pageIds.length}
                onCheckedChange={(on) => selection.onChange(pageIds, on)}
              />
            </TableHead>
          )}
          {columns.map((column) =>
            onSort && SORTABLE.has(column) ? (
              <SortableHead
                key={column}
                column={column}
                sort={sort ?? null}
                onSort={onSort}
              >
                {HEADS[column]}
              </SortableHead>
            ) : (
              <TableHead key={column}>{HEADS[column]}</TableHead>
            )
          )}
        </TableRow>
      </TableHeader>
      <TableBody>
        {releases.map((release) => (
          <TableRow key={release.id}>
            {selection && (
              <TableCell>
                <Checkbox
                  aria-label={`Select ${release.title}`}
                  checked={selection.selected.has(release.id)}
                  onCheckedChange={(on) => selection.onChange([release.id], on)}
                />
              </TableCell>
            )}
            {columns.map((column) => (
              <TableCell
                key={column}
                className={
                  column === "title" ||
                  column === "artists" ||
                  column === "labels"
                    ? "min-w-40 whitespace-normal"
                    : undefined
                }
              >
                {column === "id" && (
                  <span className="tabular-nums">{release.id}</span>
                )}
                {column === "cover" && (
                  <div className="group/cover relative size-16">
                    <Link to={`/releases/${release.id}`}>
                      <Cover file={release.cover} alt="" className="size-16" />
                    </Link>
                    <ReleasePlayButton
                      id={release.id}
                      title={release.title}
                      className="absolute right-1 bottom-1 opacity-0 group-hover/cover:opacity-100 focus-visible:opacity-100"
                    />
                  </div>
                )}
                {column === "title" && (
                  <>
                    {withArtists && release.artists?.length ? (
                      <>
                        <Names items={release.artists} to={artistUrl} /> –{" "}
                      </>
                    ) : null}
                    <Link
                      to={`/releases/${release.id}`}
                      className="font-medium hover:underline"
                    >
                      <Title
                        title={release.title}
                        language={release.titleLanguage}
                      />
                    </Link>
                    {release.alternateTitles && (
                      <ListedAlternateTitle
                        main={release.title}
                        titles={release.alternateTitles}
                        q={q}
                      />
                    )}
                  </>
                )}
                {column === "artists" && release.artists && (
                  <Names items={release.artists} to={artistUrl} />
                )}
                {column === "released" && formatReleased(release.released)}
                {column === "formats" && names(release.formats)}
                {column === "media" && names(release.media)}
                {column === "labels" && release.labels && (
                  <Names items={release.labels} to={labelUrl} />
                )}
                {column === "catalogNumber" && release.catalogNumber}
                {column === "variousArtists" &&
                  (release.variousArtists ? "Yes" : "")}
                {column === "added" &&
                  release.insertedAt &&
                  formatDateTime(release.insertedAt)}
                {column === "updated" &&
                  release.updatedAt &&
                  formatDateTime(release.updatedAt)}
              </TableCell>
            ))}
          </TableRow>
        ))}
      </TableBody>
    </Table>
  )
}
