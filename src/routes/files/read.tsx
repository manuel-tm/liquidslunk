import { useMutation } from "@tanstack/react-query"
import { CircleAlertIcon, ExternalLinkIcon, FolderOpenIcon } from "lucide-react"
import { useNavigate, useParams } from "react-router"
import { toast } from "sonner"

import { ButtonLink } from "@/components/button-link"
import { FILE_TYPE_LABELS } from "@/components/file-type"
import { FileUses } from "@/components/file-uses"
import { PageHeader } from "@/components/page-header"
import { QueryError } from "@/components/query-error"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import type { FileDetails as File } from "@/bindings"
import { openFile, revealFile } from "@/lib/files"
import { formatDateTime, formatSize } from "@/lib/format"
import { call, fileUrl, idParam, queryClient, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/** `/files/:id` (slunk's design §7.8) */
export function FileRead() {
  const { id = "" } = useParams()
  const fileId = idParam(id)
  const { data: file, error } = useCommand(
    "file",
    { id: fileId ?? 0 },
    { enabled: fileId !== null }
  )

  if (fileId === null || file === null) return <NotFound />

  return (
    <>
      <PageHeader
        crumbs={[
          { label: "Files", to: "/files" },
          { label: file ? `${file.name} (${file.id})` : id },
        ]}
      >
        {file && (
          <>
            <ButtonLink variant="outline" to={`/files/${file.id}/update`}>
              Edit
            </ButtonLink>
            <DeleteFile file={file} />
          </>
        )}
      </PageHeader>
      <div className="flex flex-col gap-4 p-4">
        {error ? (
          <QueryError error={error} title="Can't load the file" />
        ) : !file ? (
          <Skeleton className="h-64 w-full" />
        ) : (
          <>
            {!file.onDisk && (
              <Alert variant="destructive">
                <CircleAlertIcon />
                <AlertTitle>The file is missing on disk</AlertTitle>
                <AlertDescription>
                  Nothing is at its path anymore. If it moved, relink it from
                  Edit.
                </AlertDescription>
              </Alert>
            )}
            <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
              <Preview file={file} />
              <Details file={file} />
            </div>
            <Uses file={file} />
          </>
        )}
      </div>
    </>
  )
}

/** The image or an audio player, then buttons that open the file. */
function Preview({ file }: { file: File }) {
  const src = fileUrl(file.id)
  return (
    <Card>
      <CardContent className="flex flex-col items-start gap-3">
        {file.type === "image" && file.onDisk && (
          <img
            src={src}
            alt={file.name}
            className="max-h-96 max-w-full object-contain"
          />
        )}
        {file.type === "music" && file.onDisk && (
          <audio controls preload="metadata" src={src} className="w-full" />
        )}
        <div className="flex flex-wrap gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void openFile(file.id)}
          >
            <ExternalLinkIcon data-icon="inline-start" />
            Open the file
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => void revealFile(file.id)}
          >
            <FolderOpenIcon data-icon="inline-start" />
            Show in Finder
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function Details({ file }: { file: File }) {
  const rows: [string, string][] = [
    ["Path", file.path],
    ["Type", FILE_TYPE_LABELS[file.type]],
    ["Size", `${formatSize(file.sizeBytes)} (${file.sizeBytes} bytes)`],
    ["Modified", formatDateTime(file.mtime)],
    ["SHA-256", file.sha256],
    ["Added", formatDateTime(file.insertedAt)],
    ["Updated", formatDateTime(file.updatedAt)],
  ]
  return (
    <dl className="grid h-fit grid-cols-[auto_1fr] gap-x-4 gap-y-2 text-xs">
      {rows.map(([label, value]) => (
        <div key={label} className="contents">
          <dt className="text-muted-foreground">{label}</dt>
          <dd
            className={
              label === "Path" || label === "SHA-256"
                ? "font-mono break-all"
                : undefined
            }
          >
            {value}
          </dd>
        </div>
      ))}
    </dl>
  )
}

/** Everything that uses the file. Those uses are what block a delete. */
function Uses({ file }: { file: File }) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Used by</CardTitle>
      </CardHeader>
      <CardContent className="text-xs">
        {file.uses.length === 0 ? (
          <p className="text-muted-foreground">Nothing uses this file yet.</p>
        ) : (
          <FileUses uses={file.uses} />
        )}
        {file.type === "music" &&
          !file.uses.some((use) => use.kind === "track_audio") && (
            <ButtonLink
              variant="outline"
              size="sm"
              className="mt-3"
              to={`/tracks/create?file=${file.id}`}
            >
              Create track
            </ButtonLink>
          )}
      </CardContent>
    </Card>
  )
}

/**
 * Deletes the record, after a confirmation. While anything uses the file,
 * the dialog says what instead (slunk's design §7.2).
 */
function DeleteFile({ file }: { file: File }) {
  const navigate = useNavigate()
  const deleteFile = useMutation({
    mutationFn: () => call("deleteFile", { id: file.id }),
  })
  const used = file.uses.length > 0

  return (
    <AlertDialog>
      <AlertDialogTrigger render={<Button variant="destructive" />}>
        Delete
      </AlertDialogTrigger>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            {used ? "This file is in use" : "Delete this file's record?"}
          </AlertDialogTitle>
          <AlertDialogDescription>
            {used
              ? "It can't be deleted while these use it:"
              : `liquidslunk forgets ${file.name}. The file on disk isn't touched.`}
          </AlertDialogDescription>
        </AlertDialogHeader>
        {used && (
          <div className="text-xs">
            <FileUses uses={file.uses} />
          </div>
        )}
        {deleteFile.error && (
          <QueryError error={deleteFile.error} title="Can't delete it" />
        )}
        <AlertDialogFooter>
          <AlertDialogCancel>{used ? "Close" : "Cancel"}</AlertDialogCancel>
          {!used && (
            <AlertDialogAction
              variant="destructive"
              disabled={deleteFile.isPending}
              onClick={async () => {
                try {
                  await deleteFile.mutateAsync()
                } catch {
                  return // The dialog shows the error
                }
                toast.success(`Deleted the record of ${file.name}`)
                navigate("/files")
                queryClient.removeQueries({
                  queryKey: ["file", { id: file.id }],
                })
              }}
            >
              Delete
            </AlertDialogAction>
          )}
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
