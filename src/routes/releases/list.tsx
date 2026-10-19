import { useState } from "react"
import { toast } from "sonner"

import { ListLayout, PER_PAGE } from "@/components/list/list-layout"
import { LabelPicker, ListItemPicker } from "@/components/pickers"
import { ReleaseTable } from "@/components/release-table"
import { TagFilters } from "@/components/tag-filters"
import type { NamedItem } from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import { call, idParam, useCommand } from "@/lib/query"
import { tagFilterInput, tagFiltersOf } from "@/lib/tags"
import { BulkEditBar } from "@/routes/releases/bulk-edit"

/** A filter's name, as its picker shows it: options' ids are strings. */
const filterOption = (item: NamedItem | null | undefined) =>
  item ? { id: String(item.id), name: item.name } : null

/**
 * `/releases`. The filters combine: `?format=3&medium=4` is "EPs on vinyl", and
 * `?text.3=Rock&number.5.min=8` is "Genre is Rock and Score ≥ 8". Rows can be
 * selected, across pages, for a bulk edit.
 */
export function ReleaseList() {
  const list = useListParams()
  const tagFilters = tagFiltersOf(list.params)
  const filters = {
    q: list.q,
    format: idParam(list.params.get("format") ?? undefined) ?? undefined,
    medium: idParam(list.params.get("medium") ?? undefined) ?? undefined,
    label: idParam(list.params.get("label") ?? undefined) ?? undefined,
    tags: tagFilters.map(tagFilterInput),
  }
  const { data, error } = useCommand("releases", {
    page: list.page,
    perPage: PER_PAGE,
    sort: list.sort,
    ...filters,
  })
  const [selected, setSelected] = useState(new Set<number>())
  const [selectingAll, setSelectingAll] = useState(false)

  function select(ids: number[], on: boolean) {
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
      setSelected(new Set(await call("releaseIds", filters)))
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
              value={filterOption(data?.filterFormat)}
              onChange={(option) => list.update({ format: option?.id ?? null })}
            />
          </div>
          <div className="w-44">
            <ListItemPicker
              kind="release_media"
              placeholder="Any medium"
              value={filterOption(data?.filterMedium)}
              onChange={(option) => list.update({ medium: option?.id ?? null })}
            />
          </div>
          <div className="w-52">
            <LabelPicker
              placeholder="Any label"
              value={filterOption(data?.filterLabel)}
              onChange={(option) => list.update({ label: option?.id ?? null })}
            />
          </div>
          <TagFilters
            filters={tagFilters}
            onAdd={list.append}
            onRemove={(filter) => list.remove(filter.key, filter.value)}
          />
        </>
      }
      total={data?.total}
      error={error ?? undefined}
      emptyText="No releases yet. Create one, or import a folder."
    >
      {selected.size > 0 && (
        <BulkEditBar
          selected={[...selected]}
          total={data?.total ?? 0}
          selectingAll={selectingAll}
          onSelectAll={selectAll}
          onClear={() => setSelected(new Set())}
        />
      )}
      <ReleaseTable
        releases={data?.items ?? []}
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
