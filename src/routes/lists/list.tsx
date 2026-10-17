import { useQuery } from "@apollo/client/react"
import { Link } from "react-router"

import { ListLayout, PER_PAGE } from "@/components/list/list-layout"
import { SortableHead } from "@/components/list/sortable-head"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { graphql } from "@/gql"
import type { ListKind } from "@/gql/graphql"
import { useListParams } from "@/hooks/use-list-params"
import { LISTS } from "@/routes/lists/config"

const ListItemsQuery = graphql(`
  query ListItems(
    $kind: ListKind!
    $page: Int!
    $perPage: Int!
    $sort: String
    $q: String
  ) {
    listItems(kind: $kind, page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total
      items {
        id
        name
        uses
      }
    }
  }
`)

/** `/release-formats` and `/release-media` */
export function ListItemList({ kind }: { kind: ListKind }) {
  const list = useListParams()
  const { path, title } = LISTS[kind]
  const { data, error } = useQuery(ListItemsQuery, {
    variables: {
      kind,
      page: list.page,
      perPage: PER_PAGE,
      sort: list.sort,
      q: list.q,
    },
  })

  return (
    <ListLayout
      title={title}
      newTo={`${path}/create`}
      list={list}
      searchPlaceholder="Search names"
      total={data?.listItems.total}
      error={error}
      emptyText={`No ${title.toLowerCase()} yet.`}
    >
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Id</TableHead>
            <SortableHead
              column="name"
              sort={list.sort}
              onSort={(sort) => list.update({ sort })}
            >
              Name
            </SortableHead>
            <TableHead className="text-right">Releases</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {data?.listItems.items.map((item) => (
            <TableRow key={item.id}>
              <TableCell className="tabular-nums">{item.id}</TableCell>
              <TableCell>
                <Link to={`${path}/${item.id}`} className="hover:underline">
                  {item.name}
                </Link>
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {item.uses}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </ListLayout>
  )
}
