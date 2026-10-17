import { DiscIcon } from "lucide-react"

import { fileUrl } from "@/lib/query"
import { cn } from "@/lib/utils"

/**
 * A cover or picture: the original image, loaded lazily (slunk's design §7.3), or a
 * placeholder. It takes the size in `className`, and only that: the
 * `max-width: 100%` Tailwind gives images lets a table that's too wide, like
 * the release list, squeeze an image's column to nothing.
 */
export function Cover({
  file,
  alt,
  className,
}: {
  file: { id: string } | null | undefined
  alt: string
  className?: string
}) {
  if (!file) {
    return (
      <div
        className={cn(
          "flex aspect-square items-center justify-center bg-muted text-muted-foreground",
          className
        )}
      >
        <DiscIcon className="size-1/3" />
      </div>
    )
  }
  return (
    <img
      src={fileUrl(file.id)}
      alt={alt}
      loading="lazy"
      className={cn(
        "aspect-square max-w-none bg-muted object-cover",
        className
      )}
    />
  )
}
