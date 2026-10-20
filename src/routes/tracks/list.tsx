import { Link } from "react-router"

import { ListLayout, PER_PAGE } from "@/components/list/list-layout"
import { SortableHead } from "@/components/list/sortable-head"
import { artistUrl, Names } from "@/components/names"
import { QueueMenu, TrackPlayButton } from "@/components/player/play-buttons"
import { ReleasePicker } from "@/components/pickers"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { ListedAlternateTitle, Title } from "@/components/titles"
import { TagFilters } from "@/components/tag-filters"
import { useListParams } from "@/hooks/use-list-params"
import { idParam, useCommand } from "@/lib/query"
import { tagFilterInput, tagFiltersOf } from "@/lib/tags"
import { formatDateTime, formatDuration } from "@/lib/format"

/**
 * `/tracks`, with tag filters like the release list's. A row's play button
 * plays the page's rows from it, so a filtered list plays like a playlist
 * (frontend.md, "The player").
 */
export function TrackList() {
  const list = useListParams()
  const tagFilters = tagFiltersOf(list.params)
  const { data, error } = useCommand("tracks", {
    page: list.page,
    perPage: PER_PAGE,
    sort: list.sort,
    q: list.q,
    release: idParam(list.params.get("release") ?? undefined) ?? undefined,
    tags: tagFilters.map(tagFilterInput),
  })
  // Options' ids are strings, as the pickers' are
  const filterRelease = data?.filterRelease
  const pageIds = data?.items.map((track) => track.id) ?? []

  return (
    <ListLayout
      title="Tracks"
      newTo="/tracks/create"
      list={list}
      searchPlaceholder="Search titles"
      filters={
        <>
          <div className="w-64">
            <ReleasePicker
              value={
                filterRelease
                  ? { id: String(filterRelease.id), name: filterRelease.name }
                  : null
              }
              placeholder="Any release"
              onChange={(option) =>
                list.update({ release: option?.id ?? null })
              }
            />
          </div>
          <TagFilters
            filters={tagFilters}
            onAdd={list.append}
            onRemove={(filter) => list.remove(filter.key, filter.value)}
          />
        </>
      }
      total={data?.total}
      error={error ?? undefined}
      emptyText="No tracks yet. Tracks are created from registered audio files."
    >
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-14">
              <span className="sr-only">Play</span>
            </TableHead>
            <TableHead>Id</TableHead>
            <SortableHead
              column="title"
              sort={list.sort}
              onSort={(sort) => list.update({ sort })}
            >
              Title
            </SortableHead>
            <TableHead className="text-right">Duration</TableHead>
            <TableHead className="text-right">Disc</TableHead>
            <TableHead className="text-right">No.</TableHead>
            <TableHead>Release</TableHead>
            <TableHead>Artists</TableHead>
            <TableHead>File</TableHead>
            <TableHead>Added</TableHead>
            <TableHead>Updated</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {data?.items.map((track, index) => (
            <TableRow key={track.id}>
              <TableCell>
                <div className="flex">
                  <TrackPlayButton
                    trackIds={pageIds}
                    index={index}
                    title={track.title}
                  />
                  <QueueMenu trackIds={[track.id]} name={track.title} />
                </div>
              </TableCell>
              <TableCell className="tabular-nums">{track.id}</TableCell>
              <TableCell className="min-w-40 whitespace-normal">
                <Link to={`/tracks/${track.id}`} className="hover:underline">
                  <Title
                    title={track.title}
                    language={
                      track.titleLanguage ?? track.release.titleLanguage
                    }
                  />
                </Link>
                <ListedAlternateTitle
                  main={track.title}
                  titles={track.alternateTitles}
                  q={list.q}
                />
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {formatDuration(track.durationMs)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {track.cdNumber}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {track.trackNumber}
              </TableCell>
              <TableCell className="whitespace-normal">
                <Link
                  to={`/releases/${track.release.id}`}
                  className="hover:underline"
                >
                  <Title
                    title={track.release.title}
                    language={track.release.titleLanguage}
                  />
                </Link>
              </TableCell>
              <TableCell className="whitespace-normal">
                <Names items={track.performers} to={artistUrl} />
              </TableCell>
              <TableCell className="min-w-48 whitespace-normal">
                <Link
                  to={`/files/${track.file.id}`}
                  className="font-mono hover:underline"
                >
                  {track.file.path}
                </Link>
              </TableCell>
              <TableCell>{formatDateTime(track.insertedAt)}</TableCell>
              <TableCell>{formatDateTime(track.updatedAt)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </ListLayout>
  )
}
