import { useMutation } from "@tanstack/react-query"
import { CircleAlertIcon } from "lucide-react"
import { useState } from "react"
import { Link, useNavigate, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { type TagFieldValues, TagFields } from "@/components/tag-fields"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { FieldGroup } from "@/components/ui/field"
import { Skeleton } from "@/components/ui/skeleton"
import type { TagDetails } from "@/bindings"
import { call, idParam, useCommand } from "@/lib/query"
import { isTagType } from "@/lib/tags"
import { NotFound } from "@/routes/not-found"

/**
 * `/tags/create` and `/tags/:type/:id/update`: type (only when creating), name,
 * and single or multiple.
 */
export function TagForm() {
  const { type, id } = useParams()
  const editing = id !== undefined
  const tagId = idParam(id)
  // A form starts from fresh data (frontend.md, "Commands")
  const { data: tag, error } = useCommand(
    "tag",
    { type: isTagType(type) ? type : "text", id: tagId ?? 0 },
    { fresh: true, enabled: editing && isTagType(type) && tagId !== null }
  )

  if (editing && (!isTagType(type) || tagId === null || tag === null)) {
    return <NotFound />
  }

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Tags", to: "/tags" },
          ...(editing
            ? [
                {
                  label: tag ? `${tag.name} (${tag.id})` : id,
                  to: `/tags/${type}/${id}`,
                },
                { label: "Update" },
              ]
            : [{ label: "New" }]),
        ]}
      />
      <div className="max-w-md p-4">
        {error ? (
          <QueryError error={error} title="Can't load the tag" />
        ) : editing && !tag ? (
          <Skeleton className="h-48 w-full" />
        ) : (
          // Keyed, so a form for another tag, or a new one, starts afresh
          <Form
            key={tag ? `${tag.type}:${tag.id}` : "new"}
            tag={tag ?? undefined}
          />
        )}
      </div>
    </>
  )
}

function Form({ tag }: { tag?: TagDetails }) {
  const navigate = useNavigate()
  const [values, setValues] = useState<TagFieldValues>({
    type: tag?.type ?? "text",
    name: tag?.name ?? "",
    multiple: tag?.multiple ?? false,
  })
  const save = useMutation({
    mutationFn: () => {
      const input = { name: values.name, multiple: values.multiple }
      return tag
        ? call("updateTag", { type: tag.type, id: tag.id, ...input })
        : call("createTag", { type: values.type, ...input })
    },
  })
  // Switching to single fails while these have several values
  const switching = tag?.multiple && !values.multiple
  const blocking = !switching
    ? []
    : [
        ...tag.multiValueReleases.items.map((r) => ({
          key: `release:${r.id}`,
          title: r.title,
          to: `/releases/${r.id}/update/tags`,
        })),
        ...tag.multiValueTracks.items.map((t) => ({
          key: `track:${t.id}`,
          title: `${t.title} (track)`,
          to: `/tracks/${t.id}/update`,
        })),
      ]
  const blockingTotal = switching
    ? tag.multiValueReleases.total + tag.multiValueTracks.total
    : 0

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          const saved = await save.mutateAsync()
          navigate(`/tags/${saved.type}/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {save.error && <QueryError error={save.error} title="Can't save" />}
        <TagFields
          values={values}
          typeFixed={tag !== undefined}
          onChange={(changes) => setValues((v) => ({ ...v, ...changes }))}
        />
        {blockingTotal > 0 && (
          <Alert>
            <CircleAlertIcon />
            <AlertTitle>
              {blockingTotal === 1
                ? "One has several values"
                : `${blockingTotal} have several values`}
            </AlertTitle>
            <AlertDescription>
              <p>Leave one value on each before making the tag single-value:</p>
              <ul className="list-disc pl-4">
                {blocking.map((item) => (
                  <li key={item.key}>
                    <Link to={item.to} className="underline">
                      {item.title}
                    </Link>
                  </li>
                ))}
              </ul>
              {blockingTotal > blocking.length && (
                <p>And {blockingTotal - blocking.length} more.</p>
              )}
            </AlertDescription>
          </Alert>
        )}
        <div className="flex gap-2">
          <Button type="submit" disabled={save.isPending}>
            Save
          </Button>
          <ButtonLink
            variant="outline"
            to={tag ? `/tags/${tag.type}/${tag.id}` : "/tags"}
          >
            Cancel
          </ButtonLink>
        </div>
      </FieldGroup>
    </form>
  )
}
