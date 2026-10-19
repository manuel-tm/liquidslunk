import { useMutation } from "@tanstack/react-query"
import { CircleCheckIcon, RefreshCwIcon } from "lucide-react"
import type { ReactNode } from "react"
import { Link } from "react-router"
import { toast } from "sonner"

import { ButtonLink } from "@/components/button-link"
import { FILE_TYPE_LABELS } from "@/components/file-type"
import { FileUses } from "@/components/file-uses"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import { Skeleton } from "@/components/ui/skeleton"
import type { FileMoveInput, LibraryReport } from "@/bindings"
import { formatDateTime, formatSize } from "@/lib/format"
import { call, useCommand } from "@/lib/query"

type Check = LibraryReport

/**
 * `/library-check`: the library check's results, grouped by kind of problem,
 * each linking to what needs fixing. It runs each time the page opens, and
 * again after each fix.
 */
export function LibraryCheck() {
  // It reads the disk, so a cached result would be out of date
  const {
    data: check,
    error,
    isFetching: loading,
    refetch,
  } = useCommand("libraryCheck", undefined, { fresh: true })
  const problems = check
    ? check.missingFiles.length +
      check.changedFiles.length +
      unregisteredCount(check) +
      check.releasesWithoutArtists.length +
      check.tracksWithoutPerformers.length
    : 0

  return (
    <>
      <PageHeader crumbs={[{ label: "Library check" }]}>
        <Button
          variant="outline"
          disabled={loading}
          onClick={() => void refetch()}
        >
          <RefreshCwIcon data-icon="inline-start" />
          Check again
        </Button>
      </PageHeader>
      <div className="flex max-w-5xl flex-col gap-8 p-4 text-xs">
        {error ? (
          <QueryError error={error} title="Can't check the library" />
        ) : !check || loading ? (
          <div className="flex flex-col gap-2">
            <p className="text-muted-foreground">
              Reading the library. With many files, or files that moved, this
              can take a while.
            </p>
            <Skeleton className="h-64 w-full" />
          </div>
        ) : (
          <>
            <p>
              Checked {check.fileCount} registered files.{" "}
              {problems === 0
                ? "Everything's in order."
                : `${problems} thing${problems === 1 ? "" : "s"} to look at.`}
            </p>
            <MissingFiles check={check} />
            <ChangedFiles check={check} />
            <UnregisteredFiles check={check} />
            <Section
              title="Releases with no artist"
              description="Every release is by someone, even if that's an artist named Various Artists."
              count={check.releasesWithoutArtists.length}
            >
              <ul className="flex flex-col gap-1">
                {check.releasesWithoutArtists.map((release) => (
                  <li key={release.id} className="flex items-center gap-2">
                    <Link
                      to={`/releases/${release.id}`}
                      className="hover:underline"
                    >
                      {release.title}
                    </Link>
                    <ButtonLink
                      variant="ghost"
                      size="xs"
                      to={`/releases/${release.id}/update`}
                    >
                      Edit
                    </ButtonLink>
                  </li>
                ))}
              </ul>
            </Section>
            <Section
              title="Tracks nobody performs"
              description="On a various-artists release, each track names its own performers. Featured artists don't count."
              count={check.tracksWithoutPerformers.length}
            >
              <ul className="flex flex-col gap-1">
                {check.tracksWithoutPerformers.map((track) => (
                  <li key={track.id} className="flex items-center gap-2">
                    <span>
                      <Link
                        to={`/releases/${track.release.id}`}
                        className="hover:underline"
                      >
                        {track.release.title}
                      </Link>
                      <span className="text-muted-foreground">
                        {" "}
                        · disc {track.cdNumber}, track {track.trackNumber}{" "}
                        ·{" "}
                      </span>
                      <Link
                        to={`/tracks/${track.id}`}
                        className="hover:underline"
                      >
                        {track.title}
                      </Link>
                    </span>
                    <ButtonLink
                      variant="ghost"
                      size="xs"
                      to={`/releases/${track.release.id}/update/tracks`}
                    >
                      Edit tracks
                    </ButtonLink>
                  </li>
                ))}
              </ul>
            </Section>
          </>
        )}
      </div>
    </>
  )
}

function Section({
  title,
  description,
  count,
  action,
  children,
}: {
  title: string
  description: string
  count: number
  action?: ReactNode
  children: ReactNode
}) {
  return (
    <section className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <h2 className="font-heading text-base font-semibold">
          {title} <span className="text-muted-foreground">{count}</span>
        </h2>
        {count > 0 && <div className="ml-auto">{action}</div>}
      </div>
      <p className="text-muted-foreground">{description}</p>
      {count === 0 ? (
        <p className="flex items-center gap-1 text-muted-foreground">
          <CircleCheckIcon className="size-4" /> None.
        </p>
      ) : (
        children
      )}
    </section>
  )
}

/** After a fix, every query is invalidated, so the check runs again. */
function MissingFiles({ check }: { check: Check }) {
  const relink = useMutation({
    mutationFn: (moves: FileMoveInput[]) => call("relinkFiles", { moves }),
  })
  const loading = relink.isPending
  const likely = check.missingFiles.flatMap((missing) =>
    missing.likelyMove
      ? [{ fileId: missing.file.id, path: missing.likelyMove }]
      : []
  )

  async function move(moves: FileMoveInput[]) {
    try {
      await relink.mutateAsync(moves)
      toast.success(
        moves.length === 1
          ? "Relinked the file"
          : `Relinked ${moves.length} files`
      )
    } catch (error) {
      toast.error((error as Error).message)
    }
  }

  return (
    <Section
      title="Missing files"
      description="Registered files that aren't on disk anymore. A file that moved is found by its content, and relinking points its record at the new path. Otherwise, edit its path, or delete the record once nothing uses it."
      count={check.missingFiles.length}
      action={
        likely.length > 0 && (
          <Button size="sm" disabled={loading} onClick={() => move(likely)}>
            Relink {likely.length === 1 ? "the" : `all ${likely.length}`} found
          </Button>
        )
      }
    >
      <ul className="flex flex-col gap-3">
        {check.missingFiles.map(({ file, movedTo, likelyMove }) => (
          <li key={file.id} className="flex flex-col gap-1 border-l-2 pl-3">
            <span>
              <Link
                to={`/files/${file.id}`}
                className="font-mono break-all hover:underline"
              >
                {file.path}
              </Link>
              <span className="text-muted-foreground">
                {" "}
                · {FILE_TYPE_LABELS[file.type]}
              </span>
            </span>
            {file.uses.length > 0 ? (
              <FileUses uses={file.uses} />
            ) : (
              <span className="text-muted-foreground">Nothing uses it.</span>
            )}
            {movedTo.length > 0 ? (
              <ul className="flex flex-col gap-1">
                {movedTo.map((path) => (
                  <li key={path} className="flex items-center gap-2">
                    <span className="text-muted-foreground">
                      {path === likelyMove ? "Moved to" : "Same content at"}
                    </span>
                    <span className="font-mono break-all">{path}</span>
                    <Button
                      size="xs"
                      variant="outline"
                      disabled={loading}
                      onClick={() => move([{ fileId: file.id, path }])}
                    >
                      Relink
                    </Button>
                  </li>
                ))}
              </ul>
            ) : (
              <span className="flex items-center gap-2">
                <span className="text-muted-foreground">
                  Not found anywhere in the library.
                </span>
                <ButtonLink
                  variant="ghost"
                  size="xs"
                  to={`/files/${file.id}/update`}
                >
                  Edit its path
                </ButtonLink>
              </span>
            )}
          </li>
        ))}
      </ul>
    </Section>
  )
}

function ChangedFiles({ check }: { check: Check }) {
  const rehash = useMutation({
    mutationFn: (ids: number[]) => call("rehashFiles", { ids }),
  })
  const loading = rehash.isPending

  async function read(ids: number[]) {
    try {
      await rehash.mutateAsync(ids)
      toast.success(
        ids.length === 1
          ? "Read the file again"
          : `Read ${ids.length} files again`
      )
    } catch (error) {
      toast.error((error as Error).message)
    }
  }

  return (
    <Section
      title="Changed files"
      description="Files whose size or modification time isn't what liquidslunk read. Rehashing reads them again: size, time, hash, and a track's duration."
      count={check.changedFiles.length}
      action={
        <Button
          size="sm"
          disabled={loading}
          onClick={() => read(check.changedFiles.map((c) => c.file.id))}
        >
          Rehash all
        </Button>
      }
    >
      <ul className="flex flex-col gap-1">
        {check.changedFiles.map(({ file, sizeBytes, mtime }) => (
          <li key={file.id} className="flex flex-wrap items-center gap-2">
            <Link
              to={`/files/${file.id}`}
              className="font-mono break-all hover:underline"
            >
              {file.path}
            </Link>
            <span className="text-muted-foreground">
              {sizeBytes !== file.sizeBytes &&
                `${sizeChange(file.sizeBytes, sizeBytes)} · `}
              modified {formatDateTime(mtime)}
            </span>
            <Button
              size="xs"
              variant="outline"
              disabled={loading}
              onClick={() => read([file.id])}
            >
              Rehash
            </Button>
          </li>
        ))}
      </ul>
    </Section>
  )
}

/** "94 KB → 95 KB", or in bytes when they'd round the same. */
function sizeChange(was: number, now: number) {
  const [a, b] = [formatSize(was), formatSize(now)]
  return a === b
    ? `${was.toLocaleString()} → ${now.toLocaleString()} bytes`
    : `${a} → ${b}`
}

function unregisteredCount(check: Check) {
  return check.unregisteredFolders.reduce(
    (total, folder) => total + folder.files.length,
    0
  )
}

function UnregisteredFiles({ check }: { check: Check }) {
  const folders = check.unregisteredFolders.length
  return (
    <Section
      title="Unregistered files"
      description={`Files in folders that hold registered ones, but aren't registered themselves${folders ? `, in ${folders} folder${folders === 1 ? "" : "s"}` : ""}. Register the ones you want; the others can stay.`}
      count={unregisteredCount(check)}
    >
      <ul className="flex flex-col gap-2">
        {check.unregisteredFolders.map((folder) => (
          <li key={folder.path} className="flex flex-col gap-1">
            <span className="flex flex-wrap items-center gap-2">
              <span className="font-mono break-all">
                {folder.path || "(the database's folder)"}
              </span>
              <ButtonLink
                variant="outline"
                size="xs"
                to={`/files/create?${new URLSearchParams({ folder: folder.path })}`}
              >
                Register…
              </ButtonLink>
            </span>
            <span className="text-muted-foreground">
              {folder.files
                .slice(0, 8)
                .map((file) => file.name)
                .join(", ")}
              {folder.files.length > 8 &&
                ` and ${folder.files.length - 8} more`}
            </span>
          </li>
        ))}
      </ul>
    </Section>
  )
}
