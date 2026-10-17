import { ArrowDownIcon, ArrowUpIcon } from "lucide-react"
import type { ReactNode } from "react"

import { TableHead } from "@/components/ui/table"

/**
 * A column header that sorts the list: ascending (`?sort=path`), then
 * descending (`?sort=-path`), then back to the list's default.
 */
export function SortableHead({
  column,
  sort,
  onSort,
  children,
}: {
  column: string
  sort: string | null
  onSort: (sort: string | null) => void
  children: ReactNode
}) {
  const direction =
    sort === column ? "asc" : sort === `-${column}` ? "desc" : null
  const next =
    direction === null ? column : direction === "asc" ? `-${column}` : null

  return (
    <TableHead
      aria-sort={
        direction === "asc"
          ? "ascending"
          : direction === "desc"
            ? "descending"
            : undefined
      }
    >
      <button
        type="button"
        className="inline-flex items-center gap-1 hover:text-foreground"
        onClick={() => onSort(next)}
      >
        {children}
        {direction === "asc" && <ArrowUpIcon className="size-3" />}
        {direction === "desc" && <ArrowDownIcon className="size-3" />}
      </button>
    </TableHead>
  )
}
