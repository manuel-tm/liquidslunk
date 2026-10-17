import { useApolloClient, useQuery } from "@apollo/client/react"
import { useState } from "react"
import { toast } from "sonner"

import { ListLayout, PER_PAGE } from "@/components/list/list-layout"
import { LabelPicker, ListItemPicker } from "@/components/pickers"
import { ReleaseTable } from "@/components/release-table"
import { graphql } from "@/gql"
import { useListParams } from "@/hooks/use-list-params"
import {
  tagFilterInput,
  tagFilterRefs,
  tagFiltersOf,
  tagNames,
} from "@/lib/tags"
import { TagFilters } from "@/components/tag-filters"
import { BulkEditBar } from "@/routes/releases/bulk-edit"

const ReleasesQuery = graphql(`
  query Releases(
    $page: Int!
    $perPage: Int!
    $sort: String
    $q: String
    $format: ID
    $medium: ID
    $label: ID
    $formatId: ID!
    $mediumId: ID!
    $labelId: ID!
    $withFormat: Boolean!
    $withMedium: Boolean!
    $withLabel: Boolean!
    $tags: [TagFilterInput!]!
    $tagRefs: [TagRefInput!]!
    $withTags: Boolean!
  ) {
    releases(
      page: $page
      perPage: $perPage
      sort: $sort
      q: $q
      format: $format
      medium: $medium
      label: $label
      tags: $tags
    ) {
      total
      items {
        id
        title
        titleLanguage
        alternateTitles {
          title
          language
          printed
        }
        released
        catalogNumber
        variousArtists
        insertedAt
        updatedAt
        cover {
          id
        }
        artists {
          artist {
            id
            name
            nameLanguage
          }
          creditedAs {
            name
            language
          }
        }
        formats {
          id
          name
        }
        media {
          id
          name
        }
        labels {
          id
          name
          nameLanguage
        }
      }
    }
    filterFormat: listItem(kind: release_formats, id: $formatId)
      @include(if: $withFormat) {
      id
      name
    }
    filterMedium: listItem(kind: release_media, id: $mediumId)
      @include(if: $withMedium) {
      id
      name
    }
    filterLabel: label(id: $labelId) @include(if: $withLabel) {
      id
      name
    }
    filterTags: tagsByRef(refs: $tagRefs) @include(if: $withTags) {
      id
      type
      name
    }
  }
`)

const ReleaseIdsQuery = graphql(`
  query ReleaseIds(
    $q: String
    $format: ID
    $medium: ID
    $label: ID
    $tags: [TagFilterInput!]!
  ) {
    releaseIds(
      q: $q
      format: $format
      medium: $medium
      label: $label
      tags: $tags
    )
  }
`)

/**
 * `/releases` (slunk's design §7.6). The filters combine: `?format=3&medium=4` is
 * "EPs on vinyl", and `?text.3=Rock&number.5.min=8` is "Genre is Rock and
 * Score ≥ 8". Rows can be selected, across pages, for a bulk edit.
 */
export function ReleaseList() {
  const list = useListParams()
  const format = list.params.get("format")
  const medium = list.params.get("medium")
  const label = list.params.get("label")
  const tagFilters = tagFiltersOf(list.params)
  const tagRefs = tagFilterRefs(tagFilters)
  const { data, error } = useQuery(ReleasesQuery, {
    variables: {
      page: list.page,
      perPage: PER_PAGE,
      sort: list.sort,
      q: list.q,
      format,
      medium,
      label,
      formatId: format ?? "",
      mediumId: medium ?? "",
      labelId: label ?? "",
      withFormat: format !== null,
      withMedium: medium !== null,
      withLabel: label !== null,
      tags: tagFilters.map(tagFilterInput),
      tagRefs,
      withTags: tagRefs.length > 0,
    },
  })
  const client = useApolloClient()
  const [selected, setSelected] = useState(new Set<string>())
  const [selectingAll, setSelectingAll] = useState(false)

  function select(ids: string[], on: boolean) {
    setSelected((current) => {
      const next = new Set(current)
      for (const id of ids) {
        if (on) next.add(id)
        else next.delete(id)
      }
      return next
    })
  }

  /** Every release the filters match, on any page. */
  async function selectAll() {
    setSelectingAll(true)
    try {
      const { data: ids } = await client.query({
        query: ReleaseIdsQuery,
        variables: {
          q: list.q,
          format,
          medium,
          label,
          tags: tagFilters.map(tagFilterInput),
        },
        fetchPolicy: "network-only",
      })
      if (ids) setSelected(new Set(ids.releaseIds))
    } catch {
      toast.error("Can't select them all")
    } finally {
      setSelectingAll(false)
    }
  }

  return (
    <ListLayout
      title="Releases"
      newTo="/releases/create"
      list={list}
      searchPlaceholder="Search titles"
      filters={
        <>
          <div className="w-44">
            <ListItemPicker
              kind="release_formats"
              placeholder="Any format"
              value={data?.filterFormat ?? null}
              onChange={(option) => list.update({ format: option?.id ?? null })}
            />
          </div>
          <div className="w-44">
            <ListItemPicker
              kind="release_media"
              placeholder="Any medium"
              value={data?.filterMedium ?? null}
              onChange={(option) => list.update({ medium: option?.id ?? null })}
            />
          </div>
          <div className="w-52">
            <LabelPicker
              placeholder="Any label"
              value={data?.filterLabel ?? null}
              onChange={(option) => list.update({ label: option?.id ?? null })}
            />
          </div>
          <TagFilters
            filters={tagFilters}
            names={tagNames(data?.filterTags)}
            onAdd={list.append}
            onRemove={(filter) => list.remove(filter.key, filter.value)}
          />
        </>
      }
      total={data?.releases.total}
      error={error}
      emptyText="No releases yet. Create one, or import a folder."
    >
      {selected.size > 0 && (
        <BulkEditBar
          selected={[...selected]}
          total={data?.releases.total ?? 0}
          selectingAll={selectingAll}
          onSelectAll={selectAll}
          onClear={() => setSelected(new Set())}
        />
      )}
      <ReleaseTable
        releases={data?.releases.items ?? []}
        q={list.q}
        columns={[
          "id",
          "cover",
          "title",
          "artists",
          "released",
          "formats",
          "media",
          "labels",
          "catalogNumber",
          "variousArtists",
          "added",
          "updated",
        ]}
        sort={list.sort}
        onSort={(sort) => list.update({ sort })}
        selection={{ selected, onChange: select }}
      />
    </ListLayout>
  )
}
