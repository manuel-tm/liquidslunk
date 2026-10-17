import { useMutation, useQuery } from "@apollo/client/react"
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
import { graphql } from "@/gql"
import { type CreditedLink, creditedOption } from "@/lib/credited"

const ReleaseCreditsFormQuery = graphql(`
  query ReleaseCreditsForm($id: ID!) {
    release(id: $id) {
      id
      title
      credits {
        role
        artists {
          artist {
            id
            name
            nameLanguage
          }
          creditedAs {
            name
            language
          }
        }
      }
      tracks {
        id
        cdNumber
        trackNumber
        title
        credits {
          role
          artists {
            artist {
              id
              name
              nameLanguage
            }
            creditedAs {
              name
              language
            }
          }
        }
      }
    }
  }
`)

const SetReleaseCredits = graphql(`
  mutation SetReleaseCredits($id: ID!, $input: ReleaseCreditsInput!) {
    setReleaseCredits(id: $id, input: $input) {
      id
    }
  }
`)

type Track = { id: string; label: string; credits: CreditLine[] }

/** The release's credit lines, then each track's. */
export function ReleaseCreditsForm({
  releaseId,
  submitLabel = "Save",
  onSaved,
}: {
  releaseId: string
  submitLabel?: string
  onSaved: () => void
}) {
  const { data, error } = useQuery(ReleaseCreditsFormQuery, {
    variables: { id: releaseId },
    fetchPolicy: "network-only",
  })

  if (error) return <QueryError error={error} title="Can't load the credits" />
  if (!data?.release) return <Skeleton className="h-64 w-full" />
  const discs = new Set(data.release.tracks.map((t) => t.cdNumber)).size
  return (
    <Form
      releaseId={releaseId}
      initialCredits={editable(data.release.credits)}
      initialTracks={data.release.tracks.map((track) => ({
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
  releaseId: string
  initialCredits: CreditLine[]
  initialTracks: Track[]
  submitLabel: string
  onSaved: () => void
}) {
  const [credits, setCredits] = useState(initialCredits)
  const [tracks, setTracks] = useState(initialTracks)
  const [save, { loading, error }] = useMutation(SetReleaseCredits)
  const valid =
    creditsValid(credits) && tracks.every((t) => creditsValid(t.credits))

  return (
    <form
      className="flex max-w-3xl flex-col gap-6"
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await save({
            variables: {
              id: releaseId,
              input: {
                credits: creditsInput(credits),
                tracks: tracks.map((t) => ({
                  trackId: t.id,
                  credits: creditsInput(t.credits),
                })),
              },
            },
          })
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      {error && <QueryError error={error} title="Can't save the credits" />}
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
        <Button type="submit" disabled={loading || !valid}>
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
