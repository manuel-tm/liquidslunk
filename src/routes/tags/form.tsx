import { useMutation, useQuery } from "@apollo/client/react"
import { CircleAlertIcon } from "lucide-react"
import { useState } from "react"
import { Link, useNavigate, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import {
  CreateTag,
  type TagFieldValues,
  TagFields,
} from "@/components/tag-fields"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { FieldGroup } from "@/components/ui/field"
import { Skeleton } from "@/components/ui/skeleton"
import { graphql } from "@/gql"
import type { TagFormQuery } from "@/gql/graphql"
import { isTagType } from "@/lib/tags"
import { NotFound } from "@/routes/not-found"

const TagFormQueryDocument = graphql(`
  query TagForm($type: TagType!, $id: ID!) {
    tag(type: $type, id: $id) {
      id
      type
      name
      multiple
      multiValueReleases(perPage: 20) {
        total
        items {
          id
          title
        }
      }
      multiValueTracks(perPage: 20) {
        total
        items {
          id
          title
        }
      }
    }
  }
`)

const UpdateTag = graphql(`
  mutation UpdateTag($type: TagType!, $id: ID!, $input: TagInput!) {
    updateTag(type: $type, id: $id, input: $input) {
      id
      type
      name
      multiple
    }
  }
`)

type Tag = NonNullable<TagFormQuery["tag"]>

/**
 * `/tags/create` and `/tags/:type/:id/update` (slunk's design §7.8): type (only
 * when creating), name, and single or multiple.
 */
export function TagForm() {
  const { type, id } = useParams()
  const editing = id !== undefined
  const { data, error } = useQuery(TagFormQueryDocument, {
    variables: { type: isTagType(type) ? type : "text", id: id ?? "" },
    skip: !editing || !isTagType(type),
    fetchPolicy: "network-only",
  })

  if (editing && (!isTagType(type) || (data && !data.tag))) return <NotFound />
  const tag = data?.tag

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
          <Form tag={tag ?? undefined} />
        )}
      </div>
    </>
  )
}

function Form({ tag }: { tag?: Tag }) {
  const navigate = useNavigate()
  const [values, setValues] = useState<TagFieldValues>({
    type: tag?.type ?? "text",
    name: tag?.name ?? "",
    multiple: tag?.multiple ?? false,
  })
  const [create, created] = useMutation(CreateTag)
  const [update, updated] = useMutation(UpdateTag)
  const error = created.error ?? updated.error
  // Switching to single fails while these have several values (slunk's design §4.4)
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
        const input = { name: values.name, multiple: values.multiple }
        try {
          const saved = tag
            ? (
                await update({
                  variables: { type: tag.type, id: tag.id, input },
                })
              ).data!.updateTag
            : (await create({ variables: { type: values.type, input } })).data!
                .createTag
          navigate(`/tags/${saved.type}/${saved.id}`)
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup>
        {error && <QueryError error={error} title="Can't save" />}
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
          <Button type="submit" disabled={created.loading || updated.loading}>
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
