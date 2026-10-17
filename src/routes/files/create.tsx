import { useMutation } from "@tanstack/react-query"
import { useState } from "react"
import { Link, useNavigate, useSearchParams } from "react-router"
import { toast } from "sonner"

import { ButtonLink } from "@/components/button-link"
import { FILE_TYPE_LABELS } from "@/components/file-type"
import { FolderBrowser } from "@/components/folder-browser"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Progress, ProgressLabel } from "@/components/ui/progress"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import type { FolderFile as DiskFile } from "@/bindings"
import { formatSize } from "@/lib/format"
import { call, queryClient, useCommand } from "@/lib/query"

// Each call registers a few files, so the progress bar moves along
const CHUNK_FILES = 10
const CHUNK_BYTES = 100_000_000

const createUrl = (params: Record<string, string>) =>
  `/files/create?${new URLSearchParams(params)}`

/**
 * `/files/create` registers files (slunk's design §7.8). `?path=` browses for a
 * folder; `?folder=` lists the chosen folder's files to register.
 */
export function RegisterFiles() {
  const [params] = useSearchParams()
  const folder = params.get("folder")
  const path = params.get("path") ?? ""

  return (
    <>
      <PageHeader
        crumbs={[{ label: "Files", to: "/files" }, { label: "Register" }]}
      />
      <div className="flex flex-col gap-4 p-4">
        {folder === null ? (
          <>
            <p className="text-xs text-muted-foreground">
              Pick a folder. Its files are listed next, including those in
              subfolders, and you choose which to register. liquidslunk only
              reads them.
            </p>
            <FolderBrowser
              path={path}
              linkTo={(path) => createUrl({ path })}
              actions={
                <ButtonLink to={createUrl({ folder: path })}>
                  Choose this folder
                </ButtonLink>
              }
            />
          </>
        ) : (
          <FolderFiles folder={folder} />
        )}
      </div>
    </>
  )
}

function FolderFiles({ folder }: { folder: string }) {
  const { data, error } = useCommand("folderFiles", { path: folder })

  if (error) return <QueryError error={error} title="Can't list the folder" />
  if (!data) return <Skeleton className="h-64 w-full" />
  return <FilePicker folder={folder} files={data} />
}

/** Checkboxes for the folder's unregistered files, and the Register button. */
function FilePicker({ folder, files }: { folder: string; files: DiskFile[] }) {
  const navigate = useNavigate()
  const [unchecked, setUnchecked] = useState(() => new Set<string>())
  const [progress, setProgress] = useState<number | null>(null)
  // One mutation for every chunk, so the caches are invalidated once, at the
  // end, and the list doesn't change under the progress bar
  const registerFiles = useMutation({
    mutationFn: async (files: DiskFile[]) => {
      let done = 0
      for (const chunk of chunks(files)) {
        await call("registerFiles", { paths: chunk.map((file) => file.path) })
        done += sum(chunk)
        setProgress(done)
      }
    },
    // The chunks before a failure are registered, so the list shows them
    onError: () => void queryClient.invalidateQueries(),
  })
  const { error } = registerFiles

  const candidates = files.filter((file) => !file.file)
  const checked = candidates.filter((file) => !unchecked.has(file.path))
  const totalBytes = sum(checked)
  const prefix = folder === "" ? "" : `${folder}/`
  const shown = (path: string) =>
    path.startsWith(prefix) ? path.slice(prefix.length) : path

  function toggle(path: string, on: boolean) {
    setUnchecked((current) => {
      const next = new Set(current)
      if (on) next.delete(path)
      else next.add(path)
      return next
    })
  }

  async function register() {
    setProgress(0)
    try {
      await registerFiles.mutateAsync(checked)
    } catch {
      setProgress(null)
      return // The page shows the error
    }
    toast.success(
      `Registered ${checked.length} ${checked.length === 1 ? "file" : "files"}`
    )
    navigate("/files")
  }

  const busy = progress !== null
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="text-xs">
          <span className="font-mono">{folder || "."}</span>
          <span className="text-muted-foreground">
            {" "}
            · {files.length} files, {candidates.length} not registered yet
          </span>
        </p>
        <div className="flex gap-2">
          <ButtonLink variant="outline" to={createUrl({ path: folder })}>
            Change folder
          </ButtonLink>
          <Button disabled={busy || checked.length === 0} onClick={register}>
            Register {checked.length} {checked.length === 1 ? "file" : "files"}
          </Button>
        </div>
      </div>

      {error && <QueryError error={error} title="Can't register the files" />}
      {busy && (
        <Progress value={totalBytes ? (progress / totalBytes) * 100 : 100}>
          <ProgressLabel>
            Reading and hashing: {formatSize(progress)} of{" "}
            {formatSize(totalBytes)}
          </ProgressLabel>
        </Progress>
      )}

      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-8">
              <Checkbox
                aria-label="All files"
                disabled={busy || candidates.length === 0}
                checked={
                  candidates.length > 0 && checked.length === candidates.length
                }
                indeterminate={
                  checked.length > 0 && checked.length < candidates.length
                }
                onCheckedChange={(on) =>
                  setUnchecked(
                    on ? new Set() : new Set(candidates.map((f) => f.path))
                  )
                }
              />
            </TableHead>
            <TableHead>Path</TableHead>
            <TableHead>Type</TableHead>
            <TableHead className="text-right">Size</TableHead>
            <TableHead />
          </TableRow>
        </TableHeader>
        <TableBody>
          {files.map((file) => (
            <TableRow key={file.path}>
              <TableCell>
                <Checkbox
                  aria-label={file.path}
                  disabled={busy || file.file !== null}
                  checked={file.file === null && !unchecked.has(file.path)}
                  onCheckedChange={(on) => toggle(file.path, on)}
                />
              </TableCell>
              <TableCell className="whitespace-normal">
                {shown(file.path)}
              </TableCell>
              <TableCell>{FILE_TYPE_LABELS[file.type]}</TableCell>
              <TableCell className="text-right tabular-nums">
                {formatSize(file.sizeBytes)}
              </TableCell>
              <TableCell className="text-right">
                {file.file && (
                  <Badge
                    variant="secondary"
                    render={<Link to={`/files/${file.file.id}`} />}
                  >
                    Registered
                  </Badge>
                )}
              </TableCell>
            </TableRow>
          ))}
          {files.length === 0 && (
            <TableRow>
              <TableCell colSpan={5} className="text-muted-foreground">
                There are no files in this folder.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  )
}

function sum(files: DiskFile[]): number {
  return files.reduce((total, file) => total + file.sizeBytes, 0)
}

function chunks(files: DiskFile[]): DiskFile[][] {
  const result: DiskFile[][] = []
  let chunk: DiskFile[] = []
  for (const file of files) {
    if (
      chunk.length === CHUNK_FILES ||
      (chunk.length > 0 && sum(chunk) + file.sizeBytes > CHUNK_BYTES)
    ) {
      result.push(chunk)
      chunk = []
    }
    chunk.push(file)
  }
  if (chunk.length > 0) result.push(chunk)
  return result
}
