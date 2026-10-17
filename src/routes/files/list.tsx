import { FilePlusIcon } from "lucide-react"
import { Link } from "react-router"

import { ButtonLink } from "@/components/button-link"
import { FILE_TYPE_LABELS } from "@/components/file-type"
import { Pager } from "@/components/list/pager"
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
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { Skeleton } from "@/components/ui/skeleton"
import { Switch } from "@/components/ui/switch"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { FileType } from "@/bindings"
import { useListParams } from "@/hooks/use-list-params"
import { formatDateTime, formatSize } from "@/lib/format"
import { useCommand } from "@/lib/query"

const PER_PAGE = 50

const TYPE_ITEMS = { all: "All types", ...FILE_TYPE_LABELS }

/** `/files` (slunk's design §7.8) */
export function FileList() {
  const { params, page, sort, q, update } = useListParams()
  const type = (params.get("type") as FileType | null) ?? null
  const unused = params.get("unused") === "1"

  const { data, error } = useCommand("files", {
    page,
    perPage: PER_PAGE,
    sort,
    q,
    type,
    unused,
  })

  return (
    <>
      <PageHeader crumbs={[{ label: "Files" }]}>
        <ButtonLink to="/files/create">
          <FilePlusIcon data-icon="inline-start" />
          Register
        </ButtonLink>
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        <div className="flex flex-wrap items-center gap-4">
          <SearchBox
            value={q}
            placeholder="Search paths"
            onSearch={(q) => update({ q }, { replace: true })}
          />
          <Select
            items={TYPE_ITEMS}
            value={type ?? "all"}
            onValueChange={(value) =>
              update({ type: value === "all" ? null : value })
            }
          >
            <SelectTrigger aria-label="Type">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {Object.entries(TYPE_ITEMS).map(([value, label]) => (
                <SelectItem key={value} value={value}>
                  {label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Label className="gap-2">
            <Switch
              checked={unused}
              onCheckedChange={(checked) => update({ unused: checked })}
            />
            Unused only
          </Label>
          <div className="ml-auto">
            {data && (
              <Pager
                page={page}
                perPage={PER_PAGE}
                total={data.total}
                onPage={(page) => update({ page })}
              />
            )}
          </div>
        </div>

        {error ? (
          <QueryError error={error} title="Can't load the files" />
        ) : !data ? (
          <Skeleton className="h-64 w-full" />
        ) : data.total === 0 ? (
          <Empty>
            <EmptyHeader>
              <EmptyTitle>No files</EmptyTitle>
              <EmptyDescription>
                {q || type || unused
                  ? "No registered file matches."
                  : "Nothing is registered yet. Register files from a folder of your library."}
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        ) : (
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>Id</TableHead>
                <SortableHead
                  column="path"
                  sort={sort}
                  onSort={(sort) => update({ sort })}
                >
                  Path
                </SortableHead>
                <TableHead>Type</TableHead>
                <TableHead className="text-right">Size</TableHead>
                <TableHead>Modified</TableHead>
                <TableHead>SHA-256</TableHead>
                <TableHead>Track</TableHead>
                <TableHead>Added</TableHead>
                <TableHead>Updated</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {data.items.map((file) => (
                <TableRow key={file.id}>
                  <TableCell className="tabular-nums">{file.id}</TableCell>
                  <TableCell className="min-w-64 whitespace-normal">
                    <Link to={`/files/${file.id}`} className="hover:underline">
                      {file.path}
                    </Link>
                  </TableCell>
                  <TableCell>{FILE_TYPE_LABELS[file.type]}</TableCell>
                  <TableCell className="text-right tabular-nums">
                    {formatSize(file.sizeBytes)}
                  </TableCell>
                  <TableCell>{formatDateTime(file.mtime)}</TableCell>
                  <TableCell className="font-mono" title={file.sha256}>
                    {file.sha256.slice(0, 12)}…
                  </TableCell>
                  <TableCell>
                    {file.track && (
                      <Link
                        to={`/tracks/${file.track.id}`}
                        className="hover:underline"
                      >
                        {file.track.title}
                      </Link>
                    )}
                  </TableCell>
                  <TableCell>{formatDateTime(file.insertedAt)}</TableCell>
                  <TableCell>{formatDateTime(file.updatedAt)}</TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        )}
      </div>
    </>
  )
}
