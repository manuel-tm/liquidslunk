import { useQuery } from "@apollo/client/react"
import { FolderIcon, FolderUpIcon } from "lucide-react"
import type { ReactNode } from "react"
import { Link } from "react-router"

import { FILE_TYPE_LABELS, FileTypeIcon } from "@/components/file-type"
import { QueryError } from "@/components/query-error"
import { Badge } from "@/components/ui/badge"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { graphql } from "@/gql"
import type { FolderQuery } from "@/gql/graphql"
import { formatSize } from "@/lib/format"
import { cn } from "@/lib/utils"

const FolderQuery = graphql(`
  query Folder($path: String!) {
    folder(path: $path) {
      path
      absolutePath
      parent
      folders {
        name
        path
        hasRegisteredFiles
      }
      files {
        name
        path
        type
        sizeBytes
        file {
          id
        }
      }
    }
  }
`)

export type FolderEntryFile = FolderQuery["folder"]["files"][number]

/**
 * How the browser opens a folder: a link, when the folder is in the URL, or
 * in place, as in a dialog.
 */
type Navigation =
  | { linkTo: (path: string) => string; onOpen?: never }
  | { onOpen: (path: string) => void; linkTo?: never }

/**
 * Browses the disk from the folder that holds slunk.db: a folder's
 * subfolders, `..` to go up, and the files directly inside. Folders that
 * hold registered files are marked. It only reads.
 */
export function FolderBrowser({
  path,
  actions,
  pickable,
  onPick,
  ...navigation
}: Navigation & {
  /** The folder, relative to the database's folder */
  path: string
  /** Buttons for the folder, like "Choose this folder" */
  actions?: ReactNode
  /** Which files can be picked, with `onPick` */
  pickable?: (file: FolderEntryFile) => boolean
  /** Picks a file, which only files `pickable` accepts offer */
  onPick?: (file: FolderEntryFile) => void
}) {
  const { data, error } = useQuery(FolderQuery, { variables: { path } })

  if (error) {
    return (
      <div className="flex flex-col items-start gap-2">
        <QueryError error={error} title="Can't open the folder" />
        {path !== "" && (
          <OpenFolder path="" navigation={navigation}>
            Open the database's folder
          </OpenFolder>
        )}
      </div>
    )
  }
  if (!data) return <Skeleton className="h-48 w-full" />
  const { folder } = data

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <p className="font-mono text-xs break-all">{folder.absolutePath}</p>
        {actions}
      </div>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Name</TableHead>
            <TableHead>Type</TableHead>
            <TableHead className="text-right">Size</TableHead>
            <TableHead />
          </TableRow>
        </TableHeader>
        <TableBody>
          {folder.parent !== null && (
            <TableRow>
              <TableCell colSpan={4}>
                <OpenFolder path={folder.parent} navigation={navigation}>
                  <FolderUpIcon />
                  ..
                </OpenFolder>
              </TableCell>
            </TableRow>
          )}
          {folder.folders.map((sub) => (
            <TableRow key={sub.path}>
              <TableCell colSpan={3}>
                <OpenFolder path={sub.path} navigation={navigation}>
                  <FolderIcon />
                  {sub.name}
                </OpenFolder>
              </TableCell>
              <TableCell className="text-right">
                {sub.hasRegisteredFiles && (
                  <Badge variant="secondary">Has registered files</Badge>
                )}
              </TableCell>
            </TableRow>
          ))}
          {folder.files.map((file) => (
            <TableRow key={file.path}>
              <TableCell>
                {onPick && pickable?.(file) ? (
                  <button
                    type="button"
                    className={cn(ENTRY, "text-left")}
                    onClick={() => onPick(file)}
                  >
                    <FileTypeIcon type={file.type} />
                    {file.name}
                  </button>
                ) : (
                  <span
                    className={cn(
                      "flex items-center gap-2 text-muted-foreground [&_svg]:size-4",
                      // Files that can't be picked fade
                      onPick && "opacity-60"
                    )}
                  >
                    <FileTypeIcon type={file.type} />
                    <span className="text-foreground">{file.name}</span>
                  </span>
                )}
              </TableCell>
              <TableCell>{FILE_TYPE_LABELS[file.type]}</TableCell>
              <TableCell className="text-right tabular-nums">
                {formatSize(file.sizeBytes)}
              </TableCell>
              <TableCell className="text-right">
                {file.file && (
                  <Badge
                    variant="secondary"
                    // In place, a link would leave the dialog
                    render={
                      navigation.linkTo ? (
                        <Link to={`/files/${file.file.id}`} />
                      ) : undefined
                    }
                  >
                    Registered
                  </Badge>
                )}
              </TableCell>
            </TableRow>
          ))}
          {folder.folders.length === 0 && folder.files.length === 0 && (
            <TableRow>
              <TableCell colSpan={4} className="text-muted-foreground">
                This folder is empty.
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>
    </div>
  )
}

const ENTRY = "flex items-center gap-2 hover:underline [&_svg]:size-4"

/** A folder's link, or a button that opens it in place. */
function OpenFolder({
  path,
  navigation,
  children,
}: {
  path: string
  navigation: Navigation
  children: ReactNode
}) {
  if (navigation.linkTo) {
    return (
      <Link to={navigation.linkTo(path)} className={ENTRY}>
        {children}
      </Link>
    )
  }
  return (
    <button
      type="button"
      className={cn(ENTRY, "text-left")}
      onClick={() => navigation.onOpen(path)}
    >
      {children}
    </button>
  )
}
