import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import {
  type CreditLine,
  CreditsEditor,
  creditsInput,
  creditsValid,
} from "@/components/credits-editor"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { type CreditedLink, creditedOption } from "@/lib/credited"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

type Track = { id: number; label: string; credits: CreditLine[] }

/** The release's credit lines, then each track's. */
export function ReleaseCreditsForm({
  releaseId,
  submitLabel = "Save",
  onSaved,
}: {
  /** From the URL */
  releaseId: string
  submitLabel?: string
  onSaved: () => void
}) {
  const id = idParam(releaseId)
  // A form starts from fresh data (design §7.2)
  const { data: release, error } = useCommand(
    "releaseCredits",
    { id: id ?? 0 },
    { fresh: true, enabled: id !== null }
  )

  if (id === null || release === null) return <NotFound />
  if (error) return <QueryError error={error} title="Can't load the credits" />
  if (!release) return <Skeleton className="h-64 w-full" />
  const discs = new Set(release.tracks.map((t) => t.cdNumber)).size
  return (
    <Form
      key={release.id}
      releaseId={release.id}
      initialCredits={editable(release.credits)}
      initialTracks={release.tracks.map((track) => ({
        id: track.id,
        label: `${discs > 1 ? `${track.cdNumber}-` : ""}${track.trackNumber}. ${track.title}`,
        credits: editable(track.credits),
      }))}
      submitLabel={submitLabel}
      onSaved={onSaved}
    />
  )
}

function Form({
  releaseId,
  initialCredits,
  initialTracks,
  submitLabel,
  onSaved,
}: {
  releaseId: number
  initialCredits: CreditLine[]
  initialTracks: Track[]
  submitLabel: string
  onSaved: () => void
}) {
  const [credits, setCredits] = useState(initialCredits)
  const [tracks, setTracks] = useState(initialTracks)
  const save = useMutation({
    mutationFn: () =>
      call("setReleaseCredits", {
        id: releaseId,
        credits: creditsInput(credits),
        tracks: tracks.map((t) => ({
          trackId: t.id,
          credits: creditsInput(t.credits),
        })),
      }),
  })
  const valid =
    creditsValid(credits) && tracks.every((t) => creditsValid(t.credits))

  return (
    <form
      className="flex max-w-3xl flex-col gap-6"
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await save.mutateAsync()
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      {save.error && (
        <QueryError error={save.error} title="Can't save the credits" />
      )}
      <section className="flex flex-col gap-2">
        <h2 className="font-heading text-sm font-semibold">Release</h2>
        <CreditsEditor credits={credits} onChange={setCredits} />
      </section>
      {tracks.map((track) => (
        <section key={track.id} className="flex flex-col gap-2">
          <h2 className="text-xs font-medium">{track.label}</h2>
          <CreditsEditor
            credits={track.credits}
            onChange={(rows) =>
              setTracks((current) =>
                current.map((t) =>
                  t.id === track.id ? { ...t, credits: rows } : t
                )
              )
            }
          />
        </section>
      ))}
      <div>
        <Button type="submit" disabled={save.isPending || !valid}>
          {submitLabel}
        </Button>
      </div>
    </form>
  )
}

/** Credit lines as the editor's: each artist an option, with its credited name. */
function editable(
  credits: { role: string; artists: CreditedLink[] }[]
): CreditLine[] {
  return credits.map(({ role, artists }) => ({
    role,
    artists: artists.map(creditedOption),
  }))
}
