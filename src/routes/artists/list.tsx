import { useQuery } from "@apollo/client/react"
import { Link } from "react-router"

import { MatchedName } from "@/components/alternate-names"
import { Cover } from "@/components/cover"
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

const ArtistsQuery = graphql(`
  query Artists($page: Int!, $perPage: Int!, $sort: String, $q: String) {
    artists(page: $page, perPage: $perPage, sort: $sort, q: $q) {
      total
      items {
        id
        name
        nameLanguage
        alternateNames {
          name
          language
        }
        picture {
          id
        }
        insertedAt
        updatedAt
      }
    }
  }
`)

/** `/artists` (slunk's design §7.7) */
export function ArtistList() {
  const list = useListParams()
  const { data, error } = useQuery(ArtistsQuery, {
    variables: {
      page: list.page,
      perPage: PER_PAGE,
      sort: list.sort,
      q: list.q,
    },
  })

  return (
    <ListLayout
      title="Artists"
      newTo="/artists/create"
      list={list}
      searchPlaceholder="Search names"
      total={data?.artists.total}
      error={error}
      emptyText="No artists yet."
    >
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Id</TableHead>
            <TableHead />
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
          {data?.artists.items.map((artist) => (
            <TableRow key={artist.id}>
              <TableCell className="tabular-nums">{artist.id}</TableCell>
              <TableCell>
                <Link to={`/artists/${artist.id}`}>
                  <Cover file={artist.picture} alt="" className="size-16" />
                </Link>
              </TableCell>
              <TableCell>
                <Link
                  to={`/artists/${artist.id}`}
                  className="hover:underline"
                  {...textProps(artist.nameLanguage)}
                >
                  {artist.name}
                </Link>
                <MatchedName
                  main={artist.name}
                  names={artist.alternateNames}
                  q={list.q}
                />
              </TableCell>
              <TableCell>{formatDateTime(artist.insertedAt)}</TableCell>
              <TableCell>{formatDateTime(artist.updatedAt)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </ListLayout>
  )
}
