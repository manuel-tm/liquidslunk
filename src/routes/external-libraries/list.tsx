import { FolderPlusIcon, PlusIcon } from "lucide-react"
import { Link } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { ExternalKindIcon } from "@/components/external-libraries"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { KIND_LABELS } from "@/lib/external"
import { formatDateTime } from "@/lib/format"
import { useCommand } from "@/lib/query"

/**
 * `/external-libraries`: every external library, by name, with where it goes
 * and whether that's there (4_external_libraries.md §11).
 */
export function ExternalLibraryList() {
  const { data: libraries, error } = useCommand("externalLibraries")

  return (
    <>
      <PageHeader crumbs={[{ label: "External libraries" }]}>
        <ButtonLink variant="outline" to="/external-libraries/create?kind=ipod">
          <PlusIcon data-icon="inline-start" />
          New iPod library…
        </ButtonLink>
        <ButtonLink to="/external-libraries/create?kind=folder">
          <FolderPlusIcon data-icon="inline-start" />
          New folder library…
        </ButtonLink>
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        <p className="max-w-3xl text-xs text-muted-foreground">
          A selection of the library, for a player that holds less than the
          library does: releases, artists, tracks and tag values, converted to a
          format the player plays, and tagged from the catalog. A folder library
          is for anything that reads a folder of music, like a phone or a card.
          An iPod library is for an iPod with video, 5th or 5.5th generation.
        </p>
        {error ? (
          <QueryError error={error} title="Can't load the external libraries" />
        ) : !libraries ? (
          <Skeleton className="h-32 w-full" />
        ) : libraries.length === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>No external libraries yet</EmptyTitle>
              <EmptyDescription>
                Make a folder library or an iPod library, then choose what goes
                in it.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Name</TableHead>
                <TableHead>Kind</TableHead>
                <TableHead>Destination</TableHead>
                <TableHead>Connected</TableHead>
                <TableHead>Last synced</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {libraries.map((library) => (
                <TableRow key={library.id}>
                  <TableCell>
                    <Link
                      to={`/external-libraries/${library.id}`}
                      className="inline-flex items-center gap-1.5 hover:underline"
                    >
                      <ExternalKindIcon
                        kind={library.kind}
                        className="size-3.5 text-muted-foreground"
                      />
                      {library.name}
                    </Link>
                  </TableCell>
                  <TableCell>{KIND_LABELS[library.kind]}</TableCell>
                  <TableCell className="font-mono whitespace-normal">
                    {library.folder ?? "An iPod"}
                  </TableCell>
                  <TableCell>{library.connected ? "Yes" : "No"}</TableCell>
                  <TableCell>
                    {library.lastSyncedAt
                      ? formatDateTime(library.lastSyncedAt)
                      : "Never"}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>
    </>
  )
}
