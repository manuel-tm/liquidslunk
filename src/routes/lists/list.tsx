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
import type { ListKind } from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import { useCommand } from "@/lib/query"
import { LISTS } from "@/routes/lists/config"

/** `/release-formats` and `/release-media` */
export function ListItemList({ kind }: { kind: ListKind }) {
  const list = useListParams()
  const { path, title } = LISTS[kind]
  const { data, error } = useCommand("listItems", {
    kind,
    page: list.page,
    perPage: PER_PAGE,
    sort: list.sort,
    q: list.q,
  })

  return (
    <ListLayout
      title={title}
      newTo={`${path}/create`}
      list={list}
      searchPlaceholder="Search names"
      total={data?.total}
      error={error ?? undefined}
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
          {data?.items.map((item) => (
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
