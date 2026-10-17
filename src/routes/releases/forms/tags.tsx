import { useMutation, useQuery } from "@apollo/client/react"
import { useState } from "react"

import { QueryError } from "@/components/query-error"
import {
  type TagRow,
  TagsEditor,
  tagRows,
  tagValuesInput,
} from "@/components/tags-editor"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import { graphql } from "@/gql"
import { NotFound } from "@/routes/not-found"

const ReleaseTagsFormQuery = graphql(`
  query ReleaseTagsForm($id: ID!) {
    release(id: $id) {
      id
      tags {
        tag {
          id
          type
          name
          multiple
        }
        values
      }
    }
  }
`)

const SetReleaseTags = graphql(`
  mutation SetReleaseTags($id: ID!, $tags: [TagValuesInput!]!) {
    setReleaseTags(id: $id, tags: $tags) {
      id
      tags {
        tag {
          id
          type
          name
          multiple
        }
        values
      }
    }
  }
`)

/** A release's Tags tab (slunk's design §7.6), which the import's last step reuses. */
export function ReleaseTagsForm({
  releaseId,
  submitLabel = "Save",
  onSaved,
}: {
  releaseId: string
  submitLabel?: string
  onSaved: () => void
}) {
  const { data, error } = useQuery(ReleaseTagsFormQuery, {
    variables: { id: releaseId },
    fetchPolicy: "network-only",
  })

  if (error) return <QueryError error={error} title="Can't load the tags" />
  if (!data) return <Skeleton className="h-64 w-full max-w-2xl" />
  if (!data.release) return <NotFound />
  return (
    <Form
      releaseId={releaseId}
      initial={tagRows(data.release.tags)}
      submitLabel={submitLabel}
      onSaved={onSaved}
    />
  )
}

function Form({
  releaseId,
  initial,
  submitLabel,
  onSaved,
}: {
  releaseId: string
  initial: TagRow[]
  submitLabel: string
  onSaved: () => void
}) {
  const [rows, setRows] = useState(initial)
  const [save, { loading, error }] = useMutation(SetReleaseTags)

  return (
    <form
      className="flex max-w-2xl flex-col gap-6"
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await save({
            variables: { id: releaseId, tags: tagValuesInput(rows) },
          })
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      {error && <QueryError error={error} title="Can't save the tags" />}
      <TagsEditor rows={rows} onChange={setRows} />
      <div>
        <Button type="submit" disabled={loading}>
          {submitLabel}
        </Button>
      </div>
    </form>
  )
}
