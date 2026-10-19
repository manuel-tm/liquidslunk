import { useMutation } from "@tanstack/react-query"
import { PauseIcon, PlayIcon } from "lucide-react"
import { Link, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { CreditLines } from "@/components/credit-lines"
import { DeleteButton } from "@/components/delete-button"
import { artistUrl, Names } from "@/components/names"
import { PageHeader } from "@/components/page-header"
import { QueueMenu } from "@/components/player/play-buttons"
import { usePlayer } from "@/components/player/provider"
import { QueryError } from "@/components/query-error"
import { TagValues } from "@/components/tag-values"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { AlternateTitleLines, Title } from "@/components/titles"
import { formatDuration } from "@/lib/format"
import { call, idParam, queryClient, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** `/tracks/:id`: a track, with its audio, credits, tags and files. */
export function TrackRead() {
  const { id = "" } = useParams()
  const trackId = idParam(id)
  const { data: track, error } = useCommand(
    "trackPage",
    { id: trackId ?? 0 },
    { enabled: trackId !== null }
  )
  const deleteTrack = useMutation({
    mutationFn: (trackId: number) => call("deleteTrack", { id: trackId }),
  })

  if (trackId === null || track === null) return <NotFound />

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Tracks", to: "/tracks" },
          {
            label: track ? `${track.title} (${track.id})` : id,
            lang: track?.titleLanguage ?? track?.release.titleLanguage,
          },
        ]}
      >
        {track && (
          <>
            <ButtonLink variant="outline" to={`/tracks/${id}/update`}>
              Edit
            </ButtonLink>
            <DeleteButton
              name={track.title}
              blockers={[]}
              detail="Its artists, credits and file links go with it. Its audio file stays registered."
              onDelete={() => deleteTrack.mutateAsync(track.id)}
              then="/tracks"
              afterDelete={() => {
                for (const name of ["trackPage", "trackForm"]) {
                  queryClient.removeQueries({
                    queryKey: [name, { id: track.id }],
                  })
                }
              }}
            />
          </>
        )}
      </PageHeader>
      <div className="flex max-w-3xl flex-col gap-4 p-4 text-xs">
        {error ? (
          <QueryError error={error} title="Can't load the track" />
        ) : !track ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <>
            <div>
              <h1 className="font-heading text-2xl font-semibold">
                <Title
                  title={track.title}
                  language={track.titleLanguage ?? track.release.titleLanguage}
                />
              </h1>
              {/* A line per alternate title, with the performers in its language */}
              <AlternateTitleLines
                titles={track.alternateTitles}
                artists={track.performers}
                className="font-heading text-base"
              />
              <p>
                {track.performers.length > 0 && (
                  <>
                    by <Names items={track.performers} to={artistUrl} />
                  </>
                )}
                {track.features.length > 0 && (
                  <>
                    {" "}
                    (feat. <Names items={track.features} to={artistUrl} />)
                  </>
                )}
              </p>
            </div>
            <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2">
              <dt className="text-muted-foreground">Release</dt>
              <dd>
                <Link
                  to={`/releases/${track.release.id}`}
                  className="hover:underline"
                >
                  <Title
                    title={track.release.title}
                    language={track.release.titleLanguage}
                  />
                </Link>
                , disc {track.cdNumber}, track {track.trackNumber}
              </dd>
              <dt className="text-muted-foreground">Duration</dt>
              <dd className="tabular-nums">
                {formatDuration(track.durationMs) || "Unknown"}
              </dd>
              <dt className="text-muted-foreground">Audio file</dt>
              <dd className="flex flex-col gap-2">
                <Link
                  to={`/files/${track.file.id}`}
                  className="font-mono break-all hover:underline"
                >
                  {track.file.path}
                </Link>
                <div className="flex items-center gap-1">
                  <PlayButton id={track.id} />
                  <QueueMenu
                    trackIds={[track.id]}
                    name={track.title}
                    className="size-7"
                  />
                </div>
              </dd>
              {track.tags.map(({ tag, values }) => (
                <div key={`${tag.type}:${tag.id}`} className="contents">
                  <dt className="text-muted-foreground">{tag.name}</dt>
                  <dd>
                    <TagValues
                      type={tag.type}
                      tagId={tag.id}
                      values={values}
                      list="/tracks"
                    />
                  </dd>
                </div>
              ))}
              {track.credits.length > 0 && (
                <>
                  <dt className="text-muted-foreground">Credits</dt>
                  <dd>
                    <CreditLines credits={track.credits} />
                  </dd>
                </>
              )}
              {track.files.length > 0 && (
                <>
                  <dt className="text-muted-foreground">Linked files</dt>
                  <dd className="flex flex-col gap-1">
                    {track.files.map((link) => (
                      <span key={link.file.id}>
                        <Link
                          to={`/files/${link.file.id}`}
                          className="font-mono hover:underline"
                        >
                          {link.file.path}
                        </Link>
                        {link.note && (
                          <span className="text-muted-foreground">
                            {" "}
                            · {link.note}
                          </span>
                        )}
                      </span>
                    ))}
                  </dd>
                </>
              )}
            </dl>
          </>
        )}
      </div>
    </>
  )
}

/** Plays the track alone, or pauses it while it plays (2_player.md §5.2). */
function PlayButton({ id }: { id: number }) {
  const player = usePlayer()
  const playing = player.current?.trackId === id && player.playing
  return (
    <Button
      variant="outline"
      size="sm"
      onClick={() =>
        player.current?.trackId === id ? player.toggle() : player.play([id])
      }
    >
      {playing ? (
        <PauseIcon data-icon="inline-start" />
      ) : (
        <PlayIcon data-icon="inline-start" />
      )}
      {playing ? "Pause" : "Play"}
    </Button>
  )
}
