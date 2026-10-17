import { useQuery } from "@apollo/client/react"
import { PlusIcon } from "lucide-react"
import { Link } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { SearchBox } from "@/components/list/search-box"
import { SortableHead } from "@/components/list/sortable-head"
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
import { graphql } from "@/gql"
import type { TagsQuery } from "@/gql/graphql"
import { useListParams } from "@/hooks/use-list-params"
import { TAG_TYPE_LABELS, TAG_TYPES } from "@/lib/tags"

// Definitions are few, so each table shows them all, up to the API's page
// limit
const PER_TYPE = 200

const TagsQueryDocument = graphql(`
  query Tags($q: String, $sort: String, $perPage: Int!) {
    text: tags(type: text, q: $q, sort: $sort, perPage: $perPage) {
      total
      items {
        id
        type
        name
        multiple
      }
    }
    number: tags(type: number, q: $q, sort: $sort, perPage: $perPage) {
      total
      items {
        id
        type
        name
        multiple
      }
    }
    date: tags(type: date, q: $q, sort: $sort, perPage: $perPage) {
      total
      items {
        id
        type
        name
        multiple
      }
    }
  }
`)

/** `/tags`: a table per type (slunk's design §7.8). */
export function TagList() {
  const list = useListParams()
  const { data, error } = useQuery(TagsQueryDocument, {
    variables: { q: list.q, sort: list.sort, perPage: PER_TYPE },
  })
  const none = data && TAG_TYPES.every((type) => data[type].total === 0)

  return (
    <>
      <PageHeader crumbs={[{ label: "Tags" }]}>
        <ButtonLink to="/tags/create">
          <PlusIcon data-icon="inline-start" />
          New
        </ButtonLink>
      </PageHeader>
      <div className="flex flex-col gap-6 p-4">
        <SearchBox
          value={list.q}
          placeholder="Search names"
          onSearch={(q) => list.update({ q }, { replace: true })}
        />
        {error ? (
          <QueryError error={error} title="Can't load the tags" />
        ) : !data ? (
          <Skeleton className="h-64 w-full" />
        ) : none ? (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>Nothing here</EmptyTitle>
              <EmptyDescription>
                {list.q
                  ? "Nothing matches."
                  : "No tags yet. Create one, like Genre, Score or Date purchased."}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          TAG_TYPES.map((type) => (
            <TagTable
              key={type}
              title={TAG_TYPE_LABELS[type]}
              page={data[type]}
              sort={list.sort}
              onSort={(sort) => list.update({ sort })}
            />
          ))
        )}
      </div>
    </>
  )
}

function TagTable({
  title,
  page,
  sort,
  onSort,
}: {
  title: string
  page: TagsQuery["text"]
  sort: string | null
  onSort: (sort: string | null) => void
}) {
  return (
    <section className="flex flex-col gap-2">
      <h2 className="font-heading text-sm font-semibold">
        {title} <span className="text-muted-foreground">{page.total}</span>
      </h2>
      {page.total === 0 ? (
        <p className="text-xs text-muted-foreground">None.</p>
      ) : (
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-16">Id</TableHead>
              <SortableHead column="name" sort={sort} onSort={onSort}>
                Name
              </SortableHead>
              <TableHead className="w-48">Values</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {page.items.map((tag) => (
              <TableRow key={tag.id}>
                <TableCell className="tabular-nums">{tag.id}</TableCell>
                <TableCell>
                  <Link
                    to={`/tags/${tag.type}/${tag.id}`}
                    className="hover:underline"
                  >
                    {tag.name}
                  </Link>
                </TableCell>
                <TableCell>{tag.multiple ? "Multiple" : "Single"}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      )}
      {page.total > page.items.length && (
        <p className="text-xs text-muted-foreground">
          Showing the first {page.items.length} of {page.total}. Search to find
          the others.
        </p>
      )}
    </section>
  )
}
