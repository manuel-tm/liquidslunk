import type { ReactNode } from "react"
import { Link } from "react-router"

import { Cover } from "@/components/cover"
import { artistUrl, Names } from "@/components/names"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { Title } from "@/components/titles"
import { formatDuration } from "@/lib/format"
import { useCommand } from "@/lib/query"

/**
 * `/` (slunk's design §7.5): the library's status, numbers that are cheap to
 * compute, and the latest additions.
 */
export function Dashboard() {
  const totals = useCommand("catalogTotals")
  const newest = useCommand("releases", { perPage: 12 })
  const error = totals.error ?? newest.error
  const status = useCommand("libraryStatus")
  const library = status.data?.library
  const fileCounts = useCommand("fileCounts").data

  return (
    <>
      <PageHeader crumbs={[{ label: "Dashboard" }]} />
      <div className="flex flex-col gap-4 p-4">
        <Card>
          <CardHeader>
            <CardTitle>Status</CardTitle>
            <CardDescription>The library, and its database</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-col gap-4">
            {status.error ? (
              <QueryError error={status.error} />
            ) : !library ? (
              <div className="flex flex-col gap-2">
                <Skeleton className="h-4 w-24" />
                <Skeleton className="h-4 w-full" />
                <Skeleton className="h-4 w-2/3" />
              </div>
            ) : (
              <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-xs">
                <dt className="text-muted-foreground">Library</dt>
                <dd>{library.name}</dd>
                <dt className="text-muted-foreground">Database</dt>
                <dd className="font-mono break-all">{library.path}</dd>
                <dt className="text-muted-foreground">Schema</dt>
                <dd className="font-mono break-all">
                  {library.migration ?? "No migrations applied"}
                </dd>
              </dl>
            )}
          </CardContent>
        </Card>
        {error && <QueryError error={error} title="Can't load the catalog" />}
        {totals.data && newest.data && (
          <>
            <div className="grid gap-4 sm:grid-cols-3 xl:grid-cols-6">
              <Total
                title="Releases"
                to="/releases"
                value={totals.data.releases}
              />
              <Total
                title="Artists"
                to="/artists"
                value={totals.data.artists}
              />
              <Total title="Tracks" to="/tracks" value={totals.data.tracks}>
                {formatDuration(totals.data.durationMs) || "0:00"} playing time
              </Total>
              <Total title="Labels" to="/labels" value={totals.data.labels} />
              <Total title="Files" to="/files" value={fileCounts?.total} />
              <Total
                title="Unused files"
                to="/files?unused=1"
                value={fileCounts?.unused}
              >
                Registered files nothing uses yet
              </Total>
            </div>
            <Card>
              <CardHeader>
                <CardTitle>Newest releases</CardTitle>
              </CardHeader>
              <CardContent>
                {newest.data.items.length === 0 ? (
                  <p className="text-xs text-muted-foreground">
                    No releases yet.{" "}
                    <Link
                      to="/import"
                      className="text-foreground hover:underline"
                    >
                      Import a folder
                    </Link>{" "}
                    to make one.
                  </p>
                ) : (
                  <div className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-4">
                    {newest.data.items.map((release) => (
                      <div
                        key={release.id}
                        className="flex flex-col gap-1 text-xs"
                      >
                        <Link to={`/releases/${release.id}`}>
                          <Cover
                            file={release.cover}
                            alt={`${release.title} cover`}
                            className="w-full"
                          />
                        </Link>
                        <Link
                          to={`/releases/${release.id}`}
                          className="font-medium hover:underline"
                        >
                          <Title
                            title={release.title}
                            language={release.titleLanguage}
                          />
                        </Link>
                        <span className="text-muted-foreground">
                          <Names items={release.artists} to={artistUrl} />
                        </span>
                      </div>
                    ))}
                  </div>
                )}
              </CardContent>
            </Card>
          </>
        )}
      </div>
    </>
  )
}

/** A number that links to its list. */
function Total({
  title,
  to,
  value,
  children,
}: {
  title: string
  to: string
  /** Undefined while it loads */
  value: number | undefined
  children?: ReactNode
}) {
  return (
    <Link to={to} className="group">
      <Card className="h-full transition-colors group-hover:bg-muted/50">
        <CardHeader>
          <CardDescription>{title}</CardDescription>
          <CardTitle className="text-2xl tabular-nums">
            {value === undefined ? (
              <Skeleton className="h-8 w-12" />
            ) : (
              value.toLocaleString()
            )}
          </CardTitle>
        </CardHeader>
        {children && (
          <CardContent className="text-xs text-muted-foreground">
            {children}
          </CardContent>
        )}
      </Card>
    </Link>
  )
}
