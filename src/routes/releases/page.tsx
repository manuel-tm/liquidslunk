import { useMutation } from "@tanstack/react-query"
import { PauseIcon, PlayIcon } from "lucide-react"
import { type ReactNode, useRef, useState } from "react"
import { Link, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { commonFolder, CoverChooser } from "@/components/cover-chooser"
import { CreditLines } from "@/components/credit-lines"
import { DeleteButton } from "@/components/delete-button"
import { artistUrl, labelUrl, Names } from "@/components/names"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { TagValues } from "@/components/tag-values"
import { AlternateTitleLines, Title } from "@/components/titles"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import type { FileRefInput, ReleaseView } from "@/bindings"
import { formatDuration, formatReleased } from "@/lib/format"
import { call, fileUrl, idParam, queryClient, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

type Release = ReleaseView

/** The queries of one release, which a delete removes. */
const RELEASE_QUERIES = [
  "releasePage",
  "releaseHeader",
  "release",
  "releaseFiles",
  "releaseCredits",
  "releaseTags",
]

// The browser remembers whether credits and alternate titles are hidden
const HIDE_CREDITS = "slunk.release.hideCredits"
const HIDE_TITLES = "slunk.release.hideAlternateTitles"

/** A switch the browser remembers, under `key`. */
function useRemembered(key: string) {
  const [on, setOn] = useState(() => {
    try {
      return localStorage.getItem(key) === "1"
    } catch {
      return false
    }
  })
  function toggle() {
    setOn(!on)
    try {
      localStorage.setItem(key, on ? "0" : "1")
    } catch {
      // Remembering is a convenience
    }
  }
  return [on, toggle] as const
}

/** `/releases/:id`, laid out like a Discogs release page. */
export function ReleasePage() {
  const { id = "" } = useParams()
  const releaseId = idParam(id)
  const { data: release, error } = useCommand(
    "releasePage",
    { id: releaseId ?? 0 },
    { enabled: releaseId !== null }
  )
  const deleteRelease = useMutation({
    mutationFn: (id: number) => call("deleteRelease", { id }),
  })

  if (releaseId === null || release === null) return <NotFound />

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Releases", to: "/releases" },
          {
            label: release ? `${release.title} (${release.id})` : id,
            lang: release?.titleLanguage,
          },
        ]}
      >
        {release && (
          <>
            <ButtonLink variant="outline" to={`/releases/${id}/update`}>
              Edit
            </ButtonLink>
            <DeleteButton
              name={release.title}
              blockers={release.blockers}
              detail="Its artists, labels, formats, media, credits, tags and file links go with it. Its files stay registered."
              onDelete={() => deleteRelease.mutateAsync(release.id)}
              then="/releases"
              afterDelete={() => {
                for (const name of RELEASE_QUERIES) {
                  queryClient.removeQueries({
                    queryKey: [name, { id: release.id }],
                  })
                }
              }}
            />
          </>
        )}
      </PageHeader>
      <div className="flex max-w-5xl flex-col gap-8 p-4 text-xs">
        {error ? (
          <QueryError error={error} title="Can't load the release" />
        ) : !release ? (
          <Skeleton className="h-96 w-full" />
        ) : (
          <>
            <Heading release={release} />
            <Tracklist release={release} />
            {release.credits.length > 0 && (
              <section className="flex flex-col gap-2">
                <h2 className="font-heading text-base font-semibold">
                  Credits
                </h2>
                <CreditLines credits={release.credits} />
              </section>
            )}
          </>
        )}
      </div>
    </>
  )
}

function Heading({ release }: { release: Release }) {
  const setCover = useMutation({
    mutationFn: (cover: FileRefInput) =>
      call("setReleaseCover", { id: release.id, cover }),
  })
  const filterLinks = (items: { id: number; name: string }[], filter: string) =>
    items.map((item, index) => (
      <span key={item.id}>
        {index > 0 && ", "}
        <Link to={`/releases?${filter}=${item.id}`} className="hover:underline">
          {item.name}
        </Link>
      </span>
    ))

  // A row with nothing in it is hidden. Tags come last, a row each.
  const facts: [string, ReactNode][] = [
    [
      "Label",
      (release.labels.length > 0 || release.catalogNumber) && (
        <>
          <Names items={release.labels} to={labelUrl} />
          {release.catalogNumber && <> – {release.catalogNumber}</>}
        </>
      ),
    ],
    [
      "Format",
      release.formats.length > 0 && filterLinks(release.formats, "format"),
    ],
    ["Media", release.media.length > 0 && filterLinks(release.media, "medium")],
    ["Released", formatReleased(release.released)],
    ...release.tags.map(({ tag, values }): [string, ReactNode] => [
      tag.name,
      <TagValues type={tag.type} tagId={tag.id} values={values} />,
    ]),
  ]

  return (
    <div className="flex flex-wrap gap-6">
      <div className="flex w-64 flex-col gap-1">
        <CoverChooser
          file={release.cover}
          alt={`${release.title} cover`}
          className="size-64"
          noun="cover"
          // A cover usually sits with the tracks
          folder={commonFolder(release.tracks.map((track) => track.file.path))}
          onChoose={(cover) => setCover.mutateAsync(cover)}
        />
        <Link to={`/releases/${release.id}/files`} className="hover:underline">
          Files ({release.fileCount})
        </Link>
      </div>
      <div className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <h1 className="font-heading text-2xl font-semibold">
            {release.artists.length > 0 && (
              <>
                <Names items={release.artists} to={artistUrl} /> –{" "}
              </>
            )}
            <Title title={release.title} language={release.titleLanguage} />
          </h1>
          {/* A line per alternate title, with the artists in its language */}
          <AlternateTitleLines
            titles={release.alternateTitles}
            artists={release.artists}
            className="font-heading text-base"
          />
        </div>
        <dl className="grid grid-cols-[minmax(5rem,auto)_1fr] gap-x-2 gap-y-1">
          {facts
            .filter(([, value]) => value)
            .map(([label, value], index) => (
              // Labels can repeat, since a tag can be named "Format"
              <div key={index} className="contents">
                <dt className="text-muted-foreground">{label}:</dt>
                <dd>{value}</dd>
              </div>
            ))}
        </dl>
      </div>
    </div>
  )
}

/**
 * Grouped by disc. On a various-artists release, each row starts with the
 * track's artists; a regular release only shows features.
 */
function Tracklist({ release }: { release: Release }) {
  const [hideCredits, toggleCredits] = useRemembered(HIDE_CREDITS)
  const [hideTitles, toggleTitles] = useRemembered(HIDE_TITLES)
  const [playing, setPlaying] = useState<number | null>(null)
  const audio = useRef<HTMLAudioElement>(null)
  const discs = [...new Set(release.tracks.map((track) => track.cdNumber))]
  const titles = new Map(
    release.discTitles.map((disc) => [disc.cdNumber, disc])
  )
  // Tracks and discs without their own language take the release's
  const language = release.titleLanguage
  const hasAlternateTitles =
    release.tracks.some((track) => track.alternateTitles.length > 0) ||
    release.discTitles.some((disc) => disc.alternateTitles.length > 0)

  function play(track: Release["tracks"][number]) {
    const element = audio.current
    if (!element) return
    if (playing === track.id) {
      element.pause()
      setPlaying(null)
      return
    }
    element.src = fileUrl(track.file.id)
    void element.play()
    setPlaying(track.id)
  }

  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <h2 className="font-heading text-base font-semibold">Tracklist</h2>
        <div className="flex">
          {hasAlternateTitles && (
            <Button variant="ghost" size="sm" onClick={toggleTitles}>
              {hideTitles ? "Show alternate titles" : "Hide alternate titles"}
            </Button>
          )}
          {release.tracks.some((track) => track.credits.length > 0) && (
            <Button variant="ghost" size="sm" onClick={toggleCredits}>
              {hideCredits ? "Show credits" : "Hide credits"}
            </Button>
          )}
        </div>
      </div>
      <audio ref={audio} onEnded={() => setPlaying(null)} className="hidden" />
      {release.tracks.length === 0 && (
        <p className="text-muted-foreground">
          No tracks yet. Add them from Edit, Tracks.
        </p>
      )}
      {discs.map((disc) => (
        <div key={disc} className="flex flex-col">
          {(discs.length > 1 || titles.has(disc)) && (
            <h3 className="border-b py-1 font-medium text-muted-foreground">
              CD {disc}
              {titles.has(disc) && (
                <>
                  {" · "}
                  <Title
                    title={titles.get(disc)!.title}
                    language={titles.get(disc)!.language ?? language}
                    className="text-foreground"
                  />
                  {!hideTitles && (
                    <AlternateTitleLines
                      titles={titles.get(disc)!.alternateTitles}
                      className="font-normal"
                    />
                  )}
                </>
              )}
            </h3>
          )}
          {release.tracks
            .filter((track) => track.cdNumber === disc)
            .map((track) => (
              <div
                key={track.id}
                className="grid grid-cols-[2rem_1fr_auto_auto] items-start gap-2 border-b py-1.5"
              >
                <span className="text-muted-foreground tabular-nums">
                  {track.trackNumber}
                </span>
                <div className="flex flex-col gap-1">
                  <span>
                    {release.variousArtists && track.artists.length > 0 && (
                      <>
                        <Names items={track.artists} to={artistUrl} /> –{" "}
                      </>
                    )}
                    <Link
                      to={`/tracks/${track.id}`}
                      className="hover:underline"
                    >
                      <Title
                        title={track.title}
                        language={track.titleLanguage ?? language}
                      />
                    </Link>
                    {track.features.length > 0 && (
                      <>
                        {" "}
                        (feat. <Names items={track.features} to={artistUrl} />)
                      </>
                    )}
                  </span>
                  {!hideTitles && (
                    <AlternateTitleLines
                      titles={track.alternateTitles}
                      // Where the row names the track's artists
                      artists={
                        release.variousArtists && track.artists.length > 0
                          ? track.artists
                          : undefined
                      }
                    />
                  )}
                  {!hideCredits && track.credits.length > 0 && (
                    <div className="pl-4">
                      <CreditLines credits={track.credits} />
                    </div>
                  )}
                </div>
                <span className="tabular-nums">
                  {formatDuration(track.durationMs)}
                </span>
                <Button
                  variant="ghost"
                  size="icon-xs"
                  aria-label={
                    playing === track.id
                      ? `Pause ${track.title}`
                      : `Play ${track.title}`
                  }
                  onClick={() => play(track)}
                >
                  {playing === track.id ? <PauseIcon /> : <PlayIcon />}
                </Button>
              </div>
            ))}
        </div>
      ))}
      {release.durationMs !== null && release.tracks.length > 0 && (
        <p className="text-right">
          <span className="text-muted-foreground">Total </span>
          <span className="tabular-nums">
            {formatDuration(release.durationMs)}
          </span>
        </p>
      )}
    </section>
  )
}
