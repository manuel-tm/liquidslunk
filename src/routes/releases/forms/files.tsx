import { useMutation } from "@tanstack/react-query"
import { useState } from "react"

import { Cover } from "@/components/cover"
import {
  type FileLinkRow,
  FileLinksEditor,
  fileLinksInput,
} from "@/components/file-links-editor"
import type { Option } from "@/components/picker"
import { FilePicker } from "@/components/pickers"
import { QueryError } from "@/components/query-error"
import { Button } from "@/components/ui/button"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Skeleton } from "@/components/ui/skeleton"
import { call, idParam, useCommand } from "@/lib/query"
import { NotFound } from "@/routes/not-found"

/**
 * A release's cover, chosen from its linked images or any registered image, and
 * its file links, each with a note.
 */
export function ReleaseFilesForm({
  releaseId,
  onSaved,
}: {
  /** From the URL */
  releaseId: string
  onSaved: () => void
}) {
  const id = idParam(releaseId)
  // A form starts from fresh data (frontend.md, "Commands")
  const { data: release, error } = useCommand(
    "releaseFiles",
    { id: id ?? 0 },
    { fresh: true, enabled: id !== null }
  )

  if (id === null || release === null) return <NotFound />
  if (error) return <QueryError error={error} title="Can't load the files" />
  if (!release) return <Skeleton className="h-64 w-full" />
  // Options' ids are strings, as the pickers' are
  return (
    <Form
      key={release.id}
      releaseId={release.id}
      initialCover={
        release.cover
          ? { id: String(release.cover.id), name: release.cover.path }
          : null
      }
      initialLinks={release.files.map((link) => ({
        file: { id: String(link.file.id), name: link.file.path },
        note: link.note ?? "",
        type: link.file.type,
      }))}
      onSaved={onSaved}
    />
  )
}

function Form({
  releaseId,
  initialCover,
  initialLinks,
  onSaved,
}: {
  releaseId: number
  initialCover: Option | null
  initialLinks: (FileLinkRow & { type: string })[]
  onSaved: () => void
}) {
  const [cover, setCover] = useState(initialCover)
  const [links, setLinks] = useState<FileLinkRow[]>(initialLinks)
  const save = useMutation({
    mutationFn: () =>
      call("setReleaseFiles", {
        id: releaseId,
        cover: cover ? { fileId: Number(cover.id) } : null,
        files: fileLinksInput(links),
      }),
  })
  const linkedImages = initialLinks.filter(
    (link) =>
      link.type === "image" && links.some((l) => l.file.id === link.file.id)
  )

  return (
    <form
      onSubmit={async (event) => {
        event.preventDefault()
        try {
          await save.mutateAsync()
          onSaved()
        } catch {
          // The form shows the error
        }
      }}
    >
      <FieldGroup className="max-w-3xl">
        {save.error && (
          <QueryError error={save.error} title="Can't save the files" />
        )}
        <Field>
          <FieldLabel htmlFor="cover">Cover</FieldLabel>
          <div className="flex items-start gap-4">
            <Cover file={cover} alt="Cover" className="size-24" />
            <div className="flex flex-1 flex-col gap-2">
              <FilePicker
                id="cover"
                type="image"
                value={cover}
                onChange={setCover}
              />
              {linkedImages.length > 0 && (
                <div className="flex flex-wrap gap-2">
                  {linkedImages.map((link) => (
                    <Button
                      key={link.file.id}
                      type="button"
                      size="sm"
                      variant={
                        cover?.id === link.file.id ? "secondary" : "outline"
                      }
                      onClick={() => setCover(link.file)}
                    >
                      Use {link.note || link.file.name.split("/").pop()}
                    </Button>
                  ))}
                </div>
              )}
            </div>
          </div>
          <FieldDescription>
            The primary image. Other images are file links.
          </FieldDescription>
        </Field>
        <Field>
          <FieldLabel>Linked files</FieldLabel>
          <FileLinksEditor links={links} onChange={setLinks} />
        </Field>
        <div>
          <Button type="submit" disabled={save.isPending}>
            Save
          </Button>
        </div>
      </FieldGroup>
    </form>
  )
}
