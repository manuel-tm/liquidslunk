import { FileIcon } from "lucide-react"

import { Cover } from "@/components/cover"
import { openFile } from "@/lib/files"

type LinkedFile = {
  file: { id: number | string; name: string; type: string }
  note?: string | null
}

/**
 * A primary image, then linked files: images in a grid, other files (a
 * booklet, a rip log) in a list, each with its note. Each one opens the file
 * in its default app.
 */
export function FileGallery({
  primary,
  primaryLabel,
  links,
}: {
  primary: { id: number | string; name: string } | null | undefined
  primaryLabel: string
  links: LinkedFile[]
}) {
  const images = links.filter((link) => link.file.type === "image")
  const others = links.filter((link) => link.file.type !== "image")
  return (
    <div className="flex flex-col gap-6 text-xs">
      <section className="flex flex-col gap-2">
        <h2 className="font-heading text-sm font-semibold">{primaryLabel}</h2>
        {primary ? (
          <button
            type="button"
            onClick={() => void openFile(primary.id)}
            className="w-64 cursor-pointer"
          >
            <Cover file={primary} alt={primaryLabel} className="w-64" />
          </button>
        ) : (
          <p className="text-muted-foreground">None set.</p>
        )}
      </section>
      <section className="flex flex-col gap-2">
        <h2 className="font-heading text-sm font-semibold">Linked files</h2>
        {links.length === 0 && (
          <p className="text-muted-foreground">None linked.</p>
        )}
        {images.length > 0 && (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(10rem,1fr))] gap-3">
            {images.map(({ file, note }) => (
              <button
                type="button"
                key={file.id}
                onClick={() => void openFile(file.id)}
                className="flex cursor-pointer flex-col gap-1 text-left"
              >
                <Cover file={file} alt={note ?? file.name} className="w-full" />
                <span className="truncate">{note ?? file.name}</span>
              </button>
            ))}
          </div>
        )}
        {others.length > 0 && (
          <ul className="flex flex-col gap-1">
            {others.map(({ file, note }) => (
              <li key={file.id}>
                <button
                  type="button"
                  onClick={() => void openFile(file.id)}
                  className="inline-flex cursor-pointer items-center gap-2 hover:underline [&_svg]:size-4"
                >
                  <FileIcon />
                  {file.name}
                </button>
                {note && (
                  <span className="text-muted-foreground"> · {note}</span>
                )}
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  )
}
