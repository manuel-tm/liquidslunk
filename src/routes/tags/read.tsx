import { useMutation } from "@tanstack/react-query"
import { Link, useParams } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { DeleteButton } from "@/components/delete-button"
import { PER_PAGE } from "@/components/list/list-layout"
import { Pager } from "@/components/list/pager"
import { SearchBox } from "@/components/list/search-box"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { TagType } from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import { call, idParam, queryClient, useCommand } from "@/lib/query"
import { isTagType, TAG_TYPE_LABELS, tagValueUrl } from "@/lib/tags"
import { NotFound } from "@/routes/not-found"

/** `/tags/:type/:id`: the type is in the route, since ids repeat across types. */
export function TagRead() {
  const { type, id = "" } = useParams()
  const tagId = idParam(id)
  if (!isTagType(type) || tagId === null) return <NotFound />
  return <Tag type={type} id={tagId} />
}

/**
 * A tag definition and how many releases and tracks have it. A text tag also
 * lists its values, with links to what has each.
 */
function Tag({ type, id }: { type: TagType; id: number }) {
  const list = useListParams()
  const { data: tag, error } = useCommand("tag", { type, id })
  // Only text tags list their values
  const valuesQuery = useCommand(
    "tagValues",
    { id, page: list.page, perPage: PER_PAGE, q: list.q },
    { enabled: type === "text" }
  )
  const deleteTag = useMutation({
    mutationFn: () => call("deleteTag", { type, id }),
  })

  if (tag === null) return <NotFound />
  const base = `/tags/${type}/${id}`
  // A disabled query still gives what's cached for its key, and a number or
  // date tag's id can be a text tag's
  const text = type === "text"
  const values = text ? valuesQuery.data : undefined
  const failed = error ?? (text ? valuesQuery.error : null)

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Tags", to: "/tags" },
          { label: tag ? `${tag.name} (${tag.id})` : String(id) },
        ]}
      >
        {tag && (
          <>
            <ButtonLink variant="outline" to={`${base}/update`}>
              Edit
            </ButtonLink>
            <DeleteButton
              name={`the tag ${tag.name}`}
              blockers={tag.blockers}
              onDelete={() => deleteTag.mutateAsync()}
              then="/tags"
              afterDelete={() =>
                queryClient.removeQueries({ queryKey: ["tag", { type, id }] })
              }
            />
          </>
        )}
      </PageHeader>
      <div className="flex flex-col gap-6 p-4 text-xs">
        {failed ? (
          <QueryError error={failed} title="Can't load the tag" />
        ) : !tag || (text && !values) ? (
          <Skeleton className="h-32 w-64" />
        ) : (
          <>
            <div className="flex flex-col gap-3">
              <h1 className="font-heading text-2xl font-semibold">
                {tag.name}
              </h1>
              <dl className="grid grid-cols-[5rem_1fr] gap-y-1">
                <dt className="text-muted-foreground">Type:</dt>
                <dd>{TAG_TYPE_LABELS[tag.type]}</dd>
                <dt className="text-muted-foreground">Values:</dt>
                <dd>
                  {tag.multiple
                    ? "Multiple, several per release or track"
                    : "Single, one per release or track"}
                </dd>
                <dt className="text-muted-foreground">Releases:</dt>
                <dd className="tabular-nums">{tag.releaseCount}</dd>
                <dt className="text-muted-foreground">Tracks:</dt>
                <dd className="tabular-nums">{tag.trackCount}</dd>
              </dl>
            </div>
            {values && (
              <section className="flex flex-col gap-2">
                <h2 className="font-heading text-base font-semibold">Values</h2>
                <div className="flex flex-wrap items-center gap-4">
                  <SearchBox
                    value={list.q}
                    placeholder="Search values"
                    onSearch={(q) => list.update({ q }, { replace: true })}
                  />
                  <div className="ml-auto">
                    <Pager
                      page={list.page}
                      perPage={PER_PAGE}
                      total={values.total}
                      onPage={(page) => list.update({ page })}
                    />
                  </div>
                </div>
                {values.total === 0 ? (
                  <p className="text-muted-foreground">
                    {list.q
                      ? "Nothing matches."
                      : "No release or track has it yet."}
                  </p>
                ) : (
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead>Value</TableHead>
                        <TableHead className="w-24 text-right">
                          Releases
                        </TableHead>
                        <TableHead className="w-24 text-right">
                          Tracks
                        </TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {values.items.map(({ value, releases, tracks }) => (
                        <TableRow key={value}>
                          <TableCell>{value}</TableCell>
                          <TableCell className="text-right tabular-nums">
                            <CountLink
                              count={releases}
                              to={tagValueUrl(tag.type, tag.id, value)}
                            />
                          </TableCell>
                          <TableCell className="text-right tabular-nums">
                            <CountLink
                              count={tracks}
                              to={tagValueUrl(
                                tag.type,
                                tag.id,
                                value,
                                "/tracks"
                              )}
                            />
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                )}
              </section>
            )}
          </>
        )}
      </div>
    </>
  )
}

/** A count that links to what it counts, unless there's nothing. */
function CountLink({ count, to }: { count: number; to: string }) {
  if (count === 0) return <span className="text-muted-foreground">0</span>
  return (
    <Link to={to} className="hover:underline">
      {count}
    </Link>
  )
}
