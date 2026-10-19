import { Link } from "react-router"

import { MatchedName } from "@/components/alternate-names"
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
import { useListParams } from "@/hooks/use-list-params"
import { formatDateTime } from "@/lib/format"
import { textProps } from "@/lib/languages"
import { useCommand } from "@/lib/query"

/** `/labels`: the label list. */
export function LabelList() {
  const list = useListParams()
  const { data, error } = useCommand("labels", {
    page: list.page,
    perPage: PER_PAGE,
    sort: list.sort,
    q: list.q,
  })

  return (
    <ListLayout
      title="Labels"
      newTo="/labels/create"
      list={list}
      searchPlaceholder="Search names"
      total={data?.total}
      error={error ?? undefined}
      emptyText="No labels yet."
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
            <TableHead>Added</TableHead>
            <TableHead>Updated</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {data?.items.map((label) => (
            <TableRow key={label.id}>
              <TableCell className="tabular-nums">{label.id}</TableCell>
              <TableCell>
                <Link
                  to={`/labels/${label.id}`}
                  className="hover:underline"
                  {...textProps(label.nameLanguage)}
                >
                  {label.name}
                </Link>
                <MatchedName
                  main={label.name}
                  names={label.alternateNames}
                  q={list.q}
                />
              </TableCell>
              <TableCell>{formatDateTime(label.insertedAt)}</TableCell>
              <TableCell>{formatDateTime(label.updatedAt)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </ListLayout>
  )
}
