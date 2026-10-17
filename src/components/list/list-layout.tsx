import { PlusIcon } from "lucide-react"
import type { ReactNode } from "react"

import { ButtonLink } from "@/components/button-link"
import { Pager } from "@/components/list/pager"
import { SearchBox } from "@/components/list/search-box"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import type { useListParams } from "@/hooks/use-list-params"

export const PER_PAGE = 50

/**
 * A list page (slunk's design §7.2): the header with its New button, a search box,
 * filters and the pager, then the table, or what's there instead while it
 * loads, fails or is empty.
 */
export function ListLayout({
  title,
  newTo,
  newLabel = "New",
  list,
  searchPlaceholder,
  filters,
  total,
  error,
  emptyText,
  children,
}: {
  title: string
  newTo?: string
  newLabel?: string
  list: ReturnType<typeof useListParams>
  /** Without it, there's no search box */
  searchPlaceholder?: string
  filters?: ReactNode
  /** Undefined while loading */
  total: number | undefined
  error?: { message: string }
  emptyText: string
  children: ReactNode
}) {
  const filtered = list.params.size > 0
  return (
    <>
      <PageHeader crumbs={[{ label: title }]}>
        {newTo && (
          <ButtonLink to={newTo}>
            <PlusIcon data-icon="inline-start" />
            {newLabel}
          </ButtonLink>
        )}
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        <div className="flex flex-wrap items-center gap-4">
          {searchPlaceholder && (
            <SearchBox
              value={list.q}
              placeholder={searchPlaceholder}
              onSearch={(q) => list.update({ q }, { replace: true })}
            />
          )}
          {filters}
          <div className="ml-auto">
            {total !== undefined && (
              <Pager
                page={list.page}
                perPage={PER_PAGE}
                total={total}
                onPage={(page) => list.update({ page })}
              />
            )}
          </div>
        </div>
        {error ? (
          <QueryError
            error={error}
            title={`Can't load the ${title.toLowerCase()}`}
          />
        ) : total === undefined ? (
          <Skeleton className="h-64 w-full" />
        ) : total === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>Nothing here</EmptyTitle>
              <EmptyDescription>
                {filtered ? "Nothing matches." : emptyText}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          children
        )}
      </div>
    </>
  )
}
