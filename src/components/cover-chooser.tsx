import { DiscIcon, ImagePlusIcon } from "lucide-react"
import { useState } from "react"

import { Cover } from "@/components/cover"
import { FolderBrowser } from "@/components/folder-browser"
import { QueryError } from "@/components/query-error"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import type { FileRefInput, FolderFile } from "@/bindings"
import { fileUrl, useCommand } from "@/lib/query"
import { cn } from "@/lib/utils"

/**
 * The browser remembers the folder of the last image chosen, to start there.
 * The folder is relative to the library, so each library has its own
 * (design §6.4).
 */
function imageFolderKey(library: string | undefined) {
  return `slunk.imageFolder:${library ?? ""}`
}

function readImageFolder(key: string) {
  try {
    return localStorage.getItem(key)
  } catch {
    return null
  }
}

function folderOf(path: string) {
  const slash = path.lastIndexOf("/")
  return slash === -1 ? "" : path.slice(0, slash)
}

/** The deepest folder that holds every path: a release's tracks' folder. */
export function commonFolder(paths: string[]): string | null {
  if (paths.length === 0) return null
  const [first, ...rest] = paths.map((path) => folderOf(path).split("/"))
  let length = first.length
  for (const parts of rest) {
    let same = 0
    while (same < length && parts[same] === first[same]) same++
    length = same
  }
  return first.slice(0, length).join("/")
}

/**
 * A cover, picture or logo. Clicking it shows it at full size, up to the
 * window's. While there's none, the placeholder is a button that opens
 * `ChooseImageDialog`, so it's set without leaving the page.
 */
export function CoverChooser({
  file,
  alt,
  className,
  noun,
  folder,
  onChoose,
}: {
  file: { id: number | string } | null | undefined
  alt: string
  className?: string
  /** What the image is: "cover", "picture" or "logo" */
  noun: string
  /** Where to browse first, like a release's own folder */
  folder?: string | null
  onChoose: (ref: FileRefInput) => Promise<unknown>
}) {
  const [open, setOpen] = useState(false)
  if (file) {
    return (
      <Dialog>
        <DialogTrigger
          aria-label={`View ${alt} at full size`}
          className="cursor-zoom-in outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <Cover file={file} alt={alt} className={className} />
        </DialogTrigger>
        {/* Sized by the image, in viewport units: a percentage would let the
            centered popup shrink it to half the window */}
        <DialogContent className="w-auto sm:max-w-[calc(100%-2rem)]">
          <DialogTitle className="pr-6">{alt}</DialogTitle>
          <img
            src={fileUrl(file.id)}
            alt={alt}
            className="max-h-[calc(100vh-8rem)] max-w-[calc(100vw-4rem)]"
          />
        </DialogContent>
      </Dialog>
    )
  }

  return (
    <>
      <Tooltip>
        <TooltipTrigger
          render={
            <button
              type="button"
              aria-label={`Choose a ${noun}`}
              onClick={() => setOpen(true)}
              className={cn(
                "group flex aspect-square items-center justify-center bg-muted text-muted-foreground transition-colors outline-none hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring",
                className
              )}
            />
          }
        >
          <DiscIcon className="size-1/3 group-hover:hidden group-focus-visible:hidden" />
          <ImagePlusIcon className="hidden size-1/3 group-hover:block group-focus-visible:block" />
        </TooltipTrigger>
        <TooltipContent>Choose a {noun}</TooltipContent>
      </Tooltip>
      {open && (
        <ChooseImageDialog
          noun={noun}
          folder={folder}
          onChoose={onChoose}
          onClose={() => setOpen(false)}
        />
      )}
    </>
  )
}

/**
 * Browses the disk for an image. Picking one saves it with `onChoose`, which
 * registers it first when it isn't registered yet. liquidslunk only reads it.
 */
export function ChooseImageDialog({
  noun,
  folder,
  onChoose,
  onClose,
}: {
  noun: string
  /** Where to browse first. Otherwise, where the last image was chosen */
  folder?: string | null
  onChoose: (ref: FileRefInput) => Promise<unknown>
  onClose: () => void
}) {
  const { data: status } = useCommand("libraryStatus")
  const key = imageFolderKey(status?.library?.path)
  const [path, setPath] = useState(() => folder ?? readImageFolder(key) ?? "")
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<Error | null>(null)

  async function choose(file: FolderFile) {
    setSaving(true)
    setError(null)
    try {
      await onChoose(file.file ? { fileId: file.file.id } : { path: file.path })
    } catch (caught) {
      setError(caught as Error)
      setSaving(false)
      return
    }
    try {
      localStorage.setItem(key, folderOf(file.path))
    } catch {
      // Only a convenience
    }
    onClose()
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !saving && onClose()}>
      <DialogContent className="flex max-h-[85vh] flex-col sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Choose a {noun}</DialogTitle>
          <DialogDescription>
            Pick an image. One that isn't registered yet is registered as it
            becomes the {noun}. liquidslunk only reads it.
          </DialogDescription>
        </DialogHeader>
        {error && <QueryError error={error} title={`Can't set the ${noun}`} />}
        <fieldset disabled={saving} className="min-h-0 min-w-0 overflow-y-auto">
          <FolderBrowser
            path={path}
            onOpen={setPath}
            pickable={(file) => file.type === "image"}
            onPick={choose}
          />
        </fieldset>
      </DialogContent>
    </Dialog>
  )
}
