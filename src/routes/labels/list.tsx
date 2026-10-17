import { useQuery } from "@apollo/client/react"
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
import { graphql } from "@/gql"
import { useListParams } from "@/hooks/use-list-params"
import { formatDateTime } from "@/lib/format"
import { textProps } from "@/lib/languages"

const LabelsQuery = graphql(`
  query Labels($page: Int!, $perPage: Int!, $sort: String, $q: String) {
    labels(page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total
      items {
        id
        name
        nameLanguage
        alternateNames {
          name
          language
        }
        insertedAt
        updatedAt
      }
    }
  }
`)

/** `/labels` (slunk's design §7.8) */
export function LabelList() {
  const list = useListParams()
  const { data, error } = useQuery(LabelsQuery, {
    variables: {
      page: list.page,
      perPage: PER_PAGE,
      sort: list.sort,
      q: list.q,
    },
  })

  return (
    <ListLayout
      title="Labels"
      newTo="/labels/create"
      list={list}
      searchPlaceholder="Search names"
      total={data?.labels.total}
      error={error}
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
          {data?.labels.items.map((label) => (
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
