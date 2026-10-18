import { useMutation } from "@tanstack/react-query"
import type { ReactNode } from "react"
import { Link, useParams } from "react-router"

import { AlternateNames } from "@/components/alternate-names"
import { Button } from "@/components/ui/button"
import { ButtonLink } from "@/components/button-link"
import { Cover } from "@/components/cover"
import { CoverChooser } from "@/components/cover-chooser"
import { DeleteButton } from "@/components/delete-button"
import { FileGallery } from "@/components/file-gallery"
import { PER_PAGE } from "@/components/list/list-layout"
import { Pager } from "@/components/list/pager"
import { SearchBox } from "@/components/list/search-box"
import { artistUrl, CreditedAsNote, Names } from "@/components/names"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { ReleaseTable } from "@/components/release-table"
import { TabLinks } from "@/components/tab-links"
import { Title } from "@/components/titles"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { FileRefInput } from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import { formatReleased, roleKey } from "@/lib/format"
import { textProps } from "@/lib/languages"
import { call, idParam, queryClient, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** The artist's id, from the URL, or null when it isn't one. */
function useArtistId() {
  const { id = "" } = useParams()
  return { id, artistId: idParam(id) }
}

/**
 * The frame of every artist tab (slunk's design §7.7): the header with Edit and
 * Delete, the tabs with their counts, then the picture and name. It reads
 * `artistHeader`, which every tab shares, and shows the tab once its own
 * data is there.
 */
function ArtistFrame({
  error: tabError,
  loaded,
  children,
}: {
  /** The tab's own error */
  error?: Error | null
  /** Whether the tab's own data is there */
  loaded: boolean
  children: ReactNode
}) {
  const { id, artistId } = useArtistId()
  const { data: artist, error: headerError } = useCommand(
    "artistHeader",
    { id: artistId ?? 0 },
    { enabled: artistId !== null }
  )
  const error = headerError ?? tabError
  const deleteArtist = useMutation({
    mutationFn: (artistId: number) => call("deleteArtist", { id: artistId }),
  })
  const setPicture = useMutation({
    mutationFn: ({ id, picture }: { id: number; picture: FileRefInput }) =>
      call("setArtistPicture", { id, picture }),
  })
  if (artistId === null || artist === null) return <NotFound />
  const base = `/artists/${id}`

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Artists", to: "/artists" },
          {
            label: artist ? `${artist.name} (${artist.id})` : id,
            lang: artist?.nameLanguage,
          },
        ]}
      >
        {artist && (
          <>
            <ButtonLink variant="outline" to={`${base}/update`}>
              Edit
            </ButtonLink>
            <DeleteButton
              name={artist.name}
              blockers={artist.blockers}
              onDelete={() => deleteArtist.mutateAsync(artist.id)}
              then="/artists"
              afterDelete={() => {
                for (const name of ["artist", "artistHeader"]) {
                  queryClient.removeQueries({
                    queryKey: [name, { id: artist.id }],
                  })
                }
              }}
            />
          </>
        )}
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        {artist && (
          <TabLinks
            tabs={[
              { label: "Releases", to: base, count: artist.counts.releases },
              {
                label: "Appearances",
                to: `${base}/appearances`,
                count: artist.counts.appearances,
              },
              {
                label: "Credits",
                to: `${base}/credits`,
                count: artist.counts.credits,
              },
              {
                label: "Files",
                to: `${base}/files`,
                count: artist.counts.files,
              },
            ]}
          />
        )}
        {error ? (
          <QueryError error={error} title="Can't load the artist" />
        ) : !artist || !loaded ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <>
            <div className="flex items-center gap-4">
              <CoverChooser
                file={artist.picture}
                alt={artist.name}
                className="size-64"
                noun="picture"
                onChoose={(picture) =>
                  setPicture.mutateAsync({ id: artist.id, picture })
                }
              />
              <div className="flex flex-col gap-2">
                <h1
                  className="font-heading text-2xl font-semibold"
                  {...textProps(artist.nameLanguage)}
                >
                  {artist.name}
                </h1>
                <AlternateNames names={artist.alternateNames} />
              </div>
            </div>
            {children}
          </>
        )}
      </div>
    </>
  )
}

/** `/artists/:id`: the releases the artist is a release artist on. */
export function ArtistReleases() {
  const { artistId } = useArtistId()
  const list = useListParams()
  const format = idParam(list.params.get("format") ?? undefined)
  const { data, error } = useCommand(
    "artistReleases",
    {
      id: artistId ?? 0,
      page: list.page,
      perPage: PER_PAGE,
      // Oldest first, undated last
      sort: list.sort ?? "released",
      q: list.q,
      format: format ?? undefined,
    },
    { enabled: artistId !== null }
  )
  // The frame's, from the cache
  const { data: artist } = useCommand(
    "artistHeader",
    { id: artistId ?? 0 },
    { enabled: artistId !== null }
  )

  return (
    <ArtistFrame error={error} loaded={!!data}>
      {data && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Button
              size="sm"
              variant={format ? "ghost" : "secondary"}
              onClick={() => list.update({ format: null })}
            >
              All {artist?.counts.releases}
            </Button>
            {data.formatCounts.map(({ format: f, count }) => (
              <Button
                key={f.id}
                size="sm"
                variant={format === f.id ? "secondary" : "ghost"}
                onClick={() => list.update({ format: String(f.id) })}
              >
                {f.name} {count}
              </Button>
            ))}
            <div className="ml-auto flex items-center gap-4">
              <SearchBox
                value={list.q}
                placeholder="Search releases"
                onSearch={(q) => list.update({ q }, { replace: true })}
              />
              <Pager
                page={list.page}
                perPage={PER_PAGE}
                total={data.releases.total}
                onPage={(page) => list.update({ page })}
              />
            </div>
          </div>
          <ReleaseTable
            releases={data.releases.items}
            columns={[
              "cover",
              "title",
              "formats",
              "media",
              "labels",
              "catalogNumber",
              "released",
            ]}
            withArtists
            q={list.q}
            sort={list.sort ?? "released"}
            onSort={(sort) => list.update({ sort })}
          />
        </>
      )}
    </ArtistFrame>
  )
}

/** `/artists/:id/appearances`: releases where the artist is on tracks only. */
export function ArtistAppearances() {
  const { artistId } = useArtistId()
  const { data: appearances, error } = useCommand(
    "artistAppearances",
    { id: artistId ?? 0 },
    { enabled: artistId !== null }
  )

  return (
    <ArtistFrame error={error} loaded={!!appearances}>
      {appearances && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead />
              <TableHead>Release</TableHead>
              <TableHead>Tracks</TableHead>
              <TableHead>Released</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {appearances.map(({ release, tracks }) => (
              <TableRow key={release.id}>
                <TableCell>
                  <Link to={`/releases/${release.id}`}>
                    <Cover file={release.cover} alt="" className="size-16" />
                  </Link>
                </TableCell>
                <TableCell className="whitespace-normal">
                  <Names items={release.artists} to={artistUrl} /> –{" "}
                  <Link
                    to={`/releases/${release.id}`}
                    className="font-medium hover:underline"
                  >
                    <Title
                      title={release.title}
                      language={release.titleLanguage}
                    />
                  </Link>
                </TableCell>
                <TableCell className="whitespace-normal">
                  {tracks.map((track, index) => {
                    const own = (links: typeof track.features) =>
                      links.find((link) => link.artist.id === artistId)
                    const featured = own(track.features)
                    return (
                      <span key={track.id}>
                        {index > 0 && ", "}
                        <Link
                          to={`/tracks/${track.id}`}
                          className="hover:underline"
                        >
                          <Title
                            title={track.title}
                            language={
                              track.titleLanguage ?? release.titleLanguage
                            }
                          />
                        </Link>
                        {featured && " (feat.)"}
                        <CreditedAsNote
                          creditedAs={
                            (featured ?? own(track.artists))?.creditedAs
                          }
                        />
                      </span>
                    )
                  })}
                </TableCell>
                <TableCell>{formatReleased(release.released)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </ArtistFrame>
  )
}

/**
 * `/artists/:id/credits`: the credit lines the artist is on, grouped by role
 * ignoring case, so "Mixed By" and "Mixed by" share a group.
 */
export function ArtistCredits() {
  const { artistId } = useArtistId()
  const { data, error } = useCommand(
    "artistCredits",
    { id: artistId ?? 0 },
    { enabled: artistId !== null }
  )
  const credits = [...(data ?? [])].sort(
    (a, b) =>
      roleKey(a.role).localeCompare(roleKey(b.role)) ||
      (a.release.released ?? "9999").localeCompare(b.release.released ?? "9999")
  )

  return (
    <ArtistFrame error={error} loaded={!!data}>
      {data && (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Role</TableHead>
              <TableHead>Release</TableHead>
              <TableHead>Track</TableHead>
              <TableHead>Released</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {credits.map((credit, index) => (
              <TableRow key={index}>
                <TableCell>
                  {(index === 0 ||
                    roleKey(credit.role) !==
                      roleKey(credits[index - 1].role)) &&
                    credit.role}
                </TableCell>
                <TableCell>
                  <Link
                    to={`/releases/${credit.release.id}`}
                    className="hover:underline"
                  >
                    <Title
                      title={credit.release.title}
                      language={credit.release.titleLanguage}
                    />
                  </Link>
                  {!credit.track && (
                    <CreditedAsNote creditedAs={credit.creditedAs} />
                  )}
                </TableCell>
                <TableCell>
                  {credit.track && (
                    <>
                      <Link
                        to={`/tracks/${credit.track.id}`}
                        className="hover:underline"
                      >
                        <Title
                          title={credit.track.title}
                          language={
                            credit.track.titleLanguage ??
                            credit.release.titleLanguage
                          }
                        />
                      </Link>
                      <CreditedAsNote creditedAs={credit.creditedAs} />
                    </>
                  )}
                </TableCell>
                <TableCell>{formatReleased(credit.release.released)}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
    </ArtistFrame>
  )
}

/** `/artists/:id/files`: the picture, then the linked files. */
export function ArtistFiles() {
  const { artistId } = useArtistId()
  // The form's command has the picture and the files, with their names
  const { data: artist, error } = useCommand(
    "artist",
    { id: artistId ?? 0 },
    { enabled: artistId !== null }
  )

  return (
    <ArtistFrame error={error} loaded={!!artist}>
      {artist && (
        <FileGallery
          primary={artist.picture}
          primaryLabel="Picture"
          links={artist.files}
        />
      )}
    </ArtistFrame>
  )
}
